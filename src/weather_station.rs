use crate::{global::*, mqtt, wifi::*, CONFIG};
use anyhow::Result;
use as5600::As5600;
use bosch_bme680::*;
use core::sync::atomic::Ordering;
use embedded_dht_rs::dht22::Dht22;
use embedded_dht_rs::SensorReading;
use embedded_hal_bus::i2c;
use embedded_hal_bus::i2c::*;
use esp_idf_svc::hal::delay::Delay;
use esp_idf_svc::hal::{delay::Ets, gpio::*, i2c::I2cDriver};
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use esp_idf_svc::{
    hal::{
        delay::FreeRtos,
        i2c::*,
        peripherals::Peripherals,
        prelude::*,
        sys::{esp_deep_sleep_start, esp_sleep_enable_timer_wakeup},
    },
    sys::esp_sleep_enable_ext0_wakeup,
};
use ina219::measurements::BusVoltage;
use ina219::{address::Address, calibration::UnCalibrated, SyncIna219};
use log::info;
use std::cell::RefCell;
use std::time::{Duration, Instant};

pub struct WeatherStation {
    pin_rain: &'static mut PinDriver<'static, Gpio25, Input>,
    pin_anemo: &'static mut PinDriver<'static, Gpio27, Input>,
    pin_transistor: &'static mut PinDriver<'static, Gpio17, Output>,
    dht: Dht22<PinDriver<'static, Gpio32, InputOutput>, Delay>,
    as5600: As5600<i2c::RefCellDevice<'static, I2cDriver<'static>>>,
    battery_sensor: SyncIna219<RefCellDevice<'static, I2cDriver<'static>>, UnCalibrated>,
    wifi: BlockingWifi<EspWifi<'static>>,
}

impl WeatherStation {
    pub fn new() -> Self {
        let peripherals = Peripherals::take().unwrap();

        // Create and configure I2C
        let i2c_driver = I2cDriver::new(
            peripherals.i2c0,
            peripherals.pins.gpio21,
            peripherals.pins.gpio22,
            &I2cConfig::new().baudrate(Hertz(100_000)),
        )
        .expect("Failed to create I2C driver");

        // Create a static reference to the I2C driver
        // SAFETY: We ensure the I2C driver and PinDrivers lives for the entire program duration
        let i2c_bus = Box::leak(Box::new(RefCell::new(i2c_driver)));
        let pin_anemo = Box::leak(Box::new(PinDriver::input(peripherals.pins.gpio27).unwrap()));
        let pin_rain = Box::leak(Box::new(PinDriver::input(peripherals.pins.gpio25).unwrap()));
        let pin_transistor = Box::leak(Box::new(
            PinDriver::output(peripherals.pins.gpio17).unwrap(),
        ));

        //Turn on the peripherals (they are physically connected to a transistor for battery saving
        //purpose)
        pin_transistor.set_high().unwrap();

        let mut dht_pin = PinDriver::input_output_od(peripherals.pins.gpio32).unwrap();
        dht_pin.set_high().unwrap();
        let dht = Dht22::new(dht_pin, Delay::new(100));

        let as5600 = As5600::new(i2c::RefCellDevice::new(i2c_bus));
        let battery_sensor = SyncIna219::new(
            i2c::RefCellDevice::new(i2c_bus),
            Address::from_byte(0x40).unwrap(),
        )
        .expect("Fail creating Ina219 interface");
        //TODO: calibrate the ina

        let mut wifi = wifi_init(peripherals.modem).expect("Fail initiating wifi");
        connect_wifi(&mut wifi).expect("Fail connecting to nework");

        WeatherStation {
            pin_rain,
            pin_anemo,
            pin_transistor,
            dht,
            as5600,
            battery_sensor,
            wifi,
        }
    }

    pub fn set_interrupt(&mut self) -> Result<()> {
        self.pin_anemo.set_pull(Pull::Up)?;
        self.pin_rain.set_pull(Pull::Up)?;
        self.pin_anemo.set_interrupt_type(InterruptType::PosEdge)?;
        self.pin_rain.set_interrupt_type(InterruptType::PosEdge)?;

        unsafe {
            self.pin_rain.subscribe(rain_pin_callback)?;
            self.pin_anemo.subscribe(anemo_pin_callback)?;
            esp_sleep_enable_timer_wakeup(CONFIG.deep_sleep_interval_us);
            esp_sleep_enable_ext0_wakeup(25, 0);
        }

        self.pin_rain.enable_interrupt()?;
        self.pin_anemo.enable_interrupt()?;
        Ok(())
    }

