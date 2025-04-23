#![no_std]
#![no_main]

extern crate embassy_imxrt_perf_examples;

use embassy_executor::Spawner;
use embassy_imxrt::gpio;
use embassy_imxrt::i2c;
use embassy_time::Timer;
use embedded_hal_1::i2c::I2c;
use panic_probe as _;
use rtos_trace;

#[cfg(feature = "systemview-tracing")]
use embassy_imxrt_perf_examples::SYSTEMVIEW;

// System status
static mut SYSTEM_STATUS: u32 = 0;

// Accelerometer constants
const ACC_ADDR: u8 = 0x1E;
const ACC_ID: u8 = 0xC7;
const ACC_ID_REG: u8 = 0x0D;
const ACC_CTRL_REG: u8 = 0x2A;
const ACC_XYZ_DATA_CFG_REG: u8 = 0x0E;
const ACC_STATUS_REG: u8 = 0x00;
const ACC_STATUS_DATA_READY: u8 = 0xFF;

// RTOS Trace Markers
#[repr(u32)]
enum AccelTraceMarker {
    Read = 0x10,
    ReadIdErr = 0x11,
    ReadErr = 0x12,
    Write = 0x20,
    WriteErr = 0x21,
}

#[embassy_executor::task]
async fn accelerometer_task_blocking() {}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_imxrt::init(Default::default());

    #[cfg(feature = "systemview-tracing")]
    SYSTEMVIEW.init();

    // Initialize peripherals
    use embassy_imxrt::gpio::*;
    let mut led = gpio::Output::new(
        p.PIO0_26,
        gpio::Level::Low,
        gpio::DriveMode::PushPull,
        gpio::DriveStrength::Normal,
        gpio::SlewRate::Standard,
    );

    // Configure accelerometer
    let mut _reset_pin = Output::new(
        p.PIO1_7,
        Level::Low,
        DriveMode::PushPull,
        DriveStrength::Normal,
        SlewRate::Standard,
    );
    let _isr_pin = Input::new(p.PIO1_5, Pull::Down, Inverter::Disabled);
    let mut i2c =
        i2c::master::I2cMaster::new_blocking(p.FLEXCOMM2, p.PIO0_18, p.PIO0_17, i2c::master::Speed::Standard).unwrap();

    // Check accelerometer ID
    let mut reg = [0u8; 1];
    reg[0] = 0xAA;
    match i2c.write_read(ACC_ADDR, &[ACC_ID_REG], &mut reg) {
        Ok(_) if reg[0] == ACC_ID => rtos_trace::trace::marker(AccelTraceMarker::Read as u32),
        Ok(_) => {
            rtos_trace::trace::marker(AccelTraceMarker::ReadIdErr as u32);
        }
        Err(_) => {
            rtos_trace::trace::marker(AccelTraceMarker::ReadErr as u32);
        }
    }

    // Write to accelerometer control register
    let mut reg = [0u8; 2];
    reg[0] = ACC_CTRL_REG;
    reg[1] = 0x00;
    match i2c.write(ACC_ADDR, &reg) {
        Ok(_) => rtos_trace::trace::marker(AccelTraceMarker::Write as u32),
        Err(_) => rtos_trace::trace::marker(AccelTraceMarker::WriteErr as u32),
    }

    let mut reg = [0u8; 2];
    reg[0] = ACC_XYZ_DATA_CFG_REG;
    reg[1] = 0x01;
    match i2c.write(ACC_ADDR, &reg) {
        Ok(_) => rtos_trace::trace::marker(AccelTraceMarker::Write as u32),
        Err(_) => rtos_trace::trace::marker(AccelTraceMarker::WriteErr as u32),
    }

    let mut reg = [0u8; 1];
    reg[0] = 0xAA;
    while reg[0] != ACC_STATUS_DATA_READY {
        let result = i2c.write_read(ACC_ADDR, &[ACC_STATUS_REG], &mut reg);
        if result.is_ok() {
        } else {
        }
    }

    /* Accelerometer status register, first byte always 0xFF, then X:Y:Z each 2 bytes, in total 7 bytes */
    for _ in 0..10 {
        let mut reg: [u8; 7] = [0xAA; 7];
        let result = i2c.write_read(ACC_ADDR, &[ACC_STATUS_REG], &mut reg);
        if result.is_ok() {
        } else {
        }
    }

    loop {
        led.toggle();
        unsafe {
            SYSTEM_STATUS += 1;
        }
        Timer::after_millis(1000).await;
    }
}
