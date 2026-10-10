//! Minimal init system · Phase-2 Step 31
//!
//! No systemd. No dependencies beyond `asolaria-kernel-core`. ~200 LOC.
//! Boots to an envelope-REPL — the "shell" is the envelope bus, per
//! REPO_LAW Invariant 6 (everything-is-an-envelope, no out-of-band IPC).
//!
//! Lifecycle:
//!   1. Mint process PID via `sys_pid_current`
//!   2. Hookwall-pre the init-ready envelope, emit EVT-INIT-READY, hookwall-post
//!   3. Loop: recv envelope → dispatch by type-prefix → emit reply → cosign-append
//!   4. EVT-SHUTDOWN exits gracefully via `sys_exit`
//!
//! Envelope wire format (v0.1 init-shell subset):
//!   bytes[0..4]   = 4-char ASCII type-prefix (TYPE)
//!   bytes[4..12]  = u64 sequence number (LE)
//!   bytes[12..N]  = payload bytes (per-type)
//!
//! Type prefixes:
//!   "PING" → respond with "PONG"
//!   "EXEC" → invoke sys_exec on payload, respond with result
//!   "SHTD" → graceful shutdown via sys_exit(0)
//!   "VERS" → respond with kernel version string
//!   "PIDA" → respond with PID anchor
//!   other  → respond with "UNKN" + echo bytes[0..4]

use asolaria_kernel_core::envelope::ENVELOPE_SLOT_BYTES;
use asolaria_kernel_core::syscall::{
    sys_cosign_append, sys_envelope_recv, sys_envelope_send, sys_exec, sys_exit, sys_hookwall_post,
    sys_hookwall_pre, sys_pid_current, HookwallVerdict, SyscallErr,
};
use asolaria_kernel_core::{FEDERATION_ANCHOR_PID, KERNEL_VERSION};

/// Maximum envelope size accepted/emitted by init's shell loop.
/// Keeps the loop allocation-free; larger envelopes route through agent-runtime,
/// not init.
const INIT_ENVELOPE_MAX: usize = 512;

/// Maximum consecutive `sys_envelope_recv` failures before init self-halts.
/// Prevents init from busy-looping on a wedged bus.
const RECV_FAILURE_THRESHOLD: u32 = 32;

/// Maximum wait on each recv call (1 second in nanoseconds).
/// Init wakes once per second to age-out stalled subprocesses (future v0.2).
const RECV_WAIT_NS: u64 = 1_000_000_000;

/// Routing hint for init-emitted envelopes (local-tier, no GNN consultation needed).
const ROUTE_HINT_LOCAL: u32 = 0;

/// Run the init system headless. Diverges — only returns by `sys_exit`.
///
/// Caller (boot/main.rs) MUST have:
///   - heap allocator initialized
///   - kernel-core static init completed
///
/// NOTE (2026-10-09): with no input source wired, this loop can only ever read init's own
/// INIT-ready envelope, then waits forever — the "stuck on the loading screen" boot. The UEFI
/// boot path now runs `console::run`, which drives the same dispatch via [`exchange`]. This
/// headless loop is kept for a future bus-fed (agent-runtime) boot.
#[allow(dead_code)]
pub fn run() -> ! {
    let pid = sys_pid_current().unwrap_or(0);
    emit_init_ready(pid);

    let mut recv_buf = [0u8; INIT_ENVELOPE_MAX];
    let mut consecutive_failures: u32 = 0;
    let mut seq: u64 = 1;

    loop {
        match sys_envelope_recv(&mut recv_buf, RECV_WAIT_NS) {
            Ok(n) if n >= 4 => {
                consecutive_failures = 0;
                let type_prefix = [recv_buf[0], recv_buf[1], recv_buf[2], recv_buf[3]];
                dispatch_envelope(&type_prefix, &recv_buf[..n], &mut seq);
            }
            Ok(_) => {
                // n < 4: malformed, ignore but don't count as failure
            }
            Err(SyscallErr::WouldBlock) => {
                // expected on timeout — fall through and continue
            }
            Err(_) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= RECV_FAILURE_THRESHOLD {
                    // Bus is wedged — init refuses to spin. Emit final SOS and exit.
                    emit_envelope(b"SOS!", &mut seq, b"init-recv-exhausted");
                    sys_exit(2);
                }
            }
        }
    }
}

