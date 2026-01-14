//! FPS Monitor module for reading MangoHud shared memory.
//!
//! This module provides functionality to read real-time FPS data from
//! MangoHud's shared memory segment.

use crate::error::ShmError;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Shared memory segment name for MangoHud overlay data.
pub const MANGOHUD_SHM_NAME: &str = "/mangohud-overlay";

/// Ring buffer capacity for FPS samples (120 samples = 12 seconds at 100ms polling).
pub const RING_BUFFER_CAPACITY: usize = 120;

/// C-compatible struct matching MangoHud's shared memory layout.
/// 
/// This struct uses #[repr(C)] to ensure memory layout matches the C ABI,
/// allowing safe interpretation of raw bytes from shared memory.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MangoHudData {
    /// Current FPS value (frames per second).
    pub fps_val: u64,
    /// Frame time in microseconds.
    pub frametime: u64,
}

impl MangoHudData {
    /// Create a new MangoHudData instance.
    pub fn new(fps_val: u64, frametime: u64) -> Self {
        Self { fps_val, frametime }
    }

    /// Convert raw bytes to MangoHudData using unsafe pointer casting.
    /// 
    /// # Safety
    /// The caller must ensure that:
    /// - `ptr` points to valid memory of at least `size_of::<MangoHudData>()` bytes
    /// - The memory is properly aligned for MangoHudData
    /// - The memory contains valid data in the expected C layout
    pub unsafe fn from_raw_ptr(ptr: *const u8) -> Self {
        let data_ptr = ptr as *const MangoHudData;
        std::ptr::read_volatile(data_ptr)
    }

    /// Write MangoHudData to a raw byte buffer.
    /// 
    /// # Safety
    /// The caller must ensure that:
    /// - `ptr` points to valid, writable memory of at least `size_of::<MangoHudData>()` bytes
    /// - The memory is properly aligned for MangoHudData
    pub unsafe fn to_raw_ptr(&self, ptr: *mut u8) {
        let data_ptr = ptr as *mut MangoHudData;
        std::ptr::write_volatile(data_ptr, *self);
    }

    /// Get the size of the struct in bytes.
    pub const fn size() -> usize {
        std::mem::size_of::<MangoHudData>()
    }
}

/// A single FPS sample with timestamp.
#[derive(Debug, Clone)]
pub struct FpsSample {
    /// FPS value at the time of sampling.
    pub fps: u64,
    /// Frame time in microseconds.
    pub frametime: u64,
    /// Timestamp when the sample was taken.
    pub timestamp: Instant,
}

impl FpsSample {
    /// Create a new FPS sample with the current timestamp.
    pub fn new(fps: u64, frametime: u64) -> Self {
        Self {
            fps,
            frametime,
            timestamp: Instant::now(),
        }
    }

    /// Create a new FPS sample with a specific timestamp.
    pub fn with_timestamp(fps: u64, frametime: u64, timestamp: Instant) -> Self {
        Self {
            fps,
            frametime,
            timestamp,
        }
    }
}

/// Ring buffer for storing FPS samples with fixed capacity.
/// 
/// Maintains the last 120 samples for data smoothing and analysis.
#[derive(Debug)]
pub struct FpsRingBuffer {
    samples: VecDeque<FpsSample>,
    capacity: usize,
}

impl FpsRingBuffer {
    /// Create a new ring buffer with the default capacity (120 samples).
    pub fn new() -> Self {
        Self::with_capacity(RING_BUFFER_CAPACITY)
    }

    /// Create a new ring buffer with a specific capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a new sample to the buffer, removing the oldest if at capacity.
    pub fn push(&mut self, sample: FpsSample) {
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// Get the average FPS from all samples in the buffer.
    /// Returns 0.0 if the buffer is empty.
    pub fn average(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.samples.iter().map(|s| s.fps).sum();
        sum as f64 / self.samples.len() as f64
    }

    /// Get the percentile value of frametimes in the buffer.
    /// 
    /// # Arguments
    /// * `p` - Percentile value between 0.0 and 1.0 (e.g., 0.99 for P99)
    /// 
    /// Returns 0 if the buffer is empty.
    pub fn percentile(&self, p: f64) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        
        let mut frametimes: Vec<u64> = self.samples.iter().map(|s| s.frametime).collect();
        frametimes.sort_unstable();
        
        let index = ((frametimes.len() as f64 - 1.0) * p.clamp(0.0, 1.0)).round() as usize;
        frametimes[index]
    }

    /// Get the current number of samples in the buffer.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Get the capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all samples from the buffer.
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// Get samples within a specific time window from now.
    pub fn samples_in_window(&self, duration: std::time::Duration) -> Vec<&FpsSample> {
        let cutoff = Instant::now() - duration;
        self.samples
            .iter()
            .filter(|s| s.timestamp >= cutoff)
            .collect()
    }

