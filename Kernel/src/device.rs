//! Device discovery and resource ownership (Phase 19.1).
//!
//! Every piece of hardware is represented by a device record. The device
//! manager owns the registry, assigns stable device identifiers, tracks
//! resource ownership, enforces that a device has one owner at a time, and
//! validates device handles so stale handles are rejected.
//!
//! Device failure is isolated: a failed device is marked `Failed`, its
//! resources are released where safe, and unrelated devices keep working.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use spin::Mutex;

use crate::pci;
use crate::serial;
use crate::security;

/// A stable identifier for a device record.
///
/// Assigned by the device manager at discovery time. Stable for the lifetime
/// of the record; re-registering a removed device allocates a new identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceId(pub u64);

/// A handle to a device, bound to a record generation.
///
/// The generation lets the manager reject stale handles that refer to a
/// removed or re-registered device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceHandle {
    pub id: DeviceId,
    pub generation: u64,
}

/// The kind of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Input,
    Storage,
    Network,
    Audio,
    Display,
    Power,
    Bus,
    Bridge,
    Other,
}

/// The bus a device is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusType {
    Pci,
    Usb,
    Acpi,
    Platform,
    Virtual,
}

/// The lifecycle state of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    Discovered,
    Unbound,
    Binding,
    Active,
    Suspended,
    Removing,
    Removed,
    Failed,
}

/// The power state of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    On,
    LowPower,
    Off,
    Unknown,
}

/// The kind of a device resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Mmio,
    IoPort,
    Interrupt,
    Dma,
}

/// A device resource range (end is inclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resource {
    pub kind: ResourceKind,
    pub start: u64,
    pub end: u64,
}

impl Resource {
    /// Whether this resource overlaps another.
    pub fn overlaps(self, other: Resource) -> bool {
        self.start <= other.end && other.start <= self.end
    }
}

/// Who owns a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerKind {
    Driver,
    Service,
}

/// The owner of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Owner {
    pub kind: OwnerKind,
    pub name: &'static str,
}

/// Errors from device operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceError {
    NotFound,
    AlreadyClaimed,
    NotClaimed,
    InvalidState,
    ResourceConflict,
    InvalidHandle,
    StaleHandle,
    DriverFailure,
}

/// A device record.
#[derive(Debug, Clone)]
pub struct Device {
    pub id: DeviceId,
    pub class: DeviceClass,
    pub parent: Option<DeviceId>,
    pub bus: BusType,
    pub vendor_id: u16,
    pub product_id: u16,
    pub resources: Vec<Resource>,
    pub state: DeviceState,
    pub power_state: PowerState,
    pub owner: Option<Owner>,
    pub generation: u64,
}

/// A lightweight summary of a device for diagnostics.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub id: u64,
    pub class: DeviceClass,
    pub bus: BusType,
    pub vendor_id: u16,
    pub product_id: u16,
    pub state: DeviceState,
    pub power_state: PowerState,
}

/// The device manager: global registry of all devices.
pub(crate) struct DeviceManager {
    devices: BTreeMap<u64, Device>,
    next_id: u64,
    owned_resources: Vec<(Resource, u64)>, // resource + owning device id
}

