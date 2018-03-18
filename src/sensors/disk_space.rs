extern crate cadence;

use std::ffi::OsString;
use super::Sensor;

pub struct DiskSpaceSensor {
    directory_on_disk: OsString
}

impl DiskSpaceSensor {
    pub fn new(directory_on_disk: OsString) -> DiskSpaceSensor {
        DiskSpaceSensor { directory_on_disk }
     }
}

#[cfg(windows)]
mod platform {
    extern crate winapi;
    extern crate kernel32;

    use super::DiskSpaceSensor;
    use super::Sensor;
    use cadence::prelude::*;
    use cadence::StatsdClient;
    use std::os::windows::prelude::*;
    use std::io::Error;
    use self::winapi::um::winnt::LPCWSTR;
    use std::ptr;
    use std::i64;

    const FALSE: i32 = 0;
    const FATAL_ERROR: &'static str = "Fatal error counting metric";
    const METRICS_PREFIX: &'static str = "drive";
    lazy_static! {
        static ref TOTAL_BYTES: String = METRICS_PREFIX.to_string() + ".total_bytes";
        static ref FREE_BYTES: String = METRICS_PREFIX.to_string() + ".free_bytes";
    }

    impl Sensor for DiskSpaceSensor {
        fn sense(&self, statsd_client: &StatsdClient) {
            let mut total_accessible_drive_size_bytes: u64 = 0;
            let mut total_free_drive_space_bytes: u64 = 0;
            let wide_vec: Vec<u16> = self.directory_on_disk.encode_wide().collect();
            let dir_on_drive: LPCWSTR = wide_vec.as_ptr();
            let return_code: i32 = unsafe {
                    kernel32::GetDiskFreeSpaceExW(
                        dir_on_drive, ptr::null_mut(),
                        &mut total_accessible_drive_size_bytes as *mut u64,
                        &mut total_free_drive_space_bytes as *mut u64)
            };
            if return_code == FALSE {
                error!("Error getting drive usage for drive '{}': {}",
                       self.directory_on_disk.to_string_lossy(), Error::last_os_error());
            } else {
                info!("'{}' total size: {} GiB", self.directory_on_disk.to_string_lossy(),
                      total_accessible_drive_size_bytes / 1024 / 1024 / 1024);
                info!("'{}' free size: {} GiB", self.directory_on_disk.to_string_lossy(),
                      total_free_drive_space_bytes / 1024 / 1024 / 1024);
                statsd_client.count(&TOTAL_BYTES, value_or_max(total_accessible_drive_size_bytes))
                    .expect(FATAL_ERROR);
                statsd_client.count(&FREE_BYTES, value_or_max(total_free_drive_space_bytes))
                    .expect(FATAL_ERROR);
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
}

#[cfg(target_os="linux")]
mod platform {
    extern crate libc;

    use super::{Sensor, DiskSpaceSensor};
    use std::os::unix::prelude::*;
    use self::libc::statvfs64;
    use cadence::prelude::*;
    use cadence::StatsdClient;

    // I think we should use this call: https://docs.rs/libc/0.2.39/libc/fn.statfs64.html
    // Actually, use statvfs64

    const FALSE: i32 = 0;

    impl Sensor for DiskSpaceSensor {
        fn sense(&self, statsd_client: &StatsdClient) {
            let dir_on_drive: *const c_char = self.directory_on_disk.as_bytes();
            let mut info_struct: statvfs64 =
                statvfs64 {};
            let return_code: i32 = unsafe {
                libc::statvfs64(dir_on_drive, &mut statvfs64)
            };
            if return_code == FALSE {
                info!("Success, got {}", info_struct.f_blocks);
            } else {
                error!("Failure, got error code {}", return_code);
            }
        }
    }
}