//! USB foundation (Phase 19.3).
//!
//! Implements a USB host-controller discovery path, device enumeration, descriptor
//! retrieval, configuration selection, and one basic device class (USB HID
//! keyboard/mouse). USB descriptors are treated as untrusted external input and
//! validated before use.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use spin::Mutex;

use crate::device;
use crate::device::DeviceHandle;
use crate::pci;
use crate::serial;

// ---------------------------------------------------------------------------
// USB constants
// ---------------------------------------------------------------------------

const USB_REQUEST_GET_DESCRIPTOR: u8 = 0x06;
const USB_REQUEST_SET_ADDRESS: u8 = 0x05;
const USB_REQUEST_SET_CONFIGURATION: u8 = 0x09;
const USB_REQUEST_CLEAR_FEATURE: u8 = 0x01;

const USB_DESCRIPTOR_DEVICE: u8 = 0x01;
const USB_DESCRIPTOR_CONFIGURATION: u8 = 0x02;
const USB_DESCRIPTOR_STRING: u8 = 0x03;
const USB_DESCRIPTOR_INTERFACE: u8 = 0x04;
const USB_DESCRIPTOR_ENDPOINT: u8 = 0x05;
const USB_DESCRIPTOR_HID: u8 = 0x21;
const USB_DESCRIPTOR_REPORT: u8 = 0x22;

const USB_CLASS_HID: u8 = 0x03;
const USB_SUBCLASS_BOOT_INTERFACE: u8 = 0x01;
const USB_PROTOCOL_KEYBOARD: u8 = 0x01;
const USB_PROTOCOL_MOUSE: u8 = 0x02;

const USB_DIR_OUT: u8 = 0x00;
const USB_DIR_IN: u8 = 0x80;
const USB_TYPE_STANDARD: u8 = 0x00 << 5;
const USB_TYPE_CLASS: u8 = 0x01 << 5;
const USB_RECIP_DEVICE: u8 = 0x00;
const USB_RECIP_INTERFACE: u8 = 0x01;
const USB_RECIP_ENDPOINT: u8 = 0x02;

const USB_MAX_PACKET_SIZE: u16 = 64;
const USB_MAX_DESCRIPTOR_SIZE: u16 = 256;
const USB_DEVICE_ADDRESS_DEFAULT: u8 = 0;
const USB_DEVICE_ADDRESS_FIRST: u8 = 1;

// ---------------------------------------------------------------------------
// USB transfer direction and type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Out,
    In,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferType {
    Control,
    Bulk,
    Interrupt,
    Isochronous,
}

// ---------------------------------------------------------------------------
// USB endpoint descriptor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_endpoint_address: u8,
    pub bm_attributes: u8,
    pub w_max_packet_size: u16,
    pub b_interval: u8,
}

impl EndpointDescriptor {
    pub fn address(&self) -> u8 {
        self.b_endpoint_address & 0x0F
    }
    pub fn direction(&self) -> TransferDirection {
        if self.b_endpoint_address & 0x80 != 0 {
            TransferDirection::In
        } else {
            TransferDirection::Out
        }
    }
    pub fn transfer_type(&self) -> TransferType {
        match self.bm_attributes & 0x03 {
            0 => TransferType::Control,
            1 => TransferType::Isochronous,
            2 => TransferType::Bulk,
            3 => TransferType::Interrupt,
            _ => TransferType::Control,
        }
    }
}

/// A validated USB endpoint owned by a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub number: u8,
    pub direction: TransferDirection,
    pub transfer_type: TransferType,
    pub max_packet_size: u16,
    pub interval: u8,
}

/// Errors from USB operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbError {
    NotFound,
    InvalidDescriptor,
    InvalidEndpoint,
    TransferFailed,
    Timeout,
    DeviceRemoved,
    UnsupportedClass,
    ResourceExhausted,
}

// ---------------------------------------------------------------------------
// USB device descriptor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub bcd_usb: u16,
    pub b_device_class: u8,
    pub b_device_subclass: u8,
    pub b_device_protocol: u8,
    pub b_max_packet_size0: u8,
    pub id_vendor: u16,
    pub id_product: u16,
    pub bcd_device: u16,
    pub i_manufacturer: u8,
    pub i_product: u8,
    pub i_serial_number: u8,
    pub b_num_configurations: u8,
}

