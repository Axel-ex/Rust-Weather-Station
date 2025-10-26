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
    #[default(1200)]
    deep_sleep_dur_secs: u64,
    #[default(65)]
    main_task_dur_secs: u64,
    #[default(60)]
    task_dur_secs: u64,
}
