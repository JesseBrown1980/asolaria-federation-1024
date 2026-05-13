//! Syscall surface · Phase-2 Step 25
//!
//! CANONICAL 16-SYSCALL SURFACE — additions require tier-2 cosign per REPO_LAW Invariant 9.
//!
//! All syscalls return `Result<T, SyscallErr>`. Stub bodies return `Err(SyscallErr::Unimplemented)`
//! until phase-3+ wires real handlers.

use core::result::Result;

/// Canonical syscall error type. Envelopes serialize errors via `payload.err` per IX-700.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SyscallErr {
    /// Stub not yet implemented.
    Unimplemented,
    /// Invalid file descriptor or PID handle.
    Invalid,
    /// Operation would block; retry or use timeout variant.
    WouldBlock,
    /// Permission denied per tier-aware access policy (REPO_LAW Invariant 5).
    PermissionDenied,
    /// Hookwall blocked the operation (REPO_LAW Invariant 2).
    HookwallBlock,
    /// Resource exhausted (memory, file handles, envelope queue).
    Exhausted,
    /// Cosign-chain rejected the row.
    CosignReject,
}

/// File-descriptor or PID handle (BEHCS-1024 60-tuple).
pub type Handle = u64;

/// Monotonic nanoseconds since boot.
pub type NanoTime = u64;

/// Verdict emitted by hookwall pre/post hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookwallVerdict {
    Proceed,
    Hold,
    Block,
}

/// Seven tier-classes per REPO_LAW Invariant 5 + dashboard constitutional directive.
///
/// Cycle-72 extension: liris dashboard Drive Registry showed `SOVEREIGNTY` as a
/// 7th tier above `Secret` — "operator-physical-only / NEVER agent-driven".
///
/// **Per-vantage instance asymmetry (operator clarification cycle-72):**
/// The `Sovereignty` tier-CLASS is universal across all federation vantages, but
/// the drive INSTANCES are vantage-specific. Liris's sovereignty drives
/// (USB-SOVLINUX-2TB, D-DRIVE-LIRIS) physically live on liris hardware. Acer has
/// different sovereignty instances. **Cross-vantage access requires physical
/// transport (air-gap, human-mediated rehydration)** — this is the operator's
/// stated "one of the key problems" of the federation architecture.
///
/// The tier semantics (no quintuple-cosign override; no agent access; physical
/// boundary) are canon federation-wide; the per-vantage drive-list mapping is
/// dashboard-rendered context, not kernel canon.
///
/// Ordering (lowest → highest):
///   Public < Restricted < Stealth < Hidden < Shadow < Secret < Sovereignty
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessTier {
    Public,
    Restricted,
    Stealth,
    Hidden,
    Shadow,
    Secret,
    /// Operator-physical-only · NEVER agent-driven. No quintuple-cosign override.
    /// Per-vantage instance: liris has USB-SOVLINUX-2TB + D-DRIVE-LIRIS; acer's
    /// sovereignty drives are different (cross-vantage = physical transport).
    Sovereignty,
}

// ===== 16 canonical syscalls =====

/// 1. Read `len` bytes from handle `fd` into `buf`. Returns bytes read.
///
/// Phase-3 v0.3.2: FULL WIRE to `crate::vfs::vfs_read`. Keeps the `len > buf.len()`
/// signature-level guard (buffer overrun would be UB), then routes by fd value:
/// STDIN_FD (0) returns Ok(0) (EOF; no input plumbed in v0.1); STDOUT/STDERR
/// return InvalidOpForFd; all other fds return InvalidFd. Errors mapped at the
/// syscall boundary: any `VfsErr` → `SyscallErr::Invalid`.
pub fn sys_read(fd: Handle, buf: &mut [u8], len: usize) -> Result<usize, SyscallErr> {
    if len > buf.len() {
        return Err(SyscallErr::Invalid);
    }
    match crate::vfs::vfs_read(fd, buf, len) {
        Ok(n) => Ok(n),
        Err(_) => Err(SyscallErr::Invalid),
    }
}

/// 2. Write `buf` to handle `fd`. Returns bytes written.
///
/// Phase-3 v0.3.2: FULL WIRE to `crate::vfs::vfs_write`. Keeps the empty-buf
/// signature-level guard, then routes by fd value: STDOUT_FD (1) and STDERR_FD
/// (2) accept with Ok(buf.len()); STDIN_FD returns InvalidOpForFd; all other
/// fds return InvalidFd. Errors mapped at the syscall boundary: any `VfsErr`
/// → `SyscallErr::Invalid`.
pub fn sys_write(fd: Handle, buf: &[u8]) -> Result<usize, SyscallErr> {
    if buf.is_empty() {
        return Err(SyscallErr::Invalid);
    }
    match crate::vfs::vfs_write(fd, buf) {
        Ok(n) => Ok(n),
        Err(_) => Err(SyscallErr::Invalid),
    }
}

