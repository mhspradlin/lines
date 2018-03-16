#[macro_use] extern crate log;
extern crate cadence;

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use cadence::prelude::*;
use cadence::StatsdClient;
use std::f64;

static START_TIME: SystemTime = UNIX_EPOCH;
static PERIOD: Duration = Duration::from_secs(30 * 60);

pub type DiskUsageSensor = platform::DiskUsageSensor;
 
pub fn make_sensor() -> DiskUsageSensor {
    platform::make_dummy_sensor()
}

pub trait Sensor {
    fn sense(&self, stats_pipeline: &StatsdClient);
}

pub struct DummySensor(pub String, pub i64);

impl Sensor for DummySensor {
    fn sense(&self, statsd_client: &StatsdClient) {
        let now = SystemTime::now();
        let place_in_interval = (now.duration_since(START_TIME).unwrap().as_secs() % PERIOD.as_secs()) as f64 / PERIOD.as_secs() as f64;
        let sin_parameter = place_in_interval * f64::consts::PI * 2.0;
        let metric_name = "test.".to_string() + &self.0;
        let curr_value = self.1 + (self.1 as f64 * f64::sin(sin_parameter)).round() as i64;
        info!("Sense called for Sensor {}, emitting value {}", self.0, curr_value);
        if let Err(e) = statsd_client.count(&metric_name, curr_value) {
            error!("Encountered and ignoring error sending stats for metric name {} and value {}: {:?}",
                   &metric_name, curr_value, e);
        }
    }
}

#[cfg(windows)]
mod platform {
    extern crate winapi;
    extern crate kernel32;

    use super::Sensor;
    use cadence::StatsdClient;
    use std::ffi::{OsStr, OsString};
    use std::os::windows::prelude::*;
    //use self::winapi::LPCWSTR;
    use self::winapi::um::winnt::LPCWSTR;
    //use self::winapi::um::winnt::PULARGE_INTEGER;
    use std::ptr;

    //type Bool = self::winapi::BOOL;

    pub struct DiskUsageSensor {
        //directory_on_disk: OsString
    }

    pub fn make_dummy_sensor() -> DiskUsageSensor {
        DiskUsageSensor {}
    }

    impl Sensor for DiskUsageSensor {
        fn sense(&self, statsd_client: &StatsdClient) {
            let mut total_accessible_drive_size_bytes: u64 = 0;
            let mut total_free_drive_space_bytes: u64 = 0;
            let wide_vec: Vec<u16> =
                OsStr::new(r"C:\").encode_wide().collect();
            let dir_on_drive: LPCWSTR = wide_vec.as_ptr();
            unsafe {
                let success = 
                    kernel32::GetDiskFreeSpaceExW(
                        dir_on_drive, ptr::null_mut(),
                        &mut total_accessible_drive_size_bytes as *mut u64,
                        &mut total_free_drive_space_bytes as *mut u64);
                // TODO Check for success
            }
            info!("Total size: {}GiB", total_accessible_drive_size_bytes / 1024 / 1024 / 1024);
            info!("Free size: {}GiB", total_free_drive_space_bytes / 1024 / 1024 / 1024);
        }
    }
}