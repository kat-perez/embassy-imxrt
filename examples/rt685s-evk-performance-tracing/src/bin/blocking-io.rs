#![no_std]
#![no_main]

extern crate embassy_imxrt_perf_examples;

use embassy_executor::Spawner;
use embassy_imxrt::flexspi_nor_storage_bus::{
    AhbConfig, Blocking as FlexSpiBlocking, FlexSpiBusWidth, FlexSpiFlashPort, FlexSpiFlashPortDeviceInstance,
    FlexspiAhbBufferConfig, FlexspiConfig, FlexspiDeviceConfig, FlexspiNorStorageBus,
};
use embassy_imxrt::pac::flexspi::ahbcr::*;
use embassy_imxrt::pac::flexspi::flshcr1::*;
use embassy_imxrt::pac::flexspi::flshcr2::*;
use embassy_imxrt::pac::flexspi::flshcr4::*;
use embassy_imxrt::pac::flexspi::mcr0::*;
use embassy_imxrt::pac::flexspi::mcr2::*;
use embassy_time::Timer;
use embedded_storage::nor_flash::{
    ErrorType, NorFlash as BlockingNorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash as BlockingReadNorFlash,
};
use storage_bus::nor::{
    BlockingNorStorageBusDriver, NorStorageBusWidth, NorStorageCmd, NorStorageCmdMode, NorStorageCmdType,
    NorStorageDummyCycles,
};

#[cfg(feature = "systemview-tracing")]
use embassy_imxrt_perf_examples::SYSTEMVIEW;

// RTOS Trace Markers
#[repr(u32)]
enum TestTraceMarker {
    ReadStart = 0x10,
    ReadComplete,
    WriteStart,
    WriteComplete,
    EraseStart,
    EraseComplete,
    JedecStart,
    JedecComplete,
    BusConfigStart,
    BusConfigComplete,
}

static ADDR: u32 = 0x3FD0000;

mod sealed {
    /// simply seal a trait
    pub trait Sealed {}
}

impl<T> sealed::Sealed for T {}

/// Driver mode.
#[allow(private_bounds)]
pub trait Mode: sealed::Sealed {}

/// Blocking mode.
pub struct Blocking;
impl Mode for Blocking {}

/// Async mode.
pub struct Async;
impl Mode for Async {}

struct MacronixDeviceDriver<T: BlockingNorStorageBusDriver, M: Mode> {
    // Bus driver dependency
    storagebusdriver: T,
    capacity: usize,
    _phantom: core::marker::PhantomData<M>,
}
#[derive(Debug)]
pub enum NorErrorType {
    /// Nor flash error object for other errors
    FlashStorageErrorOther,

    /// Error object for overflow errror
    FlashStorageErrorOutOfBounds,

    /// Error object for alignment error
    FlashStorageErrorNotAligned,
}

impl<T: BlockingNorStorageBusDriver, M: Mode> ErrorType for MacronixDeviceDriver<T, M> {
    type Error = NorErrorType;
}

impl NorFlashError for NorErrorType {
    fn kind(&self) -> NorFlashErrorKind {
        match self {
            NorErrorType::FlashStorageErrorOther => NorFlashErrorKind::Other,
            NorErrorType::FlashStorageErrorOutOfBounds => NorFlashErrorKind::OutOfBounds,
            NorErrorType::FlashStorageErrorNotAligned => NorFlashErrorKind::NotAligned,
        }
    }
}

impl<T: BlockingNorStorageBusDriver> MacronixDeviceDriver<T, Blocking> {
    pub fn get_jedec_id(&mut self, jedec: &mut [u8]) {
        #[cfg(feature = "systemview-tracing")]
        rtos_trace::trace::marker(TestTraceMarker::JedecStart as u32);

        let read_cread_jedec_id_cmd = NorStorageCmd {
            cmd_lb: 0x9F,
            cmd_ub: Some(0x60),
            addr: Some(0x0),
            addr_width: Some(32),
            bus_width: NorStorageBusWidth::Octal, // 3 - Octal
            mode: NorStorageCmdMode::DDR,
            dummy: NorStorageDummyCycles::Clocks(0x18),
            cmdtype: Some(NorStorageCmdType::Read),
            data_bytes: Some(4),
        };

        let _ = self
            .storagebusdriver
            .send_command(read_cread_jedec_id_cmd, Some(jedec), None);

        #[cfg(feature = "systemview-tracing")]
        rtos_trace::trace::marker(TestTraceMarker::JedecComplete as u32);
    }
}

