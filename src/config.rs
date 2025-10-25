#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    ssid: &'static str,
    #[default("")]
    password: &'static str,
    #[default("")]
    broker_url: &'static str,
}
