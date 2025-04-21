#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_imxrt::gpio;
use embassy_imxrt_perf_examples::SYSTEMVIEW;
use embassy_time::Timer;
use rtos_trace;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_imxrt::init(Default::default());

    // Initialize SystemView tracing
    SYSTEMVIEW.init();

    info!("Initializing GPIO");
    let mut led = gpio::Output::new(
        p.PIO0_26,
        gpio::Level::Low,
        gpio::DriveMode::PushPull,
        gpio::DriveStrength::Normal,
        gpio::SlewRate::Standard,
    );

    loop {
        led.toggle();
        Timer::after_millis(1000).await;
    }
}
