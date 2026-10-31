//! Audio abstraction and virtual audio (Phases 19.4 and 19.5).
//!
//! Defines the audio architecture: application -> audio API/service -> audio
//! stream -> mixer -> audio device driver -> hardware. Mixing happens in a
//! user-space audio service; applications do not access device registers or
//! DMA buffers directly.
//!
//! This module provides the audio core (formats, streams, mixer, virtual
//! backend) plus the device-level audio abstraction. A physical audio driver
//! is added in Phase 19.5.

#![allow(dead_code)]

use alloc::vec::Vec;
use spin::Mutex;

use crate::serial;

// ---------------------------------------------------------------------------
// Audio core concepts
// ---------------------------------------------------------------------------

/// Audio sample format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    /// Signed 16-bit little-endian PCM.
    S16LE,
    /// Signed 8-bit PCM.
    S8,
}

/// Direction of an audio stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDirection {
    Playback,
    /// Recording is deferred to a later phase.
    Capture,
}

/// Playback state of a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Idle,
    Running,
    Paused,
    Stopped,
    Underrun,
    Failed,
}

/// An audio buffer: bounded, holding PCM samples.
pub struct AudioBuffer {
    /// The sample data.
    pub samples: Vec<i16>,
    /// The write position (frames).
    pub write_pos: usize,
    /// The read position (frames).
    pub read_pos: usize,
    /// The capacity in frames.
    capacity: usize,
}

impl AudioBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            write_pos: 0,
            read_pos: 0,
            capacity,
        }
    }

    /// Whether the buffer is full.
    pub fn is_full(&self) -> bool {
        self.write_pos - self.read_pos >= self.capacity
    }

    /// Whether the buffer is empty (underrun condition for playback).
    pub fn is_empty(&self) -> bool {
        self.write_pos == self.read_pos
    }

    /// The number of frames available to read.
    pub fn available_read(&self) -> usize {
        self.write_pos - self.read_pos
    }

    /// The number of frames available to write.
    pub fn available_write(&self) -> usize {
        self.capacity - (self.write_pos - self.read_pos)
    }

    /// Push a frame into the buffer. Returns false if the buffer is full.
    pub fn push(&mut self, sample: i16) -> bool {
        if self.is_full() {
            return false;
        }
        self.samples.push(sample);
        self.write_pos += 1;
        true
    }

    /// Pop a frame from the buffer. Returns None if empty (underrun).
    pub fn pop(&mut self) -> Option<i16> {
        if self.is_empty() {
            return None;
        }
        let sample = self.samples.remove(0);
        self.read_pos += 1;
        Some(sample)
    }

    /// Reset the buffer positions without clearing storage.
    pub fn reset(&mut self) {
        self.samples.clear();
        self.write_pos = 0;
        self.read_pos = 0;
    }
}

/// Errors from audio operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioError {
    InvalidFormat,
    InvalidRate,
    InvalidChannelCount,
    BufferFull,
    BufferEmpty,
    DeviceBusy,
    DeviceNotFound,
    DeviceFailed,
    PermissionDenied,
    StreamClosed,
}

// ---------------------------------------------------------------------------
// Audio device identity
// ---------------------------------------------------------------------------

/// An audio device (virtual or physical).
pub struct AudioDevice {
    pub id: DeviceId,
    pub name: &'static str,
    pub supported_formats: Vec<SampleFormat>,
    pub supported_rates: Vec<u32>,
    pub supported_channels: Vec<u8>,
    pub state: AudioDeviceState,
    pub generation: u64,
}

/// The state of an audio device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioDeviceState {
    Discovered,
    Unbound,
    Binding,
    Active,
    Suspended,
    Failed,
}

/// Identifier for an audio device record (distinct from the generic device
/// manager id, used internally by the audio subsystem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceId(pub u64);

// ---------------------------------------------------------------------------
// Audio stream
// ---------------------------------------------------------------------------

/// An audio playback stream owned by a client.
pub struct AudioStream {
    pub id: StreamId,
    pub format: SampleFormat,
    pub rate: u32,
    pub channels: u8,
    pub direction: StreamDirection,
    pub state: PlaybackState,
    pub volume: u8,      // 0..=100
    pub muted: bool,
    pub buffer: AudioBuffer,
    pub underruns: usize,
}

/// Identifier for an audio stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamId(pub u64);

