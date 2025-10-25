#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::future::pending;

use embassy_executor::Spawner;
use embassy_net::{Config, Ipv4Address, Ipv4Cidr, StackResources, StaticConfigV4};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Flex;
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_radio::Controller;
use heapless::Vec;

use log::info;

pub mod config;
pub mod tasks;

use crate::tasks::mqtt_task::{mqtt_task, MQTT_CHANNEL};
use crate::tasks::wifi_task::{runner_task, wifi_task};
use tasks::dht_task::dht_task;

//TODO: call the reset from esp_idf_sys in the panic handler
// #[panic_handler]
// fn panic(_: &core::panic::PanicInfo) -> ! {
//     loop {}
// }

macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        STATIC_CELL.init_with(|| $val)
    }};
}
extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 98767); // Why do we
                                                                                       // need to
                                                                                       // declare
                                                                                       // this
                                                                                       // section?

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    // Init wifi
    let radio_init = mk_static!(
        Controller<'static>,
        esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller")
    );

    let (controller, interfaces) =
        esp_radio::wifi::new(radio_init, peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");

    //DHT_PIN
    let dht_pin = Flex::new(peripherals.GPIO32);
    let sender = MQTT_CHANNEL.sender();
    let receiver = MQTT_CHANNEL.receiver();

    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    // // Init network stack, configure static IP since DHCP doesnt seem to want to colaborate
    let mut dns_servers: heapless::Vec<Ipv4Address, 3> = heapless::Vec::new();
    dns_servers.push(Ipv4Address::new(192, 168, 1, 1)).unwrap();
    dns_servers.push(Ipv4Address::new(1, 1, 1, 1)).unwrap(); // Cloudflare
    dns_servers.push(Ipv4Address::new(8, 8, 8, 8)).unwrap(); // Google
                                                             //
    let cfg = Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 1, 123), 24),
        gateway: Some(Ipv4Address::new(192, 168, 1, 1)),
        dns_servers,
    });

    let (stack, runner) = embassy_net::new(
        interfaces.sta,
        cfg,
        mk_static!(StackResources<6>, StackResources::<6>::new()),
        seed,
    );

    spawner.spawn(runner_task(runner)).ok();
    spawner.spawn(wifi_task(controller)).ok();
    spawner.spawn(mqtt_task(stack, receiver)).unwrap();
    spawner.spawn(dht_task(dht_pin, sender)).ok();

    loop {
        info!("Hello world!");
        Timer::after(Duration::from_secs(1)).await;
        pending::<()>().await;
    }
}
