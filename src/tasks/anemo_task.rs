//! anemo task
//!
//! calulate the rotations of the anemo and publish it.
//!
//! More details about this module.
use core::fmt::Write as _;
use embassy_futures::select::{select, Either};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};
use embassy_time::{Duration, Instant, Ticker};
use esp_hal::gpio::Input;
use heapless::String;

use crate::{
    config::CONFIG,
    tasks::mqtt_task::{MqttPacket, CHANNEL_SIZE, DEFAULT_STRING_SIZE},
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
        let edge = anemo_pin.wait_for_any_edge();
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

    let mut payload = String::<DEFAULT_STRING_SIZE>::new();
    write!(&mut payload, "{}", caclulate_windspeed(rotations)).unwrap();
    let mut topic = String::<DEFAULT_STRING_SIZE>::new();
    write!(&mut topic, "{}/anemo/wind_speed", CONFIG.topic).unwrap();
    let packet = MqttPacket::new(&topic, &payload);

    mqtt_sender.send(packet).await;
}

fn caclulate_windspeed(rotations: u64) -> f32 {
    rotations as f32 * (1.05 / CONFIG.task_dur_secs as f32) * 3.6
}
