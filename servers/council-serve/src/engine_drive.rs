//! engine_drive — the engine-drive (revolver + brown-hilbert spinner + omniflywheel router) on its
//! OWN thread, so the HTTP responder keeps answering while the engine (would) crank. This is cell
//! C0.1's whole point: separate the crank from the serve loop. In this STAGED build the engine is
//! GATED CLOSED — it launches nothing and fires nothing (process_launch=0, auto_fire=false). The
//! real crank only ever runs on an explicit operator spawner-PID-emit, which is NOT wired here. The
//! thread exists purely to prove the isolation shape; the hard invariant is: it must never auto-fire.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

/// Hard gate. Stays false: no code path in this build sets it. The real spawner-emit wiring is
/// operator-gated and lands in a later increment.
static SPAWNER_EMIT: AtomicBool = AtomicBool::new(false);

/// Spawn the gated engine-drive on its own (detached) thread. It idles, holding the slice-engine
/// invariant (frozen-held until an operator spawner-PID-emit), and NEVER auto-fires.
pub fn spawn_gated() {
    thread::spawn(|| loop {
        // Invariant: the engine never auto-fires. process_launch=0, auto_fire=false.
        debug_assert!(
            !SPAWNER_EMIT.load(Ordering::SeqCst),
            "engine-drive must not auto-fire (spawner-emit is operator-gated, unwired)"
        );
        thread::sleep(Duration::from_secs(60));
    });
}