impl DeviceManager {
    fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            next_id: 1,
            owned_resources: Vec::new(),
        }
    }

    /// The next device identifier to assign.
    fn alloc_id(&mut self) -> DeviceId {
        let id = self.next_id;
        self.next_id += 1;
        DeviceId(id)
    }

    /// Register a discovered device. Returns its identifier.
    pub(crate) fn register(
        &mut self,
        class: DeviceClass,
        bus: BusType,
        vendor_id: u16,
        product_id: u16,
        resources: Vec<Resource>,
        parent: Option<DeviceId>,
    ) -> DeviceId {
        let id = self.alloc_id();
        let device = Device {
            id,
            class,
            parent,
            bus,
            vendor_id,
            product_id,
            resources,
            state: DeviceState::Discovered,
            power_state: PowerState::Unknown,
            owner: None,
            generation: 1,
        };
        self.devices.insert(id.0, device);
        id
    }

    /// Transition a registered `Discovered` device to `Unbound` so a driver
    /// can claim it.
    pub(crate) fn mark_unbound(&mut self, id: DeviceId) -> Result<(), DeviceError> {
        let device = self.devices.get_mut(&id.0).ok_or(DeviceError::NotFound)?;
        if device.state != DeviceState::Discovered {
            return Err(DeviceError::InvalidState);
        }
        device.state = DeviceState::Unbound;
        Ok(())
    }

    /// Look up a device by identifier, validating the handle generation.
    fn get_validated(&self, handle: DeviceHandle) -> Result<&Device, DeviceError> {
        let device = self
            .devices
            .get(&handle.id.0)
            .ok_or(DeviceError::NotFound)?;
        if device.generation != handle.generation {
            return Err(DeviceError::StaleHandle);
        }
        Ok(device)
    }

    /// Whether any resource of `candidate` conflicts with resources owned by
    /// a different device.
    fn resource_conflict(&self, candidate: &[Resource], owner_id: u64) -> bool {
        for res in candidate {
            for (owned, owned_id) in &self.owned_resources {
                if *owned_id == owner_id {
                    continue;
                }
                if res.overlaps(*owned) {
                    return true;
                }
            }
        }
        false
    }

    /// Claim a device for an owner and move it to `Binding`.
    ///
    /// Resource conflicts are checked before the claim is recorded. The
    /// caller must later call `activate` once initialization succeeds.
    fn claim(
        &mut self,
        handle: DeviceHandle,
        owner: Owner,
    ) -> Result<DeviceHandle, DeviceError> {
        // Validate immutably first, borrowing only `devices`.
        let device = self.devices.get(&handle.id.0).ok_or(DeviceError::NotFound)?;
        if device.generation != handle.generation {
            return Err(DeviceError::StaleHandle);
        }
        if device.state != DeviceState::Unbound {
            return Err(DeviceError::InvalidState);
        }
        if device.owner.is_some() {
            return Err(DeviceError::AlreadyClaimed);
        }
        if self.resource_conflict(&device.resources, handle.id.0) {
            return Err(DeviceError::ResourceConflict);
        }

        // Now mutate: disjoint field borrows of `devices` and
        // `owned_resources`.
        let device = self
            .devices
            .get_mut(&handle.id.0)
            .ok_or(DeviceError::NotFound)?;
        device.owner = Some(owner);
        device.state = DeviceState::Binding;
        for res in &device.resources {
            self.owned_resources.push((*res, handle.id.0));
        }
        Ok(DeviceHandle {
            id: handle.id,
            generation: device.generation,
        })
    }

    /// Mark a bound device active after successful initialization.
    fn activate(&mut self, handle: DeviceHandle) -> Result<(), DeviceError> {
        let device = self.devices.get_mut(&handle.id.0).ok_or(DeviceError::NotFound)?;
        if device.generation != handle.generation {
            return Err(DeviceError::StaleHandle);
        }
        if device.state != DeviceState::Binding {
            return Err(DeviceError::InvalidState);
        }
        device.state = DeviceState::Active;
        device.power_state = PowerState::On;
        Ok(())
    }

    /// Release a device: return it to `Unbound` and free its resources.
    fn release(&mut self, handle: DeviceHandle) -> Result<(), DeviceError> {
        let device = self.devices.get(&handle.id.0).ok_or(DeviceError::NotFound)?;
        if device.generation != handle.generation {
            return Err(DeviceError::StaleHandle);
        }
        if device.state != DeviceState::Active && device.state != DeviceState::Binding {
            return Err(DeviceError::InvalidState);
        }

        let device = self
            .devices
            .get_mut(&handle.id.0)
            .ok_or(DeviceError::NotFound)?;
        self.owned_resources
            .retain(|(_, id)| *id != handle.id.0);
        device.owner = None;
        device.state = DeviceState::Unbound;
        device.power_state = PowerState::Off;
        Ok(())
    }

    /// Remove a device record entirely. Handles to it become stale.
    fn remove(&mut self, handle: DeviceHandle) -> Result<(), DeviceError> {
        let device = self.devices.get(&handle.id.0).ok_or(DeviceError::NotFound)?;
        if device.generation != handle.generation {
            return Err(DeviceError::StaleHandle);
        }

        self.owned_resources
            .retain(|(_, id)| *id != handle.id.0);
        if let Some(mut device) = self.devices.remove(&handle.id.0) {
            device.state = DeviceState::Removed;
            device.generation += 1;
            self.devices.insert(handle.id.0, device);
        }
        Ok(())
    }

    /// Mark a device failed: release resources where safe, keep it in the
    /// registry as `Failed` so it is visible in diagnostics.
    fn fail(&mut self, handle: DeviceHandle) -> Result<(), DeviceError> {
        let device = self.devices.get(&handle.id.0).ok_or(DeviceError::NotFound)?;
        if device.generation != handle.generation {
            return Err(DeviceError::StaleHandle);
        }

        self.owned_resources
            .retain(|(_, id)| *id != handle.id.0);
        if let Some(device) = self.devices.get_mut(&handle.id.0) {
            device.owner = None;
            device.state = DeviceState::Failed;
            device.power_state = PowerState::Off;
        }
        Ok(())
    }

    /// Suspend an active device.
    fn suspend(&mut self, handle: DeviceHandle) -> Result<(), DeviceError> {
        let device = self.devices.get(&handle.id.0).ok_or(DeviceError::NotFound)?;
        if device.generation != handle.generation {
            return Err(DeviceError::StaleHandle);
        }
        if device.state != DeviceState::Active {
            return Err(DeviceError::InvalidState);
        }
        if let Some(device) = self.devices.get_mut(&handle.id.0) {
            device.state = DeviceState::Suspended;
            device.power_state = PowerState::LowPower;
        }
        Ok(())
    }

    /// Resume a suspended device.
    fn resume(&mut self, handle: DeviceHandle) -> Result<(), DeviceError> {
        let device = self.devices.get(&handle.id.0).ok_or(DeviceError::NotFound)?;
        if device.generation != handle.generation {
            return Err(DeviceError::StaleHandle);
        }
        if device.state != DeviceState::Suspended {
            return Err(DeviceError::InvalidState);
        }
        if let Some(device) = self.devices.get_mut(&handle.id.0) {
            device.state = DeviceState::Active;
            device.power_state = PowerState::On;
        }
        Ok(())
    }

    /// Bind a discovered device: claim it, then activate it.
    pub(crate) fn bind(&mut self, handle: DeviceHandle, owner: Owner) -> Result<DeviceHandle, DeviceError> {
        let handle = self.claim(handle, owner)?;
        self.activate(handle)?;
        Ok(handle)
    }

    /// A summary of every device, for diagnostics.
    fn list(&self) -> Vec<DeviceInfo> {
        self.devices
            .values()
            .map(|d| DeviceInfo {
                id: d.id.0,
                class: d.class,
                bus: d.bus,
                vendor_id: d.vendor_id,
                product_id: d.product_id,
                state: d.state,
                power_state: d.power_state,
            })
            .collect()
    }

    /// Find devices by vendor and product identifier.
    pub(crate) fn find_by_id(&self, vendor: u16, product: u16) -> Vec<DeviceHandle> {
        self.devices
            .values()
            .filter(|d| d.vendor_id == vendor && d.product_id == product)
            .map(|d| DeviceHandle {
                id: d.id,
                generation: d.generation,
            })
            .collect()
    }
}

