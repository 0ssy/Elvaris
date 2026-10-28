//! Echo server (Milestone 15).

pub struct EchoServer {
    running: bool,
    port: u16,
}

impl EchoServer {
    pub fn new(port: u16) -> Self {
        EchoServer {
            running: false,
            port,
        }
    }

    pub fn start(&mut self) {
        self.running = true;
        unsafe {
            crate::serial::write_str("Elvaris kernel: network: service started (echo)\r\n");
        }
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Default for EchoServer {
    fn default() -> Self {
        Self::new(7)
    }
}
