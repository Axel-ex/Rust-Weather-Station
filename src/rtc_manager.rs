use crate::config::CONFIG;
use embassy_time::Timer;
use esp_hal::ram;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::{
    peripherals::{GPIO25, LPWR},
    rtc_cntl::sleep::{Ext0WakeupSource, RtcSleepConfig, TimerWakeupSource},
};
use log::info;

//Variables store in RTC
#[ram(unstable(rtc_fast), unstable(persistent))]
static mut NEXT_FULL_MEASUREMENT_S: u64 = 0;
#[ram(unstable(rtc_fast), unstable(persistent))]
static mut LAST_TIP: u64 = 0;
#[ram(unstable(rtc_fast), unstable(persistent))]
static mut RAIN_TIPS: u32 = 0;

//RTC utils

pub struct RtcManager {
    rtc: Rtc<'static>,
    rtc_cfg: RtcSleepConfig,
    ext0: Ext0WakeupSource<GPIO25<'static>>,
    deep_sleep_timer: TimerWakeupSource,
}

impl RtcManager {
    pub fn new(rain_pin: GPIO25<'static>, lpwr: LPWR<'static>) -> Self {
        let mut rtc_cfg = RtcSleepConfig::deep();
        rtc_cfg.set_rtc_fastmem_pd_en(false);
        let rtc = Rtc::new(lpwr);

        RtcManager {
            rtc,
            rtc_cfg,
            ext0: Ext0WakeupSource::new(rain_pin, esp_hal::rtc_cntl::sleep::WakeupLevel::Low),
            deep_sleep_timer: TimerWakeupSource::new(core::time::Duration::from_secs(
                CONFIG.deep_sleep_dur_secs,
            )),
        }
    }

    pub fn init_next_full_measurement(&self) {
        let now = self.rtc.time_since_boot().as_secs();
        let mut next_full = self.load_next_full_measurement_s();
        if next_full == 0 {
            next_full = now + CONFIG.deep_sleep_dur_secs;
            self.store_next_full_measurement_s(next_full);
        } //first boot, we set the timer to sleep for deep sleep dur
    }

    pub async fn handle_external_wakeup(&mut self) {
        let now = self.rtc.time_since_boot().as_secs();
        let next_full = self.load_next_full_measurement_s();
        let remaining = next_full - now;
        let sleep_secs = core::cmp::max(remaining, 1); //avoid 0

        self.set_deep_sleep_timer(core::time::Duration::from_secs(sleep_secs as u64));

        self.inc_rain_tips(now);
        Timer::after_millis(500).await;
        self.sleep();
    }

    pub fn set_deep_sleep_timer(&mut self, duration: core::time::Duration) {
        self.deep_sleep_timer = TimerWakeupSource::new(duration);
    }

    pub fn set_next_full_measurement_s(&self, duration: u64) {
        self.store_next_full_measurement_s(self.rtc.time_since_boot().as_secs() + duration);
    }

    pub fn sleep(&mut self) {
        self.rtc
            .sleep(&self.rtc_cfg, &[&self.ext0, &self.deep_sleep_timer]);
    }

    //direct manipulation of rtc memory
    pub fn load_rain_tips(&self) -> u32 {
        let rain_tips = unsafe { RAIN_TIPS };
        if rain_tips > 100 {
            0
        } else {
            rain_tips
        } //avoid unitialized weird values
    }

    pub fn store_rain_tips(&self, v: u32) {
        unsafe {
            RAIN_TIPS = v;
        }
    }

    pub fn load_next_full_measurement_s(&self) -> u64 {
        unsafe { NEXT_FULL_MEASUREMENT_S }
    }

    pub fn store_next_full_measurement_s(&self, v: u64) {
        unsafe {
            NEXT_FULL_MEASUREMENT_S = v;
        }
    }

    pub fn load_last_tip(&self) -> u64 {
        unsafe { LAST_TIP }
    }

    pub fn store_last_tip(&self, v: u64) {
        unsafe {
            LAST_TIP = v;
        }
    }

    pub fn inc_rain_tips(&self, now: u64) {
        let cur = self.load_rain_tips();
        let last_tip = self.load_last_tip();

        if cur == 0 {
            self.store_rain_tips(cur.saturating_add(1));
            self.store_last_tip(now);
            return;
        }

        if last_tip != 0 && now.saturating_sub(last_tip) > CONFIG.rain_debounce_s {
            self.store_rain_tips(cur.saturating_add(1));
            self.store_last_tip(now);
            info!("Incremented to {}", self.load_rain_tips());
        } else {
            info!("Sensor must be stuck");
        }
    }
}