pub(crate) static DEVICE_MANAGER: Mutex<DeviceManager> = Mutex::new(DeviceManager {
    devices: BTreeMap::new(),
    next_id: 1,
    owned_resources: Vec::new(),
});

/// Log a device event through the security log and the serial console.
fn log_event(event_type: &str, operation: &str, result: &str, id: u64) {
    security::log_event(security::SecurityEvent {
        event_type,
        pid: None,
        uid: None,
        resource_type: Some("device"),
        operation,
        result,
    });
    unsafe {
        serial::write_str("device: ");
        serial::write_str(event_type);
        serial::write_str(" id=");
        serial::write_fmt(core::format_args!("{}", id));
        serial::write_str(" op=");
        serial::write_str(operation);
        serial::write_str(" result=");
        serial::write_str(result);
        serial::write_newline();
    }
}

/// Initialize the device manager.
pub fn init() {
    log_event("device_manager", "init", "allowed", 0);
}

/// Discover the PCI devices of the machine and register them.
///
/// Safe to call on real hardware and QEMU: reading the configuration space of
/// a non-present device returns all ones and is skipped.
pub fn discover_pci() -> usize {
    let mut count = 0;
    for bus in 0..=1u8 {
        for device_number in 0..32u8 {
            for function in 0..8u8 {
                let location = pci::Location {
                    bus,
                    device: device_number,
                    function,
                };
                let candidate = pci::Device { location };
                let vendor = candidate.vendor_id();
                if vendor == 0xFFFF {
                    continue;
                }
                let product = candidate.device_id();

                let mut resources = Vec::new();
                for index in 0..6u8 {
                    if let Some(address) = candidate.bar_address(index) {
                        let size = match candidate.bar_kind(index) {
                            pci::BarKind::Memory | pci::BarKind::Memory64 => 4096,
                            pci::BarKind::InputOutput => 8,
                            pci::BarKind::Unused => continue,
                        };
                        resources.push(Resource {
                            kind: match candidate.bar_kind(index) {
                                pci::BarKind::InputOutput => ResourceKind::IoPort,
                                _ => ResourceKind::Mmio,
                            },
                            start: address,
                            end: address + size - 1,
                        });
                    }
                }

                let class = match (vendor, product) {
                    (0x1AF4, 0x1001) | (0x1AF4, 0x1042) => DeviceClass::Storage,
                    _ => DeviceClass::Other,
                };

                let mut mgr = DEVICE_MANAGER.lock();
                let id = mgr.register(
                    class,
                    BusType::Pci,
                    vendor,
                    product,
                    resources,
                    None,
                );
                drop(mgr);
                log_event("discovered", "pci_enumerate", "allowed", id.0);
                count += 1;
            }
        }
    }
    count
}