/// 3. Execute a BEHCS-1024 envelope. NOT a path — envelopes are the dispatch unit.
///
/// Phase-3 v0.3.0: FULL WIRE to envelope module. Keeps the input-validation floor
/// (rejects envelopes shorter than `MIN_ENVELOPE_BYTES` = 32, same lower bound
/// `sys_envelope_send` enforces), then dispatches via
/// `crate::envelope::dispatch_enqueue_bytes(envelope_bytes, 0)` (route_hint 0 = local
/// default; same convention as `kernel/boot/src/init.rs::ROUTE_HINT_LOCAL`). On
/// success, mints a fresh monotonic handle from `EXEC_HANDLE_NEXT` (starts at 1;
/// 0 reserved as "no handle"). Errors mapped at the syscall boundary:
/// `QueueFull` → `Exhausted`, `PayloadOversize` → `Invalid`, others → `Invalid`.
pub fn sys_exec(envelope_bytes: &[u8]) -> Result<Handle, SyscallErr> {
    /// Lower bound: minimal envelope JSON shell needs at least this many bytes.
    const MIN_ENVELOPE_BYTES: usize = 32;
    if envelope_bytes.len() < MIN_ENVELOPE_BYTES {
        return Err(SyscallErr::Invalid);
    }
    use core::sync::atomic::{AtomicU64, Ordering};
    static EXEC_HANDLE_NEXT: AtomicU64 = AtomicU64::new(1);
    match crate::envelope::dispatch_enqueue_bytes(envelope_bytes, 0) {
        Ok(()) => Ok(EXEC_HANDLE_NEXT.fetch_add(1, Ordering::Relaxed)),
        Err(crate::envelope::EnvelopeErr::QueueFull) => Err(SyscallErr::Exhausted),
        Err(crate::envelope::EnvelopeErr::PayloadOversize) => Err(SyscallErr::Invalid),
        Err(_) => Err(SyscallErr::Invalid),
    }
}

/// 4. Process clone. Returns child PID on parent, 0 on child.
///
/// Phase-3 v0.2.4: FULL WIRE to `agent_runtime::spawn_child_agent` (in-memory registry;
/// CoW page-table cloning deferred to v0.2). Each successful call mints a fresh child
/// handle from the module-level atomic counter and returns it as a `Handle`. When the
/// in-memory registry saturates at `AGENT_REGISTRY_MAX`, returns `Err(SyscallErr::Exhausted)`;
/// any other agent_runtime fault maps to `Err(SyscallErr::Invalid)`.
pub fn sys_fork() -> Result<Handle, SyscallErr> {
    match crate::agent_runtime::spawn_child_agent() {
        Ok(child_handle) => Ok(child_handle),
        Err(crate::agent_runtime::AgentErr::RegistryFull) => Err(SyscallErr::Exhausted),
        Err(_) => Err(SyscallErr::Invalid),
    }
}

/// 5. Exit current process with status code.
pub fn sys_exit(_status: i32) -> ! {
    // Diverging stub — real impl never returns.
    loop {
        core::hint::spin_loop();
    }
}

/// 6. Map anonymous pages. v0.1 supports anonymous only.
///
/// Phase-3 v0.3.1: FULL WIRE to `crate::frame_alloc::alloc_pages`. Keeps the
/// input-validation floor (rejects `len == 0` and `prot == 0`), then dispatches
/// to the frame allocator. v0.1 of the frame allocator hands out **virtual** addresses
/// from a synthetic `[0x1000, 0x1000 + 64MB)` range; pointers MUST NOT be dereferenced
/// in kernel-core until v0.2 plumbs UEFI memory-map regions. Errors mapped at the
/// syscall boundary: `PoolExhausted` → `Exhausted`, others → `Invalid`.
pub fn sys_mmap(len: usize, prot: u32) -> Result<*mut u8, SyscallErr> {
    if len == 0 {
        return Err(SyscallErr::Invalid);
    }
    if prot == 0 {
        return Err(SyscallErr::Invalid);
    }
    match crate::frame_alloc::alloc_pages(len) {
        Ok(p) => Ok(p),
        Err(crate::frame_alloc::FrameErr::PoolExhausted) => Err(SyscallErr::Exhausted),
        Err(_) => Err(SyscallErr::Invalid),
    }
}

