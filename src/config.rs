#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    ssid: &'static str,
    #[default("")]
    password: &'static str,
    #[default("test.mosquitto.org")]
    broker_url: &'static str,
    #[default("weather_station")]
    topic: &'static str,
}
