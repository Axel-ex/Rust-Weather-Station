// GLOBAL ATOMIC VAR

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub static RAIN_FLAG: AtomicBool = AtomicBool::new(false);
pub static ROTATION_FLAG: AtomicBool = AtomicBool::new(false);
pub static ROTATION_COUNT: AtomicU32 = AtomicU32::new(0);
pub static RAIN_COUNT: AtomicU32 = AtomicU32::new(0);

pub fn rain_pin_callback() {
    RAIN_FLAG.store(true, Ordering::Relaxed);
}

pub fn anemo_pin_callback() {
    ROTATION_FLAG.store(true, Ordering::Relaxed);
}
