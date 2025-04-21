#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_imxrt::gpio;
use embassy_imxrt_perf_examples::SYSTEMVIEW;
use embassy_time::Timer;
use panic_probe as _;

// System status
static mut SYSTEM_STATUS: u32 = 0;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_imxrt::init(Default::default());

    SYSTEMVIEW.init();

    let mut led = gpio::Output::new(
        p.PIO0_26,
        gpio::Level::Low,
        gpio::DriveMode::PushPull,
        gpio::DriveStrength::Normal,
        gpio::SlewRate::Standard,
    );

    loop {
        led.toggle();
        unsafe {
            SYSTEM_STATUS += 1;
        }
        Timer::after_millis(1000).await;
    }
}
