use anyhow::Result;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::modem::Modem,
    netif::NetifStatus,
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use weather_station::*;

pub fn wifi_init<'a>(modem: Modem) -> Result<BlockingWifi<EspWifi<'a>>> {
    let sys_loop = EspSystemEventLoop::take().expect("wifi_init: fail taking eventloop");
    let nvs = EspDefaultNvsPartition::take().expect("wifi_init: fail taking nvs");

    let wifi = BlockingWifi::wrap(EspWifi::new(modem, sys_loop.clone(), Some(nvs))?, sys_loop)?;

    Ok(wifi)
}

pub fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> Result<()> {
    let wifi_config: Configuration = Configuration::Client(ClientConfiguration {
        ssid: heapless::String::try_from(CONFIG.wifi_ssid).expect("Invalid WIFI SSID"),
        bssid: None,
        password: heapless::String::try_from(CONFIG.wifi_pass).expect("Invalid WiFi password"),
        ..Default::default()
    });

    wifi.set_configuration(&wifi_config)?;
    log::info!("Starting wifi");
    wifi.start()?;

    log::info!("Connecting.....");
    wifi.connect()?;

    wifi.wait_netif_up()?;
    log::info!("Netif up");

    Ok(())
}
