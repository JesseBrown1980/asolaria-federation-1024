//! VFS file-descriptor primitive · supports `sys_read` / `sys_write` / `sys_open` / `sys_close`
//!
//! Anchor PID: ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11
//!
//! v0.1 (prior): routing-by-fd-value scaffold. Three reserved FDs (`STDIN`/`STDOUT`/`STDERR`)
//! were pre-defined; all other FD values rejected as `InvalidFd`. No real I/O backing.
//!
//! v0.2 (prior): 256-entry atomic FD slot table. Slots 3..256 user-allocatable via
//! `vfs_open(path, mode)` with CAS-based atomic state transitions. `vfs_close(fd)`
//! releases. Read/write on user FDs returned `InvalidFd` (no backing yet).
//!
//! v0.3 (this file): **path-based binding-kind dispatch**. The FD state byte is now
//! a *packed encoding* — `Free=0`, `Unbacked=1`, `DevNull=2`, `BusEnvelope=3`. `vfs_open`
//! recognizes special paths and CAS-allocates to the matching binding kind:
//!   - `/dev/null` → `FD_BIND_DEVNULL`: read returns `Ok(0)` (EOF), write returns `Ok(buf.len())` (discard)
//!   - `/dev/bus/envelope` → `FD_BIND_BUSENVELOPE`: read/write route through `envelope::dispatch_*_bytes`
//!   - anything else → `FD_BIND_UNBACKED` (state byte = `FD_STATE_OPEN` for v0.2 backward compat)
//! Unrecognized paths still allocate a slot but read/write return `InvalidFd` (matches v0.2).
//! Binding kind = 0 is `FD_STATE_FREE`; any non-zero value is `Open` for the `vfs_is_open` check.
//!
//! v0.4 (future): host-callback registered from boot crate for real regular-file I/O
//! (analog of `frame_alloc::register_frame_region`). The `FileOps` trait defined here
//! is the abstraction the v0.4 backing will implement.
//!
//! Rationale (mirrors frame_alloc): kernel-core is `#![forbid(unsafe_code)]`. The FD
//! state byte is `AtomicU8` per slot — no `static mut`, no pointer-table tricks. The
//! BusEnvelope binding works because the `envelope` module already provides safe
//! atomic SPSC byte-queue primitives; vfs simply routes FD operations through them.

use core::sync::atomic::{AtomicU8, Ordering};

// ===== v0.1-compatible API surface (preserved) =====

/// Reserved FD: standard input. Pre-opened; `vfs_read` returns `Ok(0)` (EOF).
pub const STDIN_FD: u64 = 0;

/// Reserved FD: standard output. Pre-opened; `vfs_write` accepts with `Ok(buf.len())`.
pub const STDOUT_FD: u64 = 1;

/// Reserved FD: standard error. Pre-opened; `vfs_write` accepts with `Ok(buf.len())`.
pub const STDERR_FD: u64 = 2;

/// Highest reserved FD value. Special-cased — cannot be closed via `vfs_close`.
pub const RESERVED_FD_MAX: u64 = STDERR_FD;

/// VFS errors. Mirror at the syscall boundary via `SyscallErr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VfsErr {
    /// FD value is not in the v0.1 reserved range and no slot in the table is Open for it.
    InvalidFd,
    /// FD is reserved but the operation is invalid for it (e.g. read on stdout, close on stdin).
    InvalidOpForFd,
    /// Buffer is empty when bytes are required (e.g. write with `buf.is_empty()`).
    InvalidBuf,
    /// `len > buf.len()` — caller passed a length larger than the destination.
    LenExceedsBuf,
    /// FD table full: no free slot in `3..FD_TABLE_SIZE` available.
    TableFull,
    /// Path bytes are empty in a context that requires a non-empty path (`vfs_open`).
    InvalidPath,
    /// Mode bits are invalid for `vfs_open` (zero, or unknown bits set).
    InvalidMode,
    /// Payload too large for the bound destination (e.g. BusEnvelope slot capacity).
    PayloadOversize,
    /// Backing would block and caller did not request waiting (e.g. BusEnvelope empty).
    WouldBlock,
    /// Reserved for v0.4 operations not yet implemented (host-callback regular-file I/O).
    Unimplemented,
}