    /// Get an iterator over all samples.
    pub fn iter(&self) -> impl Iterator<Item = &FpsSample> {
        self.samples.iter()
    }
}

impl Default for FpsRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe reader for MangoHud shared memory.
/// 
/// This struct is only available on Unix-like systems (Linux) where POSIX
/// shared memory is supported.
#[cfg(target_family = "unix")]
pub struct MangoHudReader {
    /// Pointer to the mapped shared memory.
    shm_ptr: *const MangoHudData,
    /// File descriptor for the shared memory segment.
    shm_fd: i32,
    /// Ring buffer for storing FPS samples.
    ring_buffer: Arc<Mutex<FpsRingBuffer>>,
    /// Size of the mapped memory region.
    shm_size: usize,
}

#[cfg(target_family = "unix")]
// Safety: MangoHudReader can be sent between threads because:
// - shm_ptr points to shared memory that remains valid for the lifetime of the reader
// - shm_fd is just an integer file descriptor
// - ring_buffer is protected by Arc<Mutex<>>
unsafe impl Send for MangoHudReader {}
#[cfg(target_family = "unix")]
unsafe impl Sync for MangoHudReader {}

#[cfg(target_family = "unix")]
impl MangoHudReader {
    /// Connect to MangoHud shared memory segment.
    /// 
    /// Opens the shared memory segment named "/mangohud-overlay" and maps it
    /// into the process address space for reading.
    pub fn new() -> Result<Self, ShmError> {
        use libc::{
            c_char, close, mmap, shm_open, MAP_FAILED, MAP_SHARED, O_RDONLY, PROT_READ,
        };
        use std::ffi::CString;

        let shm_name = CString::new(MANGOHUD_SHM_NAME)
            .map_err(|_| ShmError::InvalidData("Invalid SHM name".to_string()))?;

        // Open the shared memory segment
        let shm_fd = unsafe { shm_open(shm_name.as_ptr() as *const c_char, O_RDONLY, 0) };

        if shm_fd < 0 {
            return Err(ShmError::OpenFailed {
                name: MANGOHUD_SHM_NAME.to_string(),
                source: std::io::Error::last_os_error(),
            });
        }

        let shm_size = MangoHudData::size();

        // Map the shared memory into our address space
        let shm_ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                shm_size,
                PROT_READ,
                MAP_SHARED,
                shm_fd,
                0,
            )
        };

        if shm_ptr == MAP_FAILED {
            unsafe { close(shm_fd) };
            return Err(ShmError::MmapFailed(std::io::Error::last_os_error()));
        }

        Ok(Self {
            shm_ptr: shm_ptr as *const MangoHudData,
            shm_fd,
            ring_buffer: Arc::new(Mutex::new(FpsRingBuffer::new())),
            shm_size,
        })
    }

    /// Poll current FPS data from shared memory.
    /// 
    /// Reads the current FPS and frametime values from the MangoHud shared
    /// memory segment and adds them to the ring buffer.
    pub fn poll(&self) -> Result<FpsSample, ShmError> {
        let data = unsafe { MangoHudData::from_raw_ptr(self.shm_ptr as *const u8) };

        let sample = FpsSample::new(data.fps_val, data.frametime);

        // Add to ring buffer
        if let Ok(mut buffer) = self.ring_buffer.lock() {
            buffer.push(sample.clone());
        }

        Ok(sample)
    }

    /// Get smoothed FPS average from ring buffer.
    pub fn get_smoothed_fps(&self) -> f64 {
        self.ring_buffer
            .lock()
            .map(|buffer| buffer.average())
            .unwrap_or(0.0)
    }

    /// Get P99 frametime from ring buffer.
    pub fn get_p99_frametime(&self) -> u64 {
        self.ring_buffer
            .lock()
            .map(|buffer| buffer.percentile(0.99))
            .unwrap_or(0)
    }

    /// Get a clone of the ring buffer Arc for external access.
    pub fn get_ring_buffer(&self) -> Arc<Mutex<FpsRingBuffer>> {
        Arc::clone(&self.ring_buffer)
    }
}

#[cfg(target_family = "unix")]
impl Drop for MangoHudReader {
    fn drop(&mut self) {
        use libc::{close, munmap};

        // Unmap the shared memory
        if !self.shm_ptr.is_null() {
            unsafe {
                munmap(self.shm_ptr as *mut libc::c_void, self.shm_size);
            }
        }

        // Close the file descriptor
        if self.shm_fd >= 0 {
            unsafe {
                close(self.shm_fd);
            }
        }
    }
}

/// Stub implementation for non-Unix platforms (Windows) for development/testing.
/// The actual daemon only runs on Linux (SteamOS).
#[cfg(not(target_family = "unix"))]
pub struct MangoHudReader {
    ring_buffer: Arc<Mutex<FpsRingBuffer>>,
}

#[cfg(not(target_family = "unix"))]
impl MangoHudReader {
    /// Stub: Returns NotAvailable on non-Unix platforms.
    pub fn new() -> Result<Self, ShmError> {
        Err(ShmError::NotAvailable)
    }

