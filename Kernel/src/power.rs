//! Power and battery information (Phase 19.6).
//!
//! Exposes power state and battery information without changing power state
//! yet. Applications and services read read-only information through a power
//! service; they do not access ACPI or firmware tables directly.
//!
//! Data may be unavailable on systems without batteries (e.g. desktop-class
//! QEMU VMs). Invalid firmware data is rejected and cannot crash the system.

#![allow(dead_code)]

use alloc::vec::Vec;
use spin::Mutex;

use crate::serial;

// ---------------------------------------------------------------------------
// Power information
// ---------------------------------------------------------------------------

/// The AC / external power state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcPowerState {
    /// The system is running on battery (no external power).
    Offline,
    /// External power is connected.
    Online,
    /// The external power state could not be determined.
    Unknown,
}

/// The charging state of a battery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargingState {
    /// The battery is not charging.
    NotCharging,
    /// The battery is charging.
    Charging,
    /// The battery is fully charged.
    FullyCharged,
    /// The charging state is unknown.
    Unknown,
}

/// Validated battery information. All fields are optional; absent fields are
/// reported as `None` rather than fabricated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryInfo {
    /// Whether a battery is present.
    pub present: bool,
    /// Battery charge percentage (0..=100), if available.
    pub percentage: Option<u8>,
    /// Current charging state.
    pub charging_state: ChargingState,
    /// Battery voltage in millivolts, if available.
    pub voltage_mv: Option<u32>,
    /// Battery energy in milliwatt-hours, if available.
    pub energy_mwh: Option<u32>,
    /// Estimated time remaining in seconds, if available.
    pub time_remaining_s: Option<u64>,
    /// The battery's design capacity in mWh, if available.
    pub design_capacity_mwh: Option<u32>,
    /// The battery's last-full capacity in mWh, if available.
    pub last_full_capacity_mwh: Option<u32>,
}

/// The overall power state of the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerState {
    /// Whether external (AC) power is connected.
    pub ac_power: AcPowerState,
    /// Battery information, if a battery is present.
    pub battery: Option<BatteryInfo>,
    /// Thermal information in millicelsius, if available.
    pub thermal_mc: Option<i32>,
    /// Whether the power state has changed since the last poll.
    pub changed: bool,
}

/// Errors from power-data parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerError {
    InvalidData,
    SensorUnavailable,
    NotSupported,
    OutOfRange,
}

/// Parse a battery percentage (0..=100). Rejects out-of-range values.
pub fn parse_percentage(raw: u8) -> Result<u8, PowerError> {
    if raw > 100 {
        return Err(PowerError::OutOfRange);
    }
    Ok(raw)
}

/// Parse a battery voltage (millivolts). Rejects implausible values.
pub fn parse_voltage_mv(raw: u32) -> Result<u32, PowerError> {
    // A sane battery voltage is between 0 and 50,000 mV (50 V).
    if raw > 50_000 {
        return Err(PowerError::OutOfRange);
    }
    Ok(raw)
}

/// Parse battery energy (mWh). Rejects implausible values.
pub fn parse_energy_mwh(raw: u32) -> Result<u32, PowerError> {
    if raw > 1_000_000 {
        return Err(PowerError::OutOfRange);
    }
    Ok(raw)
}

/// Parse a charging state from a raw firmware value.
pub fn parse_charging_state(raw: u8) -> Result<ChargingState, PowerError> {
    match raw {
        0 => Ok(ChargingState::NotCharging),
        1 => Ok(ChargingState::Charging),
        2 => Ok(ChargingState::FullyCharged),
        _ => Err(PowerError::InvalidData),
    }
}

/// Parse an AC power state from a raw firmware value.
pub fn parse_ac_power_state(raw: u8) -> Result<AcPowerState, PowerError> {
    match raw {
        0 => Ok(AcPowerState::Offline),
        1 => Ok(AcPowerState::Online),
        _ => Ok(AcPowerState::Unknown),
    }
}

// ---------------------------------------------------------------------------
// Discovery sources
// ---------------------------------------------------------------------------

/// ACPI battery device information (x86-64). This mirrors the fields exposed
/// by an ACPI Embedded Controller / battery device. All fields are optional
/// because real firmware may omit them.
#[derive(Debug, Clone, Copy, Default)]
pub struct AcpiBatteryData {
    pub present: bool,
    pub percentage: Option<u8>,
    pub charging_state: Option<u8>,
    pub voltage_mv: Option<u32>,
    pub energy_mwh: Option<u32>,
    pub time_remaining_s: Option<u64>,
}