/// 7. Unmap previously-mapped region.
///
/// Phase-3 v0.3.1: FULL WIRE to `crate::frame_alloc::free_pages`. Keeps the
/// input-validation floor (rejects null pointer + zero-length), then dispatches
/// to the frame allocator. v0.1 free is a no-op on success (bump allocator does
/// not reclaim); ptr must be in the synthetic virtual range AND page-aligned.
/// Errors mapped at the syscall boundary: any `FrameErr` → `Invalid`.
pub fn sys_munmap(addr: *mut u8, len: usize) -> Result<(), SyscallErr> {
    if addr.is_null() {
        return Err(SyscallErr::Invalid);
    }
    if len == 0 {
        return Err(SyscallErr::Invalid);
    }
    match crate::frame_alloc::free_pages(addr, len) {
        Ok(()) => Ok(()),
        Err(_) => Err(SyscallErr::Invalid),
    }
}

/// 8. Monotonic nanoseconds since boot.
///
/// Phase-3 v0.1.5: returns a monotonic AtomicU64 counter (NOT real-time wall-clock).
/// Each call returns counter + 1 atomically. Real RDTSC (x86_64) / CNTVCT_EL0 (ARM64)
/// platform timer binding lands in v0.2 once boot info exposes timer freq.
/// Property held: strictly monotonic, no duplicate values across concurrent calls.
pub fn sys_time() -> Result<NanoTime, SyscallErr> {
    use core::sync::atomic::{AtomicU64, Ordering};
    static MONOTONIC_COUNTER: AtomicU64 = AtomicU64::new(0);
    let v = MONOTONIC_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(v)
}

/// 9. BEHCS-1024 PID of caller.
///
/// Phase-3 v0.1.4: returns u64 derived from `crate::FEDERATION_ANCHOR_PID` sha256 first-8-bytes (big-endian).
/// Real per-task PID singleton lands in v0.2 once kernel scheduler tracks current task.
/// Until then, every caller gets the federation anchor PID — sufficient for v0.1 stubs.
pub fn sys_pid_current() -> Result<Handle, SyscallErr> {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(crate::FEDERATION_ANCHOR_PID.as_bytes());
    let digest = h.finalize();
    let mut v: u64 = 0;
    for i in 0..8 {
        v = (v << 8) | (digest[i] as u64);
    }
    Ok(v)
}

/// 10. Dispatch envelope through the kernel bus (Step 29 ring buffer).
///
/// Phase-3 v0.2.1: FULL WIRE to envelope module. Keeps the half-wire input-validation
/// floor (rejects envelopes shorter than `MIN_ENVELOPE_BYTES` = 32 — matches the same
/// lower bound `sys_exec` enforces), then dispatches to
/// `crate::envelope::dispatch_enqueue_bytes`. Errors are mapped at the syscall boundary:
/// `QueueFull` → `Exhausted`, `PayloadOversize` → `Invalid`, all other dispatch errors
/// → `Invalid`. Promotes the syscall from Unimplemented to a real round-trippable wire.
pub fn sys_envelope_send(env: &[u8], route_hint: u32) -> Result<(), SyscallErr> {
    /// Lower bound: matches sys_exec — minimal BEHCS-1024 envelope shell.
    const MIN_ENVELOPE_BYTES: usize = 32;
    if env.len() < MIN_ENVELOPE_BYTES {
        return Err(SyscallErr::Invalid);
    }
    match crate::envelope::dispatch_enqueue_bytes(env, route_hint) {
        Ok(()) => Ok(()),
        Err(crate::envelope::EnvelopeErr::QueueFull) => Err(SyscallErr::Exhausted),
        Err(crate::envelope::EnvelopeErr::PayloadOversize) => Err(SyscallErr::Invalid),
        // All other dispatch errors (SignatureInvalid, RouteUnknown, Unimplemented,
        // QueueEmpty — the latter not reachable on enqueue) map to Invalid at the
        // syscall boundary per IX-700 error-class shrink rule.
        Err(_) => Err(SyscallErr::Invalid),
    }
}

