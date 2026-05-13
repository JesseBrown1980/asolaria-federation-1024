//! Frame allocator · supports `sys_mmap` / `sys_munmap`
//!
//! Anchor PID: ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11
//!
//! v0.1 (this file): virtual-address **tracking** allocator. Hands out unique non-null
//! `*mut u8` pointers from a synthetic address range starting at `VIRTUAL_BASE_START`
//! (0x1000). No physical memory is backed — pointers MUST NOT be dereferenced in
//! kernel-core. They serve as **handles** for `sys_mmap` / `sys_munmap` round-trip
//! semantics until v0.2 plumbs UEFI memory-map regions from boot info.
//!
//! Rationale: kernel-core is `#![forbid(unsafe_code)]` per the API-layer invariant
//! (lib.rs:15). Returning real backing storage requires either `static mut` (unsafe)
//! or a cell-of-atomics layout that distorts pointer arithmetic semantics. Phase-3
//! v0.1 sidesteps this by tracking virtual addresses only; the real heap region
//! is registered by the boot crate in v0.2 via a callback `register_frame_region`
//! that converts virtual handles to physical mapping.
//!
//! API contract:
//! - `alloc_pages(len)` rounds `len` up to `PAGE_BYTES` and bumps `NEXT_VIRTUAL_BASE`.
//!   Returns a stable non-null `*mut u8` or `Err(PoolExhausted)`.
//! - `free_pages(ptr, len)` validates `ptr` is within the synthetic range, page-aligned,
//!   and `len > 0`. v0.1 is a **no-op** on success (bump allocator doesn't reclaim);
//!   v0.2 will track via a bitmap free-list and recycle pages.
//!
//! The allocator state is process-global static atomics. Thread-safe across
//! parallel callers via single CAS loop on `NEXT_VIRTUAL_BASE`.

use core::sync::atomic::{AtomicUsize, Ordering};

/// Page size — standard x86_64 4KB page (also matches ARM64 default).
pub const PAGE_BYTES: usize = 4096;

/// Total pages in the v0.1 synthetic virtual range (16384 pages × 4KB = 64 MB).
pub const POOL_PAGES: usize = 16384;

/// Total bytes in the v0.1 synthetic virtual range.
pub const POOL_BYTES: usize = POOL_PAGES * PAGE_BYTES;

/// Synthetic virtual base — first allocation returns this address. Chosen well above
/// the typical x86_64 user-space null-pointer guard region and below any plausible
/// kernel-text load address.
pub const VIRTUAL_BASE_START: usize = 0x1000;

/// One-past-end of the synthetic virtual range.
pub const VIRTUAL_BASE_END: usize = VIRTUAL_BASE_START + POOL_BYTES;

/// Frame-allocator errors. Mirror at the syscall boundary via `SyscallErr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrameErr {
    /// Requested allocation would exceed `POOL_BYTES`.
    PoolExhausted,
    /// `len` was zero (zero-byte alloc/free is a no-op trap).
    InvalidLen,
    /// `ptr` was null OR outside the synthetic range `[VIRTUAL_BASE_START, VIRTUAL_BASE_END)`.
    InvalidPointer,
    /// `ptr` was inside the range but not page-aligned (`ptr % PAGE_BYTES != 0`).
    NotPageAligned,
    /// Reserved for v0.2 — bitmap free-list operations not yet implemented.
    Unimplemented,
}

/// Atomic watermark — virtual address of the next free byte.
/// Starts at `VIRTUAL_BASE_START`; first `alloc_pages` returns this value.
static NEXT_VIRTUAL_BASE: AtomicUsize = AtomicUsize::new(VIRTUAL_BASE_START);

/// Allocate at least `len` bytes (rounded up to the nearest page).
///
/// Returns a stable non-null `*mut u8` whose numeric value falls within
/// `[VIRTUAL_BASE_START, VIRTUAL_BASE_END)` and is `PAGE_BYTES`-aligned.
///
/// Errors:
/// - `InvalidLen` if `len == 0`.
/// - `PoolExhausted` if the request would overflow the synthetic pool.
pub fn alloc_pages(len: usize) -> Result<*mut u8, FrameErr> {
    if len == 0 {
        return Err(FrameErr::InvalidLen);
    }
    let pages_needed = (len + PAGE_BYTES - 1) / PAGE_BYTES;
    let bytes_needed = pages_needed * PAGE_BYTES;
    let mut current = NEXT_VIRTUAL_BASE.load(Ordering::Relaxed);
    loop {
        let new_next = current.saturating_add(bytes_needed);
        if new_next > VIRTUAL_BASE_END {
            return Err(FrameErr::PoolExhausted);
        }
        match NEXT_VIRTUAL_BASE.compare_exchange(
            current,
            new_next,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return Ok(current as *mut u8),
            Err(actual) => current = actual,
        }
    }
}

