use crate::tasks::mqtt_task::{MqttPacket, CHANNEL_SIZE};
use embassy_futures::select::{select, Either};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};
use embassy_time::{Duration, Instant, Timer};
use esp_hal::gpio::{Input, InputConfig, Pull};
use esp_hal::peripherals::GPIO25;

const DEBOUNCE: Duration = Duration::from_millis(5);

pub async fn pluvio_window(
    pin: &mut GPIO25<'_>,
    mqtt_sender: Sender<'static, CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE>,
    dur: Duration,
) {
    // Short-lived driver from the borrowed pin
    let mut in_pin = Input::new(pin.reborrow(), InputConfig::default().with_pull(Pull::Up));

    let deadline = Instant::now() + dur;

    let mut last = Instant::now() - DEBOUNCE;
    let mut pulses: u32 = 0;

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        // Wait until either an edge or the deadline (cheap timeout)
        let remaining = deadline - now;
        match select(in_pin.wait_for_rising_edge(), Timer::after(remaining)).await {
            Either::First(()) => {
                let t = Instant::now();
                if t.duration_since(last) >= DEBOUNCE {
                    pulses += 1;
                    last = t;
                }
            }
            Either::Second(()) => break,
        }
    }

    // Publish one packet for the window
    use core::fmt::Write as _;
    let mut payload: heapless::String<32> = heapless::String::new();
    let _ = write!(&mut payload, "rain_pulses: {}", pulses);
    let pkt = MqttPacket::new("pluvio", &payload);
    mqtt_sender.send(pkt).await;
}
