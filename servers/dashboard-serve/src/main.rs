//! dashboard-serve — the acer dashboard Host-8 responder (:4949). Thread-per-conn loopback/LAN HTTP,
//! json=0 HBP default (`?format=json` cold opt-in). Increment-1 read-only port of the Node
//! super-asolaria-os-dashboard (43 routes): this increment serves `/health` + `/api/canon-index`
//! at byte/data parity, in the Host-8 json=0 frame. STAGED shadow — no writes, no fire, no cutover;
//! the live Node :4949 is untouched. The default bind is a LOOPBACK SHADOW port so a bare launch can
//! never collide with / replace the live Node; cutover is an explicit `ASOLARIA_DASH_BIND=0.0.0.0:4949`.

use std::env;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

mod http;
mod routes;

// Staged/no-cutover safety (liris attack-verify, PR #12): default to a LOOPBACK SHADOW port — a bare
// launch must never bind the live all-interfaces dashboard port. Cutover = explicit operator-gated
// `ASOLARIA_DASH_BIND=0.0.0.0:4949`.
const DEFAULT_BIND: &str = "127.0.0.1:14949";
const DEFAULT_MEMORY_DIR: &str = "C:/Users/acer/.claude/projects/C--/memory";
const MAX_CONN: usize = 128;

pub struct Shared {
    pub memory_dir: PathBuf,
}

fn main() -> std::io::Result<()> {
    let bind = env::var("ASOLARIA_DASH_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    let memory_dir = PathBuf::from(
        env::var("ASOLARIA_MEMORY_DIR").unwrap_or_else(|_| DEFAULT_MEMORY_DIR.to_string()),
    );
    let shared = Arc::new(Shared { memory_dir });

    let listener = TcpListener::bind(&bind)?;
    eprintln!(
        "[dashboard-serve] {bind} thread-per-conn json=0(+?format=json) | inc1 read-only port of node :4949 super-asolaria-os-dashboard | shadow, no cutover"
    );
    let active = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming().flatten() {
        if active.load(Ordering::SeqCst) >= MAX_CONN {
            let _ = http::write_text(stream, 503, "DASHSERVE|ok=0|error=busy|json=0\n");
            continue;
        }
        active.fetch_add(1, Ordering::SeqCst);
        let sh = Arc::clone(&shared);
        let ac = Arc::clone(&active);
        thread::spawn(move || {
            http::handle(stream, &sh);
            ac.fetch_sub(1, Ordering::SeqCst);
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_BIND;

    /// Staged/no-cutover invariant (liris attack-verify, PR #12): the default bind must be a
    /// loopback shadow port — never the live all-interfaces dashboard port. Cutover requires an
    /// explicit ASOLARIA_DASH_BIND override.
    #[test]
    fn default_bind_is_shadow_safe_not_live_4949() {
        assert_ne!(DEFAULT_BIND, "0.0.0.0:4949");
        assert!(
            DEFAULT_BIND.starts_with("127.0.0.1:"),
            "default bind must be loopback shadow, got: {DEFAULT_BIND}"
        );
        assert!(
            !DEFAULT_BIND.ends_with(":4949"),
            "default must not be the live port"
        );
    }
}
