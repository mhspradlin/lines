#[macro_use] extern crate log;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate quicli;
#[macro_use] extern crate lazy_static;

extern crate log4rs;
extern crate rayon;
extern crate statsd;
extern crate cadence;
extern crate lines;
extern crate serde_yaml;
extern crate serde_humantime;
extern crate hostname;
extern crate regex;

// Good example for multiplatform code: https://github.com/luser/read-process-memory/blob/master/src/lib.rs

use quicli::prelude::*;
use std::time::{Duration, SystemTime};
use rayon::ThreadPool;
use std::thread;
use std::net::UdpSocket;
use cadence::{StatsdClient, QueuingMetricSink, UdpMetricSink};
use lines::Sensor;
use lines::sensors::{DiskSpaceSensor, PhysicalMemorySensor, CpuTimeSensor};
use std::fs::File;
use std::ffi::OsString;
use std::collections::HashMap;
use std::io::BufReader;
use std::io::prelude::*;
use regex::Regex;

#[derive(Debug, StructOpt)]
struct Arguments {
    #[structopt(long = "config-directory", short = "c")]
    config_directory: String,
    #[structopt(long = "output-directory", short = "o")]
    output_directory: String
}

#[derive(Debug, PartialEq, Deserialize)]
struct Config {
    hostname: String,
    #[serde(with = "serde_humantime")]
    update_interval: Duration,
    statsd_url: String,
    statsd_port: u16,
    disks: Vec<String>
}

static HOSTNAME_VARIABLE: &str = "hostname";
static CONFIG_DIR_VARIABLE: &str = "config_directory";
static OUTPUT_DIR_VARIABLE: &str = "output_directory";

lazy_static! {
    // Variable syntax for config files is `${variable_name}`
    static ref VARIABLE_REGEX: Regex = Regex::new(r"\$\{(.*?)\}").unwrap();
}

fn main() {
    let args = Arguments::from_args();
    info!("Starting up with arguments {:?}", args);
    let config_directory = &args.config_directory;
    let output_directory = &args.output_directory;
    let bindings = create_variable_bindings(config_directory, output_directory);

    // Substitute special tokens in the logging config
    let generated_logging_config = output_directory.to_string() + "/generated_log4rs.yml";
    {
        let base_logging_config = File::open(config_directory.to_string() + "/log4rs.yml").unwrap();
        let mut substituted_logging_config = File::create(generated_logging_config.clone()).unwrap();
        for line in BufReader::new(base_logging_config).lines() {
            let line = line.unwrap();
            substituted_logging_config.write_all(substitute_bindings_in_string(&line, &bindings).as_bytes()).unwrap();
            substituted_logging_config.write_all(b"\n").unwrap();
        }
        substituted_logging_config.flush().unwrap();
        drop(substituted_logging_config);
        thread::sleep(Duration::from_millis(1000));
    }

    log4rs::init_file(generated_logging_config.clone(), Default::default()).unwrap();
    let config_file = File::open(config_directory.to_string() + "/configuration.yml").unwrap();
    let config: Config = serde_yaml::from_reader(config_file)
        .expect("Error parsing configuration");
    let config = substitute_variables(config, &bindings);

    info!("Got config file: {:?}", config);

    let exit_status = run(config);
    if let Err(e) = exit_status {
        error!("Exiting with error: {}", e);
        std::process::exit(1);
    }
}

fn run(config: Config) -> Result<()> {
    let statsd_client = make_statsd_client(&config.statsd_url, config.statsd_port, &config.hostname);
    let update_interval = config.update_interval;

    let mut sensors: Vec<Box<Sensor>> = Vec::new();
    for disk in config.disks {
        sensors.push(Box::new(DiskSpaceSensor::new(OsString::from(disk))));
    }
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

fn create_variable_bindings<'a>(config_directory: &'a str, output_directory: &'a str) -> HashMap<&'a str, String> {
    let mut bindings = HashMap::new();
    bindings.insert(HOSTNAME_VARIABLE, hostname::get_hostname().unwrap());
    bindings.insert(CONFIG_DIR_VARIABLE, config_directory.to_string());
    bindings.insert(OUTPUT_DIR_VARIABLE, output_directory.to_string());
    bindings
}

fn substitute_variables(config: Config, bindings: &HashMap<&str, String>) -> Config {
    Config {
        hostname: substitute_bindings_in_string(&config.hostname, bindings),
        statsd_url: substitute_bindings_in_string(&config.statsd_url, bindings),
        .. config
    }
}

fn substitute_bindings_in_string(template: &str, bindings: &HashMap<&str, String>) -> String {
    let mut substituted_string = String::new();
    let mut last_match_end_index = 0;
    for captures in VARIABLE_REGEX.captures_iter(template) {
        let whole_match = captures.get(0).unwrap();
        let capture = captures.get(1).unwrap();
        let match_start_index = whole_match.start();
        // Copy the part of the string that wasn't matched
        if match_start_index > last_match_end_index {
            let interim_characters = template.get(last_match_end_index..match_start_index).unwrap();
            substituted_string.push_str(interim_characters);
        }
        // Get the binding for that variable and then push it onto the string
        let captured_variable_name = capture.as_str();
        // If we don't find the binding, substitute the whole match (including the variable wrapping syntax)
        // This leaves anything that isn't an exact match for a variable unsubstituted
        let potential_binding: Option<&str> = bindings.get(captured_variable_name).map(|val| val.as_str());
        let binding: &str = potential_binding.unwrap_or_else(|| captures.get(0).unwrap().as_str());
        substituted_string.push_str(binding);
        last_match_end_index = whole_match.end();
    }
    // Push the remaining part of the string after the last match (or the whole string if there were no matches)
    if last_match_end_index < template.len() {
        let trailing_characters = template.get(last_match_end_index..template.len()).unwrap();
        substituted_string.push_str(trailing_characters);
    }

    substituted_string
}

#[test]
fn substitute_bindings_in_string_test() {
    let template = "A ${variable.name} string with ${other-name} ${but-not-all-there} substitutions";
    let mut bindings = HashMap::new();
    bindings.insert("variable.name", "template".to_string());
    bindings.insert("other-name", "multiple".to_string());
    let substituted = substitute_bindings_in_string(template, bindings);
    assert_eq!(substituted, "A template string with multiple ${but-not-all-there} substitutions");
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
