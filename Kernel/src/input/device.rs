//! Physical input device driver and input service (Phase 19.2).
//!
//! An `InputDevice` owns a device handle, decodes raw hardware bytes into
//! `HardwareInputEvent`s, and pushes them into a bounded queue. The input
//! service owns all input devices and routes events to the active terminal
//! or graphical session.
//!
//! Terminal and graphical systems never read hardware registers directly;
//! they consume events from the input service queue.

#![allow(dead_code)]

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use spin::Mutex;

use crate::device;
use crate::device::DeviceHandle;
use crate::input::hardware::{decode_keyboard_byte, decode_mouse_packet, HardwareInputEvent, ModifierState};
use crate::serial;

/// Maximum number of hardware input events the service queue holds.
const SERVICE_QUEUE_CAPACITY: usize = 256;

/// Maximum number of events per device queue.
const DEVICE_QUEUE_CAPACITY: usize = 64;

/// A bounded queue of hardware input events for a single device.
struct DeviceQueue {
    queue: VecDeque<HardwareInputEvent>,
    max_size: usize,
    overflows: usize,
}

impl DeviceQueue {
    fn new(max_size: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(max_size),
            max_size,
            overflows: 0,
        }
    }

    fn push(&mut self, event: HardwareInputEvent) {
        if self.queue.len() >= self.max_size {
            self.overflows += 1;
            return;
        }
        self.queue.push_back(event);
    }

    fn pop(&mut self) -> Option<HardwareInputEvent> {
        self.queue.pop_front()
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// An input device driver: owns a device handle and decodes raw bytes.
pub struct InputDevice {
    /// The device handle this driver owns. Stale handles are rejected.
    handle: DeviceHandle,
    /// The device class: keyboard or mouse.
    kind: InputKind,
    /// Decoded events awaiting dispatch.
    queue: DeviceQueue,
    /// Previous mouse button state, for edge detection.
    prev_buttons: u8,
    /// Current keyboard modifier state.
    modifiers: ModifierState,
    /// Whether the device is enabled.
    enabled: bool,
}

/// The kind of an input device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    Keyboard,
    Mouse,
}

impl InputDevice {
    /// Create a new input device bound to a device handle.
    pub fn new(handle: DeviceHandle, kind: InputKind) -> Self {
        Self {
            handle,
            kind,
            queue: DeviceQueue::new(DEVICE_QUEUE_CAPACITY),
            prev_buttons: 0,
            modifiers: ModifierState::default(),
            enabled: true,
        }
    }

    /// The device handle owned by this driver.
    pub fn handle(&self) -> DeviceHandle {
        self.handle
    }

    /// Whether the device is a keyboard.
    pub fn is_keyboard(&self) -> bool {
        self.kind == InputKind::Keyboard
    }

    /// Whether the device is a mouse.
    pub fn is_mouse(&self) -> bool {
        self.kind == InputKind::Mouse
    }

    /// Feed a raw byte from a PS/2-style keyboard into the device.
    pub fn feed_keyboard_byte(&mut self, byte: u8) {
        if !self.enabled || self.kind != InputKind::Keyboard {
            return;
        }
        if let Some(event) = decode_keyboard_byte(byte, &mut self.modifiers) {
            self.queue.push(event);
        }
    }

    /// Feed a mouse packet into the device.
    pub fn feed_mouse_packet(&mut self, dx: i32, dy: i32, buttons: u8) {
        if !self.enabled || self.kind != InputKind::Mouse {
            return;
        }
        for event in decode_mouse_packet(dx, dy, buttons, self.prev_buttons) {
            self.queue.push(event);
        }
        self.prev_buttons = buttons;
    }

    /// Pop the next decoded event, if any. Returns `None` when suspended.
    pub fn pop_event(&mut self) -> Option<HardwareInputEvent> {
        if !self.enabled {
            return None;
        }
        self.queue.pop()
    }

    /// Disable the device (e.g. on suspension or removal).
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Enable the device.
    pub fn enable(&mut self) {
        self.enabled = true;
    }
}

/// The input service: owns all input devices and dispatches events.
pub struct InputService {
    devices: Vec<InputDevice>,
    /// Global dispatch queue shared by all devices.
    dispatch: VecDeque<HardwareInputEvent>,
    max_dispatch: usize,
}

impl InputService {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            dispatch: VecDeque::with_capacity(SERVICE_QUEUE_CAPACITY),
            max_dispatch: SERVICE_QUEUE_CAPACITY,
        }
    }

    /// Register an input device owned by this service.
    pub fn register_device(&mut self, device: InputDevice) {
        self.devices.push(device);
    }

    /// Poll every enabled device and dispatch its decoded events.
    pub fn poll(&mut self) {
        for device in &mut self.devices {
            while let Some(event) = device.pop_event() {
                if self.dispatch.len() >= self.max_dispatch {
                    break;
                }
                self.dispatch.push_back(event);
            }
        }
    }

    /// Pop the next dispatched event for the active session/terminal.
    pub fn pop_dispatched(&mut self) -> Option<HardwareInputEvent> {
        self.dispatch.pop_front()
    }

    /// The number of pending dispatched events.
    pub fn pending(&self) -> usize {
        self.dispatch.len()
    }

    /// Suspend all input devices.
    pub fn suspend_all(&mut self) {
        for device in &mut self.devices {
            device.disable();
        }
    }

    /// Resume all input devices.
    pub fn resume_all(&mut self) {
        for device in &mut self.devices {
            device.enable();
        }
    }

    /// Device count for diagnostics.
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

