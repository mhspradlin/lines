#[macro_use] extern crate log;
extern crate log4rs;
use std::env;

fn main() {
    let path = env::current_dir().expect("Failure opening current dir");
    println!("Current dir: {}", path.display());

    log4rs::init_file("config/log4rs.yml", Default::default()).unwrap();
    info!("Hello, world!");
}
