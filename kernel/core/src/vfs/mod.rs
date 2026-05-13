//! VFS file-descriptor primitive · supports `sys_read` / `sys_write`
//!
//! Anchor PID: ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11
//!
//! v0.1 (this file): **routing-by-fd-value** scaffold. Three reserved FDs
//! (`STDIN`/`STDOUT`/`STDERR`) are pre-defined; all other FD values reject as
//! `InvalidFd`. No real I/O backing — `vfs_read` returns `Ok(0)` (EOF) on stdin
//! and `vfs_write` returns `Ok(buf.len())` on stdout/stderr without actually
//! emitting bytes anywhere. v0.2 will add:
//!   - A 256-entry FD slot table with atomic state per slot
//!   - `vfs_open(path, mode)` + `vfs_close(fd)` (when sys_open/sys_close are
//!     ratified as new syscalls, requiring tier-2 cosign per REPO_LAW Inv. 9)
//!   - Real I/O backing via a host-callback registered from the boot crate
//!     (analogous to the frame_alloc v0.2 `register_frame_region` plan)
//!
//! Rationale (mirrors frame_alloc): kernel-core is `#![forbid(unsafe_code)]`.
//! Real byte-buffer I/O requires either `static mut` or atomic-byte arrays
//! that distort I/O semantics; v0.1 sidesteps with pure routing. Every call
//! lands a definite `Ok` / `Err` — never `Unimplemented` — matching the FULL-wire
//! discipline of the surrounding syscall surface.

/// Reserved FD: standard input. Pre-opened; `vfs_read` returns `Ok(0)` (EOF).
pub const STDIN_FD: u64 = 0;

/// Reserved FD: standard output. Pre-opened; `vfs_write` accepts with `Ok(buf.len())`.
pub const STDOUT_FD: u64 = 1;

/// Reserved FD: standard error. Pre-opened; `vfs_write` accepts with `Ok(buf.len())`.
pub const STDERR_FD: u64 = 2;

/// Highest reserved FD value in v0.1. v0.2 raises this once the FD table lands.
pub const RESERVED_FD_MAX: u64 = STDERR_FD;

/// VFS errors. Mirror at the syscall boundary via `SyscallErr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VfsErr {
    /// FD value is not in the v0.1 reserved range and no FD table backs it.
    InvalidFd,
    /// FD is reserved but the operation is invalid for it (e.g. read on stdout).
    InvalidOpForFd,
    /// Buffer is empty when bytes are required (e.g. write with `buf.is_empty()`).
    InvalidBuf,
    /// `len > buf.len()` — caller passed a length larger than the destination.
    LenExceedsBuf,
    /// FD table full (v0.2 — n/a in v0.1).
    TableFull,
    /// Reserved for v0.2 operations not yet implemented.
    Unimplemented,
}

/// Read up to `len` bytes from `fd` into `buf`.
///
/// v0.1 semantics:
/// - `fd == STDIN_FD` (0): returns `Ok(0)` — EOF (no input plumbed yet)
/// - `fd == STDOUT_FD || fd == STDERR_FD`: returns `Err(InvalidOpForFd)`
/// - any other `fd`: returns `Err(InvalidFd)`
///
/// Errors:
/// - `LenExceedsBuf` if `len > buf.len()` (signature-level guard; matches the
///   sys_read input-validation floor)
/// - `InvalidOpForFd` for stdout/stderr
/// - `InvalidFd` for all other fd values
pub fn vfs_read(fd: u64, buf: &mut [u8], len: usize) -> Result<usize, VfsErr> {
    if len > buf.len() {
        return Err(VfsErr::LenExceedsBuf);
    }
    match fd {
        STDIN_FD => Ok(0), // EOF — no input source in v0.1
        STDOUT_FD | STDERR_FD => Err(VfsErr::InvalidOpForFd),
        _ => Err(VfsErr::InvalidFd),
    }
}

/// Write `buf` to `fd`. Returns bytes written.
///
/// v0.1 semantics:
/// - `fd == STDOUT_FD || fd == STDERR_FD`: returns `Ok(buf.len())` — bytes
///   accepted (no real emission in v0.1; v0.2 wires host-callback)
/// - `fd == STDIN_FD`: returns `Err(InvalidOpForFd)`
/// - any other `fd`: returns `Err(InvalidFd)`
///
/// Errors:
/// - `InvalidBuf` if `buf.is_empty()` (signature-level guard; matches the
///   sys_write input-validation floor)
/// - `InvalidOpForFd` for stdin
/// - `InvalidFd` for all other fd values
pub fn vfs_write(fd: u64, buf: &[u8]) -> Result<usize, VfsErr> {
    if buf.is_empty() {
        return Err(VfsErr::InvalidBuf);
    }
    match fd {
        STDOUT_FD | STDERR_FD => Ok(buf.len()),
        STDIN_FD => Err(VfsErr::InvalidOpForFd),
        _ => Err(VfsErr::InvalidFd),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
