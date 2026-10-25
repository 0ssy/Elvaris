//! Network permissions and service boundaries.

use crate::security::Credentials;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetPerm {
    BindPrivilegedPort,
    ConfigureInterface,
    ManageRoutes,
    PacketCapture,
    StartServer,
}

pub fn check_net_perm(creds: &Credentials, perm: NetPerm) -> bool {
    if creds.is_privileged() {
        return true;
    }
    match perm {
        NetPerm::BindPrivilegedPort => false,
        NetPerm::ConfigureInterface => false,
        NetPerm::ManageRoutes => false,
        NetPerm::PacketCapture => false,
        NetPerm::StartServer => false,
    }
}

pub const PRIVILEGED_PORT_MAX: u16 = 1024;
pub fn is_privileged_port(port: u16) -> bool {
    port < PRIVILEGED_PORT_MAX
}