impl<T: BlockingNorStorageBusDriver> BlockingReadNorFlash for MacronixDeviceDriver<T, Blocking> {
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        #[cfg(feature = "systemview-tracing")]
        rtos_trace::trace::marker(TestTraceMarker::ReadStart as u32);

        #[allow(const_evaluatable_unchecked)]
        let mut read_start_ptr = 0;

        if offset > self.capacity as u32 {
            return Err(NorErrorType::FlashStorageErrorOutOfBounds);
        }
        if offset + bytes.len() as u32 > self.capacity as u32 {
            return Err(NorErrorType::FlashStorageErrorOutOfBounds);
        }

        while read_start_ptr < bytes.len() {
            // Read data from the storage device
            let read_cmd = NorStorageCmd {
                cmd_lb: 0xEE,
                cmd_ub: Some(0x11),
                addr: Some(offset + read_start_ptr as u32),
                addr_width: Some(0x20),
                bus_width: NorStorageBusWidth::Octal,
                mode: NorStorageCmdMode::DDR,
                dummy: NorStorageDummyCycles::Clocks(0x29),
                cmdtype: Some(NorStorageCmdType::Read),
                data_bytes: Some(Self::READ_SIZE as u32),
            };

            let _ = self.storagebusdriver.send_command(
                read_cmd,
                Some(&mut bytes[read_start_ptr..read_start_ptr + Self::READ_SIZE]),
                None,
            );

            read_start_ptr += Self::READ_SIZE;
        }

        #[cfg(feature = "systemview-tracing")]
        rtos_trace::trace::marker(TestTraceMarker::ReadComplete as u32);