/// 11. Blocking-with-timeout envelope receive.
///
/// Phase-3 v0.2.2: FULL WIRE to envelope module. Keeps the half-wire empty-buf
/// guard (nowhere to write — would unconditionally truncate), then calls
/// `crate::envelope::dispatch_dequeue_bytes(buf, max_wait_ns)`. Errors are mapped:
/// `QueueEmpty` (bounded-wait expired) → `WouldBlock`, `PayloadOversize` (slot
/// holds more bytes than `buf` can fit) → `Invalid`, all other dispatch errors
/// → `Invalid`.
pub fn sys_envelope_recv(buf: &mut [u8], max_wait_ns: NanoTime) -> Result<usize, SyscallErr> {
    if buf.is_empty() {
        return Err(SyscallErr::Invalid);
    }
    match crate::envelope::dispatch_dequeue_bytes(buf, max_wait_ns) {
        Ok(n) => Ok(n),
        Err(crate::envelope::EnvelopeErr::QueueEmpty) => Err(SyscallErr::WouldBlock),
        Err(crate::envelope::EnvelopeErr::PayloadOversize) => Err(SyscallErr::Invalid),
        Err(_) => Err(SyscallErr::Invalid),
    }
}

/// 12. Pre-exec hookwall hook (Invariant 2).
///
/// Phase-3 v0.1.2: wired to `crate::hookwall::hookwall_pre`. Synthesizes a `HookContext`
/// from envelope bytes prefix: byte[0]=slot, byte[1]=syscall_no, default tier=Micro,
/// target_tier=Public, caller_pid=0 (kernel-state PID singleton lands in v0.2).
pub fn sys_hookwall_pre(env: &[u8]) -> Result<HookwallVerdict, SyscallErr> {
    if env.len() < 2 {
        return Err(SyscallErr::Invalid);
    }
    let ctx = crate::hookwall::HookContext {
        slot: env[0],
        syscall_no: env[1],
        caller_pid: 0,
        tier: crate::hookwall::HookTier::Micro,
        target_tier: AccessTier::Public,
    };
    crate::hookwall::hookwall_pre(&ctx)
}

/// 13. Post-exec hookwall hook with verdict.
///
/// Phase-3 v0.1.6: wired to `crate::hookwall::hookwall_post`. Synthesizes the same
/// `HookContext` shape as `sys_hookwall_pre` (byte[0]=slot, byte[1]=syscall_no,
/// default tier=Micro, target_tier=Public, caller_pid=0). Records verdict locally.
/// Cosign-chain append lands when sys_cosign_append wires (kernel-state singleton needed).
pub fn sys_hookwall_post(env: &[u8], verdict: HookwallVerdict) -> Result<(), SyscallErr> {
    if env.len() < 2 {
        return Err(SyscallErr::Invalid);
    }
    let ctx = crate::hookwall::HookContext {
        slot: env[0],
        syscall_no: env[1],
        caller_pid: 0,
        tier: crate::hookwall::HookTier::Micro,
        target_tier: AccessTier::Public,
    };
    crate::hookwall::hookwall_post(&ctx, verdict)
}

/// 14. Append a row to the cosign-chain (Invariant 4 · append-only).
///
/// Phase-3 v0.2.3: FULL WIRE to cosign_chain in-memory append (ndjson writer deferred
/// per microkernel plan). Keeps the half-wire `row.len() < 16` rejection (a cosign row
/// must carry at minimum a hash-digest prefix), then delegates to
/// `crate::cosign_chain::append` which hashes the row + bumps the monotonic sequence
/// counter. Returns the new 1-indexed chain length on success. Cosign-chain errors map
/// to `SyscallErr::CosignReject` (existing variant — no new error tags added per
/// REPO_LAW Invariant 9 syscall surface freeze).
pub fn sys_cosign_append(row: &[u8]) -> Result<u64, SyscallErr> {
    /// Lower bound: minimal cosign row needs at least a hash digest prefix.
    const MIN_COSIGN_ROW_BYTES: usize = 16;
    if row.len() < MIN_COSIGN_ROW_BYTES {
        return Err(SyscallErr::Invalid);
    }
    crate::cosign_chain::append(row).map_err(|_| SyscallErr::CosignReject)
}

/// 15. Query the access-tier of a path (Invariant 5 · 6-tier).
///
/// Phase-3 v0.1.1: wired to `crate::tier::classify_path` per microkernel-stub
/// (real implementation; demoted to `servers/tier-policy` in upcoming refactor).
pub fn sys_tier_query(path: &[u8]) -> Result<AccessTier, SyscallErr> {
    let s = core::str::from_utf8(path).map_err(|_| SyscallErr::Invalid)?;
    match crate::tier::classify_path(s) {
        Ok(t) => Ok(t),
        Err(crate::tier::TierClassifierErr::PathMalformed) => Err(SyscallErr::Invalid),
        Err(crate::tier::TierClassifierErr::UnclassifiedDefault) => Ok(AccessTier::Public),
    }
}

