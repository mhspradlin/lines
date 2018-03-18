extern crate cadence;

use super::Sensor;

pub struct PhysicalMemorySensor {
}

impl PhysicalMemorySensor {
    pub fn new() -> PhysicalMemorySensor {
        PhysicalMemorySensor {}
    }
}

#[cfg(windows)]
mod platform {
    extern crate winapi;
    extern crate kernel32;

    use super::Sensor;
    use super::PhysicalMemorySensor;
    use cadence::prelude::*;
    use cadence::StatsdClient;
    use std::mem;
    use self::winapi::um::sysinfoapi::MEMORYSTATUSEX;
    use self::winapi::um::sysinfoapi;
    use std::io::Error;

    const FALSE: i32 = 0;
    const FATAL_ERROR: &'static str = "Fatal error counting metric";
    const METRICS_PREFIX: &'static str = "physical_memory";
    lazy_static! {
        static ref TOTAL_BYTES: String = METRICS_PREFIX.to_string() + ".total_bytes";
        static ref FREE_BYTES: String = METRICS_PREFIX.to_string() + ".free_bytes";
    }

    impl Sensor for PhysicalMemorySensor {
        fn sense(&self, statsd_client: &StatsdClient) {
            let mut info_struct: MEMORYSTATUSEX = unsafe { mem::zeroed() };
            info_struct.dwLength = mem::size_of::<MEMORYSTATUSEX>() as u32;
            let return_code: i32 = unsafe { sysinfoapi::GlobalMemoryStatusEx(&mut info_struct as *mut MEMORYSTATUSEX) };
            if return_code == FALSE {
                error!("Error getting physical memory usage: {}", Error::last_os_error());
            } else {
                info!("Got physical memory usage: {}", info_struct.ullTotalPhys);
            }
        }
    }
}