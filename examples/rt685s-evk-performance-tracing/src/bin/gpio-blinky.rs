#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_imxrt::gpio;
use embassy_imxrt_perf_examples::SYSTEMVIEW;
use embassy_time::Timer;

#[embassy_executor::task]
async fn led_toggle_task(mut led: gpio::Output<'static>) {
    loop {
        led.toggle();
        Timer::after_millis(1000).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_imxrt::init(Default::default());

    #[cfg(feature = "systemview-tracing")]
    SYSTEMVIEW.init();

    let led = gpio::Output::new(
        p.PIO0_26,
        gpio::Level::Low,
        gpio::DriveMode::PushPull,
        gpio::DriveStrength::Normal,
        gpio::SlewRate::Standard,
    );

    let _ = spawner.spawn_named("led_toggle_task\0", led_toggle_task(led));
}
