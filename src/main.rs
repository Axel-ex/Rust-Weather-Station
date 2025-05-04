pub mod global;
mod mqtt;
mod weather_station;
mod wifi;
use weather_station::*;

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    mqtt_user: &'static str,
    #[default("")]
    mqtt_pass: &'static str,
    #[default("")]
    broker_url: &'static str,
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_pass: &'static str,
    #[default("")]
    topic: &'static str,
    #[default("")]
    client_id: &'static str,
    #[default(600_000_000)]
    deep_sleep_interval_us: u64,
    #[default(61)]
    active_duration_s: u64,
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    esp_idf_svc::log::set_target_level("*", log::LevelFilter::Debug).unwrap();

    let mut weather_station = WeatherStation::new();

    weather_station
        .set_interrupt()
        .expect("Fail setting interrupts");
    weather_station.run();
}
