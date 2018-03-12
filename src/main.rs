#[macro_use] extern crate log;
extern crate log4rs;
extern crate rayon;

use std::time::{Duration, Instant};
use rayon::prelude::*;
use std::thread;

fn main() {
    log4rs::init_file("config/log4rs.yml", Default::default()).unwrap();
    info!("Starting up");

    let update_interval = Duration::from_secs(1);
    let num_sensors = 10;

    let sensors = make_dummy_sensors(num_sensors);
    let sensor_pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(num_sensors as usize)
                        .thread_name(|thread_index| "sensor-pool-thread-".to_string() + &thread_index.to_string())
                        .build()
                        .unwrap();
    let mut last_update = Instant::now();
    loop {
        sensor_pool.install(||
            sensors.par_iter()
                   .for_each(|ref sensor| sensor.sense()));
        let time_until_next_update = update_interval - Instant::now().duration_since(last_update);
        thread::sleep(time_until_next_update);
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