// ===== v0.2: 256-entry FD slot table =====

/// Total FD slots in the v0.2 table. 256 chosen to match POSIX `OPEN_MAX` floor and
/// give 253 user-allocatable slots after pre-reserving 0/1/2.
pub const FD_TABLE_SIZE: usize = 256;

/// Slot state: free for allocation by `vfs_open`.
pub const FD_STATE_FREE: u8 = 0;

/// Slot state: in use, no real backing. `vfs_read`/`vfs_write` return `InvalidFd`.
/// v0.2-compatible alias for `FD_BIND_UNBACKED`; any non-`FD_STATE_FREE` value is
/// considered "Open" by `vfs_is_open`.
pub const FD_STATE_OPEN: u8 = 1;

/// Binding kind: unbacked (slot allocated, but no real I/O). v0.2 default. Read/write
/// return `InvalidFd`. Encoded as the same byte as `FD_STATE_OPEN` for backward compat.
pub const FD_BIND_UNBACKED: u8 = 1;

/// Binding kind: `/dev/null` semantics. Read returns `Ok(0)` (EOF); write discards
/// the buffer and returns `Ok(buf.len())`.
pub const FD_BIND_DEVNULL: u8 = 2;

/// Binding kind: `/dev/bus/envelope` semantics. Read dequeues from
/// `crate::envelope::dispatch_dequeue_bytes`; write enqueues via
/// `crate::envelope::dispatch_enqueue_bytes`.
pub const FD_BIND_BUSENVELOPE: u8 = 3;

/// Mode bits for `vfs_open`. v0.2 accepts O_RDONLY | O_WRONLY | O_RDWR (matches POSIX subset).
pub const O_RDONLY: u32 = 0b0000_0001;
pub const O_WRONLY: u32 = 0b0000_0010;
pub const O_RDWR: u32 = 0b0000_0011;

/// Bit mask of all VALID mode bits. Any bits outside this mask → `InvalidMode`.
pub const MODE_MASK: u32 = O_RDONLY | O_WRONLY | O_RDWR;

/// Static FD state table. Slots are `AtomicU8`; 0..=2 are conceptually "always open"
/// (special-cased in `vfs_read`/`vfs_write`/`vfs_close` regardless of table state).
/// Slots 3..256 store actual state and are CAS-mutated by `vfs_open`/`vfs_close`.
///
/// Initialization: all slots start `FD_STATE_FREE`. The reserved-FD special-case in
/// the public functions means slots 0/1/2 never have to be flipped to Open at boot
/// — they're always-open by convention, not by table state.
static FD_STATE: [AtomicU8; FD_TABLE_SIZE] =
    [const { AtomicU8::new(FD_STATE_FREE) }; FD_TABLE_SIZE];

/// Generic file-operations trait. Defined in v0.2; concrete implementors land in
/// v0.3 once host-callback backing is plumbed (regular files, pipes, bus-envelope-FDs
/// all implement this same shape).
pub trait FileOps {
    /// Read up to `len` bytes from the backing into `buf`. Returns bytes read.
    fn read(&self, buf: &mut [u8], len: usize) -> Result<usize, VfsErr>;
    /// Write `buf` to the backing. Returns bytes written.
    fn write(&self, buf: &[u8]) -> Result<usize, VfsErr>;
    /// Release the backing. After this call, the FD is invalid.
    fn close(&self) -> Result<(), VfsErr>;
}

/// Returns true if `fd` refers to a slot that is currently considered Open.
///
/// - FDs 0/1/2 are special-cased: always Open (cannot be closed; pre-allocated to stdin/stdout/stderr).
/// - FDs 3..FD_TABLE_SIZE: state-table lookup; Open iff `FD_STATE[fd] == FD_STATE_OPEN`.
/// - FDs ≥ FD_TABLE_SIZE: never Open.
pub fn vfs_is_open(fd: u64) -> bool {
    if fd <= RESERVED_FD_MAX {
        return true;
    }
    if (fd as usize) >= FD_TABLE_SIZE {
        return false;
    }
    // v0.3: any non-Free state encodes a binding kind (Unbacked / DevNull / BusEnvelope / …),
    // all of which mean "Open" for the purpose of this check.
    FD_STATE[fd as usize].load(Ordering::Acquire) != FD_STATE_FREE
}

