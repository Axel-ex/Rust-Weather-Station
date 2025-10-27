use core::fmt::Write as _;

use crate::config::{CHANNEL_SIZE, CONFIG, PAYLOAD_SIZE, TOPIC_SIZE};
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

use heapless::String;

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

    let mut topic_voltage = String::<TOPIC_SIZE>::new();
    let mut topic_percentage = String::<TOPIC_SIZE>::new();
    let mut payload_voltage = String::<PAYLOAD_SIZE>::new();
    let mut payload_percentage = String::<PAYLOAD_SIZE>::new();
    let battery_voltage = ina.bus_voltage().await.unwrap().voltage_mv() as f32;
    let battery_percentage = (battery_voltage / 1000.0 - 3.6) / (4.1 - 3.6) * 100.0;

    write!(topic_voltage, "{}/battery/voltage", CONFIG.topic).unwrap();
    write!(topic_percentage, "{}/battery/percentage", CONFIG.topic).unwrap();
    write!(payload_voltage, "{}", battery_voltage).unwrap();
    write!(payload_percentage, "{}", battery_percentage).unwrap();

    let voltage_packet = MqttPacket::new(&topic_voltage, &payload_voltage);
    let percentage_packet = MqttPacket::new(&topic_percentage, &payload_percentage);

    mqtt_sender.send(voltage_packet).await;
    mqtt_sender.send(percentage_packet).await;
}