/// Dispatch a received envelope by its 4-char ASCII type prefix.
fn dispatch_envelope(type_prefix: &[u8; 4], full_env: &[u8], seq: &mut u64) {
    // Hookwall-pre — every dispatch is gated.
    //
    // 2026-10-09 fix: `sys_hookwall_pre` reads byte[0] as the hookwall SLOT (0..63) and byte[1]
    // as the syscall number. Passing the raw envelope put the ASCII type ('P' = 0x50 = 80) in the
    // slot byte, so EVERY envelope was out of range -> Err -> Block, whatever the hookwall policy
    // said. The gate context is now the syscall being exercised (slots 0-15 map 1:1 to the 16
    // syscalls): EXEC = 3, every other envelope dispatch = 10. The hookwall decides as before.
    let hook = hook_context(type_prefix);
    let verdict = match sys_hookwall_pre(&hook) {
        Ok(v) => v,
        Err(_) => HookwallVerdict::Block,
    };

    if verdict == HookwallVerdict::Block {
        emit_envelope(b"BLOK", seq, type_prefix);
        let _ = sys_hookwall_post(&hook, HookwallVerdict::Block);
        return;
    }
    if verdict == HookwallVerdict::Hold {
        // Init does not buffer — defer to supervisor; emit HOLD ack.
        emit_envelope(b"HOLD", seq, type_prefix);
        let _ = sys_hookwall_post(&hook, HookwallVerdict::Hold);
        return;
    }

    match type_prefix {
        b"PING" => {
            emit_envelope(b"PONG", seq, &[]);
        }
        b"EXEC" => {
            handle_exec(&full_env[12..], seq);
        }
        b"SHTD" => {
            // Graceful shutdown — emit FINI and exit.
            emit_envelope(b"FINI", seq, b"init-shutdown");
            let _ = sys_hookwall_post(&hook, HookwallVerdict::Proceed);
            // Append cosign-chain row noting init-end (best-effort).
            let _ = sys_cosign_append(b"init-shutdown");
            sys_exit(0);
        }
        b"VERS" => {
            emit_envelope(b"VERS", seq, KERNEL_VERSION.as_bytes());
        }
        b"PIDA" => {
            emit_envelope(b"PIDA", seq, FEDERATION_ANCHOR_PID.as_bytes());
        }
        _ => {
            emit_envelope(b"UNKN", seq, type_prefix);
        }
    }

    let _ = sys_hookwall_post(&hook, HookwallVerdict::Proceed);
}

/// Syscall numbers (1-based; hookwall slot = number - 1) init exercises when it dispatches an envelope.
const SYSCALL_EXEC: u8 = 3;
const SYSCALL_ENVELOPE_DISPATCH: u8 = 10;

/// Hookwall context bytes `[slot, syscall_no]` for dispatching an envelope of this type.
/// Syscalls are numbered from 1 and slots from 0 (every in-tree hookwall test pairs
/// `slot: 0, syscall_no: 1`), so `slot = syscall_no - 1`.
fn hook_context(type_prefix: &[u8; 4]) -> [u8; 2] {
    let no = if type_prefix == b"EXEC" {
        SYSCALL_EXEC
    } else {
        SYSCALL_ENVELOPE_DISPATCH
    };
    [no - 1, no]
}

/// Handle `EXEC` — pass the payload through to `sys_exec` and emit reply.
fn handle_exec(payload: &[u8], seq: &mut u64) {
    if payload.is_empty() {
        emit_envelope(b"EERR", seq, b"empty-exec");
        return;
    }
    match sys_exec(payload) {
        Ok(handle) => {
            let bytes = handle.to_le_bytes();
            emit_envelope(b"EOKx", seq, &bytes);
        }
        Err(SyscallErr::Invalid) => emit_envelope(b"EERR", seq, b"invalid"),
        Err(SyscallErr::PermissionDenied) => emit_envelope(b"EERR", seq, b"perm-denied"),
        Err(SyscallErr::HookwallBlock) => emit_envelope(b"EERR", seq, b"hookwall-block"),
        Err(SyscallErr::Exhausted) => emit_envelope(b"EERR", seq, b"exhausted"),
        Err(SyscallErr::WouldBlock) => emit_envelope(b"EERR", seq, b"would-block"),
        Err(SyscallErr::Unimplemented) => emit_envelope(b"EERR", seq, b"unimplemented"),
        Err(SyscallErr::CosignReject) => emit_envelope(b"EERR", seq, b"cosign-reject"),
        Err(_) => emit_envelope(b"EERR", seq, b"other-err"),
    }
}

