#![no_std]

use mimxrt600_fcb::FlexSPIFlashConfigurationBlock;
use panic_probe as _;

// auto-generated version information from Cargo.toml
include!(concat!(env!("OUT_DIR"), "/biv.rs"));

#[link_section = ".otfad"]
#[used]
static OTFAD: [u8; 256] = [0; 256];

#[rustfmt::skip]
#[link_section = ".fcb"]
#[used]
static FCB: FlexSPIFlashConfigurationBlock = FlexSPIFlashConfigurationBlock::build();

#[link_section = ".keystore"]
#[used]
static KEYSTORE: [u8; 2048] = [0; 2048];

#[cfg(feature = "systemview-tracing")]
mod systemview_tracing {
    use systemview_target::SystemView;
    pub static SYSTEMVIEW: systemview_target::SystemView = systemview_target::SystemView::new();
    rtos_trace::global_trace! {SystemView}

    struct TraceInfo();

    impl rtos_trace::RtosTraceApplicationCallbacks for TraceInfo {
        fn system_description() {}
        fn sysclock() -> u32 {
            25_000_000
        }
    }
    rtos_trace::global_application_callbacks! {TraceInfo}

    // Stub implementations for defmt since defmt_rtt cannot be used when systemview RTT is enabled
    #[no_mangle]
    pub unsafe extern "C" fn _defmt_write(_bytes: *const u8, _len: usize) {
        // Stub implementation - does nothing
        // In a real implementation, this would write log data to some output
    }

    #[no_mangle]
    pub unsafe extern "C" fn _defmt_acquire() -> isize {
        // Stub implementation - return a dummy value
        // In a real implementation, this would acquire some resource like a mutex
        0
    }

    #[no_mangle]
    pub unsafe extern "C" fn _defmt_release(_token: isize) {
        // Stub implementation - does nothing
        // In a real implementation, this would release the resource acquired by defmt_acquire
    }

    #[no_mangle]
    pub unsafe extern "C" fn _defmt_timestamp() -> u64 {
        // Stub implementation - return a dummy timestamp
        // In a real implementation, this would return the current system time
        0
    }
}

#[cfg(feature = "systemview-tracing")]
pub use systemview_tracing::SYSTEMVIEW;
