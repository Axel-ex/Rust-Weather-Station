use crate::config::{CHANNEL_SIZE, MAX_RETRY};
use crate::tasks::mqtt_task::MqttPacket;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::Timer;
use esp_hal::Async;
use esp_hal::i2c::master::I2c;
use ina219::AsyncIna219;
use ina219::address::Address;
use ina219::calibration::{IntCalibration, MicroAmpere};
use log::error;

#[embassy_executor::task]
pub async fn ina210_task(
    i2c: &'static mut I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>,
    mqtt_sender: Sender<'static, CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE>,
) {
    let current_lsb = MicroAmpere(15); // max current (0.5A) / 32767 (size of reg)
    let r_shunt_uohm = 100_000;

    let calib = IntCalibration::new(current_lsb, r_shunt_uohm).unwrap();
    Timer::after_secs(1).await;
    let mut ina = match AsyncIna219::new_calibrated(i2c, Address::default(), calib).await {
        Ok(ina) => ina,
        Err(e) => {
            error!("Error initiating the ina219: {:?}", e);
            return;
        }
    };

    let mut retry = 0;
    while retry < MAX_RETRY {
        match ina.bus_voltage().await {
            Ok(voltage) => {
                let voltage = (voltage.voltage_mv() + 160) as f32;
                publish!(&mqtt_sender, "battery/voltage", voltage);
                publish!(
                    &mqtt_sender,
                    "battery/percentage",
                    voltage_to_soc(voltage / 1000.0)
                );
                break;
            }
            Err(e) => error!("Fail reading ina219: {:?}", e),
        }
        retry += 1;
    }
}

const SOC_TABLE: &[(f32, f32)] = &[
    (4.20, 100.0),
    (4.10, 90.0),
    (4.00, 80.0),
    (3.90, 70.0),
    (3.80, 55.0),
    (3.70, 35.0),
    (3.60, 20.0),
    (3.50, 8.0),
    (3.40, 0.0),
];

fn voltage_to_soc(v: f32) -> f32 {
    if v >= SOC_TABLE[0].0 {
        return 100.0;
    }
    if v <= SOC_TABLE[SOC_TABLE.len() - 1].0 {
        return 0.0;
    }

    // Find interval and linearly interpolate
    for win in SOC_TABLE.windows(2) {
        let (v_hi, soc_hi) = win[0];
        let (v_lo, soc_lo) = win[1];
        if v <= v_hi && v >= v_lo {
            let t = (v - v_lo) / (v_hi - v_lo);
            return soc_lo + t * (soc_hi - soc_lo);
        }
    }

    0.0 // fallback
}
