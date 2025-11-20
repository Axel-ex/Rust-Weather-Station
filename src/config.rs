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
    #[default(35)]
    main_task_dur_secs: u64,
    #[default(30)]
    task_dur_secs: u64,
    #[default(60)]
    watchdog_timeout_secs: u64,
    #[default("url")]
    ota_url: &'static str,
}

pub const MAX_RETRY: i32 = 5;
pub const SOCKET_TIMEOUT: u64 = 120;
pub const BUFFER_SIZE: usize = 2048;
pub const DEFAULT_STRING_SIZE: usize = 70;
pub const PAYLOAD_SIZE: usize = 20;
pub const TOPIC_SIZE: usize = 70;
pub const CHANNEL_SIZE: usize = 5;