impl AudioStream {
    pub fn new(
        id: StreamId,
        format: SampleFormat,
        rate: u32,
        channels: u8,
        buffer_size: usize,
    ) -> Result<Self, AudioError> {
        if channels == 0 || channels > 2 {
            return Err(AudioError::InvalidChannelCount);
        }
        match rate {
            8000 | 16000 | 44100 | 48000 => {}
            _ => return Err(AudioError::InvalidRate),
        }
        Ok(Self {
            id,
            format,
            rate,
            channels,
            direction: StreamDirection::Playback,
            state: PlaybackState::Idle,
            volume: 100,
            muted: false,
            buffer: AudioBuffer::new(buffer_size),
            underruns: 0,
        })
    }

    /// Submit PCM frames (interleaved, little-endian i16) to the stream.
    pub fn submit(&mut self, samples: &[i16]) -> Result<usize, AudioError> {
        if self.muted {
            // Silently accept but drop muted audio.
            return Ok(samples.len());
        }
        if self.state == PlaybackState::Failed || self.state == PlaybackState::Stopped {
            return Err(AudioError::DeviceFailed);
        }
        let mut written = 0;
        for &sample in samples {
            if !self.buffer.push(sample) {
                break;
            }
            written += 1;
        }
        if self.state == PlaybackState::Idle {
            self.state = PlaybackState::Running;
        }
        Ok(written)
    }

    /// Render the next frame for playback, applying volume and mute.
    pub fn render_frame(&mut self) -> Option<i16> {
        if self.muted {
            // Consume a frame but output silence to keep stream position.
            let _ = self.buffer.pop();
            return Some(0);
        }
        match self.buffer.pop() {
            Some(sample) => {
                let scaled = (sample as i32 * self.volume as i32 / 100) as i16;
                Some(scaled)
            }
            None => {
                self.underruns += 1;
                self.state = PlaybackState::Underrun;
                None
            }
        }
    }

    /// Set volume (0..=100).
    pub fn set_volume(&mut self, volume: u8) -> Result<(), AudioError> {
        if volume > 100 {
            return Err(AudioError::InvalidFormat);
        }
        self.volume = volume;
        Ok(())
    }

    /// Set mute state.
    pub fn set_mute(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Pause the stream.
    pub fn pause(&mut self) {
        self.state = PlaybackState::Paused;
    }

    /// Resume the stream.
    pub fn resume(&mut self) {
        if self.state == PlaybackState::Underrun {
            self.state = PlaybackState::Running;
        } else {
            self.state = PlaybackState::Running;
        }
    }

    /// Stop the stream.
    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.buffer.reset();
    }
}

// ---------------------------------------------------------------------------
// Mixer
// ---------------------------------------------------------------------------

/// Mixes multiple audio streams into a single output buffer.
pub struct Mixer {
    format: SampleFormat,
    rate: u32,
    channels: u8,
    streams: Vec<AudioStream>,
    output_buffer: AudioBuffer,
}

impl Mixer {
    pub fn new(format: SampleFormat, rate: u32, channels: u8, output_size: usize) -> Self {
        Self {
            format,
            rate,
            channels,
            streams: Vec::new(),
            output_buffer: AudioBuffer::new(output_size),
        }
    }

    /// Add a stream to the mixer.
    pub fn add_stream(&mut self, stream: AudioStream) {
        self.streams.push(stream);
    }

    /// Remove a stream by id.
    pub fn remove_stream(&mut self, id: StreamId) -> bool {
        if let Some(pos) = self.streams.iter().position(|s| s.id == id) {
            self.streams.remove(pos);
            true
        } else {
            false
        }
    }

    /// Mix one frame from all active streams into the output buffer.
    /// Returns None on underrun (no stream has data).
    pub fn mix_frame(&mut self) -> Option<i16> {
        let mut active = false;
        let mut sum: i32 = 0;
        let mut count = 0;
        for stream in &mut self.streams {
            if stream.state == PlaybackState::Running || stream.state == PlaybackState::Paused {
                active = true;
                if let Some(sample) = stream.render_frame() {
                    sum += sample as i32;
                    count += 1;
                }
            }
        }
        if !active || count == 0 {
            return None;
        }
        // Simple average mix with clamping.
        let avg = if count > 0 { sum / count as i32 } else { 0 };
        let clamped = avg.clamp(-32768, 32767) as i16;
        let _ = self.output_buffer.push(clamped);
        Some(clamped)
    }

    /// The number of frames pending in the output buffer.
    pub fn pending(&self) -> usize {
        self.output_buffer.available_read()
    }
}

// ---------------------------------------------------------------------------
// Virtual audio backend (test backend for Phase 19.4)
// ---------------------------------------------------------------------------

