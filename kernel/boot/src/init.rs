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

/// Run the init system. Diverges — only returns by `sys_exit`.
///
/// Caller (boot/main.rs) MUST have:
///   - heap allocator initialized
///   - kernel-core static init completed
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
    let verdict = match sys_hookwall_pre(full_env) {
        Ok(v) => v,
        Err(_) => HookwallVerdict::Block,
    };

    if verdict == HookwallVerdict::Block {
        emit_envelope(b"BLOK", seq, type_prefix);
        let _ = sys_hookwall_post(full_env, HookwallVerdict::Block);
        return;
    }
    if verdict == HookwallVerdict::Hold {
        // Init does not buffer — defer to supervisor; emit HOLD ack.
        emit_envelope(b"HOLD", seq, type_prefix);
        let _ = sys_hookwall_post(full_env, HookwallVerdict::Hold);
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
            let _ = sys_hookwall_post(full_env, HookwallVerdict::Proceed);
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

    let _ = sys_hookwall_post(full_env, HookwallVerdict::Proceed);
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

    let _ = sys_hookwall_pre(&buf);
    let _ = sys_envelope_send(&buf, ROUTE_HINT_LOCAL);
    let _ = sys_hookwall_post(&buf, HookwallVerdict::Proceed);
}

/// Emit a typed envelope from init. Best-effort, fail-silent (caller has no
/// recovery path — init is the trust-boundary of last resort).
fn emit_envelope(type_prefix: &[u8; 4], seq: &mut u64, payload: &[u8]) {
    let mut buf = [0u8; INIT_ENVELOPE_MAX];
    buf[0..4].copy_from_slice(type_prefix);
    buf[4..12].copy_from_slice(&seq.to_le_bytes());
    let n = core::cmp::min(payload.len(), INIT_ENVELOPE_MAX - 12);
    buf[12..12 + n].copy_from_slice(&payload[..n]);
    let _ = sys_envelope_send(&buf[..12 + n], ROUTE_HINT_LOCAL);
    *seq = seq.wrapping_add(1);
}
