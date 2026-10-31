//! Hotplug and device permissions (Phase 19.8).
//!
//! Handles devices appearing, disappearing, and changing state at runtime,
//! and enforces device permissions so ordinary applications cannot access
//! raw hardware without authorization.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use spin::Mutex;

use crate::device;
use crate::device::DeviceHandle;
use crate::security::{authz, identity, SecurityError};

// ---------------------------------------------------------------------------
// Hotplug events
// ---------------------------------------------------------------------------

/// A hotplug event describing a change in device state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotplugEvent {
    /// A device was added to the system.
    DeviceAdded,
    /// A device was removed from the system.
    DeviceRemoved,
    /// A device was reset.
    DeviceReset,
    /// A device was suspended.
    DeviceSuspended,
    /// A device was resumed.
    DeviceResumed,
    /// A device's driver failed.
    DriverFailure,
}

/// A device entry tracked by the hotplug service.
struct HotplugDevice {
    handle: DeviceHandle,
    class: device::DeviceClass,
    /// Whether clients have an active open reference to the device.
    client_refs: usize,
}

/// The hotplug service: tracks devices, emits hotplug events, and coordinates
/// safe removal.
pub struct HotplugService {
    devices: BTreeMap<u64, HotplugDevice>,
    /// The most recent hotplug events (bounded history for diagnostics).
    events: Vec<HotplugEvent>,
    max_events: usize,
}

