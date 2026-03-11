use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, TimeoutError, WithTimeout};
use esp_hal::{peripherals::WIFI, rng::Rng};
use esp_radio::{
    wifi::{WifiController, WifiDevice},
    Controller,
};

pub fn init_network(
    wifi: WIFI<'static>,
) -> (
    WifiController<'static>,
    Stack<'static>,
    Runner<'static, WifiDevice<'static>>,
) {
    let radio_init = mk_static!(
        Controller<'static>,
        esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller")
    );
    let (controller, interfaces) = esp_radio::wifi::new(radio_init, wifi, Default::default())
        .expect("Failed to initialize Wi-Fi/BLE controller");

    // Net stack
    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    let (stack, runner) = embassy_net::new(
        interfaces.sta,
        embassy_net::Config::dhcpv4(Default::default()),
        mk_static!(StackResources<6>, StackResources::<6>::new()),
        seed,
    );

    (controller, stack, runner)
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
