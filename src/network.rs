use embassy_net::{Runner, Stack, StackResources};
use esp_hal::{peripherals::WIFI, rng::Rng};
use esp_radio::{
    wifi::{WifiController, WifiDevice},
    Controller,
};

pub struct NetworkManager {
    pub controller: WifiController<'static>,
    pub stack: Stack<'static>,
    pub runner: Runner<'static, WifiDevice<'static>>,
}

impl NetworkManager {
    pub fn new(wifi: WIFI<'static>) -> Self {
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

        Self {
            controller,
            stack,
            runner,
        }
    }
    //
    // pub async fn wait_for_stack(&self) -> Result<(), TimeoutError> {
    //     self.stack
    //         .wait_config_up()
    //         .with_timeout(Duration::from_secs(30))
    //         .await?;
    //
    //     self.stack
    //         .wait_link_up()
    //         .with_timeout(Duration::from_secs(30))
    //         .await?;
    //
    //     Ok(())
    // }
}