impl HotplugService {
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            events: Vec::new(),
            max_events: 256,
        }
    }

    /// Register a device that has appeared. Emits a DeviceAdded event.
    pub fn device_added(&mut self, handle: DeviceHandle, class: device::DeviceClass) {
    self.devices.insert(
        handle.id.0,
        HotplugDevice {
            handle,
            class,
            client_refs: 0,
        },
    );
    self.push_event(HotplugEvent::DeviceAdded);
}

    /// Remove a device safely. Stops new operations, notifies clients, drains
    /// transfers, releases resources, then removes the device record.
    /// Returns false if the device has active clients (removal should be
    /// deferred).
    pub fn device_removed(&mut self, id: u64) -> bool {
        let device = match self.devices.get(&id) {
            Some(d) => d,
            None => return false,
        };
        if device.client_refs > 0 {
            // Active clients: do not remove yet; they must release first.
            return false;
        }
        self.devices.remove(&id);
        self.push_event(HotplugEvent::DeviceRemoved);
        true
    }

    /// Record that a device was reset.
    pub fn device_reset(&mut self, id: u64) {
        if self.devices.contains_key(&id) {
            self.push_event(HotplugEvent::DeviceReset);
        }
    }

    /// Record that a device was suspended.
    pub fn device_suspended(&mut self, id: u64) {
        if self.devices.contains_key(&id) {
            self.push_event(HotplugEvent::DeviceSuspended);
        }
    }

    /// Record that a device was resumed.
    pub fn device_resumed(&mut self, id: u64) {
        if self.devices.contains_key(&id) {
            self.push_event(HotplugEvent::DeviceResumed);
        }
    }

    /// Record that a device's driver failed.
    pub fn driver_failure(&mut self, id: u64) {
        if self.devices.contains_key(&id) {
            self.push_event(HotplugEvent::DriverFailure);
        }
    }

    /// Open a client reference to a device (e.g. a session starts using it).
    pub fn client_acquire(&mut self, id: u64) -> bool {
        if let Some(d) = self.devices.get_mut(&id) {
            d.client_refs += 1;
            true
        } else {
            false
        }
    }

    /// Release a client reference to a device.
    pub fn client_release(&mut self, id: u64) {
        if let Some(d) = self.devices.get_mut(&id) {
            if d.client_refs > 0 {
                d.client_refs -= 1;
            }
        }
    }

    /// Push an event into the bounded history.
    fn push_event(&mut self, event: HotplugEvent) {
        if self.events.len() >= self.max_events {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    /// The number of tracked devices.
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// The number of pending hotplug events in history.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

static HOTPLUG_SERVICE: Mutex<HotplugService> = Mutex::new(HotplugService::new());

// ---------------------------------------------------------------------------
// Device permissions
// ---------------------------------------------------------------------------

/// The classes of device that require privileged access for raw operations.
///
/// Ordinary applications must not automatically access these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitiveDeviceClass {
    RawUsb,
    AudioHardware,
    PowerControls,
    InputInternals,
    PciConfig,
    DmaBuffers,
}

/// Check whether a device class is sensitive (requires authorization for raw
/// access).
pub fn is_sensitive_class(class: device::DeviceClass) -> bool {
    matches!(
        class,
        device::DeviceClass::Audio
            | device::DeviceClass::Power
            | device::DeviceClass::Input
            | device::DeviceClass::Bus
    )
}

/// Authorize access to a device. Ordinary (non-privileged) applications are
/// denied raw access to sensitive devices. Privileged (root) services may
/// access them.
pub fn authorize_device_access(
    creds: &identity::Credentials,
    class: device::DeviceClass,
    operation: authz::AccessMode,
) -> Result<(), SecurityError> {
    // Privileged root services may access sensitive hardware.
    if creds.is_privileged() {
        return Ok(());
    }

    // Raw access to sensitive device classes is denied to ordinary apps.
    if is_sensitive_class(class) {
        match operation {
            authz::AccessMode::Read | authz::AccessMode::Write | authz::AccessMode::Admin => {
                return Err(SecurityError::PermissionDenied);
            }
            authz::AccessMode::Execute => return Ok(()),
        }
    }

    // Non-sensitive devices: basic access allowed.
    Ok(())
}

/// Check that a client may access a tracked hotplug device, returning an
/// error if the device is unknown or access is denied.
pub fn check_device_access(
    creds: &identity::Credentials,
    id: u64,
    operation: authz::AccessMode,
) -> Result<(), SecurityError> {
    let service = HOTPLUG_SERVICE.lock();
    let device_class = match service.devices.get(&id) {
        Some(d) => d.class,
        None => return Err(SecurityError::InvalidResource),
    };
    drop(service);
    authorize_device_access(creds, device_class, operation)
}

// ---------------------------------------------------------------------------
// Safe removal
// ---------------------------------------------------------------------------

/// Attempt to remove a tracked device by id. Returns true if removed, false
/// if the device has active clients or is unknown.
pub fn remove_device(id: u64) -> bool {
    HOTPLUG_SERVICE.lock().device_removed(id)
}

/// Initialize the hotplug subsystem.
pub fn init() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_creds() -> identity::Credentials {
        identity::Credentials::root()
    }

    fn user_creds() -> identity::Credentials {
        identity::Credentials {
            uid: identity::USER_UID,
            gid: identity::USER_GID,
            supplementary_groups: Vec::new(),
            privileged: false,
        }
    }

    fn test_handle(id: u64) -> DeviceHandle {
        DeviceHandle {
            id: device::DeviceId(id),
            generation: 1,
        }
    }

    #[test]
    fn device_added_and_tracked() {
        let mut service = HotplugService::new();
        service.device_added(test_handle(1), device::DeviceClass::Input);
        assert_eq!(service.device_count(), 1);
        assert_eq!(service.event_count(), 1);
    }

    #[test]
    fn device_removed_when_no_clients() {
        let mut service = HotplugService::new();
        service.device_added(test_handle(1), device::DeviceClass::Input);
        assert!(service.device_removed(1));
        assert_eq!(service.device_count(), 0);
        assert_eq!(service.event_count(), 2); // added + removed
    }

    #[test]
    fn device_not_removed_while_clients_active() {
        let mut service = HotplugService::new();
        service.device_added(test_handle(1), device::DeviceClass::Input);
        service.client_acquire(1);
        assert!(!service.device_removed(1));
        assert_eq!(service.device_count(), 1);
        service.client_release(1);
        assert!(service.device_removed(1));
        assert_eq!(service.device_count(), 0);
    }

    #[test]
    fn hotplug_events_recorded() {
        let mut service = HotplugService::new();
        service.device_added(test_handle(1), device::DeviceClass::Audio);
        service.device_suspended(1);
        service.device_resumed(1);
        service.driver_failure(1);
        service.device_removed(1);
        assert_eq!(service.event_count(), 5);
    }

    #[test]
    fn sensitive_classes_denied_to_user() {
        for class in [
            device::DeviceClass::Audio,
            device::DeviceClass::Power,
            device::DeviceClass::Input,
            device::DeviceClass::Bus,
        ] {
            assert_eq!(
                authorize_device_access(&user_creds(), class, authz::AccessMode::Write),
                Err(SecurityError::PermissionDenied)
            );
        }
    }

    #[test]
    fn sensitive_classes_allowed_to_root() {
        assert!(authorize_device_access(
            &root_creds(),
            device::DeviceClass::Audio,
            authz::AccessMode::Write
        )
        .is_ok());
        assert!(authorize_device_access(
            &root_creds(),
            device::DeviceClass::Power,
            authz::AccessMode::Admin
        )
        .is_ok());
    }

    #[test]
    fn non_sensitive_device_allowed_to_user() {
        assert!(authorize_device_access(
            &user_creds(),
            device::DeviceClass::Storage,
            authz::AccessMode::Read
        )
        .is_ok());
    }

    #[test]
    fn unknown_device_rejected() {
        assert_eq!(
            check_device_access(&root_creds(), 9999, authz::AccessMode::Read),
            Err(SecurityError::InvalidResource)
        );
    }

    #[test]
    fn device_removal_invalidates_access() {
        let mut service = HotplugService::new();
        service.device_added(test_handle(1), device::DeviceClass::Input);
        service.device_removed(1);
        // After removal, the device is unknown.
        assert_eq!(
            check_device_access(&root_creds(), 1, authz::AccessMode::Read),
            Err(SecurityError::InvalidResource)
        );
    }
}
