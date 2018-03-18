pub mod disk_space;
pub mod physical_memory;

use super::Sensor;

pub type DiskSpaceSensor = self::disk_space::DiskSpaceSensor;
pub type PhysicalMemorySensor = self::physical_memory::PhysicalMemorySensor;