/// A deterministic virtual audio device used for testing before physical
/// hardware is available.
pub struct VirtualAudioBackend {
    device: AudioDevice,
    mixer: Mixer,
    next_stream_id: u64,
    playback_position: usize,
    failed: bool,
}

impl VirtualAudioBackend {
    pub const fn new() -> Self {
        Self {
            device: AudioDevice {
                id: DeviceId(0),
                name: "virtual-audio",
                supported_formats: Vec::new(),
                supported_rates: Vec::new(),
                supported_channels: Vec::new(),
                state: AudioDeviceState::Active,
                generation: 1,
            },
            mixer: Mixer {
                format: SampleFormat::S16LE,
                rate: 44100,
                channels: 2,
                streams: Vec::new(),
                output_buffer: AudioBuffer {
                    samples: Vec::new(),
                    write_pos: 0,
                    read_pos: 0,
                    capacity: 4096,
                },
            },
            next_stream_id: 1,
            playback_position: 0,
            failed: false,
        }
    }

    /// Create a playback stream.
    pub fn create_stream(
        &mut self,
        format: SampleFormat,
        rate: u32,
        channels: u8,
        buffer_size: usize,
    ) -> Result<StreamId, AudioError> {
        if self.failed {
            return Err(AudioError::DeviceFailed);
        }
        let id = StreamId(self.next_stream_id);
        self.next_stream_id += 1;
        let stream = AudioStream::new(id, format, rate, channels, buffer_size)?;
        self.mixer.add_stream(stream);
        Ok(id)
    }

    /// Submit samples to a stream.
    pub fn submit(&mut self, id: StreamId, samples: &[i16]) -> Result<usize, AudioError> {
        if self.failed {
            return Err(AudioError::DeviceFailed);
        }
        let stream = self
            .mixer
            .streams
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(AudioError::StreamClosed)?;
        stream.submit(samples)
    }

    /// Render the next output frame through the mixer.
    pub fn render(&mut self) -> Option<i16> {
        if self.failed {
            return None;
        }
        let frame = self.mixer.mix_frame();
        if frame.is_some() {
            self.playback_position += 1;
        }
        frame
    }

    /// Simulate an underrun (drain all stream buffers).
    pub fn simulate_underrun(&mut self) {
        for stream in &mut self.mixer.streams {
            stream.buffer.reset();
            stream.state = PlaybackState::Underrun;
        }
    }

    /// Fail the device.
    pub fn fail(&mut self) {
        self.failed = true;
        self.device.state = AudioDeviceState::Failed;
        for stream in &mut self.mixer.streams {
            stream.state = PlaybackState::Failed;
        }
    }

    /// Recover the device after failure.
    pub fn recover(&mut self) {
        self.failed = false;
        self.device.state = AudioDeviceState::Active;
    }

    /// Whether the device is in a failed state.
    pub fn is_failed(&self) -> bool {
        self.failed
    }

    /// Current playback position in frames.
    pub fn playback_position(&self) -> usize {
        self.playback_position
    }

    /// Total underruns across all streams.
    pub fn total_underruns(&self) -> usize {
        self.mixer.streams.iter().map(|s| s.underruns).sum()
    }

    /// Close a stream.
    pub fn close_stream(&mut self, id: StreamId) -> bool {
        self.mixer.remove_stream(id)
    }

    /// Device state.
    pub fn device_state(&self) -> AudioDeviceState {
        self.device.state
    }
}

static VIRTUAL_AUDIO: Mutex<VirtualAudioBackend> =
    Mutex::new(VirtualAudioBackend::new());

// ---------------------------------------------------------------------------
// Audio service (transitional, in-kernel until moved to user space)
// ---------------------------------------------------------------------------

/// The audio service: owns streams, mixing, volume, mute, output selection.
pub struct AudioService {
    backend: VirtualAudioBackend,
}

impl AudioService {
    pub const fn new() -> Self {
        Self {
            backend: VirtualAudioBackend::new(),
        }
    }

    pub fn create_stream(
        &mut self,
        format: SampleFormat,
        rate: u32,
        channels: u8,
        buffer_size: usize,
    ) -> Result<StreamId, AudioError> {
        self.backend.create_stream(format, rate, channels, buffer_size)
    }

    pub fn submit(&mut self, id: StreamId, samples: &[i16]) -> Result<usize, AudioError> {
        self.backend.submit(id, samples)
    }

    pub fn render(&mut self) -> Option<i16> {
        self.backend.render()
    }