/// 16. GNN inference primitive (Invariant 3). Deterministic fallback when model unloaded.
///
/// Phase-3 v0.1.3: wired to `crate::gnn::GnnInference::predict_route`. Returns a single
/// byte representing the RoutingDecision (0=Local..6=UnicastAether). When model is unloaded
/// (v0.1 default), `predict_route` returns Local (0) per deterministic fallback.
/// Microkernel refactor will demote this to `servers/gnn-oracle` userspace RPC.
pub fn sys_gnn_infer(input: &[u8], out: &mut [u8]) -> Result<usize, SyscallErr> {
    if out.is_empty() {
        return Err(SyscallErr::Invalid);
    }
    let inference = crate::gnn::GnnInference::new();
    let decision = inference
        .predict_route(input)
        .map_err(|_| SyscallErr::Invalid)?;
    let encoded: u8 = match decision {
        crate::gnn::RoutingDecision::Local => 0,
        crate::gnn::RoutingDecision::TriadBroadcast => 1,
        crate::gnn::RoutingDecision::QuadBroadcast => 2,
        crate::gnn::RoutingDecision::UnicastAcer => 3,
        crate::gnn::RoutingDecision::UnicastLiris => 4,
        crate::gnn::RoutingDecision::UnicastFalcon => 5,
        crate::gnn::RoutingDecision::UnicastAether => 6,
    };
    out[0] = encoded;
    Ok(1)
}

