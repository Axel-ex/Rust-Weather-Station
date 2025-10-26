use crate::tasks::mqtt_task::MqttPacket;

use super::mqtt_task::CHANNEL_SIZE;
use alloc::format;
use dht_sensor::dht22::r#async as dht22_async;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};
use embassy_time::{Delay, Timer};
use esp_hal::gpio::{DriveMode, Flex, OutputConfig, Pull};
use log::{error, info};

#[embassy_executor::task]
pub async fn dht_task(
    mut dht_pin: Flex<'static>,
    mqtt_sender: Sender<'static, CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE>,
) {
    let mut delay = Delay;
    info!("Starting the dht");
    // Configure as open-drain with pull-up, then enable output+input
    dht_pin.apply_output_config(
        &OutputConfig::default()
            .with_drive_mode(DriveMode::OpenDrain)
            .with_pull(Pull::Up),
    );
    dht_pin.set_output_enable(true);
    dht_pin.set_input_enable(true);
    dht_pin.set_high(); // release the bus (idle high via pull-up)

    //WARN: maybe while is err we keep trying
    Timer::after_secs(30).await;
    match dht22_async::read(&mut delay, &mut dht_pin).await {
        Ok(reading) => {
            info!(
                "Got {}C {}%",
                reading.temperature, reading.relative_humidity
            );
            let payload = format!(
                "temparature: {}, humidity: {}",
                reading.temperature, reading.relative_humidity
            );
            let packet = MqttPacket::new("dht22", payload.as_str());
            mqtt_sender.send(packet).await;
        }
        Err(e) => {
            error!("Fail reading DHT {e:?}");
        }
    }
}