    /// Stub: Returns NotAvailable on non-Unix platforms.
    pub fn poll(&self) -> Result<FpsSample, ShmError> {
        Err(ShmError::NotAvailable)
    }

    /// Stub: Returns 0.0 on non-Unix platforms.
    pub fn get_smoothed_fps(&self) -> f64 {
        0.0
    }

    /// Stub: Returns 0 on non-Unix platforms.
    pub fn get_p99_frametime(&self) -> u64 {
        0
    }

    /// Get a clone of the ring buffer Arc for external access.
    pub fn get_ring_buffer(&self) -> Arc<Mutex<FpsRingBuffer>> {
        Arc::clone(&self.ring_buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // **Feature: smart-refresh-daemon, Property 1: Shared Memory Data Round-Trip**
    // **Validates: Requirements 1.2**
    proptest! {
        #[test]
        fn test_mangohud_data_round_trip(fps_val in 0u64..1000u64, frametime in 0u64..100000u64) {
            let original = MangoHudData::new(fps_val, frametime);
            
            // Allocate a buffer to simulate shared memory
            let mut buffer = vec![0u8; MangoHudData::size()];
            
            // Write to buffer
            unsafe {
                original.to_raw_ptr(buffer.as_mut_ptr());
            }
            
            // Read back from buffer
            let recovered = unsafe {
                MangoHudData::from_raw_ptr(buffer.as_ptr())
            };
            
            // Verify round-trip consistency
            prop_assert_eq!(original.fps_val, recovered.fps_val);
            prop_assert_eq!(original.frametime, recovered.frametime);
            prop_assert_eq!(original, recovered);
        }
    }

    // **Feature: smart-refresh-daemon, Property 2: Ring Buffer Capacity Invariant**
    // **Validates: Requirements 1.5**
    proptest! {
        #[test]
        fn test_ring_buffer_capacity_invariant(
            samples in prop::collection::vec((0u64..1000u64, 0u64..100000u64), 0..300)
        ) {
            let mut buffer = FpsRingBuffer::new();
            
            for (fps, frametime) in samples.iter() {
                buffer.push(FpsSample::new(*fps, *frametime));
                
                // Invariant: buffer size never exceeds capacity
                prop_assert!(buffer.len() <= RING_BUFFER_CAPACITY);
            }
            
            // After pushing N samples where N > 120, buffer should contain exactly 120
            if samples.len() > RING_BUFFER_CAPACITY {
                prop_assert_eq!(buffer.len(), RING_BUFFER_CAPACITY);
                
                // Verify we have the most recent samples
                let expected_start = samples.len() - RING_BUFFER_CAPACITY;
                for (i, sample) in buffer.iter().enumerate() {
                    let (expected_fps, expected_frametime) = samples[expected_start + i];
                    prop_assert_eq!(sample.fps, expected_fps);
                    prop_assert_eq!(sample.frametime, expected_frametime);
                }
            } else {
                prop_assert_eq!(buffer.len(), samples.len());
            }
        }
    }

    #[test]
    fn test_mangohud_data_size() {
        // MangoHudData should be 16 bytes (2 x u64)
        assert_eq!(MangoHudData::size(), 16);
    }

    #[test]
    fn test_fps_ring_buffer_average() {
        let mut buffer = FpsRingBuffer::new();
        
        // Empty buffer should return 0.0
        assert_eq!(buffer.average(), 0.0);
        
        // Add some samples
        buffer.push(FpsSample::new(60, 16666));
        buffer.push(FpsSample::new(30, 33333));
        buffer.push(FpsSample::new(90, 11111));
        
        // Average should be (60 + 30 + 90) / 3 = 60
        assert_eq!(buffer.average(), 60.0);
    }

    #[test]
    fn test_fps_ring_buffer_percentile() {
        let mut buffer = FpsRingBuffer::new();
        
        // Empty buffer should return 0
        assert_eq!(buffer.percentile(0.99), 0);
        
        // Add samples with known frametimes
        for i in 1..=100 {
            buffer.push(FpsSample::new(60, i * 100));
        }
        
        // P99 should be close to the 99th value (9900)
        let p99 = buffer.percentile(0.99);
        assert!(p99 >= 9800 && p99 <= 10000);
    }

    #[test]
    fn test_fps_ring_buffer_capacity_enforcement() {
        let mut buffer = FpsRingBuffer::with_capacity(5);
        
        for i in 0..10 {
            buffer.push(FpsSample::new(i, i * 1000));
        }
        
        // Should only have 5 samples
        assert_eq!(buffer.len(), 5);
        
        // Should have the last 5 samples (5, 6, 7, 8, 9)
        let fps_values: Vec<u64> = buffer.iter().map(|s| s.fps).collect();
        assert_eq!(fps_values, vec![5, 6, 7, 8, 9]);
    }
}
