use anyhow::Result;
use as5600::As5600;
use bosch_bme680::*;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use embedded_hal_bus::i2c::*;
use esp_idf_svc::{
    hal::{delay::Ets, gpio::*, i2c::I2cDriver},
    sys::{esp_sleep_enable_gpio_wakeup, esp_sleep_enable_timer_wakeup},
};
use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::time::{Duration, Instant};

//CONFIG
#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    mqtt_user: &'static str,
    #[default("")]
    mqtt_pass: &'static str,
    #[default("")]
    broker_url: &'static str,
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_pass: &'static str,
    #[default("")]
    topic: &'static str,
}

// GLOBAL ATOMIC VAR
pub static RAIN_FLAG: AtomicBool = AtomicBool::new(false);
pub static ROTATION_FLAG: AtomicBool = AtomicBool::new(false);
pub static ROTATION_COUNT: AtomicU32 = AtomicU32::new(0);
pub static RAIN_COUNT: AtomicU32 = AtomicU32::new(0);

fn rain_pin_callback() {
    RAIN_FLAG.store(true, Ordering::Relaxed);
}

fn anemo_pin_callback() {
    ROTATION_FLAG.store(true, Ordering::Relaxed);
}

pub fn set_intterupt(
    pin_rain: &mut PinDriver<Gpio25, Input>,
    pin_anemo: &mut PinDriver<Gpio27, Input>,
) -> Result<()> {
    pin_anemo.set_pull(Pull::Up)?;
    pin_rain.set_pull(Pull::Up)?;
    pin_anemo.set_interrupt_type(InterruptType::PosEdge)?;
    pin_rain.set_interrupt_type(InterruptType::PosEdge)?;

    unsafe {
        pin_rain.subscribe(rain_pin_callback)?;
        pin_anemo.subscribe(anemo_pin_callback)?;
        esp_sleep_enable_gpio_wakeup();
        esp_sleep_enable_timer_wakeup(60_000_000); //wake up every 60 seconds
    }

    pin_rain.enable_interrupt()?;
    pin_anemo.enable_interrupt()?;

    Ok(())
}

pub fn check_time_passed() -> bool {
    static LAST_TIME: Lazy<Mutex<Instant>> = Lazy::new(|| Mutex::new(Instant::now()));

    let now = Instant::now();
    let mut last_time = LAST_TIME.lock().unwrap();

    if now.duration_since(*last_time) >= Duration::from_secs(60) {
        *last_time = now; // Reset the last time
        return true;
    }

    false
}

//Check if the flag was set to true, add to the global count and reset it. The function is needed
//to be able to reactivate interrupt which are automatically disabled upon fireing once.
pub fn check_rain_flag(pin_rain: &mut PinDriver<Gpio25, Input>) {
    if RAIN_FLAG.load(Ordering::Relaxed) {
        RAIN_COUNT.store(RAIN_COUNT.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
        RAIN_FLAG.store(false, Ordering::Relaxed);
        pin_rain
            .enable_interrupt()
            .map_err(|e| log::error!("fail enabling rain interrupt: {e}"))
            .ok();
    }
}

pub fn check_rotation_flag(pin_anemo: &mut PinDriver<Gpio27, Input>) {
    if ROTATION_FLAG.load(Ordering::Relaxed) {
        ROTATION_COUNT.store(
            ROTATION_COUNT.load(Ordering::Relaxed) + 1,
            Ordering::Relaxed,
        );
        ROTATION_FLAG.store(false, Ordering::Relaxed);
        pin_anemo
            .enable_interrupt()
            .map_err(|e| log::error!("fail enabling rain interrupt: {e}"))
            .ok();
    }
}

pub fn get_bme_readings(bme: &mut Bme680<RefCellDevice<I2cDriver>, &mut Ets>) -> MeasurmentData {
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

pub fn get_wind_direction(as5600: &mut As5600<RefCellDevice<I2cDriver>>) -> String {
    let reading = match as5600.angle() {
        Ok(value) => value,
        Err(_) => {
            log::error!("Couldn't read wind direction");
            return "NA".to_string();
        }
    };

    let angle = (reading as f32) * (360.0 / 4096.0);
    let direction = match angle {
        angle if angle >= 0.0 && angle < 45.0 => "N",
        angle if angle >= 45.0 && angle < 90.0 => "NE",
        angle if angle >= 90.0 && angle < 135.0 => "E",
        angle if angle >= 135.0 && angle < 180.0 => "SE",
        angle if angle >= 180.0 && angle < 225.0 => "S",
        angle if angle >= 225.0 && angle < 270.0 => "SW",
        angle if angle >= 270.0 && angle < 315.0 => "W",
        angle if angle >= 315.0 && angle < 360.0 => "NW",
        _ => "Invalid Angle",
    };

    direction.to_string()
}
