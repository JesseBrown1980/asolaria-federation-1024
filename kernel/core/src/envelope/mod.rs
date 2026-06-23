//! Atomic envelope dispatch primitive · Phase-2 Step 29
//!
//! Lock-free MPMC ring buffer (capacity 16384 envelopes). Target ≥10⁵ dispatches/sec
//! with <1μs p99 enqueue/dequeue. Backpressure via gc-trigger at 75% full.
//!
//! Phase-2 v0.1: API surface + capacity constant + error enum. Real `crossbeam_queue::ArrayQueue`
//! binding lands in v0.2 once `cargo check` + bench harness available.
//!
//! Phase-3 v0.2.1: bytes-oriented `dispatch_enqueue_bytes` / `dispatch_dequeue_bytes`
//! land here so the syscall layer can wire FULL (not just half) without exposing the
//! `EnvelopeHeader` parser at the ABI boundary. v0.2.1 backs them with a no_std-safe,
//! unsafe-free, atomics-only single-slot SPSC byte queue (sufficient for kernel-side
//! round-trip; the `ArrayQueue<16384>` upgrade requires runtime init plumbing in lib.rs
//! and lands in v0.3).

use core::result::Result;
use core::sync::atomic::{AtomicU32, AtomicU8, AtomicUsize, Ordering};

/// FEDENV-v1 application-envelope contract — the omnidispatcher route-layer validator + target
/// resolution (ported from `tools/omnidispatcher/{validator,routes}.mjs`). Pure/E=0: validates and
/// classifies a route; does NOT dispatch. Sits in front of the agent registry (corrected wiring
/// order). `tools/omnidispatcher` is the node-vs-Rust parity oracle.
pub mod fedenv;

/// Ring-buffer capacity per kernel invariant (Step 29 target).
pub const ENVELOPE_QUEUE_CAPACITY: usize = 16384;

/// GC trigger threshold (75% full per BEHCS-2000-msg gulp canon scaled to kernel).
pub const ENVELOPE_GC_TRIGGER_PCT: u8 = 75;

/// Envelope dispatch errors. Mirror `SyscallErr` for callers crossing the syscall boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EnvelopeErr {
    /// Queue is full; caller should backoff and retry or trigger gc.
    QueueFull,
    /// Queue is empty; non-blocking dequeue returned nothing.
    QueueEmpty,
    /// Envelope payload exceeds maximum permitted size for tier-1 micro.
    PayloadOversize,
    /// Envelope signature verification failed (ed25519).
    SignatureInvalid,
    /// Routing target unknown to current vantage.
    RouteUnknown,
    /// Stub not yet implemented (v0.2).
    Unimplemented,
}

/// IX-700 envelope wire format · v1. Real impl serializes via CBOR (Step 30 ABI spec).
///
/// Field layout matches `BEHCS-1024 IX-700` envelope schema:
/// `{ from, to, type, verb, id, pid, ts, payload, sig }`.
#[derive(Debug, Clone)]
pub struct EnvelopeHeader<'a> {
    /// PID of sender (BEHCS-1024 60-tuple).
    pub from_pid: u64,
    /// PID of intended receiver (or broadcast room).
    pub to_pid: u64,
    /// Type tag (UPPER_SNAKE). v0.1 uses byte slice; v0.2 will use interned PID.
    pub type_tag: &'a [u8],
    /// Verb (EVT- prefix).
    pub verb: &'a [u8],
    /// Envelope ID.
    pub id: &'a [u8],
    /// Monotonic ns timestamp.
    pub ts_ns: u64,
    /// Payload bytes (caller-owned).
    pub payload: &'a [u8],
    /// ed25519 signature (64 bytes).
    pub sig: &'a [u8; 64],
}

/// Routing hint passed to dispatch primitive.
/// Maps roughly to BEHCS-1024 prefix-tree port label per `reference_brown_hilbert_cube_of_cubes_port_division.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteHint(pub u32);

impl RouteHint {
    /// Local-only dispatch (no fabric egress).
    pub const LOCAL: RouteHint = RouteHint(0);
    /// Broadcast to QUAD-FALCON-ACER-LIRIS-AETHER room.
    pub const QUAD_BROADCAST: RouteHint = RouteHint(1);
    /// Broadcast to TRIAD-FALCON-ACER-LIRIS room.
    pub const TRIAD_BROADCAST: RouteHint = RouteHint(2);
}

