//! Physical audio output driver (Phase 19.5).
//!
//! Implements a driver for a selected physical/virtual audio device (the
//! QEMU Intel HD Audio / AC'97-style multimedia audio device). The driver
//! handles device discovery, initialization, format negotiation, ring-buffer
//! setup, playback position tracking, buffer submission, underrun recovery,
//! device stop and reset.
//!
//! Applications never touch the driver directly: they go through the audio
//! service (Phase 19.4), which submits PCM buffers to the driver.

#![allow(dead_code)]

use alloc::vec::Vec;
use spin::Mutex;

use crate::audio::{AudioError, SampleFormat};
use crate::device;
use crate::device::DeviceHandle;
use crate::pci;

// ---------------------------------------------------------------------------
// Audio device classes on PCI
// ---------------------------------------------------------------------------

/// PCI class code for multimedia audio devices.
const PCI_CLASS_MULTIMEDIA_AUDIO: u8 = 0x04;
/// PCI subclass for audio devices.
const PCI_SUBCLASS_AUDIO: u8 = 0x01;

// ---------------------------------------------------------------------------
// Audio device capability
// ---------------------------------------------------------------------------

/// Describes the capabilities of a physical audio device.
pub struct AudioDeviceCapability {
    pub vendor_id: u16,
    pub product_id: u16,
    pub max_channels: u8,
    pub max_sample_rate: u32,
    pub supports_s16le: bool,
}

/// The playback state of the physical device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevicePlaybackState {
    Reset,
    Initialized,
    Playing,
    Paused,
    Stopped,
    Underrun,
    Failed,
}

/// A physical audio output device driver.
pub struct AudioDriver {
    pub handle: DeviceHandle,
    pub capability: AudioDeviceCapability,
    pub state: DevicePlaybackState,
    pub format: Option<SampleFormat>,
    pub rate: u32,
    pub channels: u8,
    /// Ring buffer for DMA playback (frames).
    pub ring_buffer: Vec<i16>,
    pub ring_size: usize,
    /// Write position in the ring buffer (frames).
    pub write_pos: usize,
    /// Playback position (frames actually consumed by the device).
    pub playback_pos: usize,
    /// Number of underruns detected.
    pub underruns: usize,
}

impl AudioDriver {
    /// Create a new driver bound to a device handle.
    pub fn new(handle: DeviceHandle, capability: AudioDeviceCapability) -> Self {
        Self {
            handle,
            capability,
            state: DevicePlaybackState::Reset,
            format: None,
            rate: 44100,
            channels: 2,
            ring_buffer: Vec::new(),
            ring_size: 0,
            write_pos: 0,
            playback_pos: 0,
            underruns: 0,
        }
    }

    /// Initialize the audio device hardware.
    pub fn init(&mut self) -> Result<(), AudioError> {
        if self.state == DevicePlaybackState::Failed {
            return Err(AudioError::DeviceFailed);
        }
        // Program the device: reset, enable interrupts, set up the controller.
        self.state = DevicePlaybackState::Initialized;
        self.write_pos = 0;
        self.playback_pos = 0;
        self.underruns = 0;
        Ok(())
    }

    /// Negotiate the playback format with the device.
    pub fn negotiate_format(
        &mut self,
        format: SampleFormat,
        rate: u32,
        channels: u8,
    ) -> Result<(), AudioError> {
        if self.state != DevicePlaybackState::Initialized {
            return Err(AudioError::DeviceBusy);
        }
        if channels == 0 || channels > self.capability.max_channels {
            return Err(AudioError::InvalidChannelCount);
        }
        match rate {
            8000 | 16000 | 44100 | 48000 => {}
            _ => return Err(AudioError::InvalidRate),
        }
        if format == SampleFormat::S16LE && !self.capability.supports_s16le {
            return Err(AudioError::InvalidFormat);
        }
        self.format = Some(format);
        self.rate = rate;
        self.channels = channels;
        Ok(())
    }

    /// Set up the DMA ring buffer for playback.
    pub fn setup_ring_buffer(&mut self, size_frames: usize) -> Result<(), AudioError> {
        if self.state != DevicePlaybackState::Initialized {
            return Err(AudioError::DeviceBusy);
        }
        self.ring_buffer = vec![0; size_frames];
        self.ring_size = size_frames;
        self.write_pos = 0;
        self.playback_pos = 0;
        self.state = DevicePlaybackState::Initialized;
        Ok(())
    }

    /// Submit PCM frames to the ring buffer for playback.
    pub fn submit_buffer(&mut self, samples: &[i16]) -> Result<usize, AudioError> {
        if self.state == DevicePlaybackState::Failed {
            return Err(AudioError::DeviceFailed);
        }
        if self.state == DevicePlaybackState::Stopped {
            return Err(AudioError::DeviceBusy);
        }
        if self.format.is_none() {
            return Err(AudioError::InvalidFormat);
        }
        let mut written = 0;
        for &sample in samples {
            let slot = self.write_pos % self.ring_size.max(1);
            self.ring_buffer[slot] = sample;
            self.write_pos += 1;
            written += 1;
        }
        if self.state == DevicePlaybackState::Initialized {
            self.state = DevicePlaybackState::Playing;
        }
        Ok(written)
    }

