#[macro_use] extern crate log;
extern crate log4rs;
extern crate rayon;

use std::time::{Duration, Instant};
use rayon::prelude::*;
use rayon::ThreadPool;
use std::thread;

fn main() {
    log4rs::init_file("config/log4rs.yml", Default::default()).unwrap();
    info!("Starting up");

    let update_interval = Duration::from_secs(1);
    let num_sensors = 10;

    let sensor_pool = make_sensor_thread_pool(num_sensors as usize);
    let sensors = make_dummy_sensors(num_sensors);
    let mut last_update = Instant::now();
    loop {
        debug!("Running all sensors in parallel");
        run_all_sensors_in_parallel(&sensor_pool, &sensors);
        sleep_until_target_time(last_update, update_interval);
        last_update = Instant::now();
    }
}

trait Sensor {
    fn sense(&self);
}

struct DummySensor(String);

impl Sensor for DummySensor {
    fn sense(&self) {
        info!("Sense called for Sensor {}", self.0);
    }
}

fn make_dummy_sensors(number: u8) -> Vec<DummySensor> {
    let mut dummy_sensors = Vec::new();
    for index in 0..number {
        dummy_sensors.push(DummySensor("Sensor".to_string() + &index.to_string()));
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

fn run_all_sensors_in_parallel(sensor_pool: &ThreadPool, sensors: &Vec<DummySensor>) {
    sensor_pool.install(||
        sensors.par_iter()
                .for_each(|ref sensor| sensor.sense()))
}

fn sleep_until_target_time(last_wakeup: Instant, target_interval: Duration) {
    let time_until_next_wakeup = target_interval - Instant::now().duration_since(last_wakeup);
    debug!("Time until next wakeup: {:.3}s", duration_in_seconds(&time_until_next_wakeup));
    if time_until_next_wakeup > Duration::from_millis(0) {
        thread::sleep(time_until_next_wakeup);
    }
}

fn duration_in_seconds(duration: &Duration) -> f64 {
    duration.as_secs() as f64 + duration.subsec_nanos() as f64 / 1_000_000_000 as f64
}