/// Validate a pair `(ptr, len)` originated from `alloc_pages` and accept the release.
///
/// v0.1 is a no-op on success — bump-style allocation cannot recycle.
/// v0.2 will move this to a bitmap free-list.
///
/// Errors:
/// - `InvalidPointer` if `ptr` is null OR outside `[VIRTUAL_BASE_START, VIRTUAL_BASE_END)`.
/// - `InvalidLen` if `len == 0`.
/// - `NotPageAligned` if `ptr` is in range but not page-aligned.
pub fn free_pages(ptr: *mut u8, len: usize) -> Result<(), FrameErr> {
    if ptr.is_null() {
        return Err(FrameErr::InvalidPointer);
    }
    if len == 0 {
        return Err(FrameErr::InvalidLen);
    }
    let p = ptr as usize;
    if p < VIRTUAL_BASE_START || p >= VIRTUAL_BASE_END {
        return Err(FrameErr::InvalidPointer);
    }
    if p % PAGE_BYTES != 0 {
        return Err(FrameErr::NotPageAligned);
    }
    Ok(())
}

/// Bytes currently consumed by `alloc_pages` calls (test/probe).
pub fn used_bytes() -> usize {
    NEXT_VIRTUAL_BASE.load(Ordering::Relaxed) - VIRTUAL_BASE_START
}

/// Bytes still available in the synthetic pool (test/probe).
pub fn free_bytes() -> usize {
    VIRTUAL_BASE_END - NEXT_VIRTUAL_BASE.load(Ordering::Relaxed)
}

/// Reset the allocator watermark to `VIRTUAL_BASE_START`. TEST USE ONLY.
/// Lets tests run in any order without bleeding state into each other.
#[cfg(test)]
pub fn reset_for_tests() {
    NEXT_VIRTUAL_BASE.store(VIRTUAL_BASE_START, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_zero_len_returns_invalid_len() {
        reset_for_tests();
        assert_eq!(alloc_pages(0), Err(FrameErr::InvalidLen));
    }

    #[test]
    fn alloc_one_byte_rounds_to_full_page() {
        reset_for_tests();
        let p = alloc_pages(1).expect("first single-byte alloc must succeed");
        assert_eq!(
            p as usize, VIRTUAL_BASE_START,
            "first alloc returns virtual base"
        );
        assert_eq!(
            used_bytes(),
            PAGE_BYTES,
            "one-byte request consumes one full page"
        );
    }

    #[test]
    fn alloc_returns_page_aligned_pointers() {
        reset_for_tests();
        let p1 = alloc_pages(8000).expect("alloc must succeed");
        let p2 = alloc_pages(4096).expect("alloc must succeed");
        assert_eq!((p1 as usize) % PAGE_BYTES, 0, "p1 must be page-aligned");
        assert_eq!((p2 as usize) % PAGE_BYTES, 0, "p2 must be page-aligned");
        assert!((p2 as usize) > (p1 as usize), "p2 must come after p1");
    }

    #[test]
    fn alloc_exhausting_pool_returns_pool_exhausted() {
        reset_for_tests();
        let p = alloc_pages(POOL_BYTES).expect("one-shot pool-sized alloc must succeed");
        assert_eq!(p as usize, VIRTUAL_BASE_START);
        assert_eq!(used_bytes(), POOL_BYTES);
        assert_eq!(alloc_pages(1), Err(FrameErr::PoolExhausted));
    }

    #[test]
    fn alloc_oversized_first_request_returns_pool_exhausted() {
        reset_for_tests();
        assert_eq!(alloc_pages(POOL_BYTES + 1), Err(FrameErr::PoolExhausted));
    }

    #[test]
    fn free_null_pointer_returns_invalid_pointer() {
        assert_eq!(
            free_pages(core::ptr::null_mut(), PAGE_BYTES),
            Err(FrameErr::InvalidPointer)
        );
    }

    #[test]
    fn free_zero_len_returns_invalid_len() {
        // Non-null in-range pointer + zero len -> InvalidLen.
        assert_eq!(
            free_pages(VIRTUAL_BASE_START as *mut u8, 0),
            Err(FrameErr::InvalidLen)
        );
    }

    #[test]
    fn free_out_of_range_pointer_returns_invalid_pointer() {
        // Below the range.
        assert_eq!(
            free_pages(0x800 as *mut u8, PAGE_BYTES),
            Err(FrameErr::InvalidPointer)
        );
        // Above the range.
        assert_eq!(
            free_pages(VIRTUAL_BASE_END as *mut u8, PAGE_BYTES),
            Err(FrameErr::InvalidPointer)
        );
    }

    #[test]
    fn free_unaligned_in_range_pointer_returns_not_page_aligned() {
        // VIRTUAL_BASE_START + 1 is in range but not page-aligned.
        assert_eq!(
            free_pages((VIRTUAL_BASE_START + 1) as *mut u8, PAGE_BYTES),
            Err(FrameErr::NotPageAligned)
        );
    }

    #[test]
    fn alloc_then_free_round_trip_succeeds() {
        reset_for_tests();
        let p = alloc_pages(8192).expect("alloc must succeed");
        assert_eq!(
            free_pages(p, 8192),
            Ok(()),
            "free of fresh alloc must succeed (no-op)"
        );
    }
}