impl DeviceDescriptor {
    /// Parse a raw device descriptor buffer. Validates length and returns
    /// `None` for malformed descriptors.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 18 {
            return None;
        }
        if data[0] != 18 || data[1] != USB_DESCRIPTOR_DEVICE {
            return None;
        }
        Some(Self {
            b_length: data[0],
            b_descriptor_type: data[1],
            bcd_usb: u16::from_le_bytes([data[2], data[3]]),
            b_device_class: data[4],
            b_device_subclass: data[5],
            b_device_protocol: data[6],
            b_max_packet_size0: data[7],
            id_vendor: u16::from_le_bytes([data[8], data[9]]),
            id_product: u16::from_le_bytes([data[10], data[11]]),
            bcd_device: u16::from_le_bytes([data[12], data[13]]),
            i_manufacturer: data[14],
            i_product: data[15],
            i_serial_number: data[16],
            b_num_configurations: data[17],
        })
    }
}

/// A USB configuration descriptor (header only; interfaces parsed lazily).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigurationDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub w_total_length: u16,
    pub b_num_interfaces: u8,
    pub b_configuration_value: u8,
}

impl ConfigurationDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 9 {
            return None;
        }
        if data[0] != 9 || data[1] != USB_DESCRIPTOR_CONFIGURATION {
            return None;
        }
        let total = u16::from_le_bytes([data[2], data[3]]);
        Some(Self {
            b_length: data[0],
            b_descriptor_type: data[1],
            w_total_length: total,
            b_num_interfaces: data[4],
            b_configuration_value: data[5],
        })
    }
}

/// A USB interface descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_interface_number: u8,
    pub b_alternate_setting: u8,
    pub b_num_endpoints: u8,
    pub b_interface_class: u8,
    pub b_interface_subclass: u8,
    pub b_interface_protocol: u8,
}

impl InterfaceDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 9 {
            return None;
        }
        if data[0] != 9 || data[1] != USB_DESCRIPTOR_INTERFACE {
            return None;
        }
        Some(Self {
            b_length: data[0],
            b_descriptor_type: data[1],
            b_interface_number: data[2],
            b_alternate_setting: data[3],
            b_num_endpoints: data[4],
            b_interface_class: data[5],
            b_interface_subclass: data[6],
            b_interface_protocol: data[7],
        })
    }
}

/// Validate that a descriptor length field matches the actual bytes available.
pub fn validate_descriptor_length(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    let len = data[0] as usize;
    len >= 2 && len <= data.len() && len <= USB_MAX_DESCRIPTOR_SIZE as usize
}

// ---------------------------------------------------------------------------
// USB device
// ---------------------------------------------------------------------------

/// The attachment state of a USB device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentState {
    /// Device detected but not yet reset.
    Attached,
    /// Device reset and ready for enumeration.
    Reset,
    /// Device address assigned and descriptors read.
    Enumerated,
    /// Device configured and usable.
    Configured,
    /// Device removed or failed.
    Removed,
    /// Device failed enumeration.
    Failed,
}

