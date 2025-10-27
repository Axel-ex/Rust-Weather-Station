use crate::config::{CHANNEL_SIZE, MAX_RETRY};
use crate::tasks::mqtt_task::MqttPacket;
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
    Timer::after_secs(3).await;

    let mut retry = 0;

    while retry < MAX_RETRY {
        match dht22_async::read(&mut delay, &mut dht_pin).await {
            Ok(reading) => {
                info!(
                    "Got {}C {}%",
                    reading.temperature, reading.relative_humidity
                );
                publish!(&mqtt_sender, "temperature", reading.temperature);
                publish!(&mqtt_sender, "humidity", reading.relative_humidity);
                break;
            }
            Err(e) => {
                error!("Fail reading DHT {e:?}");
            }
        }
        retry += 1;
        Timer::after_secs(1).await;
    }
}