/// Compile-time enforcement: this list MUST equal exactly 16 entries.
/// Adding any syscall here without a tier-2 cosign in AUTHORIZATION.ndjson violates Invariant 9.
pub const CANONICAL_SYSCALLS: [&str; 16] = [
    "read",
    "write",
    "exec",
    "fork",
    "exit",
    "mmap",
    "munmap",
    "time",
    "pid_current",
    "envelope_send",
    "envelope_recv",
    "hookwall_pre",
    "hookwall_post",
    "cosign_append",
    "tier_query",
    "gnn_infer",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_syscall_count_is_exactly_16() {
        assert_eq!(CANONICAL_SYSCALLS.len(), 16);
    }

    #[test]
    fn unwired_stubs_still_return_unimplemented() {
        // Per Phase-3 doctrine, every syscall has an explicit body (FULL wire / HALF wire /
        // doc-only / diverging spin-loop). This test enumerates the syscalls still returning
        // Unimplemented for VALID inputs (half-wires return Unimplemented for valid; their
        // dedicated tests verify the Invalid-rejection path separately).
        //
        // Removed from this list after wiring:
        //   sys_cosign_append (cycle-67 v0.2.3 → cosign_chain::append, returns Ok(seq))
        //   sys_envelope_send (cycle-67 v0.2.1 → envelope::dispatch_enqueue_bytes)
        //   sys_envelope_recv (cycle-67 v0.2.2 → envelope::dispatch_dequeue_bytes)
        //   sys_fork          (cycle-67 v0.2.4 → agent_runtime::spawn_child_agent)
        //   sys_exec          (2026-05-13 v0.3.0 → envelope::dispatch_enqueue_bytes + EXEC_HANDLE_NEXT)
        //   sys_mmap          (2026-05-13 v0.3.1 → frame_alloc::alloc_pages, virtual-range scaffold)
        //   sys_munmap        (2026-05-13 v0.3.1 → frame_alloc::free_pages, v0.1 no-op release)
        //   sys_read          (2026-05-13 v0.3.2 → vfs::vfs_read, routing-by-fd-value scaffold)
        //   sys_write         (2026-05-13 v0.3.2 → vfs::vfs_write, routing-by-fd-value scaffold)
        // All 16 canonical syscalls are now FULL-wired (or DIVERGING STUB for sys_exit).
        // No `assert sys_X returns Unimplemented for valid inputs` assertions remain.
    }

    #[test]
    fn sys_read_stdin_returns_eof_v0_3_2() {
        // Phase-3 v0.3.2: FULL WIRE — fd 0 (stdin) returns Ok(0) (no input source v0.1).
        let mut buf = [0u8; 16];
        assert_eq!(sys_read(0, &mut buf, 8), Ok(0));
    }

    #[test]
    fn sys_read_invalid_fd_returns_invalid() {
        // Phase-3 v0.3.2: FULL WIRE — fds outside reserved range return Invalid.
        let mut buf = [0u8; 16];
        assert_eq!(sys_read(99, &mut buf, 8), Err(SyscallErr::Invalid));
        assert_eq!(sys_read(1, &mut buf, 8), Err(SyscallErr::Invalid)); // stdout
        assert_eq!(sys_read(2, &mut buf, 8), Err(SyscallErr::Invalid)); // stderr
    }

    #[test]
    fn sys_write_stdout_returns_buf_len_v0_3_2() {
        // Phase-3 v0.3.2: FULL WIRE — fd 1 (stdout) accepts; returns buf.len().
        assert_eq!(sys_write(1, b"hello"), Ok(5));
        assert_eq!(sys_write(2, b"err msg"), Ok(7)); // stderr
    }

    #[test]
    fn sys_write_invalid_fd_returns_invalid() {
        // Phase-3 v0.3.2: FULL WIRE — fds outside reserved range OR stdin return Invalid.
        assert_eq!(sys_write(0, b"x"), Err(SyscallErr::Invalid)); // stdin
        assert_eq!(sys_write(99, b"x"), Err(SyscallErr::Invalid));
    }

    #[test]
    fn sys_pid_current_returns_federation_anchor_fingerprint() {
        // Phase-3 v0.1.4: sys_pid_current wired to FEDERATION_ANCHOR_PID sha256 first-8-bytes BE.
        // Expected u64 = 0xe00b1a465d6dcb50 (matches cross-runtime triple-parity verdict :82272).
        assert_eq!(sys_pid_current(), Ok(0xe00b_1a46_5d6d_cb50));
    }

    #[test]
    fn sys_munmap_rejects_null_pointer() {
        // Phase-3 v0.1.7: input-validation half-wire.
        assert_eq!(
            sys_munmap(core::ptr::null_mut(), 4096),
            Err(SyscallErr::Invalid)
        );
    }

    #[test]
    fn sys_munmap_rejects_zero_length() {
        let mut byte: u8 = 0;
        assert_eq!(
            sys_munmap(&mut byte as *mut u8, 0),
            Err(SyscallErr::Invalid)
        );
    }

    #[test]
    fn sys_munmap_releases_allocated_page_v0_3_1() {
        // Phase-3 v0.3.1: FULL WIRE — alloc a real virtual page via sys_mmap,
        // then release it via sys_munmap. v0.1 free is a no-op on success so
        // releasing the same page twice ALSO succeeds (bitmap reclaim lands v0.2).
        crate::frame_alloc::reset_for_tests();
        let p = sys_mmap(4096, 0x3).expect("sys_mmap must allocate");
        assert_eq!(
            sys_munmap(p, 4096),
            Ok(()),
            "sys_munmap of a fresh allocation must succeed"
        );
    }

    #[test]
    fn sys_munmap_out_of_range_pointer_returns_invalid() {
        // A stack-resident byte is non-null + non-zero-len but outside the synthetic
        // virtual range. Frame allocator maps InvalidPointer → SyscallErr::Invalid.
        let mut byte: u8 = 0;
        assert_eq!(
            sys_munmap(&mut byte as *mut u8, 4096),
            Err(SyscallErr::Invalid)
        );
    }

    #[test]
    fn sys_exec_rejects_undersized_envelope() {
        // Phase-3 v0.1.8: minimal envelope shell ≥ 32 bytes.
        assert_eq!(sys_exec(b"too short"), Err(SyscallErr::Invalid));
        assert_eq!(sys_exec(&[]), Err(SyscallErr::Invalid));
    }

    #[test]
    fn sys_exec_dispatches_valid_envelope_and_returns_monotonic_handle() {
        // Phase-3 v0.3.0: FULL WIRE — a valid (≥ 32 byte) envelope must enqueue
        // and return Ok(handle). Handle ≥ 1 (0 reserved as "no handle") and
        // strictly increases across consecutive successful calls.
        crate::envelope::reset_slot_for_tests();
        let env = b"{\"from\":\"X\",\"to\":\"Y\",\"type\":\"EXEC\"}padding";
        assert!(env.len() >= 32);
        let h1 = sys_exec(env).expect("valid envelope must dispatch");
        assert!(h1 >= 1, "first exec handle must be >= 1 (0 is reserved)");
        // Drain the slot between sends — single-slot SPSC queue per v0.2.1.
        crate::envelope::reset_slot_for_tests();
        let h2 = sys_exec(env).expect("second valid envelope must dispatch");
        assert!(h2 > h1, "exec handles must be monotonically increasing");
        crate::envelope::reset_slot_for_tests();
    }

    #[test]
    fn sys_read_rejects_len_greater_than_buf_len() {
        // Phase-3 v0.1.9: signature soundness — len > buf.len() would overrun.
        let mut buf = [0u8; 8];
        assert_eq!(sys_read(0, &mut buf, 9), Err(SyscallErr::Invalid));
        assert_eq!(sys_read(0, &mut buf, usize::MAX), Err(SyscallErr::Invalid));
    }

    #[test]
    fn sys_read_stdin_returns_ok_zero_v0_3_2() {
        // Phase-3 v0.3.2: FULL WIRE — sys_read on STDIN_FD (0) returns Ok(0) (EOF).
        // No source plumbed in v0.1; vfs::vfs_read backs this.
        let mut buf = [0u8; 8];
        assert_eq!(sys_read(0, &mut buf, 8), Ok(0));
        assert_eq!(sys_read(0, &mut buf, 0), Ok(0));
    }

    #[test]
    fn sys_write_rejects_empty_buf() {
        // Phase-3 v0.1.10: zero-byte write is meaningless.
        assert_eq!(sys_write(0, &[]), Err(SyscallErr::Invalid));
    }

    #[test]
    fn sys_write_to_stdin_returns_invalid_v0_3_2() {
        // Phase-3 v0.3.2: FULL WIRE — sys_write on STDIN_FD (0) returns Invalid
        // (can't write to stdin). vfs::vfs_write returns InvalidOpForFd, mapped to Invalid.
        let buf = [1u8, 2, 3];
        assert_eq!(sys_write(0, &buf), Err(SyscallErr::Invalid));
    }

    #[test]
    fn sys_mmap_rejects_zero_length() {
        // Phase-3 v0.1.11: zero-byte mapping is meaningless.
        assert_eq!(sys_mmap(0, 0x3), Err(SyscallErr::Invalid));
    }

    #[test]
    fn sys_mmap_rejects_zero_prot() {
        // Phase-3 v0.1.11: prot == 0 requests no access bits — no-op trap.
        assert_eq!(sys_mmap(4096, 0), Err(SyscallErr::Invalid));
    }

    #[test]
    fn sys_mmap_allocates_non_null_page_aligned_pointer_v0_3_1() {
        // Phase-3 v0.3.1: FULL WIRE — valid (len > 0, prot > 0) inputs return a
        // non-null, page-aligned, in-range virtual pointer from the frame allocator.
        crate::frame_alloc::reset_for_tests();
        let p = sys_mmap(4096, 0x3).expect("sys_mmap of one page with prot=R|W must succeed");
        assert!(!p.is_null(), "sys_mmap must return non-null on success");
        assert_eq!(
            (p as usize) % 4096,
            0,
            "returned pointer must be page-aligned"
        );
        let p2 = sys_mmap(8192, 0x7).expect("second sys_mmap (two pages, prot=R|W|X) must succeed");
        assert!(
            (p2 as usize) > (p as usize),
            "second alloc address must be higher than first"
        );
        // Round-trip via sys_munmap.
        assert_eq!(sys_munmap(p, 4096), Ok(()));
        assert_eq!(sys_munmap(p2, 8192), Ok(()));
    }

    #[test]
    fn sys_envelope_send_rejects_undersized_envelope() {
        // Phase-3 v0.1.12: same MIN_ENVELOPE_BYTES (32) as sys_exec.
        assert_eq!(sys_envelope_send(b"too short", 0), Err(SyscallErr::Invalid));
        assert_eq!(sys_envelope_send(&[], 0), Err(SyscallErr::Invalid));
    }

    #[test]
    fn sys_envelope_send_dispatches_to_envelope_module() {
        // Phase-3 v0.2.1: FULL WIRE — a valid (≥ 32 byte) envelope must enqueue
        // successfully and return Ok(()). Reset the slot first so the test is
        // order-independent w.r.t. other envelope tests.
        crate::envelope::reset_slot_for_tests();
        let env = b"{\"from\":\"X\",\"to\":\"Y\",\"type\":\"Z\"}padding-padding";
        assert!(env.len() >= 32);
        assert_eq!(sys_envelope_send(env, 0), Ok(()));
        // Restore empty state so any concurrent envelope-module test that reads
        // `queue_depth()` continues to observe an empty slot.
        crate::envelope::reset_slot_for_tests();
    }

    #[test]
    fn sys_envelope_recv_rejects_empty_buf() {
        // Phase-3 v0.1.13: nowhere to write the received envelope.
        let mut empty: [u8; 0] = [];
        assert_eq!(sys_envelope_recv(&mut empty, 0), Err(SyscallErr::Invalid));
    }

    #[test]
    fn sys_envelope_recv_returns_data_or_would_block() {
        // Phase-3 v0.2.2: FULL WIRE — when the queue is empty, recv returns
        // WouldBlock (mapped from EnvelopeErr::QueueEmpty). When data is enqueued,
        // recv returns Ok(bytes_read) with the exact byte length the sender pushed.
        crate::envelope::reset_slot_for_tests();
        let mut buf = [0u8; 128];

        // Empty path: bounded wait expires immediately → WouldBlock.
        assert_eq!(sys_envelope_recv(&mut buf, 0), Err(SyscallErr::WouldBlock));

        // Filled path: enqueue then dequeue must round-trip exactly.
        let env = b"{\"from\":\"acer\",\"to\":\"liris\",\"type\":\"TEST\"}xx";
        assert!(env.len() >= 32);
        assert_eq!(sys_envelope_send(env, 1), Ok(()));
        let n = sys_envelope_recv(&mut buf, 0).expect("filled slot must dequeue");
        assert_eq!(n, env.len(), "dequeued length must equal enqueued length");
        assert_eq!(&buf[..n], env, "dequeued bytes must equal enqueued bytes");
        crate::envelope::reset_slot_for_tests();
    }

    #[test]
    fn sys_cosign_append_rejects_undersized_row() {
        // Phase-3 v0.1.14: minimal cosign row needs hash-digest prefix (≥ 16 bytes).
        let short = [0u8; 8];
        assert_eq!(sys_cosign_append(&short), Err(SyscallErr::Invalid));
        assert_eq!(sys_cosign_append(&[]), Err(SyscallErr::Invalid));
    }

    #[test]
    fn sys_cosign_append_returns_sequence_number_for_valid_row() {
        // Phase-3 v0.2.3: FULL WIRE — valid (≥ 16 byte) rows succeed and return a
        // monotonic sequence number ≥ 1 (1-indexed chain length).
        // NOTE: parallel-test-safe — APPEND_SEQ_COUNTER is module-global, so we assert
        // the returned value is ≥ 1 rather than an exact value (other parallel tests
        // may have already bumped the counter).
        let row = [0u8; 16];
        let seq = sys_cosign_append(&row).expect("valid 16-byte row must append");
        assert!(
            seq >= 1,
            "first append must return a sequence number ≥ 1, got {}",
            seq
        );
    }

    #[test]
    fn sys_cosign_append_increments_sequence_across_appends() {
        // Phase-3 v0.2.3: FULL WIRE — successive appends must return strictly
        // increasing sequence numbers (monotonic chain length). Parallel-test-safe:
        // asserts relative monotonicity between three back-to-back appends in this
        // test rather than absolute counter values.
        let row_a = [0xAAu8; 16];
        let row_b = [0xBBu8; 32];
        let row_c = [0xCCu8; 16];
        let s1 = sys_cosign_append(&row_a).expect("append A");
        let s2 = sys_cosign_append(&row_b).expect("append B");
        let s3 = sys_cosign_append(&row_c).expect("append C");
        assert!(s1 < s2, "s2 ({}) must exceed s1 ({})", s2, s1);
        assert!(s2 < s3, "s3 ({}) must exceed s2 ({})", s3, s2);
    }

    #[test]
    fn sys_fork_returns_child_handle() {
        // Phase-3 v0.2.4: FULL WIRE — sys_fork mints a fresh child handle (≥ 1) via
        // agent_runtime::spawn_child_agent. The exact value depends on counter state at
        // call-time (tests can run in any order), but it must be a positive Handle.
        let res = sys_fork();
        match res {
            Ok(handle) => {
                assert!(handle >= 1, "child handle must be >= 1 (0 reserved for child-side fork return convention), got {}", handle);
            }
            Err(SyscallErr::Exhausted) => {
                // Acceptable end-state: registry saturated by prior tests within same process.
            }
            Err(other) => panic!("sys_fork returned unexpected error {:?}", other),
        }
    }

    #[test]
    fn sys_fork_increments_registry() {
        // Phase-3 v0.2.4: each successful sys_fork bumps the agent_runtime child counter.
        let before = crate::agent_runtime::spawn_child_agent_count();
        let res = sys_fork();
        let after = crate::agent_runtime::spawn_child_agent_count();
        match res {
            Ok(_) => {
                assert_eq!(
                    after,
                    before + 1,
                    "child counter must increment by exactly 1 on Ok"
                );
            }
            Err(SyscallErr::Exhausted) => {
                // Saturated state: counter already pinned at AGENT_REGISTRY_MAX; no increment.
                assert!(after >= before, "counter must not regress");
            }
            Err(other) => panic!("sys_fork returned unexpected error {:?}", other),
        }
    }
}