    pub fn set_volume(&mut self, id: StreamId, volume: u8) -> Result<(), AudioError> {
        let stream = self
            .backend
            .mixer
            .streams
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(AudioError::StreamClosed)?;
        stream.set_volume(volume)
    }

    pub fn set_mute(&mut self, id: StreamId, muted: bool) {
        if let Some(stream) = self
            .backend
            .mixer
            .streams
            .iter_mut()
            .find(|s| s.id == id)
        {
            stream.set_mute(muted);
        }
    }

    pub fn close_stream(&mut self, id: StreamId) -> bool {
        self.backend.close_stream(id)
    }

    pub fn fail_device(&mut self) {
        self.backend.fail();
    }

    pub fn recover_device(&mut self) {
        self.backend.recover();
    }

    pub fn is_failed(&self) -> bool {
        self.backend.is_failed()
    }

    /// Whether the audio device is failed (public API alias).
    pub fn audio_device_failed(&self) -> bool {
        self.backend.is_failed()
    }

    pub fn playback_position(&self) -> usize {
        self.backend.playback_position()
    }

    pub fn total_underruns(&self) -> usize {
        self.backend.total_underruns()
    }

    pub fn pending(&self) -> usize {
        self.backend.mixer.pending()
    }
}

static AUDIO_SERVICE: Mutex<AudioService> = Mutex::new(AudioService::new());

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn init() {
    unsafe {
        serial::write_str("audio: subsystem initialized\r\n");
    }
}

/// Create a playback stream through the audio service.
pub fn create_stream(
    format: SampleFormat,
    rate: u32,
    channels: u8,
    buffer_size: usize,
) -> Result<StreamId, AudioError> {
    AUDIO_SERVICE.lock().create_stream(format, rate, channels, buffer_size)
}

/// Submit samples to a stream.
pub fn submit(id: StreamId, samples: &[i16]) -> Result<usize, AudioError> {
    AUDIO_SERVICE.lock().submit(id, samples)
}

/// Render the next output frame.
pub fn render() -> Option<i16> {
    AUDIO_SERVICE.lock().render()
}

/// Set stream volume.
pub fn set_volume(id: StreamId, volume: u8) -> Result<(), AudioError> {
    AUDIO_SERVICE.lock().set_volume(id, volume)
}

/// Set stream mute.
pub fn set_mute(id: StreamId, muted: bool) {
    AUDIO_SERVICE.lock().set_mute(id, muted)
}

/// Close a stream.
pub fn close_stream(id: StreamId) -> bool {
    AUDIO_SERVICE.lock().close_stream(id)
}

/// Fail the audio device (failure-injection test hook).
pub fn fail_audio_device() {
    AUDIO_SERVICE.lock().fail_device();
}

/// Recover the audio device after failure.
pub fn recover_audio_device() {
    AUDIO_SERVICE.lock().recover_device();
}

/// Whether the audio device is failed.
pub fn audio_device_failed() -> bool {
    AUDIO_SERVICE.lock().is_failed()
}

/// Current playback position in frames.
pub fn playback_position() -> usize {
    AUDIO_SERVICE.lock().playback_position()
}

/// Total underruns across all streams.
pub fn total_underruns() -> usize {
    AUDIO_SERVICE.lock().total_underruns()
}

