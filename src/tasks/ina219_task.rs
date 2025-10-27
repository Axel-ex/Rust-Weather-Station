use crate::config::{CHANNEL_SIZE, MAX_RETRY};
use crate::tasks::mqtt_task::MqttPacket;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;
use esp_hal::i2c::master::I2c;
use esp_hal::Async;
use ina219::address::Address;
use ina219::calibration::{IntCalibration, MicroAmpere};
use ina219::AsyncIna219;
use log::error;

#[embassy_executor::task]
pub async fn ina210_task(
    i2c: &'static mut I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>,
    mqtt_sender: Sender<'static, CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE>,
) {
    let calib = IntCalibration::new(MicroAmpere(1_000_000), 1_000).unwrap();
    let mut ina =
        match AsyncIna219::new_calibrated(i2c, Address::from_byte(0x40).unwrap(), calib).await {
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
                let battery_percentage =
                    (voltage.voltage_mv() as f32 / 1000.0 - 3.6) / (4.1 - 3.6) * 100.0;
                publish!(&mqtt_sender, "battery/voltage", voltage.voltage_mv());
                publish!(&mqtt_sender, "battery/percentage", battery_percentage);
                break;
            }
            Err(e) => error!("Fail reading ina219: {:?}", e),
        }
        retry += 1;
    }
}