/// Map a path to its binding kind. Recognized paths get a typed binding; everything
/// else falls through to `FD_BIND_UNBACKED` (v0.2 backward-compat).
fn binding_kind_for_path(path: &[u8]) -> u8 {
    if path == b"/dev/null" {
        FD_BIND_DEVNULL
    } else if path == b"/dev/bus/envelope" {
        FD_BIND_BUSENVELOPE
    } else {
        FD_BIND_UNBACKED
    }
}

/// Open a new FD for `path` with `mode`. Returns the lowest free slot in `3..FD_TABLE_SIZE`
/// as the new fd, atomically transitioning that slot from Free → binding-kind via CAS.
///
/// v0.3 semantics:
/// - `path.is_empty()` → `Err(InvalidPath)`
/// - `mode == 0` or `mode` contains bits outside `MODE_MASK` → `Err(InvalidMode)`
/// - No free slot in `3..FD_TABLE_SIZE` → `Err(TableFull)`
/// - On success, returns `Ok(fd)` where `fd ∈ 3..FD_TABLE_SIZE` is the slot index. The
///   slot's state byte encodes the binding kind: see `binding_kind_for_path`.
///   Recognized paths (`/dev/null`, `/dev/bus/envelope`) get typed bindings; all other
///   paths get `FD_BIND_UNBACKED` (read/write return `InvalidFd`, preserving v0.2 behavior).
pub fn vfs_open(path: &[u8], mode: u32) -> Result<u64, VfsErr> {
    if path.is_empty() {
        return Err(VfsErr::InvalidPath);
    }
    if mode == 0 || (mode & !MODE_MASK) != 0 {
        return Err(VfsErr::InvalidMode);
    }
    let kind = binding_kind_for_path(path);
    // Lowest-free-index scan starting at slot 3 (skip reserved 0/1/2).
    let start: usize = (RESERVED_FD_MAX as usize) + 1;
    for fd in start..FD_TABLE_SIZE {
        // CAS Free→kind. If another caller raced and won this slot, advance to the next.
        match FD_STATE[fd].compare_exchange(
            FD_STATE_FREE,
            kind,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return Ok(fd as u64),
            Err(_) => continue,
        }
    }
    Err(VfsErr::TableFull)
}

/// Close `fd`, atomically transitioning its slot to Free via swap.
///
/// v0.3 semantics:
/// - `fd ≤ RESERVED_FD_MAX` (0/1/2): `Err(InvalidOpForFd)` — stdin/stdout/stderr cannot be closed.
/// - `fd ≥ FD_TABLE_SIZE`: `Err(InvalidFd)`.
/// - Slot was already Free: `Err(InvalidFd)` (double-close rejected).
/// - Slot held any binding kind (Unbacked / DevNull / BusEnvelope): swap-to-Free → `Ok(())`.
pub fn vfs_close(fd: u64) -> Result<(), VfsErr> {
    if fd <= RESERVED_FD_MAX {
        return Err(VfsErr::InvalidOpForFd);
    }
    if (fd as usize) >= FD_TABLE_SIZE {
        return Err(VfsErr::InvalidFd);
    }
    let old = FD_STATE[fd as usize].swap(FD_STATE_FREE, Ordering::AcqRel);
    if old == FD_STATE_FREE {
        Err(VfsErr::InvalidFd)
    } else {
        Ok(())
    }
}

/// Reset the entire FD state table to all-Free. Reserved FDs 0/1/2 are conceptually
/// always-open and are unaffected (they don't live in the table state). Test-only helper
/// analogous to `crate::envelope::reset_slot_for_tests` and `crate::frame_alloc::reset_for_tests`.
///
/// Required before any test that calls `vfs_open` to avoid cross-test state bleed.
pub fn reset_for_tests() {
    for fd in 0..FD_TABLE_SIZE {
        FD_STATE[fd].store(FD_STATE_FREE, Ordering::Release);
    }
}

// ===== v0.1-compatible read/write (preserved + slot-state-aware) =====