    pub fn run(&mut self) {
        let (mut mqtt_cli, mut mqtt_conn) = mqtt::mqtt_create().expect("Fail creating mqtt client");
        std::thread::scope(|s| {
            info!("Starting MQTT client");

            // Create a thread that will keep alive the connection between broker and client.
            std::thread::Builder::new()
                .stack_size(6000)
                .spawn_scoped(s, move || {
                    info!("MQTT Listening for messages");
                    while let Ok(event) = mqtt_conn.next() {
                        info!("[Queue] Event: {}", event.payload());
                    }
                    info!("Connection closed");
                })
                .expect("An error occurred with mqtt client");

            let active_duration = Duration::from_secs(CONFIG.active_duration_s);
            let start_time = Instant::now();

            info!("active duration: {:?}", active_duration);
            while start_time.elapsed() < active_duration {
                self.check_rain_flag();
                self.check_rotation_flag();

                FreeRtos::delay_ms(50);
            }

            let wind_direction = self.get_wind_direction();
            let dht_readings = self.get_dht_readings();
            let bus_voltage = self.get_battery_readings();
            info!(
                "wind: {:?}, temp: {:?}, hum: {:?}",
                wind_direction, dht_readings.temperature, dht_readings.humidity
            );

            mqtt::publish_wifi_data(&mut mqtt_cli, &mut self.wifi);
            mqtt::publish_dht_data(&mut mqtt_cli, dht_readings);
            mqtt::publish_anemo_data(&mut mqtt_cli, wind_direction);
            mqtt::publish_battery_readings(&mut mqtt_cli, bus_voltage);
            mqtt::publish_rain_data(&mut mqtt_cli);
            FreeRtos::delay_ms(5000);

            info!("Going to deep sleep...");
            self.pin_transistor.set_low().unwrap();
            unsafe {
                esp_deep_sleep_start();
            }
        });
    }

    //Check if the flag was set to true, add to the global count and reset it. The function is needed
    //to be able to reactivate interrupt which are automatically disabled upon fireing once.
    pub fn check_rain_flag(&mut self) {
        if RAIN_FLAG.load(Ordering::Relaxed) {
            RAIN_COUNT.store(RAIN_COUNT.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
            RAIN_FLAG.store(false, Ordering::Relaxed);
            if let Err(e) = self.pin_rain.enable_interrupt() {
                log::error!("Failed to re-enable rain interrupt: {e}");
            }
        }
    }

    pub fn check_rotation_flag(&mut self) {
        if ROTATION_FLAG.load(Ordering::Relaxed) {
            ROTATION_COUNT.store(
                ROTATION_COUNT.load(Ordering::Relaxed) + 1,
                Ordering::Relaxed,
            );
            ROTATION_FLAG.store(false, Ordering::Relaxed);
            if let Err(e) = self.pin_anemo.enable_interrupt() {
                log::error!("Failed to re-enable anemo interrupt: {e}");
            }
        }
    }

    pub fn get_dht_readings(&mut self) -> SensorReading<f32> {
        log::info!("get dht readings");
        match self.dht.read() {
            Ok(readings) => readings,
            Err(e) => {
                log::error!("Failed to get dht measurement: {:#?}", e);
                SensorReading {
                    humidity: 0f32,
                    temperature: 0f32,
                }
            }
        }
    }

    #[allow(unused)]
    pub fn get_bme_readings(bme: &mut Bme680<RefCellDevice<I2cDriver>, Ets>) -> MeasurmentData {
        match bme.measure() {
            Ok(readings) => readings,
            Err(e) => {
                log::error!("Failed to get BME readings: {:?}", e);
                MeasurmentData {
                    temperature: 0.0,
                    pressure: 0.0,
                    humidity: 0.0,
                    gas_resistance: None,
                }
            }
        }
    }

    pub fn get_wind_direction(&mut self) -> String {
        let reading = match self.as5600.angle() {
            Ok(value) => value,
            Err(_) => {
                log::error!("Couldn't read wind direction");
                return "NA".to_string();
            }
        };

        let angle = (reading as f32) * (360.0 / 4096.0);
        let direction = match angle {
            0.0..45.0 => "N",
            45.0..90.0 => "NE",
            90.0..135.0 => "E",
            135.0..180.0 => "SE",
            180.0..225.0 => "S",
            225.0..270.0 => "SW",
            270.0..315.0 => "W",
            315.0..360.0 => "NW",
            _ => "Invalid Angle",
        };

        direction.to_string()
    }

    pub fn get_battery_readings(&mut self) -> BusVoltage {
        if let Ok(voltage) = self.battery_sensor.bus_voltage() {
            voltage
        } else {
            BusVoltage::from_mv(0)
        }
    }
}
