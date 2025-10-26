use esp_hal::gpio::Input;

#[embassy_executor::task]
pub async fn pluvio_task(mut input: Input<'static>) {
    let mut counts: u32 = 0;

    loop {
        input.wait_for_low().await;
        counts += 1;
    }
}
