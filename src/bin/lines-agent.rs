#[macro_use] extern crate log;
#[macro_use] extern crate serde_derive;

extern crate log4rs;
extern crate rayon;
extern crate statsd;
extern crate cadence;
extern crate lines;
extern crate serde_yaml;

// Good example for multiplatform code: https://github.com/luser/read-process-memory/blob/master/src/lib.rs

use std::time::{Duration, SystemTime};
use rayon::ThreadPool;
use std::thread;
use std::net::UdpSocket;
use cadence::{StatsdClient, QueuingMetricSink, UdpMetricSink,
              DEFAULT_PORT};
use lines::Sensor;
use lines::sensors::{DiskSpaceSensor, PhysicalMemorySensor, CpuTimeSensor};
use std::env;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    hostname: String,
    update_interval: u64,
    statsd_url: String,
    statsd_port: String
}

fn main() {
    log4rs::init_file("config/log4rs.yml", Default::default()).unwrap();
    info!("Starting up");

    let statsd_client = make_statsd_client("stats.home", DEFAULT_PORT, "cronus");
    let update_interval = Duration::from_secs(60);

    let home_dir = env::home_dir().expect("Can't find home directory");

    let mut sensors: Vec<Box<Sensor>> = Vec::new();
    sensors.push(Box::new(DiskSpaceSensor::new(home_dir.into_os_string())));
    sensors.push(Box::new(PhysicalMemorySensor::new()));
    sensors.push(Box::new(CpuTimeSensor::new()));
    let num_sensors = sensors.len();
    let sensor_pool = make_sensor_thread_pool(num_sensors as usize);
    let mut last_update = SystemTime::now();
    loop {
        debug!("Running all sensors in parallel");
        run_all_sensors_in_parallel(&sensor_pool, &mut sensors, &statsd_client);
        sleep_until_target_time(last_update, update_interval);
        last_update = SystemTime::now();
    }
}

fn make_statsd_client(host: &str, port: u16, metrics_prefix: &str) -> StatsdClient {
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.set_nonblocking(true).unwrap();
    let host = (host, port);
    // Use an unbuffered UdpMetricsSink because we only occasionally emit metrics
    let udp_sink = UdpMetricSink::from(&host, socket).unwrap();
    let queuing_sink = QueuingMetricSink::from(udp_sink);
    StatsdClient::from_sink(metrics_prefix, queuing_sink)
}

fn make_sensor_thread_pool(num_sensors: usize) -> ThreadPool {
    rayon::ThreadPoolBuilder::new()
                        .num_threads(num_sensors as usize)
                        .thread_name(|thread_index| "sensor-pool-thread-".to_string() + &thread_index.to_string())
                        .build()
                        .unwrap()
}

fn run_all_sensors_in_parallel<T>(sensor_pool: &ThreadPool, sensors: &mut Vec<Box<T>>,
                                  statsd_client: &StatsdClient)
        where T: Sensor + ?Sized {
    sensor_pool.scope(|scope|
        for sensor in sensors {
            scope.spawn(move |_| sensor.sense(statsd_client));
        }
    );
}

fn sleep_until_target_time(last_wakeup: SystemTime, target_interval: Duration) {
    let time_until_next_wakeup = target_interval - SystemTime::now().duration_since(last_wakeup).unwrap();
    debug!("Time until next wakeup: {:.3}s", duration_in_seconds(&time_until_next_wakeup));
    if time_until_next_wakeup > Duration::from_millis(0) {
        thread::sleep(time_until_next_wakeup);
    }
}

fn duration_in_seconds(duration: &Duration) -> f64 {
    duration.as_secs() as f64 + duration.subsec_nanos() as f64 / 1_000_000_000 as f64
}