/// Convert raw ACPI battery data into validated `BatteryInfo`. Invalid fields
/// are dropped rather than propagated.
pub fn acpi_to_battery_info(data: AcpiBatteryData) -> BatteryInfo {
    let mut present = data.present;
    let mut percentage = None;
    let mut charging_state = ChargingState::Unknown;
    let mut voltage_mv = None;
    let mut energy_mwh = None;
    let mut time_remaining_s = None;

    if let Some(pct) = data.percentage {
        if parse_percentage(pct).is_ok() {
            percentage = Some(pct);
        } else {
            // Invalid percentage: treat as data problem but keep battery present.
            percentage = None;
        }
    }

    if let Some(chg) = data.charging_state {
        if let Ok(state) = parse_charging_state(chg) {
            charging_state = state;
        }
    }

    if let Some(v) = data.voltage_mv {
        if parse_voltage_mv(v).is_ok() {
            voltage_mv = Some(v);
        }
    }

    if let Some(e) = data.energy_mwh {
        if parse_energy_mwh(e).is_ok() {
            energy_mwh = Some(e);
        }
    }

    time_remaining_s = data.time_remaining_s;

    BatteryInfo {
        present,
        percentage,
        charging_state,
        voltage_mv,
        energy_mwh,
        time_remaining_s,
        design_capacity_mwh: None,
        last_full_capacity_mwh: None,
    }
}

/// Build a `BatteryInfo` for a system with no battery.
pub fn no_battery() -> BatteryInfo {
    BatteryInfo {
        present: false,
        percentage: None,
        charging_state: ChargingState::Unknown,
        voltage_mv: None,
        energy_mwh: None,
        time_remaining_s: None,
        design_capacity_mwh: None,
        last_full_capacity_mwh: None,
    }
}

// ---------------------------------------------------------------------------
// Power service (transitional, in-kernel until moved to user space)
// ---------------------------------------------------------------------------

/// The power service: polls or receives power events, caches current state,
/// and publishes read-only information to authorized applications.
pub struct PowerService {
    state: PowerState,
    generation: u64,
}

impl PowerService {
    pub const fn new() -> Self {
        Self {
            state: PowerState {
                ac_power: AcPowerState::Unknown,
                battery: None,
                thermal_mc: None,
                changed: false,
            },
            generation: 0,
        }
    }

    /// Update the cached power state from a new sample. Returns true if the
    /// state changed.
    pub fn update(&mut self, ac: AcPowerState, battery: Option<BatteryInfo>, thermal_mc: Option<i32>) -> bool {
        let mut changed = false;
        if self.state.ac_power != ac {
            self.state.ac_power = ac;
            changed = true;
        }
        if self.state.battery != battery {
            self.state.battery = battery;
            changed = true;
        }
        if self.state.thermal_mc != thermal_mc {
            self.state.thermal_mc = thermal_mc;
            changed = true;
        }
        if changed {
            self.state.changed = true;
            self.generation += 1;
        } else {
            self.state.changed = false;
        }
        changed
    }

    /// Read the current (read-only) power state.
    pub fn state(&self) -> PowerState {
        self.state
    }

    /// Whether external (AC) power is connected.
    pub fn ac_power(&self) -> AcPowerState {
        self.state.ac_power
    }

    /// Whether a battery is present.
    pub fn battery_present(&self) -> bool {
        self.state.battery.as_ref().map(|b| b.present).unwrap_or(false)
    }

    /// Battery percentage, if available.
    pub fn battery_percentage(&self) -> Option<u8> {
        self.state.battery.as_ref().and_then(|b| b.percentage)
    }

    /// Battery charging state.
    pub fn battery_charging_state(&self) -> ChargingState {
        self.state
            .battery
            .as_ref()
            .map(|b| b.charging_state)
            .unwrap_or(ChargingState::Unknown)
    }

