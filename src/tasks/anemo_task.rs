use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};
use embassy_time::{Instant, Timer};
use esp_hal::gpio::Input;

use crate::tasks::mqtt_task::{MqttPacket, CHANNEL_SIZE};

#[embassy_executor::task]
pub async fn anemo_task(
    mut anemo_pin: Input<'static>,
    mqtt_sender: Sender<'static, CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE>,
) {
    let mut rotations: u64 = 0;
    let start = Instant::now();

    loop {
        anemo_pin.wait_for_any_edge().await;
        rotations += 1;
        Timer::after_millis(200).await; // debounce
    }
}