/// Bind every discovered virtio-block storage device to the block driver.
pub fn bind_virtio_block() -> usize {
    let mgr = DEVICE_MANAGER.lock();
    let devices = mgr.find_by_id(pci::VENDOR_VIRTIO, pci::DEVICE_VIRTIO_BLK_MODERN);
    let transitional = mgr.find_by_id(pci::VENDOR_VIRTIO, pci::DEVICE_VIRTIO_BLK_TRANSITIONAL);
    drop(mgr);

    let mut bound = 0;
    for handle in devices.into_iter().chain(transitional) {
        if bind_device(handle, Owner {
            kind: OwnerKind::Driver,
            name: "virtio-blk",
        })
        .is_some()
        {
            bound += 1;
        }
    }
    bound
}

/// Claim and activate a device by handle. Returns `Some(handle)` on success.
pub fn bind_device(handle: DeviceHandle, owner: Owner) -> Option<DeviceHandle> {
    let mut mgr = DEVICE_MANAGER.lock();
    let result = mgr.bind(handle, owner);
    match result {
        Ok(h) => {
            log_event("bound", "driver_claim", "allowed", h.id.0);
            Some(h)
        }
        Err(DeviceError::ResourceConflict) => {
            log_event("bound", "driver_claim", "denied", handle.id.0);
            None
        }
        Err(DeviceError::AlreadyClaimed) => {
            log_event("bound", "driver_claim", "denied", handle.id.0);
            None
        }
        Err(_) => None,
    }
}

/// Release a device by handle.
pub fn release_device(handle: DeviceHandle) -> bool {
    let mut mgr = DEVICE_MANAGER.lock();
    mgr.release(handle).is_ok()
}

/// Mark a device failed by handle.
pub fn fail_device(handle: DeviceHandle) -> bool {
    let mut mgr = DEVICE_MANAGER.lock();
    mgr.fail(handle).is_ok()
}

/// Suspend a device by handle.
pub fn suspend_device(handle: DeviceHandle) -> bool {
    let mut mgr = DEVICE_MANAGER.lock();
    mgr.suspend(handle).is_ok()
}

/// Resume a device by handle.
pub fn resume_device(handle: DeviceHandle) -> bool {
    let mut mgr = DEVICE_MANAGER.lock();
    mgr.resume(handle).is_ok()
}

/// Look up a device by handle, validating the generation. Returns `None` for
/// unknown or stale handles.
pub fn get_device(handle: DeviceHandle) -> Option<Device> {
    let mgr = DEVICE_MANAGER.lock();
    mgr.get_validated(handle).cloned().ok()
}

/// A copy of a device's public fields.
pub struct DeviceSnapshot {
    pub id: u64,
    pub class: DeviceClass,
    pub bus: BusType,
    pub vendor_id: u16,
    pub product_id: u16,
    pub state: DeviceState,
    pub power_state: PowerState,
    pub owner: Option<&'static str>,
}

