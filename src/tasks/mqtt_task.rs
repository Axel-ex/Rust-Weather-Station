use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use heapless::String;

pub const DEFAULT_STRING_SIZE: usize = 30;
pub const CHANNEL_SIZE: usize = 5;
pub static MQTT_CHANNEL: Channel<
    CriticalSectionRawMutex,
    String<DEFAULT_STRING_SIZE>,
    CHANNEL_SIZE,
> = Channel::new();

pub async fn mqtt_task(
    mqtt_receiver: Receiver<
        'static,
        CriticalSectionRawMutex,
        String<DEFAULT_STRING_SIZE>,
        CHANNEL_SIZE,
    >,
) {

    //TODO: Create client config
    //TODO: Initiate client
    //TODO: need to publish: dht readings, anemo readings, wind speed, rain, battery before
    //entering deep sleep.
}
