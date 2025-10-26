use embassy_sync::channel::Sender;

use crate::tasks::mqtt_task::{MqttPacket, CHANNEL_SIZE};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

#[embassy_executor::task]
pub async fn as5600_task(
    mqtt_sender: Sender<'static, CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE>,
) {
}
