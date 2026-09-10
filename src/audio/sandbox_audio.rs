//! Shared-memory audio transport for the out-of-process plugin sandbox
//! (Fas 4.5b).
//!
//! The host and the sandbox worker map the *same* anonymous memory region
//! (created with `memfd_create`) and exchange stereo blocks through two
//! single-producer/single-consumer rings: the host publishes input blocks and
//! consumes output blocks, the worker does the opposite. Because the host never
//! blocks, the transport adds a fixed latency of one block; the plugin host
//! reports that latency so the engine's PDC aligns every track.
//!
//! Layout of the region:
//!
//! ```text
//! [ AudioHeader ][ input ring: slots × block × channels f32 ][ output ring: … ]
//! ```
//!
//! All indices are monotonically increasing sample-block counters; the slot is
//! `counter % slots`. Only the header is touched with atomics, the sample
//! arrays are plain `f32` and rely on the SPSC discipline (one writer per ring
//! end) plus acquire/release on the counters.

use std::io;
use std::os::fd::RawFd;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Magic value in the header, "SNXS" in little endian.
const MAGIC: u32 = 0x5358_4e53;
/// Version of the shared layout.
const VERSION: u32 = 1;
/// Number of stereo frames per block (must match the plugin block size).
pub const DEFAULT_SLOTS: usize = 4;
/// Number of channels carried across the boundary.
pub const CHANNELS: usize = 2;

#[repr(C)]
struct AudioHeader {
    magic: AtomicU32,
    version: AtomicU32,
    block_frames: AtomicU32,
    channels: AtomicU32,
    slots: AtomicU32,
    /// Worker sets this to 1 on attach; the host clears it and resets indices.
    reset: AtomicU32,
    /// Set to 1 to ask the worker to stop.
    shutdown: AtomicU32,
    _reserved: AtomicU32,
    in_write: AtomicU64,
    in_read: AtomicU64,
    out_write: AtomicU64,
    out_read: AtomicU64,
    heartbeat: AtomicU64,
    underruns: AtomicU64,
    overflows: AtomicU64,
    _reserved2: AtomicU64,
}

/// Outcome of one host-side block exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockStatus {
    /// The input block was accepted into the input ring.
    pub input_accepted: bool,
    /// A processed output block was available and copied out.
    pub output_available: bool,
}

/// One endpoint of the shared-memory audio ring.
///
/// Both the host and the worker hold one; they refer to the same mapping.
pub struct AudioBridge {
    base: *mut u8,
    total: usize,
    fd: RawFd,
    block_frames: usize,
    channels: usize,
    slots: usize,
    header_offset: usize,
    in_offset: usize,
    out_offset: usize,
}

// The bridge is just a mapping plus atomics; it is moved to the audio thread on
// the host and to the worker thread in the child process.
unsafe impl Send for AudioBridge {}
unsafe impl Sync for AudioBridge {}

