use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub struct ResourceMetrics {
    pub kernel_image_size: u64,
    pub system_image_size: u64,
    pub boot_to_kernel_ms: u64,
    pub boot_to_init_ms: u64,
    pub boot_to_service_ms: u64,
    pub boot_to_app_ready_ms: u64,
    pub peak_ram_kb: u64,
    pub idle_ram_kb: u64,
    pub kernel_heap_kb: u64,
    measurements: Vec<(String, u64, String)>,
}

impl ResourceMetrics {
    pub fn new() -> Self {
        Self {
            kernel_image_size: 0,
            system_image_size: 0,
            boot_to_kernel_ms: 0,
            boot_to_init_ms: 0,
            boot_to_service_ms: 0,
            boot_to_app_ready_ms: 0,
            peak_ram_kb: 0,
            idle_ram_kb: 0,
            kernel_heap_kb: 0,
            measurements: Vec::new(),
        }
    }

    pub fn record(&mut self, name: &str, value: u64, unit: &str) {
        self.measurements.push((name.to_string(), value, unit.to_string()));
    }

    pub fn count(&self) -> usize {
        self.measurements.len()
    }

    pub fn total_boot_ms(&self) -> u64 {
        self.boot_to_app_ready_ms
    }
}

impl Default for ResourceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_record() {
        let mut m = ResourceMetrics::new();
        m.record("boot_time", 150, "ms");
        assert_eq!(m.count(), 1);
    }
}
