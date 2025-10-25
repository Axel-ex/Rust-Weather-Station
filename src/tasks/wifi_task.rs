use crate::config::CONFIG;
use embassy_net::Runner;
use embassy_time::Timer;
use esp_radio::wifi::{
    AccessPointConfig, ClientConfig, ModeConfig, WifiApState, WifiController, WifiDevice, WifiEvent,
};
use log::info;

#[embassy_executor::task]
pub async fn runner_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await;
}

#[embassy_executor::task]
pub async fn wifi_task(mut controller: WifiController<'static>) {
    let client_config = ModeConfig::Client(
        ClientConfig::default()
            .with_ssid(CONFIG.ssid.into())
            .with_password(CONFIG.password.into()),
    );
    controller.set_config(&client_config).unwrap();

    info!("start wifi task");
    info!("Device capabilities: {:?}", controller.capabilities());

    info!("Starting wifi");
    info!(
        "Trying to connect to {}, pass: {}",
        CONFIG.ssid, CONFIG.password
    );

    controller.start_async().await.unwrap();
    info!("Wifi started!");

    loop {
        match esp_radio::wifi::ap_state() {
            WifiApState::Started => {
                info!("About to connect...");

                match controller.connect_async().await {
                    Ok(_) => {
                        // wait until we're no longer connected
                        info!("STA connected!");
                        controller.wait_for_event(WifiEvent::StaDisconnected).await;
                        info!("STA disconnected");
                    }
                    Err(e) => {
                        info!("Failed to connect to wifi: {e:?}");
                        Timer::after_millis(5000).await
                    }
                }
            }
            _ => return,
        }
    }
}