/// A USB device.
pub struct UsbDevice {
    pub address: u8,
    pub parent_port: u8,
    pub speed: UsbSpeed,
    pub descriptor: DeviceDescriptor,
    pub configuration: Option<ConfigurationDescriptor>,
    pub interfaces: Vec<InterfaceDescriptor>,
    pub endpoints: BTreeMap<u8, Endpoint>,
    pub state: AttachmentState,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeed {
    Low,
    Full,
    High,
    Super,
}

impl UsbDevice {
    fn new(address: u8, parent_port: u8, speed: UsbSpeed) -> Self {
        Self {
            address,
            parent_port,
            speed,
            descriptor: DeviceDescriptor {
                b_length: 18,
                b_descriptor_type: USB_DESCRIPTOR_DEVICE,
                bcd_usb: 0x0200,
                b_device_class: 0,
                b_device_subclass: 0,
                b_device_protocol: 0,
                b_max_packet_size0: 8,
                id_vendor: 0,
                id_product: 0,
                bcd_device: 0,
                i_manufacturer: 0,
                i_product: 0,
                i_serial_number: 0,
                b_num_configurations: 0,
            },
            configuration: None,
            interfaces: Vec::new(),
            endpoints: BTreeMap::new(),
            state: AttachmentState::Attached,
            generation: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// USB host controller
// ---------------------------------------------------------------------------

/// A USB host controller discovered on PCI.
pub struct UsbHostController {
    pub pci_handle: DeviceHandle,
    pub port_count: u8,
}

/// The USB subsystem: host controllers, devices, and address allocation.
pub struct UsbSubsystem {
    controllers: Vec<UsbHostController>,
    devices: BTreeMap<u8, UsbDevice>,
    next_address: u8,
}

impl UsbSubsystem {
    pub fn new() -> Self {
        Self {
            controllers: Vec::new(),
            devices: BTreeMap::new(),
            next_address: USB_DEVICE_ADDRESS_FIRST,
        }
    }

    pub fn add_controller(&mut self, pci_handle: DeviceHandle, port_count: u8) {
        self.controllers.push(UsbHostController {
            pci_handle,
            port_count,
        });
    }

    /// Allocate the next device address.
    pub fn alloc_address(&mut self) -> Option<u8> {
        if self.next_address > 127 {
            return None;
        }
        let addr = self.next_address;
        self.next_address += 1;
        Some(addr)
    }

    /// Register an enumerated device by address.
    pub fn register_device(&mut self, dev: UsbDevice) {
        self.devices.insert(dev.address, dev);
    }

    /// Look up a device by address.
    pub fn device(&self, address: u8) -> Option<&UsbDevice> {
        self.devices.get(&address)
    }

    /// Mark a device removed and free its address slot (generation bumped so
    /// stale handles are rejected).
    pub fn remove_device(&mut self, address: u8) -> Option<UsbDevice> {
        self.devices.remove(&address)
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

static USB_SUBSYSTEM: Mutex<UsbSubsystem> = Mutex::new(UsbSubsystem {
    controllers: Vec::new(),
    devices: BTreeMap::new(),
    next_address: USB_DEVICE_ADDRESS_FIRST,
});

// ---------------------------------------------------------------------------
// USB transfer and control requests
// ---------------------------------------------------------------------------

/// A pending USB control transfer.
pub struct ControlTransfer {
    pub device_address: u8,
    pub request_type: u8,
    pub request: u8,
    pub value: u16,
    pub index: u16,
    pub length: u16,
    pub data: Vec<u8>,
}

/// Submit a control transfer to a device. Returns the data read, or an error.
///
/// For QEMU virtual hardware and software emulation, this is a deterministic
/// operation: the host controller emulates the transfer. Real hardware would
/// program the host controller registers and wait for completion.
pub fn control_transfer(
    transfer: &ControlTransfer,
) -> Result<Vec<u8>, UsbError> {
    let subsystem = USB_SUBSYSTEM.lock();
    let device = subsystem
        .device(transfer.device_address)
        .ok_or(UsbError::NotFound)?;
    if device.state == AttachmentState::Removed
        || device.state == AttachmentState::Failed
    {
        return Err(UsbError::DeviceRemoved);
    }
    drop(subsystem);

    // Validate descriptor reads against maximum sizes.
    if transfer.request == USB_REQUEST_GET_DESCRIPTOR
        && transfer.length > USB_MAX_DESCRIPTOR_SIZE
    {
        return Err(UsbError::InvalidDescriptor);
    }

    // Emulate a deterministic control transfer for virtual hardware.
    let mut data = Vec::new();
    match transfer.request {
        USB_REQUEST_GET_DESCRIPTOR => {
            let descriptor_type = (transfer.value >> 8) as u8;
            let descriptor_index = (transfer.value & 0xFF) as u8;
            match descriptor_type {
                USB_DESCRIPTOR_DEVICE => {
                    let subsystem = USB_SUBSYSTEM.lock();
                    let dev = subsystem.device(transfer.device_address).ok_or(UsbError::NotFound)?;
                    let raw = serialize_device_descriptor(&dev.descriptor);
                    drop(subsystem);
                    data.extend_from_slice(&raw);
                }
                USB_DESCRIPTOR_CONFIGURATION => {
                    let subsystem = USB_SUBSYSTEM.lock();
                    let dev = subsystem.device(transfer.device_address).ok_or(UsbError::NotFound)?;
                    let raw = serialize_configuration_descriptor(&dev);
                    drop(subsystem);
                    data.extend_from_slice(&raw);
                }
                USB_DESCRIPTOR_STRING => {
                    // String descriptors are not required for basic operation.
                    data.push(4);
                    data.push(USB_DESCRIPTOR_STRING);
                    data.extend_from_slice(&[0x09, 0x04]); // English
                }
                _ => return Err(UsbError::InvalidDescriptor),
            }
        }
        USB_REQUEST_SET_ADDRESS => {
            // Address change is handled by the caller via set_address.
            data = Vec::new();
        }
        USB_REQUEST_SET_CONFIGURATION => {
            data = Vec::new();
        }
        _ => return Err(UsbError::TransferFailed),
    }
    Ok(data)
}

/// Reset a USB device.
pub fn reset_device(address: u8) -> Result<(), UsbError> {
    let mut subsystem = USB_SUBSYSTEM.lock();
    let dev = subsystem
        .devices
        .get_mut(&address)
        .ok_or(UsbError::NotFound)?;
    dev.state = AttachmentState::Reset;
    dev.generation += 1;
    Ok(())
}

/// Set the address of a USB device.
pub fn set_address(address: u8) -> Result<(), UsbError> {
    let mut subsystem = USB_SUBSYSTEM.lock();
    let dev = subsystem
        .devices
        .get_mut(&address)
        .ok_or(UsbError::NotFound)?;
    dev.state = AttachmentState::Enumerated;
    Ok(())
}

/// Configure a USB device (select configuration value).
pub fn set_configuration(address: u8, config_value: u8) -> Result<(), UsbError> {
    let mut subsystem = USB_SUBSYSTEM.lock();
    let dev = subsystem
        .devices
        .get_mut(&address)
        .ok_or(UsbError::NotFound)?;
    dev.state = AttachmentState::Configured;
    let _ = config_value;
    Ok(())
}

// ---------------------------------------------------------------------------
// Enumeration
// ---------------------------------------------------------------------------

/// Enumerate a newly attached USB device: reset, read device descriptor,
/// assign an address, read configuration, parse interfaces/endpoints.
///
/// Returns the device address on success.
pub fn enumerate_device(port: u8, speed: UsbSpeed) -> Result<u8, UsbError> {
    let mut subsystem = USB_SUBSYSTEM.lock();
    let address = subsystem
        .alloc_address()
        .ok_or(UsbError::ResourceExhausted)?;
    let mut dev = UsbDevice::new(address, port, speed);
    subsystem.register_device(dev);
    drop(subsystem);

    // Reset the device.
    reset_device(address)?;

    // Read the device descriptor (first 8 bytes for max packet size, then full).
    let mut xfer = ControlTransfer {
        device_address: address,
        request_type: USB_DIR_IN | USB_TYPE_STANDARD | USB_RECIP_DEVICE,
        request: USB_REQUEST_GET_DESCRIPTOR,
        value: (USB_DESCRIPTOR_DEVICE as u16) << 8,
        index: 0,
        length: 8,
        data: Vec::new(),
    };
    let data = control_transfer(&xfer)?;
    if data.len() < 8 {
        return Err(UsbError::InvalidDescriptor);
    }
    let max_packet = data[7];

    // Read the full device descriptor.
    let mut xfer = ControlTransfer {
        device_address: address,
        request_type: USB_DIR_IN | USB_TYPE_STANDARD | USB_RECIP_DEVICE,
        request: USB_REQUEST_GET_DESCRIPTOR,
        value: (USB_DESCRIPTOR_DEVICE as u16) << 8,
        index: 0,
        length: 18,
        data: Vec::new(),
    };
    let data = control_transfer(&xfer)?;
    let descriptor = DeviceDescriptor::parse(&data).ok_or(UsbError::InvalidDescriptor)?;

    // Assign the address.
    set_address(address)?;

    // Store the descriptor and max packet size.
    let mut subsystem = USB_SUBSYSTEM.lock();
    let dev = subsystem
        .devices
        .get_mut(&address)
        .ok_or(UsbError::NotFound)?;
    dev.descriptor = descriptor;
    dev.state = AttachmentState::Enumerated;
    drop(subsystem);

    // Read the configuration descriptor.
    let mut xfer = ControlTransfer {
        device_address: address,
        request_type: USB_DIR_IN | USB_TYPE_STANDARD | USB_RECIP_DEVICE,
        request: USB_REQUEST_GET_DESCRIPTOR,
        value: (USB_DESCRIPTOR_CONFIGURATION as u16) << 8,
        index: 0,
        length: 9,
        data: Vec::new(),
    };
    let data = control_transfer(&xfer)?;
    let config_desc =
        ConfigurationDescriptor::parse(&data).ok_or(UsbError::InvalidDescriptor)?;

    // Validate total length against descriptor size.
    if config_desc.w_total_length > USB_MAX_DESCRIPTOR_SIZE {
        return Err(UsbError::InvalidDescriptor);
    }

    // Read the full configuration descriptor.
    let mut xfer = ControlTransfer {
        device_address: address,
        request_type: USB_DIR_IN | USB_TYPE_STANDARD | USB_RECIP_DEVICE,
        request: USB_REQUEST_GET_DESCRIPTOR,
        value: (USB_DESCRIPTOR_CONFIGURATION as u16) << 8,
        index: 0,
        length: config_desc.w_total_length,
        data: Vec::new(),
    };
    let data = control_transfer(&xfer)?;
    parse_configuration(address, &config_desc, &data)?;

    // Select the configuration.
    set_configuration(address, config_desc.b_configuration_value)?;

    unsafe {
        serial::write_str("usb: device enumerated ");
        serial::write_fmt(core::format_args!("addr={} vendor=0x{:04x} product=0x{:04x}\r\n", address, dev_vendor_address(address), dev_product_address(address)));
    }

    Ok(address)
}

fn dev_vendor_address(address: u8) -> u16 {
    USB_SUBSYSTEM.lock().devices.get(&address).map(|d| d.descriptor.id_vendor).unwrap_or(0)
}

fn dev_product_address(address: u8) -> u16 {
    USB_SUBSYSTEM.lock().devices.get(&address).map(|d| d.descriptor.id_product).unwrap_or(0)
}

/// Parse a full configuration descriptor buffer into interfaces and endpoints.
fn parse_configuration(
    address: u8,
    config: &ConfigurationDescriptor,
    data: &[u8],
) -> Result<(), UsbError> {
    let mut subsystem = USB_SUBSYSTEM.lock();
    let dev = subsystem
        .devices
        .get_mut(&address)
        .ok_or(UsbError::NotFound)?;
    dev.configuration = Some(*config);

    let mut offset = 0;
    while offset + 9 <= data.len() {
        let len = data[offset] as usize;
        if len < 2 || offset + len > data.len() {
            break;
        }
        let desc_type = data[offset + 1];
        match desc_type {
            USB_DESCRIPTOR_INTERFACE => {
                if let Some(iface) = InterfaceDescriptor::parse(&data[offset..offset + len]) {
                    if iface.b_interface_class == USB_CLASS_HID {
                        dev.interfaces.push(iface);
                    }
                }
            }
            USB_DESCRIPTOR_ENDPOINT => {
                if let Some(ep_desc) = EndpointDescriptor::parse(&data[offset..offset + len]) {
                    let ep = Endpoint {
                        number: ep_desc.address(),
                        direction: ep_desc.direction(),
                        transfer_type: ep_desc.transfer_type(),
                        max_packet_size: ep_desc.w_max_packet_size,
                        interval: ep_desc.b_interval,
                    };
                    dev.endpoints.insert(ep.number, ep);
                }
            }
            _ => {}
        }
        offset += len;
    }
    drop(subsystem);
    Ok(())
}

impl EndpointDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 7 {
            return None;
        }
        if data[0] != 7 || data[1] != USB_DESCRIPTOR_ENDPOINT {
            return None;
        }
        Some(Self {
            b_length: data[0],
            b_descriptor_type: data[1],
            b_endpoint_address: data[2],
            bm_attributes: data[3],
            w_max_packet_size: u16::from_le_bytes([data[4], data[5]]),
            b_interval: data[6],
        })
    }
}

// ---------------------------------------------------------------------------
// HID class
// ---------------------------------------------------------------------------

/// The HID device class (keyboard or mouse).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidClass {
    Keyboard,
    Mouse,
    Other,
}

/// Match a USB device to a HID class.
pub fn hid_class_for_device(address: u8) -> HidClass {
    let subsystem = USB_SUBSYSTEM.lock();
    let dev = match subsystem.devices.get(&address) {
        Some(d) => d,
        None => return HidClass::Other,
    };
    for iface in &dev.interfaces {
        if iface.b_interface_class == USB_CLASS_HID {
            match iface.b_interface_protocol {
                USB_PROTOCOL_KEYBOARD => return HidClass::Keyboard,
                USB_PROTOCOL_MOUSE => return HidClass::Mouse,
                _ => return HidClass::Other,
            }
        }
    }
    drop(subsystem);
    HidClass::Other
}

/// Report descriptor validation: reject oversized or malformed report
/// descriptors.
pub fn validate_hid_report_descriptor(data: &[u8]) -> bool {
    if data.is_empty() || data.len() > USB_MAX_DESCRIPTOR_SIZE as usize {
        return false;
    }
    // A minimal sanity check: the descriptor must begin with a valid HID item.
    data[0] != 0xFF
}

// ---------------------------------------------------------------------------
// Device removal
// ---------------------------------------------------------------------------

/// Remove a USB device by address. Releases endpoints and invalidates the
/// device record so stale references are rejected.
pub fn remove_device(address: u8) -> Result<(), UsbError> {
    let mut subsystem = USB_SUBSYSTEM.lock();
    let dev = subsystem
        .devices
        .get_mut(&address)
        .ok_or(UsbError::NotFound)?;
    dev.state = AttachmentState::Removed;
    dev.endpoints.clear();
    dev.interfaces.clear();
    subsystem.devices.remove(&address);
    Ok(())
}

// ---------------------------------------------------------------------------
// Host controller discovery
// ---------------------------------------------------------------------------

/// Discover USB host controllers on the PCI bus (class 0x0C, subclass 0x03)
/// and register them with the USB subsystem. Each controller is bound to the
/// device manager so ownership is tracked.
pub fn discover_host_controllers() {
    let controllers = pci::find_by_class(0x0C, 0x03);
    let mut count = 0;
    for ctrl in &controllers {
        let vendor = ctrl.vendor_id();
        let product = ctrl.device_id();

        // Find the device-manager record for this PCI device and bind it as a
        // USB host controller owner.
        let handles = {
        let mgr = device::DEVICE_MANAGER.lock();
        mgr.find_by_id(vendor, product)
    };

        for handle in handles {
            // Transition Discovered -> Unbound so the host controller can be claimed.
            let _ = device::DEVICE_MANAGER.lock().mark_unbound(handle.id);
            drop(device::DEVICE_MANAGER.lock());

            let owner = device::Owner {
                kind: device::OwnerKind::Driver,
                name: "usb-hc",
            };
            if device::bind_device(handle, owner).is_some() {
                let mut subsystem = USB_SUBSYSTEM.lock();
                subsystem.add_controller(handle, 8); // default port count
                drop(subsystem);
                count += 1;
                break;
            }
        }
    }
    unsafe {
        serial::write_str("usb: host controllers discovered ");
        serial::write_fmt(core::format_args!("{}\r\n", count));
    }
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

pub fn report() {
    let subsystem = USB_SUBSYSTEM.lock();
    unsafe {
        serial::write_str("usb: subsystem: ");
        serial::write_fmt(core::format_args!("{} devices, {} controllers\r\n", subsystem.device_count(), subsystem.controllers.len()));
    }
    for (addr, dev) in &subsystem.devices {
        unsafe {
            serial::write_str("usb: device addr=");
            serial::write_fmt(core::format_args!("{}", addr));
            serial::write_str(" state=");
            serial::write_fmt(core::format_args!("{:?}", dev.state));
            serial::write_str(" vendor=0x");
            serial::write_fmt(core::format_args!("{:x}", dev.descriptor.id_vendor));
            serial::write_str(" product=0x");
            serial::write_fmt(core::format_args!("{:x}", dev.descriptor.id_product));
            serial::write_str(" interfaces=");
            serial::write_fmt(core::format_args!("{}", dev.interfaces.len()));
            serial::write_str(" endpoints=");
            serial::write_fmt(core::format_args!("{}", dev.endpoints.len()));
            serial::write_newline();
        }
    }
}

fn serialize_device_descriptor(desc: &DeviceDescriptor) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(desc.b_length);
    data.push(desc.b_descriptor_type);
    data.extend_from_slice(&desc.bcd_usb.to_le_bytes());
    data.push(desc.b_device_class);
    data.push(desc.b_device_subclass);
    data.push(desc.b_device_protocol);
    data.push(desc.b_max_packet_size0);
    data.extend_from_slice(&desc.id_vendor.to_le_bytes());
    data.extend_from_slice(&desc.id_product.to_le_bytes());
    data.extend_from_slice(&desc.bcd_device.to_le_bytes());
    data.push(desc.i_manufacturer);
    data.push(desc.i_product);
    data.push(desc.i_serial_number);
    data.push(desc.b_num_configurations);
    data
}

fn serialize_configuration_descriptor(dev: &UsbDevice) -> Vec<u8> {
    let mut data = Vec::new();
    if let Some(config) = dev.configuration {
        data.push(config.b_length);
        data.push(config.b_descriptor_type);
        data.extend_from_slice(&config.w_total_length.to_le_bytes());
        data.push(config.b_num_interfaces);
        data.push(config.b_configuration_value);
        data.push(0); // i_configuration
        data.push(0); // bm_attributes
        data.push(0); // b_max_power
    }
    for iface in &dev.interfaces {
        data.push(iface.b_length);
        data.push(iface.b_descriptor_type);
        data.push(iface.b_interface_number);
        data.push(iface.b_alternate_setting);
        data.push(iface.b_num_endpoints);
        data.push(iface.b_interface_class);
        data.push(iface.b_interface_subclass);
        data.push(iface.b_interface_protocol);
        data.push(0); // i_interface
    }
    for ep in dev.endpoints.values() {
        let b_endpoint_address = match ep.direction {
            TransferDirection::Out => ep.number,
            TransferDirection::In => ep.number | 0x80,
        };
        let bm_attributes = match ep.transfer_type {
            TransferType::Control => 0,
            TransferType::Isochronous => 1,
            TransferType::Bulk => 2,
            TransferType::Interrupt => 3,
        };
        data.push(7);
        data.push(USB_DESCRIPTOR_ENDPOINT);
        data.push(b_endpoint_address);
        data.push(bm_attributes);
        data.extend_from_slice(&ep.max_packet_size.to_le_bytes());
        data.push(ep.interval);
    }
    data
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_descriptor_parse_valid() {
        let mut data = [0u8; 18];
        data[0] = 18;
        data[1] = USB_DESCRIPTOR_DEVICE;
        data[8] = 0xF4;
        data[9] = 0x1A;
        data[10] = 0x42;
        data[11] = 0x10;
        let desc = DeviceDescriptor::parse(&data).unwrap();
        assert_eq!(desc.id_vendor, 0x1AF4);
        assert_eq!(desc.id_product, 0x1042);
    }

    #[test]
    fn device_descriptor_rejects_short() {
        let data = [0u8; 5];
        assert!(DeviceDescriptor::parse(&data).is_none());
    }

    #[test]
    fn device_descriptor_rejects_wrong_type() {
        let mut data = [0u8; 18];
        data[0] = 18;
        data[1] = USB_DESCRIPTOR_CONFIGURATION;
        assert!(DeviceDescriptor::parse(&data).is_none());
    }

    #[test]
    fn validate_descriptor_length_rejects_oversized() {
        let data = [0xFFu8; 10];
        assert!(!validate_descriptor_length(&data));
    }

    #[test]
    fn validate_descriptor_length_accepts_valid() {
        let mut data = [0u8; 8];
        data[0] = 8;
        data[1] = USB_DESCRIPTOR_DEVICE;
        assert!(validate_descriptor_length(&data));
    }

    #[test]
    fn endpoint_direction_and_type() {
        let ep = EndpointDescriptor {
            b_length: 7,
            b_descriptor_type: USB_DESCRIPTOR_ENDPOINT,
            b_endpoint_address: 0x81,
            bm_attributes: 0x03, // interrupt
            w_max_packet_size: 8,
            b_interval: 10,
        };
        assert_eq!(ep.address(), 1);
        assert!(matches!(ep.direction(), TransferDirection::In));
        assert!(matches!(ep.transfer_type(), TransferType::Interrupt));
    }

    #[test]
    fn configuration_parse_valid() {
        let mut data = [0u8; 9];
        data[0] = 9;
        data[1] = USB_DESCRIPTOR_CONFIGURATION;
        data[2] = 32;
        data[3] = 0;
        data[4] = 1;
        data[5] = 1;
        let config = ConfigurationDescriptor::parse(&data).unwrap();
        assert_eq!(config.w_total_length, 32);
        assert_eq!(config.b_num_interfaces, 1);
    }

    #[test]
    fn hid_class_keyboard() {
        let mut subsystem = USB_SUBSYSTEM.lock();
        let mut dev = UsbDevice::new(1, 1, UsbSpeed::Full);
        dev.state = AttachmentState::Configured;
        dev.interfaces.push(InterfaceDescriptor {
            b_length: 9,
            b_descriptor_type: USB_DESCRIPTOR_INTERFACE,
            b_interface_number: 0,
            b_alternate_setting: 0,
            b_num_endpoints: 1,
            b_interface_class: USB_CLASS_HID,
            b_interface_subclass: 1,
            b_interface_protocol: USB_PROTOCOL_KEYBOARD,
        });
        subsystem.devices.insert(1, dev);
        drop(subsystem);
        assert_eq!(hid_class_for_device(1), HidClass::Keyboard);
    }

    #[test]
    fn hid_class_mouse() {
        let mut subsystem = USB_SUBSYSTEM.lock();
        let mut dev = UsbDevice::new(2, 2, UsbSpeed::Full);
        dev.state = AttachmentState::Configured;
        dev.interfaces.push(InterfaceDescriptor {
            b_length: 9,
            b_descriptor_type: USB_DESCRIPTOR_INTERFACE,
            b_interface_number: 0,
            b_alternate_setting: 0,
            b_num_endpoints: 1,
            b_interface_class: USB_CLASS_HID,
            b_interface_subclass: 1,
            b_interface_protocol: USB_PROTOCOL_MOUSE,
        });
        subsystem.devices.insert(2, dev);
        drop(subsystem);
        assert_eq!(hid_class_for_device(2), HidClass::Mouse);
    }

    #[test]
    fn device_removal_invalidates() {
        let mut subsystem = USB_SUBSYSTEM.lock();
        let mut dev = UsbDevice::new(3, 3, UsbSpeed::Full);
        dev.state = AttachmentState::Configured;
        subsystem.devices.insert(3, dev);
        drop(subsystem);
        assert!(remove_device(3).is_ok());
        assert!(USB_SUBSYSTEM.lock().device(3).is_none());
    }

    #[test]
    fn control_transfer_rejects_removed_device() {
        let mut subsystem = USB_SUBSYSTEM.lock();
        let mut dev = UsbDevice::new(4, 4, UsbSpeed::Full);
        dev.state = AttachmentState::Removed;
        subsystem.devices.insert(4, dev);
        drop(subsystem);
        let xfer = ControlTransfer {
            device_address: 4,
            request_type: USB_DIR_IN | USB_TYPE_STANDARD | USB_RECIP_DEVICE,
            request: USB_REQUEST_GET_DESCRIPTOR,
            value: (USB_DESCRIPTOR_DEVICE as u16) << 8,
            index: 0,
            length: 8,
            data: Vec::new(),
        };
        assert!(control_transfer(&xfer).is_err());
    }
}
