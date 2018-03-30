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
    extern crate libc;
    extern crate regex;
    
    use super::Sensor;
    use cadence::prelude::*;
    use cadence::StatsdClient;
    use std::io::{Error, ErrorKind, Result};
    use std::fs::File;
    use std::io::BufReader;
    use std::io::prelude::*;
    use self::regex::{Regex, Captures};

    const FATAL_ERROR: &'static str = "Fatal error counting metric";
    const METRICS_PREFIX: &'static str = "cpu_time";
    lazy_static! {
        static ref IDLE_TIME: String = METRICS_PREFIX.to_string() + ".idle_time";
        static ref BUSY_TIME: String = METRICS_PREFIX.to_string() + ".busy_time";
        static ref CPU_TIME: Regex =
           Regex::new(&(r"^cpu\s+(?P<user_ticks>\d+)\s+(?P<nice_ticks>\d+)\s+(?P<system_ticks>\d+)\s+".to_string() +
                          r"(?P<idle_ticks>\d+)\s+(?P<iowait_ticks>\d+)\s+(?P<irq_ticks>\d+)\s+" +
                          r"(?P<softirq_ticks>\d+)\s+(?P<steal_ticks>\d+)")).unwrap(); 
    }

    pub struct PlatformCpuTimeSensor {
        last_idle_ticks: u64,
        last_busy_ticks: u64
    }

    impl PlatformCpuTimeSensor {
        pub fn init() -> PlatformCpuTimeSensor {
            let (starting_idle_ticks, starting_busy_ticks) =
                cpu_time_from_stat().expect("Error getting initial cpu times from /proc/stat");
            PlatformCpuTimeSensor { last_idle_ticks: starting_idle_ticks, last_busy_ticks: starting_busy_ticks }
        }
    }

    impl Sensor for PlatformCpuTimeSensor {
        fn sense(&mut self, statsd_client: &StatsdClient) {
            match cpu_time_from_stat() {
                Err(e) => {
                    error!("Error getting cpu times from /proc/stat: {:?}", e);
                    return
                },
                Ok((total_idle_ticks, total_busy_ticks)) => {
                    let elapsed_idle_ticks = total_idle_ticks - self.last_idle_ticks;     
                    let elapsed_busy_ticks = total_busy_ticks - self.last_busy_ticks;
                    let total_elapsed_ticks = elapsed_idle_ticks + elapsed_busy_ticks;
                    let busy_percentage_during_interval: f64 = (elapsed_busy_ticks as f64 / total_elapsed_ticks as f64) * 100 as f64;
                    info!("CPU busy percentage: {:.3}", busy_percentage_during_interval);
                    let rounded_busy_percentage: i64 = busy_percentage_during_interval.round() as i64;
                    statsd_client.count(&BUSY_TIME, rounded_busy_percentage)
                        .expect(FATAL_ERROR);
                    statsd_client.count(&IDLE_TIME, 100 - rounded_busy_percentage)
                        .expect(FATAL_ERROR);
                }
            }
        }
    }

    fn cpu_time_from_stat() -> Result<(u64, u64)> {
        let cpu_info = File::open("/proc/stat")?;
        for line in BufReader::new(cpu_info).lines() {
            match line {
                Ok(line) => {
                    if let Some(captures) = CPU_TIME.captures(&line) {
                        let total_idle_ticks =
                            parse_capture("idle_ticks", &captures)? +
                            parse_capture("iowait_ticks", &captures)?;
                        let total_busy_ticks =
                            parse_capture("user_ticks", &captures)? +
                            parse_capture("nice_ticks", &captures)? +
                            parse_capture("system_ticks", &captures)? +
                            parse_capture("irq_ticks", &captures)? +
                            parse_capture("softirq_ticks", &captures)? +
                            parse_capture("steal_ticks", &captures)?;
                        return Ok((total_idle_ticks, total_busy_ticks))
                    }
                },
                Err(_) => break
            } 
        }
        
        let error_message = format!("Could not find match for cpu time regex {}",
                                    CPU_TIME.as_str());
        return Err(Error::new(ErrorKind::NotFound, error_message));
    }

    fn parse_capture(capture_key: &str, captures: &Captures) -> Result<u64> {
        match captures[capture_key].parse() {
            Ok(num) => return Ok(num),
            Err(e) => {
                let error_message = format!("Unable to parse regex match as number, got error {:?}", e);
                return Err(Error::new(ErrorKind::NotFound, error_message))
            }
        } 
    }
}
