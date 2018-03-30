extern crate cadence;

use super::Sensor;

pub type CpuTimeSensor = platform::PlatformCpuTimeSensor;

impl CpuTimeSensor {
    pub fn new() -> CpuTimeSensor {
        platform::PlatformCpuTimeSensor::init()
    }
}

#[cfg(windows)]
mod platform {
    extern crate winapi;
    extern crate kernel32;

    use super::Sensor;
    use super::CpuTimeSensor;
    use cadence::prelude::*;
    use cadence::StatsdClient;
    use std::mem;
    use self::winapi::um::pdh;
    use self::winapi::um::pdh::{PDH_HQUERY, PDH_HCOUNTER, PDH_FMT_DOUBLE, PDH_FMT_COUNTERVALUE};
    use std::os::windows::prelude::*;
    use std::ptr;
    use std::ffi::OsString;

    const FALSE: i32 = 0;
    const ALL_CPU_TIME_PERFORMANCE_QUERY_STRING: &'static str = r"\Processor(_Total)\% Processor Time";
    const FATAL_ERROR: &'static str = "Fatal error counting metric";
    const METRICS_PREFIX: &'static str = "cpu_time";
    lazy_static! {
        static ref IDLE_TIME: String = METRICS_PREFIX.to_string() + ".idle_time";
        static ref BUSY_TIME: String = METRICS_PREFIX.to_string() + ".busy_time";
    }

    pub struct PlatformCpuTimeSensor {
        query: PDH_HQUERY,
        cpu_percent_counter: PDH_HCOUNTER
    }

    // It's safe to send counter IDs and query IDs within this struct across thread boundaries
    // because only one instance of PlatformCpuTimeSensor is ever referencing a given ID at any
    // given time (nobody, say, will call PdhCloseQuery on the query when we still might use it)
    // This struct is not Sync, but there are no trait methods that take the struct by non-mut
    // reference, so I don't think it's an issue (you'll need exclusive access to call sense or
    // ownership to drop)
    unsafe impl Send for PlatformCpuTimeSensor {}

    impl PlatformCpuTimeSensor {
        pub fn init() -> CpuTimeSensor {
            let all_cpu_time_query: Vec<u16> =
                OsString::from(ALL_CPU_TIME_PERFORMANCE_QUERY_STRING.to_string()).encode_wide().collect();
            let mut query: PDH_HQUERY = unsafe { mem::zeroed() };
            let mut cpu_percent_counter: PDH_HCOUNTER = unsafe { mem::zeroed() };
            unsafe {
                panic_on_pdh_failure(pdh::PdhOpenQueryW(ptr::null_mut(), 0x0 as usize, &mut query as *mut PDH_HQUERY),
                                     "opening Performance Data Helper query");
                panic_on_pdh_failure(pdh::PdhAddEnglishCounterW(query, all_cpu_time_query.as_ptr(), 0,
                                                                &mut cpu_percent_counter as *mut PDH_HCOUNTER),
                                    "adding CPU % Performance Data Helper counter");
                // We need to collect on this query at least twice before trying to read it
                panic_on_pdh_failure(pdh::PdhCollectQueryData(query), "collecting Performance Data Helper query");
            }
            PlatformCpuTimeSensor { query, cpu_percent_counter }
        }
    }

    fn panic_on_pdh_failure(return_code: i32, operation: &str) {
        if return_code != FALSE {
            let error_suffix = format!(": {:08X} (PDH_STATUS)", return_code);
            panic!("Error ".to_string() + operation + &error_suffix);
        }
    }

    impl Sensor for PlatformCpuTimeSensor {
        fn sense(&mut self, statsd_client: &StatsdClient) {
            let busy_percentage_during_interval = single_double_sample(self.query, self.cpu_percent_counter);
            info!("CPU busy percentage: {:.3}", busy_percentage_during_interval);
            let rounded_busy_percentage: i64 = busy_percentage_during_interval.round() as i64;
            statsd_client.count(&BUSY_TIME, rounded_busy_percentage)
                .expect(FATAL_ERROR);
            statsd_client.count(&IDLE_TIME, 100 - rounded_busy_percentage)
                .expect(FATAL_ERROR);
        }
    }

    fn single_double_sample(query: PDH_HQUERY, counter: PDH_HCOUNTER) -> f64 {
        unsafe {
            let mut counter_value: PDH_FMT_COUNTERVALUE = mem::zeroed();
            panic_on_pdh_failure(pdh::PdhCollectQueryData(query), "collecting Performance Data Helper query");
            panic_on_pdh_failure(pdh::PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, ptr::null_mut(),
                                                                  &mut counter_value as *mut PDH_FMT_COUNTERVALUE),
                                "getting Performance Data Helper counter value");
            *counter_value.u.doubleValue()
        }
    }

    impl Drop for PlatformCpuTimeSensor {
        fn drop(&mut self) {
            let return_code = unsafe { pdh::PdhCloseQuery(self.query) };
            if return_code == FALSE {
                info!("Closed Performance Data Helper query successfully");
            } else {
                error!("Error closing Performance Data Helper query, ignoring: {:08X} (PDH_STATUS)", return_code);
            }
        }
    }
}

#[cfg(target_os="linux")]
mod platform {
    
    use super::Sensor;
    use super::CpuTimeSensor;
    use cadence::prelude::*;
    use cadence::StatsdClient;

    pub struct PlatformCpuTimeSensor {
    }

    impl PlatformCpuTimeSensor {
        pub fn init() -> PlatformCpuTimeSensor {
            PlatformCpuTimeSensor {}
        }
    }

    impl Sensor for PlatformCpuTimeSensor {
        fn sense(&mut self, statsd_client: &StatsdClient) {
            info!("TODO");
        }
    }

}