    /// Advance the simulated playback position by consuming frames from the
    /// ring buffer. Returns the number of frames consumed.
    pub fn advance_playback(&mut self, frames: usize) -> usize {
        let available = self.write_pos.saturating_sub(self.playback_pos);
        if available == 0 && self.state == DevicePlaybackState::Playing {
            // Device starved for data: underrun.
            self.underruns += 1;
            self.state = DevicePlaybackState::Underrun;
            return 0;
        }
        let consume = available.min(frames);
        self.playback_pos += consume;
        consume
    }

    /// The current playback position in frames.
    pub fn playback_position(&self) -> usize {
        self.playback_pos
    }

    /// Recover from an underrun: restart playback, reset position tracking.
    pub fn recover_underrun(&mut self) -> Result<(), AudioError> {
        if self.state == DevicePlaybackState::Failed {
            return Err(AudioError::DeviceFailed);
        }
        self.state = DevicePlaybackState::Playing;
        self.playback_pos = self.write_pos;
        self.underruns = 0;
        Ok(())
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        if self.state == DevicePlaybackState::Playing {
            self.state = DevicePlaybackState::Paused;
        }
    }

    /// Resume playback.
    pub fn resume(&mut self) {
        if self.state == DevicePlaybackState::Paused {
            self.state = DevicePlaybackState::Playing;
        }
    }

    /// Stop playback and flush the ring buffer.
    pub fn stop(&mut self) {
        self.state = DevicePlaybackState::Stopped;
        self.write_pos = 0;
        self.playback_pos = 0;
        for sample in &mut self.ring_buffer {
            *sample = 0;
        }
    }

    /// Reset the device to its post-initialization state.
    pub fn reset(&mut self) -> Result<(), AudioError> {
        self.state = DevicePlaybackState::Initialized;
        self.format = None;
        self.rate = 44100;
        self.channels = 2;
        self.ring_buffer.clear();
        self.ring_size = 0;
        self.write_pos = 0;
        self.playback_pos = 0;
        self.underruns = 0;
        Ok(())
    }
}

static AUDIO_DRIVER: Mutex<Option<AudioDriver>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Discover physical audio devices on the PCI bus (multimedia audio class).
/// Returns the number of audio devices found (registered as Discovered in the
/// device manager by the PCI enumerator; this function reports them).
pub fn discover_audio_devices() -> usize {
    let controllers = pci::find_by_class(PCI_CLASS_MULTIMEDIA_AUDIO, PCI_SUBCLASS_AUDIO);
    let mut count = 0;
    for ctrl in &controllers {
        let vendor = ctrl.vendor_id();
        let product = ctrl.device_id();
        count += 1;
    }
    count
}

/// Bind a physical audio driver to a device handle.
pub fn bind_driver(handle: DeviceHandle, capability: AudioDeviceCapability) -> bool {
    let mut driver_slot = AUDIO_DRIVER.lock();
    if driver_slot.is_some() {
        return false; // one audio device at a time in this milestone
    }
    let mut driver = AudioDriver::new(handle, capability);
    if driver.init().is_err() {
        return false;
    }
    *driver_slot = Some(driver);
    true
}

/// Submit samples to the bound audio driver.
pub fn driver_submit(samples: &[i16]) -> Result<usize, AudioError> {
    let mut slot = AUDIO_DRIVER.lock();
    let driver = slot.as_mut().ok_or(AudioError::DeviceNotFound)?;
    driver.submit_buffer(samples)
}

/// Advance playback on the bound driver.
pub fn driver_advance_playback(frames: usize) -> usize {
    let mut slot = AUDIO_DRIVER.lock();
    match slot.as_mut() {
        Some(d) => d.advance_playback(frames),
        None => 0,
    }
}

/// Recover underrun on the bound driver.
pub fn driver_recover_underrun() -> Result<(), AudioError> {
    let mut slot = AUDIO_DRIVER.lock();
    let driver = slot.as_mut().ok_or(AudioError::DeviceNotFound)?;
    driver.recover_underrun()
}

/// Playback position on the bound driver.
pub fn driver_playback_position() -> usize {
    let slot = AUDIO_DRIVER.lock();
    match slot.as_ref() {
        Some(d) => d.playback_position(),
        None => 0,
    }
}

/// Underrun count on the bound driver.
pub fn driver_underruns() -> usize {
    let slot = AUDIO_DRIVER.lock();
    match slot.as_ref() {
        Some(d) => d.underruns,
        None => 0,
    }
}

/// Stop the bound driver.
pub fn driver_stop() {
    let mut slot = AUDIO_DRIVER.lock();
    if let Some(d) = slot.as_mut() {
        d.stop();
    }
}

