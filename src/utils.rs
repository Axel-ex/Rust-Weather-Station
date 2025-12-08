use crate::config::CONFIG;
use embassy_net::Stack;
use embassy_time::{Duration, TimeoutError, WithTimeout};
use esp_hal::ram;
use esp_hal::rtc_cntl::Rtc;
use log::info;

// seconds since boot for next full measurement
#[ram(unstable(rtc_fast), unstable(persistent))]
static mut NEXT_FULL_MEASUREMENT_S: u64 = 0;
#[ram(unstable(rtc_fast), unstable(persistent))]
static mut LAST_TIP: u64 = 0;

pub fn load_next_full_measurement_s() -> u64 {
    unsafe { NEXT_FULL_MEASUREMENT_S }
}

pub fn store_next_full_measurement_s(v: u64) {
    unsafe {
        NEXT_FULL_MEASUREMENT_S = v;
    }
}

pub fn load_last_tip() -> u64 {
    unsafe { LAST_TIP }
}

pub fn store_last_tip(v: u64) {
    unsafe {
        LAST_TIP = v;
    }
}

// helper to get “now” in seconds since boot, from Rtc
pub fn now_s(rtc: &Rtc<'_>) -> u64 {
    let d = rtc.time_since_boot(); // esp-hal API
    d.as_secs()
}

// Instead of waking up with the rain snesor and powering up the whole wifi stack, its preferable
// to do a minimum of work -> just record the tipping of the pluvimeter in the RAIN_TIPS and sleep.
#[ram(unstable(rtc_fast), unstable(persistent))]
static mut RAIN_TIPS: u32 = 0;

pub fn load_rain_tips() -> u32 {
    let rain_tips = unsafe { RAIN_TIPS };
    if rain_tips > 100 { 0 } else { rain_tips } //avoid unitialized weird values
}

pub fn store_rain_tips(v: u32) {
    unsafe {
        RAIN_TIPS = v;
    }
}

pub fn inc_rain_tips(now: u64) {
    let cur = load_rain_tips();
    let last_tip = load_last_tip();

    if cur == 0 {
        store_rain_tips(cur.saturating_add(1));
        store_last_tip(now);
        return;
    }

    if last_tip != 0 && now.saturating_sub(last_tip) > CONFIG.rain_debounce_s {
        store_rain_tips(cur.saturating_add(1));
        store_last_tip(now);
        info!("Incremented to {}", load_rain_tips());
    }
}

pub async fn wait_for_stack(stack: &Stack<'static>) -> Result<(), TimeoutError> {
    stack
        .wait_config_up()
        .with_timeout(Duration::from_secs(30))
        .await?;

    stack
        .wait_link_up()
        .with_timeout(Duration::from_secs(30))
        .await?;

    Ok(())
}

#[macro_export]
macro_rules! publish {
    ($sender:expr, $suffix:expr, $val:expr) => {{
        // Absolute paths + $crate to avoid hygiene issues.
        let mut topic: ::heapless::String<{ $crate::config::TOPIC_SIZE }> =
            ::heapless::String::new();
        let mut payload: ::heapless::String<{ $crate::config::PAYLOAD_SIZE }> =
            ::heapless::String::new();

        let _ = ::core::fmt::Write::write_fmt(
            &mut topic,
            ::core::format_args!("{}/{}", $crate::config::CONFIG.topic, $suffix),
        );
        // This is effectively "{}"
        let _ = ::core::fmt::Write::write_fmt(&mut payload, ::core::format_args!("{}", $val));

        $sender
            .send($crate::tasks::mqtt_task::MqttPacket::new(topic, payload))
            .await;
    }};
}

#[macro_export]
macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        STATIC_CELL.init_with(|| $val)
    }};
}
