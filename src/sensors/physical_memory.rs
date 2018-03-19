extern crate cadence;

use super::Sensor;
use std::i64;

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
                let total_accessible_bytes = info_struct.ullTotalPhys;
                let total_free_bytes = info_struct.ullAvailPhys;
                info!("Total accessible physical memory: {} MiB", total_accessible_bytes / 1024 / 1024);
                info!("Total free physical memory: {} MiB", total_free_bytes / 1024 / 1024);
                statsd_client.count(&TOTAL_BYTES, super::value_or_max(total_accessible_bytes))
                    .expect(FATAL_ERROR);
                statsd_client.count(&FREE_BYTES, super::value_or_max(total_free_bytes))
                    .expect(FATAL_ERROR);
            }
        }
    }
}

#[cfg(target_os="linux")]
mod platform {
    extern crate libc;

    use super::Sensor;
    use super::PhysicalMemorySensor;
    use cadence::prelude::*;
    use cadence::StatsdClient;
    use std::mem;
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
            let mut info_struct: libc::sysinfo = unsafe { mem::zeroed() };
            let return_code = unsafe { libc::sysinfo(&mut info_struct as *mut libc::sysinfo) };
            if return_code == FALSE {
                let total_accessible_bytes = info_struct.totalram * info_struct.mem_unit;
                let total_free_bytes = info_struct.freeram * info_struct.mem_unit;
                info!("Total accessible physical memory: {} MiB", total_accessible_bytes / 1024 / 1024);
                info!("Total free physical memory: {} MiB", total_free_bytes / 1024 / 1024);
                statsd_client.count(&TOTAL_BYTES, super::value_or_max(total_accessible_bytes))
                    .expect(FATAL_ERROR);
                statsd_client.count(&FREE_BYTES, super::value_or_max(total_free_bytes))
                    .expect(FATAL_ERROR);
            } else {
                error!("Error getting physical memory usage: {}", Error::last_os_error());
            }
        }
    }
}

fn value_or_max(value: u64) -> i64 {
    if value >= i64::MAX as u64 {
        warn!("Value {} larger than max value of {}, reporting max value {} instead",
                value, i64::MAX, i64::MAX);
        i64::MAX
    } else {
        value as i64
    }
}