static INPUT_SERVICE: Mutex<InputService> = Mutex::new(InputService {
    devices: Vec::new(),
    dispatch: VecDeque::new(),
    max_dispatch: SERVICE_QUEUE_CAPACITY,
});

/// Register a discovered input device with the device manager and the input
/// service. Returns the device handle on success.
pub fn register_input_device(
    class: device::DeviceClass,
    bus: device::BusType,
    vendor_id: u16,
    product_id: u16,
    resources: Vec<device::Resource>,
    kind: InputKind,
) -> Option<DeviceHandle> {
    let mut mgr = device::DEVICE_MANAGER.lock();
    let id = mgr.register(class, bus, vendor_id, product_id, resources, None);
    mgr.mark_unbound(id).ok()?;
    let handle = DeviceHandle { id, generation: 1 };
    let owner = device::Owner {
        kind: device::OwnerKind::Driver,
        name: "input",
    };
    let bound = mgr.bind(handle, owner).ok();
    drop(mgr);

    let _ = bound?;
    let mut service = INPUT_SERVICE.lock();
    let mut device = InputDevice::new(handle, kind);
    device.enable();
    service.register_device(device);

    unsafe {
        serial::write_str("input: device ready ");
        serial::write_fmt(core::format_args!("id={} kind={:?}\r\n", id.0, kind));
    }
    Some(handle)
}

/// Poll the input service for new events.
pub fn poll_input() {
    INPUT_SERVICE.lock().poll();
}

/// Pop a dispatched hardware input event, if any.
pub fn pop_input_event() -> Option<HardwareInputEvent> {
    INPUT_SERVICE.lock().pop_dispatched()
}

/// Suspend all input devices.
pub fn suspend_input() {
    INPUT_SERVICE.lock().suspend_all();
}

/// Resume all input devices.
pub fn resume_input() {
    INPUT_SERVICE.lock().resume_all();
}

/// Initialize the input subsystem.
pub fn init() {
    unsafe {
        serial::write_str("input: subsystem initialized\r\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_device_keyboard_feed_and_pop() {
        let handle = DeviceHandle {
            id: device::DeviceId(1),
            generation: 1,
        };
        let mut device = InputDevice::new(handle, InputKind::Keyboard);
        device.feed_keyboard_byte(0x1C);
        let ev = device.pop_event().unwrap();
        assert!(matches!(ev, HardwareInputEvent::Key(k) if k.scan_code == 0x1C));
    }

    #[test]
    fn input_device_mouse_packet() {
        let handle = DeviceHandle {
            id: device::DeviceId(2),
            generation: 1,
        };
        let mut device = InputDevice::new(handle, InputKind::Mouse);
        device.feed_mouse_packet(10, -5, 0b001);
        let mut found_pointer = false;
        let mut found_button = false;
        while let Some(ev) = device.pop_event() {
            match ev {
                HardwareInputEvent::Pointer(p) => {
                    assert_eq!(p.dx, 10);
                    assert_eq!(p.dy, -5);
                    found_pointer = true;
                }
                HardwareInputEvent::Button(b) => {
                    assert_eq!(b.button, 0);
                    assert!(b.pressed);
                    found_button = true;
                }
                _ => {}
            }
        }
        assert!(found_pointer && found_button);
    }

    #[test]
    fn disabled_device_produces_no_events() {
        let handle = DeviceHandle {
            id: device::DeviceId(3),
            generation: 1,
        };
        let mut device = InputDevice::new(handle, InputKind::Keyboard);
        device.disable();
        device.feed_keyboard_byte(0x1C);
        assert!(device.pop_event().is_none());
    }

    #[test]
    fn service_poll_dispatches_events() {
        let mut service = InputService::new();
        let handle = DeviceHandle {
            id: device::DeviceId(4),
            generation: 1,
        };
        let mut device = InputDevice::new(handle, InputKind::Keyboard);
        device.feed_keyboard_byte(0x1C);
        service.register_device(device);
        service.poll();
        assert_eq!(service.pending(), 1);
        let ev = service.pop_dispatched().unwrap();
        assert!(matches!(ev, HardwareInputEvent::Key(_)));
    }

    #[test]
    fn suspend_and_resume_all() {
        let mut service = InputService::new();
        let handle = DeviceHandle {
            id: device::DeviceId(5),
            generation: 1,
        };
        let mut device = InputDevice::new(handle, InputKind::Keyboard);
        device.feed_keyboard_byte(0x1C);
        service.register_device(device);
        service.suspend_all();
        service.poll();
        assert_eq!(service.pending(), 0);
        // Resume: the previously queued event must flow again.
        service.resume_all();
        service.poll();
        assert_eq!(service.pending(), 1);
        let ev = service.pop_dispatched().unwrap();
        assert!(matches!(ev, HardwareInputEvent::Key(_)));
        // A second device confirms events keep flowing after resume.
        let handle2 = DeviceHandle {
            id: device::DeviceId(6),
            generation: 1,
        };
        let mut device2 = InputDevice::new(handle2, InputKind::Keyboard);
        device2.feed_keyboard_byte(0x1C);
        service.register_device(device2);
        service.poll();
        assert_eq!(service.pending(), 1);
    }
}
