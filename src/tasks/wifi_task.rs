use crate::config::CONFIG;
use embassy_net::Runner;
use esp_radio::wifi::{ClientConfig, ModeConfig, WifiController, WifiDevice, WifiEvent};

#[embassy_executor::task]
pub async fn runner_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await;
}

#[embassy_executor::task]
pub async fn wifi_task(mut controller: WifiController<'static>) {
    // STA-only
    let client_cfg = ModeConfig::Client(
        ClientConfig::default()
            .with_ssid(CONFIG.ssid.into())
            .with_password(CONFIG.password.into()),
    );

    controller.set_config(&client_cfg).unwrap();

    log::info!("Starting WiFi (STA)...");

    controller.start_async().await.unwrap();

    loop {
        match controller.connect_async().await {
            Ok(()) => {
                log::info!("STA connected");
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                log::info!("STA disconnected, retrying in 5s");
            }
            Err(e) => {
                log::info!("connect_async() failed: {e:?}. Retrying in 5s");
            }
        }
        embassy_time::Timer::after_millis(5000).await;
    }
}
