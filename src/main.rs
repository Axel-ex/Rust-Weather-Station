#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_hal::{
    clock::CpuClock,
    rtc_cntl::wakeup_cause,
    system::{software_reset, SleepSource},
    timer::timg::TimerGroup,
};
use log::info;
use weather_station_embassy::{
    init_watchdog,
    network::bring_network_up,
    rtc_manager::RtcManager,
    run_active_window,
    sensors::Sensors,
    tasks::ota_task::{check_for_ota, init_ota},
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    software_reset();
}

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let p = esp_hal::init(config);
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 98767);

    let mut watchdog = init_watchdog(p.TIMG1);

    //Instanciate peripherals and i2c bus
    let sensors = Sensors::new(p.GPIO17, p.GPIO32, p.GPIO27, p.GPIO21, p.GPIO22, p.I2C0);

    esp_rtos::start(TimerGroup::new(p.TIMG0).timer0);

    let mut rtc_manager = RtcManager::new(p.GPIO25, p.LPWR);
    rtc_manager.init_next_full_measurement();

    if let SleepSource::Ext0 = wakeup_cause() {
        rtc_manager.handle_external_wakeup().await;
    }

    let stack = bring_network_up(p.WIFI, &spawner).await;

    let ota_handle = init_ota(p.FLASH);
    check_for_ota(stack, ota_handle, &mut watchdog).await;

    run_active_window(&spawner, &mut rtc_manager, &mut watchdog, sensors, stack).await;
    watchdog.disable();

    info!("Going to sleep...");
    Timer::after_secs(1).await;
    rtc_manager.sleep();
    panic!();
}
