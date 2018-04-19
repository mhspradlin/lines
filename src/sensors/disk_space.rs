extern crate cadence;

use std::ffi::OsString;
use std::i64;
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

    static FALSE: i32 = 0;
    static FATAL_ERROR: &str = "Fatal error counting metric";
    static METRICS_PREFIX: &str = "drive";
    static TOTAL_BYTES: &str = "total_bytes";
    static FREE_BYTES: &str = "free_bytes";

    impl Sensor for DiskSpaceSensor {
        fn sense(&mut self, statsd_client: &StatsdClient) {
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
                let directory_name = self.directory_on_disk.to_string_lossy();
                statsd_client.count(&create_drive_metric_name(&directory_name, TOTAL_BYTES),
                                    super::value_or_max(total_accessible_drive_size_bytes))
                    .expect(FATAL_ERROR);
                statsd_client.count(&create_drive_metric_name(&directory_name, FREE_BYTES),
                                    super::value_or_max(total_free_drive_space_bytes))
                    .expect(FATAL_ERROR);
            }
        }
    }

    fn create_drive_metric_name(drive: &str, suffix: &str) -> String {
        METRICS_PREFIX.to_string() + "." + &drive.replace(":", "") + "." + suffix
    }
}

#[cfg(target_os="linux")]
mod platform {
    extern crate libc;

    use super::{Sensor, DiskSpaceSensor};
    use std::os::unix::prelude::*;
    use std::ffi::CString;
    use self::libc::statvfs64;
    use cadence::prelude::*;
    use cadence::StatsdClient;
    use std::mem;
    use std::io::Error;

    const FALSE: i32 = 0;
    const FATAL_ERROR: &'static str = "Fatal error counting metric";
    const METRICS_PREFIX: &'static str = "drive";
    static TOTAL_BYTES: &str = "total_bytes";
    static FREE_BYTES: &str = "free_bytes";

    impl Sensor for DiskSpaceSensor {
        fn sense(&mut self, statsd_client: &StatsdClient) {
            let dir_on_drive = CString::new(self.directory_on_disk.as_bytes()).unwrap();
            let mut info_struct: statvfs64 = unsafe { mem::zeroed() };
            let return_code: i32 = unsafe {
                libc::statvfs64(dir_on_drive.as_ptr(), &mut info_struct)
            };
            if return_code == FALSE {
                let total_accessible_drive_size_bytes = info_struct.f_frsize * info_struct.f_blocks;
                let total_free_drive_space_bytes = info_struct.f_bsize * info_struct.f_bfree;
                info!("'{}' total size: {} GiB", self.directory_on_disk.to_string_lossy(),
                      total_accessible_drive_size_bytes / 1024 / 1024 / 1024);
                info!("'{}' free size: {} GiB", self.directory_on_disk.to_string_lossy(),
                      total_free_drive_space_bytes / 1024 / 1024 / 1024);

                let directory_name = self.directory_on_disk.to_string_lossy();
                statsd_client.count(&create_drive_metric_name(&directory_name, TOTAL_BYTES),
                                    super::value_or_max(total_accessible_drive_size_bytes))
                    .expect(FATAL_ERROR);
                statsd_client.count(&create_drive_metric_name(&directory_name, FREE_BYTES),
                                    super::value_or_max(total_free_drive_space_bytes))
                    .expect(FATAL_ERROR);
            } else {
                error!("Error getting drive usage for drive '{}': {}",
                       self.directory_on_disk.to_string_lossy(), Error::last_os_error());
            }
        }
    }

    fn create_drive_metric_name(drive: &str, suffix: &str) -> String {
        METRICS_PREFIX.to_string() + "." + &drive.replace(":", "") + "." + suffix
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