/// Read up to `len` bytes from `fd` into `buf`.
///
/// v0.3 semantics:
/// - `fd == STDIN_FD` (0): returns `Ok(0)` — EOF (no input plumbed yet)
/// - `fd == STDOUT_FD || fd == STDERR_FD`: returns `Err(InvalidOpForFd)`
/// - `fd ∈ 3..FD_TABLE_SIZE` AND slot is Open: route by binding kind
///   - `FD_BIND_UNBACKED`: returns `Err(InvalidFd)` (preserves v0.2 behavior)
///   - `FD_BIND_DEVNULL`: returns `Ok(0)` (EOF)
///   - `FD_BIND_BUSENVELOPE`: delegates to `envelope::dispatch_dequeue_bytes(&mut buf[..len], 0)`;
///     `QueueEmpty` → `WouldBlock`; `PayloadOversize` → `PayloadOversize`; other → `InvalidFd`
/// - any other `fd`: returns `Err(InvalidFd)`
///
/// Errors:
/// - `LenExceedsBuf` if `len > buf.len()` (signature-level guard)
/// - `InvalidOpForFd` for stdout/stderr
/// - `InvalidFd` for unrecognized or free fds
/// - `WouldBlock` for BusEnvelope FDs with empty queue
/// - `PayloadOversize` for BusEnvelope reads where slot holds more bytes than buf can fit
pub fn vfs_read(fd: u64, buf: &mut [u8], len: usize) -> Result<usize, VfsErr> {
    if len > buf.len() {
        return Err(VfsErr::LenExceedsBuf);
    }
    match fd {
        STDIN_FD => return Ok(0), // EOF — no input source in v0.1
        STDOUT_FD | STDERR_FD => return Err(VfsErr::InvalidOpForFd),
        _ => {}
    }
    let idx = fd as usize;
    if idx >= FD_TABLE_SIZE {
        return Err(VfsErr::InvalidFd);
    }
    let kind = FD_STATE[idx].load(Ordering::Acquire);
    match kind {
        FD_STATE_FREE => Err(VfsErr::InvalidFd),
        FD_BIND_UNBACKED => Err(VfsErr::InvalidFd),
        FD_BIND_DEVNULL => Ok(0),
        FD_BIND_BUSENVELOPE => match crate::envelope::dispatch_dequeue_bytes(&mut buf[..len], 0) {
            Ok(n) => Ok(n),
            Err(crate::envelope::EnvelopeErr::QueueEmpty) => Err(VfsErr::WouldBlock),
            Err(crate::envelope::EnvelopeErr::PayloadOversize) => Err(VfsErr::PayloadOversize),
            Err(_) => Err(VfsErr::InvalidFd),
        },
        _ => Err(VfsErr::InvalidFd),
    }
}