/// Pending frames in the mixer output buffer.
pub fn pending_frames() -> usize {
    AUDIO_SERVICE.lock().pending()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_create_validate_format() {
        assert!(AudioStream::new(StreamId(1), SampleFormat::S16LE, 44100, 2, 256).is_ok());
        assert!(AudioStream::new(StreamId(2), SampleFormat::S16LE, 8000, 0, 256).is_err());
        assert!(AudioStream::new(StreamId(3), SampleFormat::S16LE, 44100, 3, 256).is_err());
        assert!(AudioStream::new(StreamId(4), SampleFormat::S16LE, 99999, 1, 256).is_err());
    }

    #[test]
    fn stream_submit_and_render() {
        let mut stream =
            AudioStream::new(StreamId(1), SampleFormat::S16LE, 44100, 2, 256).unwrap();
        assert!(stream.submit(&[1000, -1000]).is_ok());
        assert_eq!(stream.state, PlaybackState::Running);
        let frame = stream.render_frame();
        assert!(frame.is_some());
    }

    #[test]
    fn stream_volume_clamp() {
        let mut stream =
            AudioStream::new(StreamId(1), SampleFormat::S16LE, 44100, 1, 256).unwrap();
        assert!(stream.set_volume(150).is_err());
        assert!(stream.set_volume(50).is_ok());
        assert_eq!(stream.volume, 50);
    }

    #[test]
    fn stream_mute() {
        let mut stream =
            AudioStream::new(StreamId(1), SampleFormat::S16LE, 44100, 1, 256).unwrap();
        stream.submit(&[3000, -3000]).unwrap();
        stream.set_mute(true);
        assert_eq!(stream.render_frame(), Some(0));
        // The muted frame was consumed; one remains.
        stream.set_mute(false);
        let frame = stream.render_frame();
        assert!(frame.is_some() && frame != Some(0));
    }

    #[test]
    fn buffer_underrun() {
        let mut stream =
            AudioStream::new(StreamId(1), SampleFormat::S16LE, 44100, 1, 256).unwrap();
        // Empty buffer -> underrun.
        assert_eq!(stream.render_frame(), None);
        assert_eq!(stream.state, PlaybackState::Underrun);
        assert_eq!(stream.underruns, 1);
    }

    #[test]
    fn mixer_mix_frame() {
        let mut mixer = Mixer::new(SampleFormat::S16LE, 44100, 2, 1024);
        let mut s1 = AudioStream::new(StreamId(1), SampleFormat::S16LE, 44100, 2, 256).unwrap();
        let mut s2 = AudioStream::new(StreamId(2), SampleFormat::S16LE, 44100, 2, 256).unwrap();
        s1.submit(&[2000]).unwrap();
        s2.submit(&[2000]).unwrap();
        mixer.add_stream(s1);
        mixer.add_stream(s2);
        let frame = mixer.mix_frame();
        assert_eq!(frame, Some(2000));
    }

    #[test]
    fn virtual_backend_create_and_submit() {
        let mut backend = VirtualAudioBackend::new();
        let id = backend
            .create_stream(SampleFormat::S16LE, 44100, 2, 256)
            .unwrap();
        assert!(backend.submit(id, &[1000, -1000]).is_ok());
        assert!(backend.render().is_some());
    }

    #[test]
    fn virtual_backend_underrun_simulation() {
        let mut backend = VirtualAudioBackend::new();
        let id = backend
            .create_stream(SampleFormat::S16LE, 44100, 1, 256)
            .unwrap();
        backend.simulate_underrun();
        assert_eq!(backend.render(), None);
        // Underrun simulation drains buffers without counting a render underrun.
        assert_eq!(backend.total_underruns(), 0);
    }

    #[test]
    fn virtual_backend_fail_and_recover() {
        let mut backend = VirtualAudioBackend::new();
        let id = backend
            .create_stream(SampleFormat::S16LE, 44100, 1, 256)
            .unwrap();
        backend.submit(id, &[1000]).unwrap();
        backend.fail();
        assert!(backend.is_failed());
        assert_eq!(backend.device_state(), AudioDeviceState::Failed);
        assert!(backend.submit(id, &[1000]).is_err());
        backend.recover();
        assert!(!backend.is_failed());
        assert_eq!(backend.device_state(), AudioDeviceState::Active);
    }

    #[test]
    fn audio_service_volume_and_mute() {
        let mut service = AudioService::new();
        let id = service
            .create_stream(SampleFormat::S16LE, 44100, 1, 256)
            .unwrap();
        assert!(service.set_volume(id, 50).is_ok());
        // Submit while unmuted so the stream transitions to Running.
        service.submit(id, &[3000, -3000]).unwrap();
        service.set_mute(id, true);
        assert_eq!(service.render(), Some(0));
        // Unmute: the remaining frame is rendered (scaled by volume).
        service.set_mute(id, false);
        let frame = service.render();
        assert!(frame.is_some() && frame != Some(0));
    }

    #[test]
    fn audio_service_close_stream() {
        let mut service = AudioService::new();
        let id = service
            .create_stream(SampleFormat::S16LE, 44100, 1, 256)
            .unwrap();
        assert!(service.close_stream(id));
        assert!(!service.close_stream(id));
    }

    #[test]
    fn audio_service_fail_and_recover() {
        let mut service = AudioService::new();
        let id = service
            .create_stream(SampleFormat::S16LE, 44100, 1, 256)
            .unwrap();
        service.submit(id, &[1000]).unwrap();
        service.fail_device();
        assert!(service.audio_device_failed());
        assert!(service.submit(id, &[1000]).is_err());
        service.recover_device();
        assert!(!service.audio_device_failed());
    }
}
