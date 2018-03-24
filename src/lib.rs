#[macro_use] extern crate log;
#[macro_use] extern crate lazy_static;

extern crate cadence;

pub mod sensors;

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use cadence::prelude::*;
use cadence::StatsdClient;
use std::f64;

static START_TIME: SystemTime = UNIX_EPOCH;
static PERIOD: Duration = Duration::from_secs(30 * 60);

pub trait Sensor: Send {
    fn sense(&mut self, stats_pipeline: &StatsdClient);
}

pub struct DummySensor(pub String, pub i64);

impl Sensor for DummySensor {
    fn sense(&mut self, statsd_client: &StatsdClient) {
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