/// Emit the bootstrap EVT-INIT-READY envelope. Best-effort — if hookwall blocks
/// or send fails, init still proceeds to its main loop (it has no other choice).
fn emit_init_ready(pid: u64) {
    let mut buf = [0u8; 32];
    buf[0..4].copy_from_slice(b"INIT");
    buf[4..12].copy_from_slice(&0u64.to_le_bytes());
    buf[12..20].copy_from_slice(&pid.to_le_bytes());

    let hook = hook_context(b"INIT");
    let _ = sys_hookwall_pre(&hook);
    let _ = sys_envelope_send(&buf, ROUTE_HINT_LOCAL);
    let _ = sys_hookwall_post(&hook, HookwallVerdict::Proceed);
}

/// Emit a typed envelope from init. Best-effort, fail-silent (caller has no
/// recovery path — init is the trust-boundary of last resort).
///
/// 2026-10-09 fix: `sys_envelope_send` rejects envelopes under 32 bytes, so every short reply
/// (`PONG` = 12 B, `UNKN` = 16 B, ...) used to be dropped silently. Replies are now zero-padded
/// to [`ENVELOPE_MIN`]; readers trim trailing zeros.
fn emit_envelope(type_prefix: &[u8; 4], seq: &mut u64, payload: &[u8]) {
    let mut buf = [0u8; INIT_ENVELOPE_MAX];
    buf[0..4].copy_from_slice(type_prefix);
    buf[4..12].copy_from_slice(&seq.to_le_bytes());
    let n = core::cmp::min(payload.len(), INIT_ENVELOPE_MAX - 12);
    buf[12..12 + n].copy_from_slice(&payload[..n]);
    let len = core::cmp::max(12 + n, ENVELOPE_MIN);
    let _ = sys_envelope_send(&buf[..len], ROUTE_HINT_LOCAL);
    *seq = seq.wrapping_add(1);
}

/// Smallest envelope `sys_envelope_send` accepts.
const ENVELOPE_MIN: usize = 32;

/// Empty the bus so a caller starts clean (also the recovery path if a reply was left in the
/// single bus slot).
pub fn drain() {
    // Slot-sized: a smaller buffer cannot take an oversized envelope, which would stay in the
    // slot and wedge the bus for good.
    let mut buf = [0u8; ENVELOPE_SLOT_BYTES];
    for _ in 0..4 {
        if sys_envelope_recv(&mut buf, 0).is_err() {
            break;
        }
    }
}

/// One request/reply round trip through the REAL init dispatch: build a `TYPE|seq|0..` envelope,
/// put it on the bus, let init receive + dispatch it (hookwall-gated), then read init's reply
/// back off the bus into `reply`. Returns the reply length. Used by the on-screen console so the
/// envelope-REPL is reachable from a keyboard instead of waiting on a bus nobody writes to.
pub fn exchange(ty: &[u8; 4], seq: &mut u64, reply: &mut [u8]) -> Result<usize, &'static [u8]> {
    let mut req = [0u8; ENVELOPE_MIN];
    req[0..4].copy_from_slice(ty);
    req[4..12].copy_from_slice(&seq.to_le_bytes());
    *seq = seq.wrapping_add(1);
    if sys_envelope_send(&req, ROUTE_HINT_LOCAL).is_err() {
        // Something is occupying the slot; clear it so the next exchange can go through.
        drain();
        return Err(b"send-rejected");
    }
    let mut buf = [0u8; ENVELOPE_SLOT_BYTES];
    let n = sys_envelope_recv(&mut buf, 0).map_err(|_| &b"request-lost"[..])?;
    if n < 4 {
        return Err(b"malformed");
    }
    let prefix = [buf[0], buf[1], buf[2], buf[3]];
    // init stamps its reply with the next sequence number.
    let want = *seq;
    dispatch_envelope(&prefix, &buf[..n], seq);
    match sys_envelope_recv(reply, 0) {
        Ok(n) if n >= 12 && reply[4..12] != want.to_le_bytes() => Err(b"stale-reply"),
        Ok(n) => Ok(n),
        Err(_) => {
            // A reply larger than `reply` stays in the single bus slot and would make every
            // later send fail with QueueFull. Drain so the bus can never wedge.
            drain();
            Err(b"no-reply")
        }
    }
}