        Ok(())
    }

    fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<T: BlockingNorStorageBusDriver> BlockingNorFlash for MacronixDeviceDriver<T, Blocking> {
    const WRITE_SIZE: usize = 1;
    const ERASE_SIZE: usize = 4096;

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        #[cfg(feature = "systemview-tracing")]
        rtos_trace::trace::marker(TestTraceMarker::EraseStart as u32);

        // subtracting 1 as align will give next sector start address
        let mut sector_start_addr = from;
        let sector_end_addr = to;
        let mut status = [0_u8; 4];

        if from > to {
            return Err(NorErrorType::FlashStorageErrorOther);
        }

        if from > self.capacity as u32 {
            return Err(NorErrorType::FlashStorageErrorOutOfBounds);
        }

        if to > self.capacity as u32 {
            return Err(NorErrorType::FlashStorageErrorOutOfBounds);
        }

        if from % Self::ERASE_SIZE as u32 != 0 {
            return Err(NorErrorType::FlashStorageErrorNotAligned);
        }

        if to % Self::ERASE_SIZE as u32 != 0 {
            return Err(NorErrorType::FlashStorageErrorNotAligned);
        }

        // Enable Write
        let write_enable_cmd = NorStorageCmd {
            cmd_lb: 0x06,
            cmd_ub: Some(0xF9),
            addr: None,
            addr_width: None,
            bus_width: NorStorageBusWidth::Octal,
            mode: NorStorageCmdMode::DDR,
            dummy: NorStorageDummyCycles::Clocks(0),
            cmdtype: None,
            data_bytes: None,
        };
        let _ = self.storagebusdriver.send_command(write_enable_cmd, None, None);

        // Check write enable status
        let read_status_cmd = NorStorageCmd {
            cmd_lb: 0x05,
            cmd_ub: Some(0xFA),
            addr: Some(0),
            addr_width: Some(0x20),
            bus_width: NorStorageBusWidth::Octal,
            mode: NorStorageCmdMode::DDR,
            dummy: NorStorageDummyCycles::Clocks(0x14),
            cmdtype: Some(NorStorageCmdType::Read),
            data_bytes: Some(0x4),
        };
        let _ = self
            .storagebusdriver
            .send_command(read_status_cmd, Some(&mut status), None);

        loop {
            if sector_start_addr > sector_end_addr {
                break;
            }
            let _ = self.storagebusdriver.send_command(
                NorStorageCmd {
                    cmd_lb: 0x21,
                    cmd_ub: Some(0xDE),
                    addr: Some(sector_start_addr),
                    addr_width: Some(0x20),
                    bus_width: NorStorageBusWidth::Octal,
                    mode: NorStorageCmdMode::DDR,
                    dummy: NorStorageDummyCycles::Clocks(0),
                    cmdtype: None,
                    data_bytes: None,
                },
                None,
                None,
            );
            loop {
                // Check Erase status
                let read_status_cmd = NorStorageCmd {
                    cmd_lb: 0x05,
                    cmd_ub: Some(0xFA),
                    addr: Some(0),
                    addr_width: Some(0x20),
                    bus_width: NorStorageBusWidth::Octal,
                    mode: NorStorageCmdMode::DDR,
                    dummy: NorStorageDummyCycles::Clocks(0x14),
                    cmdtype: Some(NorStorageCmdType::Read),
                    data_bytes: Some(0x4),
                };
                let _ = self
                    .storagebusdriver
                    .send_command(read_status_cmd, Some(&mut status), None);

                if status[0] & 0x01 == 0 {
                    break;
                }
            }
            sector_start_addr += Self::ERASE_SIZE as u32;
        }

        #[cfg(feature = "systemview-tracing")]
        rtos_trace::trace::marker(TestTraceMarker::EraseComplete as u32);

        Ok(())
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        #[cfg(feature = "systemview-tracing")]
        rtos_trace::trace::marker(TestTraceMarker::WriteStart as u32);

        let bus_ref = &mut self.storagebusdriver;
        let mut status = [0_u8; 3];

        if offset > self.capacity as u32 {
            return Err(NorErrorType::FlashStorageErrorOutOfBounds);
        }

        if offset + bytes.len() as u32 > self.capacity as u32 {
            return Err(NorErrorType::FlashStorageErrorOutOfBounds);
        }

        // Enable Write
        let write_enable_cmd = NorStorageCmd {
            cmd_lb: 0x06,
            cmd_ub: Some(0xF9),
            addr: None,
            addr_width: None,
            bus_width: NorStorageBusWidth::Octal,
            mode: NorStorageCmdMode::DDR,
            dummy: NorStorageDummyCycles::Clocks(0),
            cmdtype: None,
            data_bytes: None,
        };
        let _ = bus_ref.send_command(write_enable_cmd, None, None);

        // Check write enable status
        let read_status_cmd = NorStorageCmd {
            cmd_lb: 0x05,
            cmd_ub: Some(0xFA),
            addr: None,
            addr_width: Some(0x20),
            bus_width: NorStorageBusWidth::Octal,
            mode: NorStorageCmdMode::DDR,
            dummy: NorStorageDummyCycles::Clocks(0x18),
            cmdtype: Some(NorStorageCmdType::Read),
            data_bytes: Some(1),
        };
        let _ = bus_ref.send_command(read_status_cmd, Some(&mut status), None);

        // Page Program
        let mut write_start_ptr = 0;
        let mut write_end_ptr = bytes.len() as u32;

        loop {
            if write_start_ptr > bytes.len() as u32 {
                break;
            }
            let write_cmd = NorStorageCmd {
                cmd_lb: 0x12,
                cmd_ub: Some(0xED),
                addr: Some(offset),
                addr_width: Some(4),
                bus_width: NorStorageBusWidth::Octal,
                mode: NorStorageCmdMode::DDR,
                dummy: NorStorageDummyCycles::Clocks(0),
                cmdtype: Some(NorStorageCmdType::Write),
                data_bytes: Some(Self::WRITE_SIZE as u32),
            };
            let _ = bus_ref.send_command(
                write_cmd,
                None,
                Some(&bytes[write_start_ptr as usize..write_end_ptr as usize]),
            );
            write_end_ptr += Self::WRITE_SIZE as u32;
            write_start_ptr = write_end_ptr;
        }

        #[cfg(feature = "systemview-tracing")]
        rtos_trace::trace::marker(TestTraceMarker::WriteComplete as u32);

        Ok(())
    }
}

impl<T: BlockingNorStorageBusDriver> MacronixDeviceDriver<T, Blocking> {
    pub fn new_blocking(storagebusdriver: T, capacity: usize) -> Result<Self, ()> {
        Ok(Self {
            storagebusdriver,
            capacity,
            _phantom: core::marker::PhantomData,
        })
    }
}

