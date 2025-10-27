#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    ssid: &'static str,
    #[default("")]
    wifi_pass: &'static str,
    #[default("test.mosquitto.org")]
    broker_url: &'static str,
    #[default("192.168.1.69")]
    broker_ip: &'static str,
    #[default(1883)]
    broker_port: u16,
    #[default("mqtt_user")]
    mqtt_user: &'static str,
    #[default("")]
    mqtt_pass: &'static str,
    #[default("weather_station")]
    topic: &'static str,
    #[default(1200)]
    deep_sleep_dur_secs: u64,
    #[default(65)]
    main_task_dur_secs: u64,
    #[default(60)]
    task_dur_secs: u64,
}