/// Write `buf` to `fd`. Returns bytes written.
///
/// v0.3 semantics:
/// - `fd == STDOUT_FD || fd == STDERR_FD`: returns `Ok(buf.len())` — accepted (no real
///   emission until v0.4 host-callback)
/// - `fd == STDIN_FD`: returns `Err(InvalidOpForFd)`
/// - `fd ∈ 3..FD_TABLE_SIZE` AND slot is Open: route by binding kind
///   - `FD_BIND_UNBACKED`: returns `Err(InvalidFd)` (preserves v0.2 behavior)
///   - `FD_BIND_DEVNULL`: discards `buf` and returns `Ok(buf.len())`
///   - `FD_BIND_BUSENVELOPE`: delegates to `envelope::dispatch_enqueue_bytes(buf, 0)`;
///     returns `Ok(buf.len())` on success; `QueueFull` → `WouldBlock`; `PayloadOversize`
///     → `PayloadOversize`; other → `InvalidFd`
/// - any other `fd`: returns `Err(InvalidFd)`
///
/// Errors:
/// - `InvalidBuf` if `buf.is_empty()` (signature-level guard)
/// - `InvalidOpForFd` for stdin
/// - `InvalidFd` for unrecognized or free fds
/// - `WouldBlock` for BusEnvelope FDs with full queue
/// - `PayloadOversize` for BusEnvelope writes exceeding slot capacity
pub fn vfs_write(fd: u64, buf: &[u8]) -> Result<usize, VfsErr> {
    if buf.is_empty() {
        return Err(VfsErr::InvalidBuf);
    }
    match fd {
        STDOUT_FD | STDERR_FD => return Ok(buf.len()),
        STDIN_FD => return Err(VfsErr::InvalidOpForFd),
        _ => {}
    }
    let idx = fd as usize;
    if idx >= FD_TABLE_SIZE {
        return Err(VfsErr::InvalidFd);
    }
    let kind = FD_STATE[idx].load(Ordering::Acquire);
    match kind {
        FD_STATE_FREE => Err(VfsErr::InvalidFd),
        FD_BIND_UNBACKED => Err(VfsErr::InvalidFd),
        FD_BIND_DEVNULL => Ok(buf.len()),
        FD_BIND_BUSENVELOPE => match crate::envelope::dispatch_enqueue_bytes(buf, 0) {
            Ok(()) => Ok(buf.len()),
            Err(crate::envelope::EnvelopeErr::QueueFull) => Err(VfsErr::WouldBlock),
            Err(crate::envelope::EnvelopeErr::PayloadOversize) => Err(VfsErr::PayloadOversize),
            Err(_) => Err(VfsErr::InvalidFd),
        },
        _ => Err(VfsErr::InvalidFd),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== v0.1 tests (preserved verbatim) =====

    #[test]
    fn reserved_fd_constants_are_canonical() {
        assert_eq!(STDIN_FD, 0);
        assert_eq!(STDOUT_FD, 1);
        assert_eq!(STDERR_FD, 2);
        assert_eq!(RESERVED_FD_MAX, 2);
    }

    #[test]
    fn vfs_read_stdin_returns_eof() {
        let mut buf = [0u8; 16];
        assert_eq!(vfs_read(STDIN_FD, &mut buf, 8), Ok(0));
    }

    #[test]
    fn vfs_read_stdout_or_stderr_rejects() {
        let mut buf = [0u8; 16];
        assert_eq!(
            vfs_read(STDOUT_FD, &mut buf, 8),
            Err(VfsErr::InvalidOpForFd)
        );
        assert_eq!(
            vfs_read(STDERR_FD, &mut buf, 8),
            Err(VfsErr::InvalidOpForFd)
        );
    }

    #[test]
    fn vfs_read_other_fd_returns_invalid_fd() {
        let mut buf = [0u8; 16];
        assert_eq!(vfs_read(99, &mut buf, 8), Err(VfsErr::InvalidFd));
        assert_eq!(vfs_read(u64::MAX, &mut buf, 8), Err(VfsErr::InvalidFd));
    }

    #[test]
    fn vfs_read_len_exceeding_buf_rejects() {
        let mut buf = [0u8; 4];
        assert_eq!(vfs_read(STDIN_FD, &mut buf, 5), Err(VfsErr::LenExceedsBuf));
    }

    #[test]
    fn vfs_write_stdout_accepts() {
        assert_eq!(vfs_write(STDOUT_FD, b"hello"), Ok(5));
    }

    #[test]
    fn vfs_write_stderr_accepts() {
        assert_eq!(vfs_write(STDERR_FD, b"error message"), Ok(13));
    }

    #[test]
    fn vfs_write_stdin_rejects() {
        assert_eq!(vfs_write(STDIN_FD, b"x"), Err(VfsErr::InvalidOpForFd));
    }

    #[test]
    fn vfs_write_other_fd_returns_invalid_fd() {
        assert_eq!(vfs_write(99, b"x"), Err(VfsErr::InvalidFd));
        assert_eq!(vfs_write(u64::MAX, b"x"), Err(VfsErr::InvalidFd));
    }

    #[test]
    fn vfs_write_empty_buf_returns_invalid_buf() {
        assert_eq!(vfs_write(STDOUT_FD, b""), Err(VfsErr::InvalidBuf));
    }

    // ===== v0.2 tests (atomic FD slot table) =====

    #[test]
    fn vfs_v02_fd_table_size_is_256() {
        assert_eq!(FD_TABLE_SIZE, 256);
    }

    #[test]
    fn vfs_v02_mode_bits_are_canonical() {
        assert_eq!(O_RDONLY, 0b01);
        assert_eq!(O_WRONLY, 0b10);
        assert_eq!(O_RDWR, 0b11);
        assert_eq!(MODE_MASK, 0b11);
    }

    #[test]
    fn vfs_v02_reserved_fds_are_always_open() {
        // Stdin/stdout/stderr are special-cased — always Open regardless of table state.
        reset_for_tests();
        assert!(vfs_is_open(STDIN_FD));
        assert!(vfs_is_open(STDOUT_FD));
        assert!(vfs_is_open(STDERR_FD));
    }

    #[test]
    fn vfs_v02_is_open_rejects_out_of_range() {
        reset_for_tests();
        assert!(!vfs_is_open(FD_TABLE_SIZE as u64));
        assert!(!vfs_is_open(u64::MAX));
    }

    #[test]
    fn vfs_v02_open_rejects_empty_path() {
        reset_for_tests();
        assert_eq!(vfs_open(b"", O_RDONLY), Err(VfsErr::InvalidPath));
    }

    #[test]
    fn vfs_v02_open_rejects_zero_mode() {
        reset_for_tests();
        assert_eq!(vfs_open(b"/foo", 0), Err(VfsErr::InvalidMode));
    }

    #[test]
    fn vfs_v02_open_rejects_unknown_mode_bits() {
        reset_for_tests();
        // bit 7 is outside MODE_MASK
        assert_eq!(vfs_open(b"/foo", 0b1000_0000), Err(VfsErr::InvalidMode));
        // O_RDWR | unknown
        assert_eq!(
            vfs_open(b"/foo", O_RDWR | 0b1000_0000),
            Err(VfsErr::InvalidMode)
        );
    }

    #[test]
    fn vfs_v02_open_allocates_lowest_free_slot_starting_at_3() {
        reset_for_tests();
        let fd = vfs_open(b"/dev/null", O_RDONLY).expect("first open must succeed");
        assert_eq!(
            fd, 3,
            "first user-allocated fd must be slot 3 (after reserved 0/1/2)"
        );
        assert!(vfs_is_open(fd));
    }

    #[test]
    fn vfs_v02_open_returns_monotonic_lowest_free() {
        reset_for_tests();
        let a = vfs_open(b"/a", O_RDONLY).expect("a");
        let b = vfs_open(b"/b", O_WRONLY).expect("b");
        let c = vfs_open(b"/c", O_RDWR).expect("c");
        assert_eq!(a, 3);
        assert_eq!(b, 4);
        assert_eq!(c, 5);
    }

    #[test]
    fn vfs_v02_open_then_close_recycles_lowest_slot() {
        reset_for_tests();
        let a = vfs_open(b"/a", O_RDONLY).expect("a");
        let b = vfs_open(b"/b", O_RDONLY).expect("b");
        assert_eq!(a, 3);
        assert_eq!(b, 4);
        assert_eq!(vfs_close(a), Ok(()));
        // After closing slot 3, the next open must reuse it (lowest-free rule per POSIX).
        let c = vfs_open(b"/c", O_RDONLY).expect("c");
        assert_eq!(c, 3, "vfs_open must reuse the lowest free slot");
    }

    #[test]
    fn vfs_v02_close_rejects_reserved_fds() {
        reset_for_tests();
        assert_eq!(vfs_close(STDIN_FD), Err(VfsErr::InvalidOpForFd));
        assert_eq!(vfs_close(STDOUT_FD), Err(VfsErr::InvalidOpForFd));
        assert_eq!(vfs_close(STDERR_FD), Err(VfsErr::InvalidOpForFd));
    }

    #[test]
    fn vfs_v02_close_rejects_out_of_range_fd() {
        reset_for_tests();
        assert_eq!(vfs_close(FD_TABLE_SIZE as u64), Err(VfsErr::InvalidFd));
        assert_eq!(vfs_close(u64::MAX), Err(VfsErr::InvalidFd));
    }

    #[test]
    fn vfs_v02_close_rejects_free_slot() {
        reset_for_tests();
        // Slot 5 was never opened — closing it must error.
        assert_eq!(vfs_close(5), Err(VfsErr::InvalidFd));
    }

    #[test]
    fn vfs_v02_double_close_rejects() {
        reset_for_tests();
        let fd = vfs_open(b"/foo", O_RDONLY).expect("open");
        assert_eq!(vfs_close(fd), Ok(()));
        // Second close must reject (slot is now Free).
        assert_eq!(vfs_close(fd), Err(VfsErr::InvalidFd));
    }

    #[test]
    fn vfs_v02_reset_clears_user_slots_but_reserved_stay_open() {
        reset_for_tests();
        let fd = vfs_open(b"/foo", O_RDONLY).expect("open");
        assert!(vfs_is_open(fd));
        reset_for_tests();
        assert!(
            !vfs_is_open(fd),
            "user-allocated slot must be Free after reset"
        );
        assert!(vfs_is_open(STDIN_FD), "stdin remains conceptually open");
        assert!(vfs_is_open(STDOUT_FD), "stdout remains conceptually open");
        assert!(vfs_is_open(STDERR_FD), "stderr remains conceptually open");
    }

    /// Table-full exhaustion. Marked `#[ignore]` because FD_STATE is a process-global
    /// static and sibling tests call `reset_for_tests()` in parallel, freeing slots
    /// mid-fill. Run explicitly:
    ///   `cargo test --lib vfs_v02_table_full -- --ignored --test-threads=1`
    #[test]
    #[ignore]
    fn vfs_v02_table_full_after_exhausting_user_slots() {
        reset_for_tests();
        let user_slot_count = FD_TABLE_SIZE - ((RESERVED_FD_MAX as usize) + 1);
        for _ in 0..user_slot_count {
            vfs_open(b"/x", O_RDONLY).expect("user slot must allocate");
        }
        assert_eq!(vfs_open(b"/y", O_RDONLY), Err(VfsErr::TableFull));
        reset_for_tests();
    }

    #[test]
    fn vfs_v02_read_on_user_slot_still_returns_invalid_fd() {
        // v0.2 has no backing for user FDs — vfs_read on them returns InvalidFd
        // even though the slot is Open. v0.3 will wire real backing via FileOps.
        reset_for_tests();
        let fd = vfs_open(b"/foo", O_RDWR).expect("open");
        let mut buf = [0u8; 16];
        assert_eq!(vfs_read(fd, &mut buf, 8), Err(VfsErr::InvalidFd));
        reset_for_tests();
    }

    #[test]
    fn vfs_v02_write_on_user_slot_still_returns_invalid_fd() {
        reset_for_tests();
        let fd = vfs_open(b"/foo", O_RDWR).expect("open");
        assert_eq!(vfs_write(fd, b"hello"), Err(VfsErr::InvalidFd));
        reset_for_tests();
    }

    // ===== v0.3 tests (path-based binding-kind dispatch) =====

    #[test]
    fn vfs_v03_binding_kind_constants_canonical() {
        assert_eq!(FD_BIND_UNBACKED, 1);
        assert_eq!(FD_BIND_DEVNULL, 2);
        assert_eq!(FD_BIND_BUSENVELOPE, 3);
        // Backward-compat alias.
        assert_eq!(FD_STATE_OPEN, FD_BIND_UNBACKED);
    }

    #[test]
    fn vfs_v03_binding_kind_for_path_dispatch() {
        assert_eq!(binding_kind_for_path(b"/dev/null"), FD_BIND_DEVNULL);
        assert_eq!(
            binding_kind_for_path(b"/dev/bus/envelope"),
            FD_BIND_BUSENVELOPE
        );
        assert_eq!(binding_kind_for_path(b"/foo"), FD_BIND_UNBACKED);
        assert_eq!(binding_kind_for_path(b"/dev/null/extra"), FD_BIND_UNBACKED);
    }

    #[test]
    fn vfs_v03_open_dev_null_returns_devnull_binding() {
        reset_for_tests();
        let fd = vfs_open(b"/dev/null", O_RDWR).expect("open /dev/null");
        let kind = FD_STATE[fd as usize].load(Ordering::Acquire);
        assert_eq!(kind, FD_BIND_DEVNULL);
        assert!(vfs_is_open(fd));
        reset_for_tests();
    }

    #[test]
    fn vfs_v03_open_dev_bus_envelope_returns_busenvelope_binding() {
        reset_for_tests();
        let fd = vfs_open(b"/dev/bus/envelope", O_RDWR).expect("open envelope FD");
        let kind = FD_STATE[fd as usize].load(Ordering::Acquire);
        assert_eq!(kind, FD_BIND_BUSENVELOPE);
        assert!(vfs_is_open(fd));
        reset_for_tests();
    }

    #[test]
    fn vfs_v03_read_devnull_returns_eof() {
        reset_for_tests();
        let fd = vfs_open(b"/dev/null", O_RDONLY).expect("open");
        let mut buf = [0u8; 16];
        assert_eq!(vfs_read(fd, &mut buf, 8), Ok(0));
        reset_for_tests();
    }

    #[test]
    fn vfs_v03_write_devnull_discards_and_returns_buf_len() {
        reset_for_tests();
        let fd = vfs_open(b"/dev/null", O_WRONLY).expect("open");
        assert_eq!(vfs_write(fd, b"hello world"), Ok(11));
        // Empty buf is still rejected by the signature-level guard.
        assert_eq!(vfs_write(fd, b""), Err(VfsErr::InvalidBuf));
        reset_for_tests();
    }

    #[test]
    fn vfs_v03_read_busenvelope_empty_returns_would_block() {
        reset_for_tests();
        crate::envelope::reset_slot_for_tests();
        let fd = vfs_open(b"/dev/bus/envelope", O_RDWR).expect("open");
        let mut buf = [0u8; 128];
        assert_eq!(vfs_read(fd, &mut buf, 128), Err(VfsErr::WouldBlock));
        reset_for_tests();
    }

    #[test]
    fn vfs_v03_busenvelope_write_then_read_round_trip() {
        reset_for_tests();
        crate::envelope::reset_slot_for_tests();
        let fd = vfs_open(b"/dev/bus/envelope", O_RDWR).expect("open");
        let env = b"{\"from\":\"acer\",\"to\":\"liris\",\"type\":\"VFS\"}padding";
        assert!(env.len() >= 32);
        // Write enqueues; vfs_write returns buf.len() on success.
        assert_eq!(vfs_write(fd, env), Ok(env.len()));
        // Read dequeues exactly what was enqueued.
        let mut buf = [0u8; 128];
        let n = vfs_read(fd, &mut buf, 128).expect("dequeue must succeed");
        assert_eq!(n, env.len());
        assert_eq!(&buf[..n], env);
        // Slot is now empty; next read would-blocks.
        assert_eq!(vfs_read(fd, &mut buf, 128), Err(VfsErr::WouldBlock));
        crate::envelope::reset_slot_for_tests();
        reset_for_tests();
    }

    #[test]
    fn vfs_v03_busenvelope_write_oversize_returns_payload_oversize() {
        reset_for_tests();
        crate::envelope::reset_slot_for_tests();
        let fd = vfs_open(b"/dev/bus/envelope", O_WRONLY).expect("open");
        // Envelope slot is 1024 bytes; 1025 must overflow.
        let oversize = [0xAAu8; 1025];
        assert_eq!(vfs_write(fd, &oversize), Err(VfsErr::PayloadOversize));
        reset_for_tests();
    }

    #[test]
    fn vfs_v03_close_releases_devnull_binding() {
        reset_for_tests();
        let fd = vfs_open(b"/dev/null", O_RDWR).expect("open");
        assert_eq!(vfs_close(fd), Ok(()));
        // Now the slot is Free — read/write must fail.
        let mut buf = [0u8; 16];
        assert_eq!(vfs_read(fd, &mut buf, 8), Err(VfsErr::InvalidFd));
        assert_eq!(vfs_write(fd, b"x"), Err(VfsErr::InvalidFd));
        reset_for_tests();
    }

    #[test]
    fn vfs_v03_close_releases_busenvelope_binding() {
        reset_for_tests();
        let fd = vfs_open(b"/dev/bus/envelope", O_RDWR).expect("open");
        assert_eq!(vfs_close(fd), Ok(()));
        // Closed BusEnvelope FD does NOT route to envelope module.
        let mut buf = [0u8; 64];
        assert_eq!(vfs_read(fd, &mut buf, 64), Err(VfsErr::InvalidFd));
        assert_eq!(vfs_write(fd, b"x"), Err(VfsErr::InvalidFd));
        reset_for_tests();
    }
}