    /// The current generation (increments on each change).
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

static POWER_SERVICE: Mutex<PowerService> = Mutex::new(PowerService {
    state: PowerState {
        ac_power: AcPowerState::Unknown,
        battery: None,
        thermal_mc: None,
        changed: false,
    },
    generation: 0,
});

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn init() {
    // No hardware access yet: report unknown power state.
    let mut svc = POWER_SERVICE.lock();
    svc.update(AcPowerState::Unknown, None, None);
    unsafe {
        serial::write_str("power: subsystem initialized\r\n");
    }
}

/// Update the power service with a new sample from a discovery source (ACPI,
/// device tree, or QEMU-provided power device).
pub fn update_power_state(ac: AcPowerState, battery: Option<BatteryInfo>, thermal_mc: Option<i32>) -> bool {
    POWER_SERVICE.lock().update(ac, battery, thermal_mc)
}

/// Read the current read-only power state.
pub fn power_state() -> PowerState {
    POWER_SERVICE.lock().state()
}

/// Whether external (AC) power is connected.
pub fn ac_power() -> AcPowerState {
    POWER_SERVICE.lock().ac_power()
}

/// Whether a battery is present.
pub fn battery_present() -> bool {
    POWER_SERVICE.lock().battery_present()
}

/// Battery percentage, if available.
pub fn battery_percentage() -> Option<u8> {
    POWER_SERVICE.lock().battery_percentage()
}

/// Battery charging state.
pub fn battery_charging_state() -> ChargingState {
    POWER_SERVICE.lock().battery_charging_state()
}

/// The current generation (increments on each change).
pub fn generation() -> u64 {
    POWER_SERVICE.lock().generation()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_percentage_valid_and_invalid_test() {
        assert_eq!(parse_percentage(0).unwrap(), 0);
        assert_eq!(parse_percentage(100).unwrap(), 100);
        assert_eq!(parse_percentage(50).unwrap(), 50);
        assert_eq!(parse_percentage(101).unwrap_err(), PowerError::OutOfRange);
        assert_eq!(parse_percentage(255).unwrap_err(), PowerError::OutOfRange);
    }

    #[test]
    fn parse_voltage_valid_and_invalid_test() {
        assert_eq!(parse_voltage_mv(12000).unwrap(), 12000);
        assert_eq!(parse_voltage_mv(0).unwrap(), 0);
        assert_eq!(parse_voltage_mv(50_000).unwrap(), 50_000);
        assert_eq!(parse_voltage_mv(50_001).unwrap_err(), PowerError::OutOfRange);
    }

    #[test]
    fn parse_energy_valid_and_invalid_test() {
        assert_eq!(parse_energy_mwh(50_000).unwrap(), 50_000);
        assert_eq!(parse_energy_mwh(1_000_000).unwrap(), 1_000_000);
        assert_eq!(parse_energy_mwh(1_000_001).unwrap_err(), PowerError::OutOfRange);
    }

    #[test]
    fn parse_charging_state_valid_and_invalid_test() {
        assert_eq!(parse_charging_state(0).unwrap(), ChargingState::NotCharging);
        assert_eq!(parse_charging_state(1).unwrap(), ChargingState::Charging);
        assert_eq!(parse_charging_state(2).unwrap(), ChargingState::FullyCharged);
        assert_eq!(parse_charging_state(3).unwrap_err(), PowerError::InvalidData);
        assert_eq!(parse_charging_state(255).unwrap_err(), PowerError::InvalidData);
    }

    #[test]
    fn parse_ac_power_state_test() {
        assert_eq!(parse_ac_power_state(0).unwrap(), AcPowerState::Offline);
        assert_eq!(parse_ac_power_state(1).unwrap(), AcPowerState::Online);
        assert_eq!(parse_ac_power_state(255).unwrap(), AcPowerState::Unknown);
    }

    #[test]
    fn acpi_to_battery_info_drops_invalid_fields() {
        let data = AcpiBatteryData {
            present: true,
            percentage: Some(150), // invalid: dropped
            charging_state: Some(9), // invalid: dropped -> Unknown
            voltage_mv: Some(999_999), // invalid: dropped
            energy_mwh: Some(50_000),
            time_remaining_s: Some(3600),
        };
        let info = acpi_to_battery_info(data);
        assert!(info.present);
        assert_eq!(info.percentage, None); // invalid dropped
        assert_eq!(info.charging_state, ChargingState::Unknown); // invalid dropped
        assert_eq!(info.voltage_mv, None); // invalid dropped
        assert_eq!(info.energy_mwh, Some(50_000));
        assert_eq!(info.time_remaining_s, Some(3600));
    }

    #[test]
    fn acpi_to_battery_info_valid() {
        let data = AcpiBatteryData {
            present: true,
            percentage: Some(75),
            charging_state: Some(1),
            voltage_mv: Some(12600),
            energy_mwh: Some(45_000),
            time_remaining_s: Some(7200),
        };
        let info = acpi_to_battery_info(data);
        assert!(info.present);
        assert_eq!(info.percentage, Some(75));
        assert_eq!(info.charging_state, ChargingState::Charging);
        assert_eq!(info.voltage_mv, Some(12600));
        assert_eq!(info.energy_mwh, Some(45_000));
    }

    #[test]
    fn no_battery_reports_absent() {
        let info = no_battery();
        assert!(!info.present);
        assert_eq!(info.percentage, None);
        assert_eq!(info.charging_state, ChargingState::Unknown);
    }

    #[test]
    fn power_service_update_detects_change() {
        let mut svc = PowerService::new();
        assert!(svc.update(AcPowerState::Online, None, None));
        assert_eq!(svc.ac_power(), AcPowerState::Online);
        assert!(!svc.update(AcPowerState::Online, None, None)); // no change
        assert_eq!(svc.generation(), 1);
    }

    #[test]
    fn power_service_battery_sample() {
        let mut svc = PowerService::new();
        let battery = Some(BatteryInfo {
            present: true,
            percentage: Some(42),
            charging_state: ChargingState::Charging,
            voltage_mv: Some(12600),
            energy_mwh: Some(45_000),
            time_remaining_s: Some(3600),
            design_capacity_mwh: None,
            last_full_capacity_mwh: None,
        });
        assert!(svc.update(AcPowerState::Online, battery, None));
        assert!(svc.battery_present());
        assert_eq!(svc.battery_percentage(), Some(42));
        assert_eq!(svc.battery_charging_state(), ChargingState::Charging);
    }

    #[test]
    fn power_service_no_battery() {
        let mut svc = PowerService::new();
        svc.update(AcPowerState::Offline, None, None);
        assert!(!svc.battery_present());
        assert_eq!(svc.battery_percentage(), None);
        assert_eq!(svc.ac_power(), AcPowerState::Offline);
    }

    #[test]
    fn public_api_no_battery() {
        // Public API reads the shared service; no battery by default.
        assert_eq!(battery_present(), false);
        assert_eq!(battery_percentage(), None);
    }
}
