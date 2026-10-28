//! Security logging.

use crate::serial;

#[derive(Debug, Clone)]
pub struct SecurityEvent<'a> {
    pub event_type: &'a str,
    pub pid: Option<u64>,
    pub uid: Option<u64>,
    pub resource_type: Option<&'a str>,
    pub operation: &'a str,
    pub result: &'a str,
}

/// Log a security event (safe information only, no secrets)
pub fn log_event(event: SecurityEvent) {
    unsafe {
        serial::write_str("Elvaris kernel: security: event=");
        serial::write_str(event.event_type);
        serial::write_str(" op=");
        serial::write_str(event.operation);
        serial::write_str(" result=");
        serial::write_str(event.result);
        if let Some(pid) = event.pid {
            serial::write_str(" pid=");
            serial::write_fmt(core::format_args!("{}", pid));
        }
        if let Some(uid) = event.uid {
            serial::write_str(" uid=");
            serial::write_fmt(core::format_args!("{}", uid));
        }
        if let Some(rt) = event.resource_type {
            serial::write_str(" resource=");
            serial::write_str(rt);
        }
        serial::write_newline();
    }
}
