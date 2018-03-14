#[macro_use] extern crate log;
extern crate log4rs;
extern crate rayon;
extern crate statsd;
extern crate cadence;

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use rayon::ThreadPool;
use std::thread;
use std::f64;
use std::net::UdpSocket;
use cadence::prelude::*;
use cadence::{StatsdClient, QueuingMetricSink, BufferedUdpMetricSink,
              DEFAULT_PORT};

static START_TIME: SystemTime = UNIX_EPOCH;
static PERIOD: Duration = Duration::from_secs(5 * 60);

fn main() {
    log4rs::init_file("config/log4rs.yml", Default::default()).unwrap();
    info!("Starting up");

    let mut statsd_client = make_statsd_client("stats.home", DEFAULT_PORT, "cronus");
    let update_interval = Duration::from_secs(60);
    let num_sensors = 4;

    let sensor_pool = make_sensor_thread_pool(num_sensors as usize);
    let sensors = make_dummy_sensors(num_sensors);
    let mut last_update = SystemTime::now();
    loop {
        debug!("Running all sensors in parallel");
        run_all_sensors_in_parallel(&sensor_pool, &sensors, &mut statsd_client);
        sleep_until_target_time(last_update, update_interval);
        last_update = SystemTime::now();
    }
}

fn make_statsd_client(host: &str, port: u16, metrics_prefix: &str) -> StatsdClient {
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.set_nonblocking(true).unwrap();
    let host = (host, port);
    let udp_sink = BufferedUdpMetricSink::from(&host, socket).unwrap();
    let queuing_sink = QueuingMetricSink::from(udp_sink);
    StatsdClient::from_sink(metrics_prefix, queuing_sink)
}

trait Sensor {
    fn sense(&self, stats_pipeline: &StatsdClient);
}

struct DummySensor(String, u64);

impl Sensor for DummySensor {
    fn sense(&self, statsd_client: &StatsdClient) {
        let now = SystemTime::now();
        let place_in_interval = (now.duration_since(START_TIME).unwrap().as_secs() % PERIOD.as_secs()) as f64 / PERIOD.as_secs() as f64;
        let sin_parameter = place_in_interval / (f64::consts::PI / 2.0);
        let metric_name = "test.".to_string() + &self.0;
        let curr_value = self.1 + (self.1 as f64 * f64::sin(sin_parameter)).round() as u64;
        info!("Sense called for Sensor {}, emitting value {}", self.0, curr_value);
        if let Err(e) = statsd_client.gauge(&metric_name, curr_value) {
            error!("Encountered and ignoring error sending stats for metric name {} and value {}: {:?}",
                   &metric_name, curr_value, e);
        }
    }
}

fn make_dummy_sensors(number: u8) -> Vec<DummySensor> {
    let mut dummy_sensors = Vec::new();
    for index in 0..number {
        dummy_sensors.push(DummySensor("Sensor ".to_string() + &index.to_string(), (index as u64 + 1) * 1000));
    }
    return dummy_sensors;
}

fn make_sensor_thread_pool(num_sensors: usize) -> ThreadPool {
    rayon::ThreadPoolBuilder::new()
                        .num_threads(num_sensors as usize)
                        .thread_name(|thread_index| "sensor-pool-thread-".to_string() + &thread_index.to_string())
                        .build()
                        .unwrap()
}

fn run_all_sensors_in_parallel(_sensor_pool: &ThreadPool, sensors: &Vec<DummySensor>,
                               statsd_client: &StatsdClient) {
    for sensor in sensors {
        sensor.sense(statsd_client);
    }
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