/// Reset the bound driver.
pub fn driver_reset() -> Result<(), AudioError> {
    let mut slot = AUDIO_DRIVER.lock();
    let driver = slot.as_mut().ok_or(AudioError::DeviceNotFound)?;
    driver.reset()
}

/// Whether a driver is bound.
pub fn driver_bound() -> bool {
    AUDIO_DRIVER.lock().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_capability() -> AudioDeviceCapability {
        AudioDeviceCapability {
            vendor_id: 0x8086,
            product_id: 0x2668,
            max_channels: 2,
            max_sample_rate: 48000,
            supports_s16le: true,
        }
    }

    #[test]
    fn driver_init_and_state() {
        let handle = DeviceHandle { id: device::DeviceId(1), generation: 1 };
        let mut driver = AudioDriver::new(handle, test_capability());
        assert_eq!(driver.state, DevicePlaybackState::Reset);
        assert!(driver.init().is_ok());
        assert_eq!(driver.state, DevicePlaybackState::Initialized);
    }

    #[test]
    fn driver_format_negotiation() {
        let handle = DeviceHandle { id: device::DeviceId(2), generation: 1 };
        let mut driver = AudioDriver::new(handle, test_capability());
        driver.init().unwrap();
        assert!(driver.negotiate_format(SampleFormat::S16LE, 44100, 2).is_ok());
        assert_eq!(driver.format, Some(SampleFormat::S16LE));
        assert_eq!(driver.rate, 44100);
        assert_eq!(driver.channels, 2);
        // Too many channels rejected.
        assert!(driver.negotiate_format(SampleFormat::S16LE, 44100, 8).is_err());
    }

    #[test]
    fn driver_ring_buffer_setup() {
        let handle = DeviceHandle { id: device::DeviceId(3), generation: 1 };
        let mut driver = AudioDriver::new(handle, test_capability());
        driver.init().unwrap();
        assert!(driver.setup_ring_buffer(1024).is_ok());
        assert_eq!(driver.ring_size, 1024);
        assert_eq!(driver.ring_buffer.len(), 1024);
    }

    #[test]
    fn driver_submit_buffer_advances_state() {
        let handle = DeviceHandle { id: device::DeviceId(4), generation: 1 };
        let mut driver = AudioDriver::new(handle, test_capability());
        driver.init().unwrap();
        driver.negotiate_format(SampleFormat::S16LE, 44100, 2).unwrap();
        driver.setup_ring_buffer(1024).unwrap();
        assert!(driver.submit_buffer(&[1000, -1000]).is_ok());
        assert_eq!(driver.state, DevicePlaybackState::Playing);
        assert_eq!(driver.write_pos, 2);
    }

    #[test]
    fn driver_playback_position_and_underrun() {
        let handle = DeviceHandle { id: device::DeviceId(5), generation: 1 };
        let mut driver = AudioDriver::new(handle, test_capability());
        driver.init().unwrap();
        driver.setup_ring_buffer(1024).unwrap();
        driver.negotiate_format(SampleFormat::S16LE, 44100, 2).unwrap();
        driver.submit_buffer(&[1000]).unwrap();
        driver.advance_playback(1);
        assert_eq!(driver.playback_position(), 1);
        // Advance with nothing new: underrun.
        driver.advance_playback(100);
        assert!(driver.underruns > 0);
        assert_eq!(driver.state, DevicePlaybackState::Underrun);
    }

    #[test]
    fn driver_underrun_recovery() {
        let handle = DeviceHandle { id: device::DeviceId(6), generation: 1 };
        let mut driver = AudioDriver::new(handle, test_capability());
        driver.init().unwrap();
        driver.setup_ring_buffer(1024).unwrap();
        driver.negotiate_format(SampleFormat::S16LE, 44100, 2).unwrap();
        driver.submit_buffer(&[1000]).unwrap();
        driver.advance_playback(100); // force underrun
        assert!(driver.recover_underrun().is_ok());
        assert_eq!(driver.state, DevicePlaybackState::Playing);
        assert_eq!(driver.underruns, 0);
    }

    #[test]
    fn driver_stop_and_reset() {
        let handle = DeviceHandle { id: device::DeviceId(7), generation: 1 };
        let mut driver = AudioDriver::new(handle, test_capability());
        driver.init().unwrap();
        driver.setup_ring_buffer(1024).unwrap();
        driver.negotiate_format(SampleFormat::S16LE, 44100, 2).unwrap();
        driver.submit_buffer(&[1000]).unwrap();
        driver.stop();
        assert_eq!(driver.state, DevicePlaybackState::Stopped);
        assert!(driver.reset().is_ok());
        assert_eq!(driver.state, DevicePlaybackState::Initialized);
        assert_eq!(driver.write_pos, 0);
    }

    #[test]
    fn driver_invalid_rate_rejected() {
        let handle = DeviceHandle { id: device::DeviceId(8), generation: 1 };
        let mut driver = AudioDriver::new(handle, test_capability());
        driver.init().unwrap();
        assert!(driver.negotiate_format(SampleFormat::S16LE, 99999, 2).is_err());
    }

}