impl AudioBridge {
    /// Creates a fresh region and maps it for the host. The returned bridge owns
    /// the `memfd`; pass [`AudioBridge::raw_fd`] to the worker so it can attach.
    pub fn create(block_frames: usize, slots: usize) -> io::Result<AudioBridge> {
        if block_frames == 0 || slots < 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ogiltiga sandbox-audio-parametrar",
            ));
        }
        let header_offset = 0usize;
        let header_bytes = align_up(std::mem::size_of::<AudioHeader>(), 64);
        let in_offset = header_bytes;
        let block_bytes = block_frames * CHANNELS * std::mem::size_of::<f32>();
        let out_offset = in_offset + slots * block_bytes;
        let total = out_offset + slots * block_bytes;

        let name = std::ffi::CString::new(format!(
            "sonix-sandbox-{}-{}",
            std::process::id(),
            monotonic_nanos()
        ))
        .map_err(io::Error::other)?;
        let fd = unsafe { libc::memfd_create(name.as_ptr(), 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { libc::ftruncate(fd, total as libc::off_t) } != 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(fd) };
            return Err(err);
        }
        let base = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                total,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if base == libc::MAP_FAILED {
            let err = io::Error::last_os_error();
            unsafe { libc::close(fd) };
            return Err(err);
        }
        let bridge = AudioBridge {
            base: base as *mut u8,
            total,
            fd,
            block_frames,
            channels: CHANNELS,
            slots,
            header_offset,
            in_offset,
            out_offset,
        };
        // Zero the region before publishing the header.
        unsafe { std::ptr::write_bytes(bridge.base, 0, total) };
        let header = bridge.header();
        header.magic.store(MAGIC, Ordering::Release);
        header.version.store(VERSION, Ordering::Release);
        header
            .block_frames
            .store(block_frames as u32, Ordering::Release);
        header.channels.store(CHANNELS as u32, Ordering::Release);
        header.slots.store(slots as u32, Ordering::Release);
        Ok(bridge)
    }

    /// Attaches to an existing region from its file descriptor (worker side).
    pub fn attach(fd: RawFd) -> io::Result<AudioBridge> {
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd, &mut stat) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let total = stat.st_size as usize;
        if total < std::mem::size_of::<AudioHeader>() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "sandbox-audio-regionen är för liten",
            ));
        }
        let base = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                total,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if base == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        let bridge = AudioBridge {
            base: base as *mut u8,
            total,
            fd,
            block_frames: 0,
            channels: 0,
            slots: 0,
            header_offset: 0,
            in_offset: 0,
            out_offset: 0,
        };
        let header = bridge.header();
        if header.magic.load(Ordering::Acquire) != MAGIC
            || header.version.load(Ordering::Acquire) != VERSION
        {
            unsafe {
                libc::munmap(base, total);
                libc::close(fd);
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ogiltig sandbox-audio-header",
            ));
        }
        let block_frames = header.block_frames.load(Ordering::Acquire) as usize;
        let channels = header.channels.load(Ordering::Acquire) as usize;
        let slots = header.slots.load(Ordering::Acquire) as usize;
        let header_bytes = align_up(std::mem::size_of::<AudioHeader>(), 64);
        let block_bytes = block_frames * channels * std::mem::size_of::<f32>();
        let out_offset = header_bytes + slots * block_bytes;
        if channels != CHANNELS || block_frames == 0 || slots < 2 || out_offset + slots * block_bytes > total
        {
            unsafe {
                libc::munmap(base, total);
                libc::close(fd);
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "inkonsekvent sandbox-audio-layout",
            ));
        }
        let mut bridge = bridge;
        bridge.block_frames = block_frames;
        bridge.channels = channels;
        bridge.slots = slots;
        bridge.in_offset = header_bytes;
        bridge.out_offset = out_offset;
        Ok(bridge)
    }

    /// The `memfd` descriptor, to be inherited by the worker.
    pub fn raw_fd(&self) -> RawFd {
        self.fd
    }

    pub fn block_frames(&self) -> usize {
        self.block_frames
    }

    #[allow(dead_code)]
    pub fn slots(&self) -> usize {
        self.slots
    }

    fn header(&self) -> &AudioHeader {
        unsafe { &*(self.base.add(self.header_offset) as *const AudioHeader) }
    }

    fn input_slot(&self, index: u64) -> *mut f32 {
        let slot = (index % self.slots as u64) as usize;
        unsafe {
            self.base
                .add(self.in_offset + slot * self.block_frames * self.channels * 4)
                as *mut f32
        }
    }

    fn output_slot(&self, index: u64) -> *mut f32 {
        let slot = (index % self.slots as u64) as usize;
        unsafe {
            self.base
                .add(self.out_offset + slot * self.block_frames * self.channels * 4)
                as *mut f32
        }
    }

    fn write_slot(&self, ptr: *mut f32, left: &[f32], right: &[f32]) {
        let frames = self.block_frames.min(left.len()).min(right.len());
        unsafe {
            let l = std::slice::from_raw_parts_mut(ptr, self.block_frames);
            let r = std::slice::from_raw_parts_mut(ptr.add(self.block_frames), self.block_frames);
            l[..frames].copy_from_slice(&left[..frames]);
            r[..frames].copy_from_slice(&right[..frames]);
            for i in frames..self.block_frames {
                l[i] = 0.0;
                r[i] = 0.0;
            }
        }
    }

    fn read_slot(&self, ptr: *const f32, left: &mut [f32], right: &mut [f32]) {
        let frames = self.block_frames.min(left.len()).min(right.len());
        unsafe {
            let l = std::slice::from_raw_parts(ptr, self.block_frames);
            let r = std::slice::from_raw_parts(ptr.add(self.block_frames), self.block_frames);
            left[..frames].copy_from_slice(&l[..frames]);
            right[..frames].copy_from_slice(&r[..frames]);
        }
    }

    // ------------------------------------------------------------------
    // Host side
    // ------------------------------------------------------------------

    /// Exchanges one block: publishes `left`/`right` as input, then (if the
    /// worker has produced one) copies the next processed block back into
    /// `left`/`right`. Never blocks.
    pub fn process_block(&self, left: &mut [f32], right: &mut [f32]) -> BlockStatus {
        let input_accepted = self.publish_input(left, right);
        let output_available = self.take_output(left, right);
        BlockStatus {
            input_accepted,
            output_available,
        }
    }

    /// Publishes one input block. Returns `false` if the input ring is full
    /// (the worker is not keeping up); the block is then dropped.
    pub fn publish_input(&self, left: &[f32], right: &[f32]) -> bool {
        self.apply_reset();
        let header = self.header();
        let write = header.in_write.load(Ordering::Relaxed);
        let read = header.in_read.load(Ordering::Acquire);
        if write.wrapping_sub(read) >= self.slots as u64 {
            header.overflows.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let ptr = self.input_slot(write);
        self.write_slot(ptr, left, right);
        header.in_write.store(write.wrapping_add(1), Ordering::Release);
        true
    }

    /// Copies the next processed output block into `left`/`right`. Returns
    /// `false` on underrun (the caller should emit silence).
    pub fn take_output(&self, left: &mut [f32], right: &mut [f32]) -> bool {
        let header = self.header();
        let write = header.out_write.load(Ordering::Acquire);
        let read = header.out_read.load(Ordering::Relaxed);
        if read >= write {
            header.underruns.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let ptr = self.output_slot(read);
        self.read_slot(ptr, left, right);
        header.out_read.store(read.wrapping_add(1), Ordering::Release);
        true
    }

    fn apply_reset(&self) {
        let header = self.header();
        if header.reset.swap(0, Ordering::AcqRel) == 1 {
            header.in_write.store(0, Ordering::Release);
            header.in_read.store(0, Ordering::Release);
            header.out_write.store(0, Ordering::Release);
            header.out_read.store(0, Ordering::Release);
        }
    }

    // ------------------------------------------------------------------
    // Worker side
    // ------------------------------------------------------------------

    /// Marks the ring for a fresh start. Called once when the worker attaches;
    /// the host acknowledges it on its next block and resets the indices. Does
    /// not block.
    pub fn worker_begin(&self) {
        self.header().reset.store(1, Ordering::Release);
    }

    /// Processes one pending input block, if any. Returns `true` when a block
    /// was processed. Returns `false` while a reset is pending (the host has not
    /// acknowledged the worker yet) or when no input is available.
    pub fn process_one(
        &self,
        processor: &mut dyn crate::audio::plugin_host_live::PluginProcessor,
    ) -> bool {
        let header = self.header();
        if header.reset.load(Ordering::Acquire) == 1 {
            return false;
        }
        let read = header.in_read.load(Ordering::Relaxed);
        let write = header.in_write.load(Ordering::Acquire);
        if read >= write {
            return false;
        }
        let in_ptr = self.input_slot(read);
        let out_write = header.out_write.load(Ordering::Relaxed);
        let out_read = header.out_read.load(Ordering::Acquire);
        unsafe {
            let l = std::slice::from_raw_parts_mut(in_ptr, self.block_frames);
            let r = std::slice::from_raw_parts_mut(in_ptr.add(self.block_frames), self.block_frames);
            processor.process_stereo(l, r);
            // Drop the output if the host has not drained the ring.
            if out_write.wrapping_sub(out_read) < self.slots as u64 {
                let out_ptr = self.output_slot(out_write);
                let ol = std::slice::from_raw_parts_mut(out_ptr, self.block_frames);
                let or = std::slice::from_raw_parts_mut(out_ptr.add(self.block_frames), self.block_frames);
                ol.copy_from_slice(l);
                or.copy_from_slice(r);
                header
                    .out_write
                    .store(out_write.wrapping_add(1), Ordering::Release);
            }
        }
        header.in_read.store(read.wrapping_add(1), Ordering::Release);
        true
    }

    /// Records that the worker is alive.
    pub fn worker_heartbeat(&self) {
        self.header().heartbeat.fetch_add(1, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn heartbeat(&self) -> u64 {
        self.header().heartbeat.load(Ordering::Relaxed)
    }

    pub fn shutdown_requested(&self) -> bool {
        self.header().shutdown.load(Ordering::Acquire) == 1
    }

    pub fn request_shutdown(&self) {
        self.header().shutdown.store(1, Ordering::Release);
    }

    #[allow(dead_code)]
    pub fn underruns(&self) -> u64 {
        self.header().underruns.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    pub fn overflows(&self) -> u64 {
        self.header().overflows.load(Ordering::Relaxed)
    }
}

impl Drop for AudioBridge {
    fn drop(&mut self) {
        if !self.base.is_null() {
            unsafe {
                libc::munmap(self.base as *mut libc::c_void, self.total);
            }
        }
        if self.fd >= 0 {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

fn monotonic_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::plugin_host_live::{PluginInfo, PluginParameter};

    struct GainProcessor {
        info: PluginInfo,
        parameters: Vec<PluginParameter>,
        gain: f32,
    }

    impl GainProcessor {
        fn new(gain: f32) -> Self {
            Self {
                info: PluginInfo {
                    id: "test.gain".to_string(),
                    name: "Test Gain".to_string(),
                    ..PluginInfo::default()
                },
                parameters: Vec::new(),
                gain,
            }
        }
    }

    impl crate::audio::plugin_host_live::PluginProcessor for GainProcessor {
        fn backend(&self) -> &'static str {
            "test"
        }
        fn info(&self) -> &PluginInfo {
            &self.info
        }
        fn parameters(&self) -> &[PluginParameter] {
            &self.parameters
        }
        fn latency_frames(&self) -> u32 {
            0
        }
        fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
            for s in left.iter_mut() {
                *s *= self.gain;
            }
            for s in right.iter_mut() {
                *s *= self.gain;
            }
        }
        fn set_parameter(&mut self, _id: u32, _value: f64) -> bool {
            false
        }
        fn reset(&mut self) {}
    }

    fn attach_second(bridge: &AudioBridge) -> AudioBridge {
        let fd = unsafe { libc::dup(bridge.raw_fd()) };
        assert!(fd >= 0);
        AudioBridge::attach(fd).expect("attach")
    }

    #[test]
    fn blocks_round_trip_through_shared_memory() {
        let block = 8usize;
        let host = AudioBridge::create(block, DEFAULT_SLOTS).expect("create");
        let worker = attach_second(&host);
        assert_eq!(host.block_frames(), block);
        worker.worker_begin();

        let mut left = vec![1.0f32; block];
        let mut right = vec![-1.0f32; block];
        let status = host.process_block(&mut left, &mut right);
        assert!(status.input_accepted);
        assert!(!status.output_available, "first block has no output yet");

        let mut processor = GainProcessor::new(0.5);
        assert!(worker.process_one(&mut processor));

        let mut out_l = vec![0.0f32; block];
        let mut out_r = vec![0.0f32; block];
        assert!(host.take_output(&mut out_l, &mut out_r));
        assert_eq!(out_l, vec![0.5f32; block]);
        assert_eq!(out_r, vec![-0.5f32; block]);
    }

    #[test]
    fn worker_processes_a_stream_in_lockstep() {
        let block = 4usize;
        let host = AudioBridge::create(block, DEFAULT_SLOTS).expect("create");
        let worker = attach_second(&host);
        worker.worker_begin();
        let mut processor = GainProcessor::new(2.0);

        for round in 0..20 {
            let value = round as f32 + 1.0;
            let left = vec![value; block];
            let right = vec![value; block];
            assert!(host.publish_input(&left, &right), "round {round}");
            assert!(worker.process_one(&mut processor), "round {round}");
            let mut out_l = vec![0.0f32; block];
            let mut out_r = vec![0.0f32; block];
            assert!(host.take_output(&mut out_l, &mut out_r), "round {round}");
            assert_eq!(out_l, vec![value * 2.0; block]);
        }
        assert_eq!(host.underruns(), 0);
        assert_eq!(host.overflows(), 0);
    }

    #[test]
    fn underrun_is_reported_when_the_worker_has_not_run() {
        let block = 4usize;
        let host = AudioBridge::create(block, DEFAULT_SLOTS).expect("create");
        let _worker = attach_second(&host);
        let mut left = vec![1.0f32; block];
        let mut right = vec![1.0f32; block];
        let status = host.process_block(&mut left, &mut right);
        assert!(status.input_accepted);
        assert!(!status.output_available);
        assert_eq!(host.underruns(), 1);
    }

    #[test]
    fn reset_gates_the_worker_until_the_host_acknowledges() {
        let block = 4usize;
        let host = AudioBridge::create(block, DEFAULT_SLOTS).expect("create");
        let worker = attach_second(&host);
        let mut processor = GainProcessor::new(1.0);

        // The worker attached: nothing is processed until the host runs a block.
        worker.worker_begin();
        assert!(!worker.process_one(&mut processor));

        let mut left = vec![3.0f32; block];
        let mut right = vec![3.0f32; block];
        host.process_block(&mut left, &mut right);
        assert!(worker.process_one(&mut processor));

        let mut out_l = vec![0.0f32; block];
        let mut out_r = vec![0.0f32; block];
        assert!(host.take_output(&mut out_l, &mut out_r));
        assert_eq!(out_l, vec![3.0f32; block]);
    }

    #[test]
    fn attach_rejects_a_bad_descriptor() {
        assert!(AudioBridge::attach(-1).is_err());
    }
}