#[embassy_executor::task]
async fn flash_test_task(
    mut device_driver: MacronixDeviceDriver<FlexspiNorStorageBus<'static, FlexSpiBlocking>, Blocking>,
) {
    // Read JEDEC ID
    let mut jedec_id = [0_u8; 4];
    device_driver.get_jedec_id(&mut jedec_id);

    // Erase the flash sectors
    let _ = device_driver.erase(ADDR, ADDR);

    // Test data for writing
    let write_data = [0x55_u8; 4];
    let mut read_data = [0_u8; 4];

    // Program the flash
    let _ = device_driver.write(ADDR, &write_data);

    // Read back the data
    let _ = device_driver.read(ADDR, &mut read_data);

    // Continue with periodic operations
    loop {
        Timer::after_millis(2000).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_imxrt::init(Default::default());

    #[cfg(feature = "systemview-tracing")]
    SYSTEMVIEW.init();

    #[cfg(feature = "systemview-tracing")]
    rtos_trace::trace::marker(TestTraceMarker::BusConfigStart as u32);

    // Configure FlexSPI parameters
    let flash_config = FlexspiDeviceConfig {
        flexspi_root_clk: 48000000,
        is_sck2_enabled: false,
        // Flash size in this struct is in KB, so divide by 1KB
        flash_size_kb: 0x10000, // 64 MB
        cs_interval_unit: Csintervalunit::Csintervalunit0,
        cs_interval: 0,
        cs_hold_time: 3,
        cs_setup_time: 3,
        data_valid_time: 2,
        columnspace: 0,
        enable_word_address: false,
        awr_seq_index: 0,
        awr_seq_number: 0,
        ard_seq_index: 0,
        ard_seq_number: 0,
        ahb_write_wait_unit: Awrwaitunit::Awrwaitunit2,
        ahb_write_wait_interval: 0,
        enable_write_mask_port_a: Wmena::Wmena0,
        enable_write_mask_port_b: Wmenb::Wmenb0,
    };
    let ahb_buffer_config = FlexspiAhbBufferConfig {
        priority: 0,
        master_index: 0,
        buffer_size: 256,
        enable_prefetch: true,
    };

    let ahb_config = AhbConfig {
        enable_ahb_write_ip_rx_fifo: false,
        enable_ahb_write_ip_tx_fifo: false,
        ahb_grant_timeout_cycle: 0xff,
        ahb_bus_timeout_cycle: 0xffff,
        resume_wait_cycle: 0x20,
        buffer: [ahb_buffer_config; 8],
        enable_clear_ahb_buffer_opt: Clrahbbufopt::Clrahbbufopt0,
        enable_read_address_opt: Readaddropt::Readaddropt1,
        enable_ahb_prefetch: true,
        enable_ahb_bufferable: Bufferableen::Bufferableen1,
        enable_ahb_cachable: Cachableen::Cachableen1,
    };

    let flexspi_config = FlexspiConfig {
        rx_sample_clock: Rxclksrc::Rxclksrc0,
        enable_sck_free_running: Sckfreerunen::Sckfreerunen0,
        enable_combination: false,
        enable_doze: Dozeen::Dozeen0, // TODO - Check back after analyzing system low power mode requirements
        enable_half_speed_access: Hsen::Hsen0,
        enable_sck_b_diff_opt: Sckbdiffopt::Sckbdiffopt0,
        enable_same_config_for_all: Samedeviceen::Samedeviceen0,
        seq_timeout_cycle: 0xFFFF,
        ip_grant_timeout_cycle: 0xff,
        tx_watermark: 0x08,
        rx_watermark: 0x08,
        ahb_config,
    };

    let mut flexspi_storage = FlexspiNorStorageBus::new_blocking(
        p.FLEXSPI,       // FlexSPI peripheral
        Some(p.PIO1_11), // FlexSPI DATA 0 pin
        Some(p.PIO1_12),
        Some(p.PIO1_13),
        Some(p.PIO1_14),
        Some(p.PIO2_17),
        Some(p.PIO2_18),
        Some(p.PIO2_22),
        Some(p.PIO2_23),
        p.PIO1_29,
        p.PIO2_19,
        FlexSpiFlashPort::PortB,                         // FlexSPI port
        FlexSpiBusWidth::Octal,                          // FlexSPI bus width
        FlexSpiFlashPortDeviceInstance::DeviceInstance0, // FlexSPI device instance
    );

    // Configure the Flexspi controller
    let _ = flexspi_storage.configport.configure_flexspi(&flexspi_config); // Configure the Flexspi controller

    let _ = flexspi_storage
        .configport
        .configure_device_port(&flash_config, &flexspi_config); // Configure the Flash device specific parameters like CS time, etc

    #[cfg(feature = "systemview-tracing")]
    rtos_trace::trace::marker(TestTraceMarker::BusConfigComplete as u32);

    // Instantiate the storage device driver and inject the bus driver dependency
    let device_driver = MacronixDeviceDriver::new_blocking(flexspi_storage, 0x4000000).unwrap();

    // Spawn the flash test task
    #[cfg(feature = "systemview-tracing")]
    {
        let _ = spawner.spawn_named("flash_test_task\0", flash_test_task(device_driver));
    }

    #[cfg(not(feature = "systemview-tracing"))]
    {
        let _ = spawner.spawn(flash_test_task(device_driver));
    }
}
