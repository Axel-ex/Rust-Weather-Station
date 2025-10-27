use core::fmt::Write as _;

use crate::{config::CONFIG, tasks::mqtt_task::MqttPacket};

use super::mqtt_task::{CHANNEL_SIZE, PAYLOAD_SIZE, TOPIC_SIZE};
use dht_sensor::dht22::r#async as dht22_async;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};
use embassy_time::{Delay, Timer};
use esp_hal::gpio::{DriveMode, Flex, OutputConfig, Pull};
use heapless::String;
use log::{debug, error, info};

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
    Timer::after_secs(3).await;

    //WARN: maybe while is err we keep trying
    match dht22_async::read(&mut delay, &mut dht_pin).await {
        Ok(reading) => {
            info!(
                "Got {}C {}%",
                reading.temperature, reading.relative_humidity
            );
            let mut dht_topic = String::<TOPIC_SIZE>::new();
            let mut hum_topic = String::<TOPIC_SIZE>::new();
            write!(&mut dht_topic, "{}/temperature", CONFIG.topic).ok();
            write!(&mut hum_topic, "{}/humidity", CONFIG.topic).ok();

            let mut temp_payload = String::<PAYLOAD_SIZE>::new();
            let mut hum_payload = String::<PAYLOAD_SIZE>::new();
            write!(&mut temp_payload, "{}", reading.temperature).ok();
            write!(&mut hum_payload, "{}", reading.relative_humidity).ok();

            let temp_packet = MqttPacket::new(&dht_topic, &temp_payload);
            let hum_packet = MqttPacket::new(&hum_topic, &hum_payload);
            mqtt_sender.send(temp_packet).await;
            mqtt_sender.send(hum_packet).await;
        }
        Err(e) => {
            error!("Fail reading DHT {e:?}");
        }
    }
}