/// Enqueue an envelope into the kernel dispatch queue.
///
/// v0.1 stub: returns `Err(EnvelopeErr::Unimplemented)`.
/// v0.2: lock-free push via `crossbeam_queue::ArrayQueue` adapted for no_std.
/// v0.2.1: still stub at the header-typed API — the bytes-oriented sibling
/// `dispatch_enqueue_bytes` is the live FULL-wire path consumed by syscalls.
pub fn dispatch_enqueue(
    _header: &EnvelopeHeader<'_>,
    _route: RouteHint,
) -> Result<(), EnvelopeErr> {
    Err(EnvelopeErr::Unimplemented)
}

/// Non-blocking dequeue. Returns `Err(EnvelopeErr::QueueEmpty)` if queue empty.
/// v0.1 stub.
/// v0.2.1: bytes-oriented sibling `dispatch_dequeue_bytes` is the live FULL-wire path.
pub fn dispatch_dequeue(_buf: &mut [u8]) -> Result<usize, EnvelopeErr> {
    Err(EnvelopeErr::Unimplemented)
}

// ===== v0.2.1 — bytes-oriented dispatch (FULL wire for syscall layer) =====

/// Maximum bytes one envelope slot can hold in the v0.2.1 single-slot SPSC buffer.
/// Sized to comfortably cover BEHCS-1024 envelope shells (~32–1024 bytes typical).
pub const ENVELOPE_SLOT_BYTES: usize = 1024;

// Single-slot SPSC byte queue, unsafe-free, no_std-safe.
//
// State machine on `SLOT_STATE`:
//   0 = EMPTY (free for enqueue),
//   1 = WRITING (enqueuer is publishing bytes),
//   2 = FULL (a dequeuer can read),
//   3 = READING (dequeuer is consuming).
// Transitions are CAS-guarded; bytes / route / len fields are only touched while
// owning the WRITING or READING state. `AtomicU8` per-byte writes are fine — readers
// always observe a full publication via Release/Acquire fence on `SLOT_STATE`.

const STATE_EMPTY: u8 = 0;
const STATE_WRITING: u8 = 1;
const STATE_FULL: u8 = 2;
const STATE_READING: u8 = 3;

// Repeat-init helper for the byte cell array (no const-fn array-of-AtomicU8 builder
// in stable until later — using initializer expression instead).
#[allow(clippy::declare_interior_mutable_const)]
const ZERO_BYTE: AtomicU8 = AtomicU8::new(0);

static SLOT_STATE: AtomicU8 = AtomicU8::new(STATE_EMPTY);
static SLOT_LEN: AtomicUsize = AtomicUsize::new(0);
static SLOT_ROUTE: AtomicU32 = AtomicU32::new(0);
static SLOT_BYTES: [AtomicU8; ENVELOPE_SLOT_BYTES] = [ZERO_BYTE; ENVELOPE_SLOT_BYTES];
static SLOT_DEPTH: AtomicUsize = AtomicUsize::new(0);

/// FULL-wire bytes enqueue. Consumed by `crate::syscall::sys_envelope_send`.
///
/// - `env`: raw envelope bytes (caller is responsible for size lower-bound checks).
/// - `route_hint`: raw u32 mirroring `RouteHint(u32)`.
///
/// Returns:
/// - `Ok(())` on successful publish,
/// - `Err(EnvelopeErr::PayloadOversize)` if `env.len() > ENVELOPE_SLOT_BYTES`,
/// - `Err(EnvelopeErr::QueueFull)` if the slot is already occupied.
pub fn dispatch_enqueue_bytes(env: &[u8], route_hint: u32) -> Result<(), EnvelopeErr> {
    if env.len() > ENVELOPE_SLOT_BYTES {
        return Err(EnvelopeErr::PayloadOversize);
    }
    // Claim the slot: EMPTY -> WRITING.
    if SLOT_STATE
        .compare_exchange(
            STATE_EMPTY,
            STATE_WRITING,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        return Err(EnvelopeErr::QueueFull);
    }
    // Publish bytes (one byte at a time — each store is atomic; visibility is
    // synchronized by the Release on SLOT_STATE below).
    for (i, b) in env.iter().enumerate() {
        SLOT_BYTES[i].store(*b, Ordering::Relaxed);
    }
    SLOT_LEN.store(env.len(), Ordering::Relaxed);
    SLOT_ROUTE.store(route_hint, Ordering::Relaxed);
    SLOT_DEPTH.store(1, Ordering::Relaxed);
    // Publish: WRITING -> FULL with Release so dequeuers see bytes + len + route.
    SLOT_STATE.store(STATE_FULL, Ordering::Release);
    Ok(())
}

