//! Resource and startup measurements.

pub mod metrics;

pub use metrics::ResourceMetrics;

use core::fmt;

#[derive(Debug, Clone)]
pub struct Measurement {
    pub name: String,
    pub value: u64,
    pub unit: String,
}

impl fmt::Display for Measurement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} {}", self.name, self.value, self.unit)
    }
}