/// Snapshot of every device for diagnostics.
pub fn device_snapshots() -> Vec<DeviceSnapshot> {
    let mgr = DEVICE_MANAGER.lock();
    let list = mgr.list();
    list.into_iter()
        .map(|info| {
            let owner = mgr
                .devices
                .get(&info.id)
                .and_then(|d| d.owner.map(|o| o.name));
            DeviceSnapshot {
                id: info.id,
                class: info.class,
                bus: info.bus,
                vendor_id: info.vendor_id,
                product_id: info.product_id,
                state: info.state,
                power_state: info.power_state,
                owner,
            }
        })
        .collect()
}

/// Print every device to the serial console.
pub fn report() {
    let snapshots = device_snapshots();
    unsafe {
        serial::write_str("device: registry contains ");
        serial::write_fmt(core::format_args!("{} devices\r\n", snapshots.len()));
    }
    for s in snapshots {
        unsafe {
            serial::write_str("device: id=");
            serial::write_fmt(core::format_args!("{}", s.id));
            serial::write_str(" class=");
            serial::write_fmt(core::format_args!("{:?}", s.class));
            serial::write_str(" bus=");
            serial::write_fmt(core::format_args!("{:?}", s.bus));
            serial::write_str(" vendor=0x");
            serial::write_fmt(core::format_args!("{:x}", s.vendor_id));
            serial::write_str(" product=0x");
            serial::write_fmt(core::format_args!("{:x}", s.product_id));
            serial::write_str(" state=");
            serial::write_fmt(core::format_args!("{:?}", s.state));
            serial::write_str(" power=");
            serial::write_fmt(core::format_args!("{:?}", s.power_state));
            if let Some(owner) = s.owner {
                serial::write_str(" owner=");
                serial::write_str(owner);
            }
            serial::write_newline();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> Owner {
        Owner {
            kind: OwnerKind::Driver,
            name: "test-driver",
        }
    }

    fn mmio_resource(start: u64) -> Resource {
        Resource {
            kind: ResourceKind::Mmio,
            start,
            end: start + 4095,
        }
    }

    fn register_unbound_storage(mgr: &mut DeviceManager) -> DeviceHandle {
        let id = mgr.register(
            DeviceClass::Storage,
            BusType::Pci,
            0x1AF4,
            0x1042,
            vec![mmio_resource(0xF000_0000)],
            None,
        );
        let dev = mgr.devices.get_mut(&id.0).unwrap();
        dev.state = DeviceState::Unbound;
        DeviceHandle { id, generation: 1 }
    }

    #[test]
    fn device_registered_is_discovered() {
        let mut mgr = DeviceManager::new();
        let id = mgr.register(
            DeviceClass::Input,
            BusType::Pci,
            0x1234,
            0x5678,
            Vec::new(),
            None,
        );
        let dev = mgr.devices.get(&id.0).unwrap();
        assert_eq!(dev.state, DeviceState::Discovered);
        assert_eq!(dev.vendor_id, 0x1234);
        assert_eq!(dev.product_id, 0x5678);
        assert_eq!(dev.generation, 1);
    }

    #[test]
    fn claim_unbound_device_succeeds() {
        let mut mgr = DeviceManager::new();
        let handle = register_unbound_storage(&mut mgr);
        let h = mgr.claim(handle, owner()).unwrap();
        assert_eq!(h.generation, handle.generation);
        let dev = mgr.devices.get(&handle.id.0).unwrap();
        assert_eq!(dev.state, DeviceState::Binding);
        assert!(dev.owner.is_some());
    }

    #[test]
    fn duplicate_claim_is_rejected() {
        let mut mgr = DeviceManager::new();
        let handle = register_unbound_storage(&mut mgr);
        assert!(mgr.claim(handle, owner()).is_ok());
        // A second claim while the device is bound must be rejected.
        assert!(mgr.claim(handle, owner()).is_err());
    }

    #[test]
    fn resource_conflict_is_rejected() {
        let mut mgr = DeviceManager::new();
        let h1 = register_unbound_storage(&mut mgr);
        mgr.claim(h1, owner()).unwrap();

        let id2 = mgr.register(
            DeviceClass::Storage,
            BusType::Pci,
            0x1AF4,
            0x1001,
            vec![mmio_resource(0xF000_0000)],
            None,
        );
        let mut h2 = DeviceHandle { id: id2, generation: 1 };
        let dev2 = mgr.devices.get_mut(&id2.0).unwrap();
        dev2.state = DeviceState::Unbound;
        assert_eq!(
            mgr.claim(h2, owner()).unwrap_err(),
            DeviceError::ResourceConflict
        );
    }

    #[test]
    fn non_overlapping_resources_can_coexist() {
        let mut mgr = DeviceManager::new();
        let h1 = register_unbound_storage(&mut mgr);
        mgr.claim(h1, owner()).unwrap();

        let id2 = mgr.register(
            DeviceClass::Storage,
            BusType::Pci,
            0x1AF4,
            0x1001,
            vec![mmio_resource(0xF001_0000)],
            None,
        );
        let h2 = DeviceHandle { id: id2, generation: 1 };
        let dev2 = mgr.devices.get_mut(&id2.0).unwrap();
        dev2.state = DeviceState::Unbound;
        assert!(mgr.claim(h2, owner()).is_ok());
    }

    #[test]
    fn activate_then_release_returns_to_unbound() {
        let mut mgr = DeviceManager::new();
        let handle = register_unbound_storage(&mut mgr);
        mgr.claim(handle, owner()).unwrap();
        mgr.activate(handle).unwrap();
        assert!(mgr.release(handle).is_ok());
        let dev = mgr.devices.get(&handle.id.0).unwrap();
        assert_eq!(dev.state, DeviceState::Unbound);
        assert!(dev.owner.is_none());
        assert_eq!(dev.power_state, PowerState::Off);
    }

    #[test]
    fn stale_handle_is_rejected_after_removal() {
        let mut mgr = DeviceManager::new();
        let handle = register_unbound_storage(&mut mgr);
        mgr.claim(handle, owner()).unwrap();
        mgr.remove(handle).unwrap();
        assert_eq!(
            mgr.get_validated(handle).unwrap_err(),
            DeviceError::StaleHandle
        );
    }

    #[test]
    fn failed_device_releases_resources() {
        let mut mgr = DeviceManager::new();
        let handle = register_unbound_storage(&mut mgr);
        mgr.claim(handle, owner()).unwrap();
        mgr.activate(handle).unwrap();
        assert!(mgr.fail(handle).is_ok());
        let dev = mgr.devices.get(&handle.id.0).unwrap();
        assert_eq!(dev.state, DeviceState::Failed);
        assert!(dev.owner.is_none());
        // Resources freed: a new device may use the same MMIO range.
        let id2 = mgr.register(
            DeviceClass::Storage,
            BusType::Pci,
            0x1AF4,
            0x1001,
            vec![mmio_resource(0xF000_0000)],
            None,
        );
        let mut h2 = DeviceHandle { id: id2, generation: 1 };
        let dev2 = mgr.devices.get_mut(&id2.0).unwrap();
        dev2.state = DeviceState::Unbound;
        assert!(mgr.claim(h2, owner()).is_ok());
    }

    #[test]
    fn suspend_and_resume_transition() {
        let mut mgr = DeviceManager::new();
        let handle = register_unbound_storage(&mut mgr);
        mgr.claim(handle, owner()).unwrap();
        mgr.activate(handle).unwrap();
        assert!(mgr.suspend(handle).is_ok());
        assert_eq!(
            mgr.devices.get(&handle.id.0).unwrap().state,
            DeviceState::Suspended
        );
        assert!(mgr.resume(handle).is_ok());
        assert_eq!(
            mgr.devices.get(&handle.id.0).unwrap().state,
            DeviceState::Active
        );
    }

    #[test]
    fn find_by_id_returns_matching_handles() {
        let mut mgr = DeviceManager::new();
        let h1 = register_unbound_storage(&mut mgr);
        let id2 = mgr.register(
            DeviceClass::Network,
            BusType::Pci,
            0x1AF4,
            0x1042,
            Vec::new(),
            None,
        );
        let h2 = DeviceHandle { id: id2, generation: 1 };
        let found = mgr.find_by_id(0x1AF4, 0x1042);
        assert_eq!(found.len(), 2);
        assert!(found.contains(&h1));
        assert!(found.contains(&h2));
    }
}