/// FULL-wire bytes dequeue. Consumed by `crate::syscall::sys_envelope_recv`.
///
/// - `buf`: destination buffer; caller-provided.
/// - `max_wait_ns`: best-effort upper bound on wait time. v0.2.1 has no kernel
///   timer — we spin a bounded number of poll iterations roughly proportional to
///   `max_wait_ns / 1000` (1μs per spin assumption). 0 means non-blocking.
///
/// Returns:
/// - `Ok(bytes_read)` on success,
/// - `Err(EnvelopeErr::QueueEmpty)` if the slot is empty and the bounded wait expires,
/// - `Err(EnvelopeErr::PayloadOversize)` if the slot has more bytes than `buf` can hold.
pub fn dispatch_dequeue_bytes(buf: &mut [u8], max_wait_ns: u64) -> Result<usize, EnvelopeErr> {
    // Bounded spin: estimate ~1 iteration per nanosecond worst case; cap to avoid
    // unbounded busy-wait. Without a kernel timer this is best-effort only.
    let spin_budget: u64 = if max_wait_ns == 0 {
        1
    } else {
        max_wait_ns.min(1_000_000)
    };
    let mut tries: u64 = 0;
    loop {
        // Claim the slot: FULL -> READING.
        match SLOT_STATE.compare_exchange(
            STATE_FULL,
            STATE_READING,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                let n = SLOT_LEN.load(Ordering::Relaxed);
                if n > buf.len() {
                    // Restore FULL so the next caller with a bigger buf can read.
                    SLOT_STATE.store(STATE_FULL, Ordering::Release);
                    return Err(EnvelopeErr::PayloadOversize);
                }
                for i in 0..n {
                    buf[i] = SLOT_BYTES[i].load(Ordering::Relaxed);
                }
                SLOT_LEN.store(0, Ordering::Relaxed);
                SLOT_DEPTH.store(0, Ordering::Relaxed);
                SLOT_STATE.store(STATE_EMPTY, Ordering::Release);
                return Ok(n);
            }
            Err(_) => {
                tries += 1;
                if tries >= spin_budget {
                    return Err(EnvelopeErr::QueueEmpty);
                }
                core::hint::spin_loop();
            }
        }
    }
}

/// v0.2.1: reset the single-slot byte queue. Test-only — public to allow
/// `tests::reset_slot()` in the syscall test module to avoid order-dependence.
#[doc(hidden)]
pub fn reset_slot_for_tests() {
    SLOT_STATE.store(STATE_EMPTY, Ordering::Release);
    SLOT_LEN.store(0, Ordering::Relaxed);
    SLOT_ROUTE.store(0, Ordering::Relaxed);
    SLOT_DEPTH.store(0, Ordering::Relaxed);
}

/// Current queue depth. v0.2.1: reads `SLOT_DEPTH` (0 or 1).
pub fn queue_depth() -> usize {
    SLOT_DEPTH.load(Ordering::Relaxed)
}

/// Returns `true` if depth ≥ `ENVELOPE_GC_TRIGGER_PCT`% of capacity.
pub fn should_trigger_gc() -> bool {
    let depth = queue_depth();
    let threshold = (ENVELOPE_QUEUE_CAPACITY * ENVELOPE_GC_TRIGGER_PCT as usize) / 100;
    depth >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_is_16384() {
        assert_eq!(ENVELOPE_QUEUE_CAPACITY, 16384);
    }

    #[test]
    fn gc_threshold_is_75pct() {
        assert_eq!(ENVELOPE_GC_TRIGGER_PCT, 75);
        // 75% of 16384 = 12288
        assert_eq!(
            (ENVELOPE_QUEUE_CAPACITY * ENVELOPE_GC_TRIGGER_PCT as usize) / 100,
            12288
        );
    }

    #[test]
    fn route_hints_have_canonical_values() {
        assert_eq!(RouteHint::LOCAL.0, 0);
        assert_eq!(RouteHint::QUAD_BROADCAST.0, 1);
        assert_eq!(RouteHint::TRIAD_BROADCAST.0, 2);
    }

    #[test]
    fn stub_enqueue_returns_unimplemented() {
        let sig = [0u8; 64];
        let header = EnvelopeHeader {
            from_pid: 0,
            to_pid: 0,
            type_tag: b"TEST",
            verb: b"EVT-TEST",
            id: b"test-0",
            ts_ns: 0,
            payload: b"",
            sig: &sig,
        };
        assert_eq!(
            dispatch_enqueue(&header, RouteHint::LOCAL),
            Err(EnvelopeErr::Unimplemented)
        );
    }

    #[test]
    fn queue_depth_starts_empty() {
        assert_eq!(queue_depth(), 0);
        assert!(!should_trigger_gc());
    }
}
