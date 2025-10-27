//! anemo task
//!
//! calulate the rotations of the anemo and publish it.
//!
//! More details about this module.
use embassy_futures::select::{select, Either};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};
use embassy_time::{Duration, Instant, Ticker};
use esp_hal::gpio::Input;

use crate::{
    config::{CHANNEL_SIZE, CONFIG},
    tasks::mqtt_task::MqttPacket,
};

const DEBOUNCE: Duration = Duration::from_millis(5);

#[embassy_executor::task]
pub async fn anemo_task(
    mut anemo_pin: Input<'static>,
    mqtt_sender: Sender<'static, CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE>,
) {
    let mut rotations: u64 = 0;
    let mut ticker = Ticker::every(Duration::from_secs(CONFIG.task_dur_secs));
    let mut last = Instant::now() - DEBOUNCE;

    loop {
        let edge = anemo_pin.wait_for_falling_edge();
        let tick = ticker.next();

        match select(edge, tick).await {
            Either::First(()) => {
                let now = Instant::now();
                if now.duration_since(last) >= DEBOUNCE {
                    rotations += 1;
                    last = now;
                }
            }
            Either::Second(()) => {
                break;
            }
        }
    }
    publish!(
        &mqtt_sender,
        "anemo/wind_speed",
        caclulate_windspeed(rotations)
    );
}

fn caclulate_windspeed(rotations: u64) -> f32 {
    rotations as f32 * (1.05 / CONFIG.task_dur_secs as f32) * 3.6 * 5.0
}
