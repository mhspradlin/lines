pub mod disk_space;
pub mod physical_memory;
pub mod cpu_time;

use super::Sensor;

pub type DiskSpaceSensor = self::disk_space::DiskSpaceSensor;
pub type PhysicalMemorySensor = self::physical_memory::PhysicalMemorySensor;
pub type CpuTimeSensor = self::cpu_time::CpuTimeSensor;