//! council-serve — the council/loop Host-8 responder (cell C0.1). Thread-per-conn loopback HTTP on
//! :5090, json=0 hot path. It answers WHILE a separate, GATED engine-drive thread exists — the fix
//! for the :4949 single-thread crank-wedge (the live Node can't crank + serve on one thread).
//! STAGED: read-only over the live vote ledgers, the engine NEVER fires (process_launch=0,
//! auto_fire=false, spawner-emit only — not wired here), no cutover, live :4949/:4952/:4953 untouched.

use std::env;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

mod engine_drive;
mod http;
mod routes;

const DEFAULT_BIND: &str = "127.0.0.1:5090";
const DEFAULT_VOTE_DIR: &str = "C:/HyperBEHCS/data/vote-quorum";
const MAX_CONN: usize = 128;

pub struct Shared {
    pub vote_dir: PathBuf,
}

fn main() -> std::io::Result<()> {
    let bind = env::var("ASOLARIA_COUNCIL_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    let vote_dir = PathBuf::from(
        env::var("ASOLARIA_VOTE_DIR").unwrap_or_else(|_| DEFAULT_VOTE_DIR.to_string()),
    );
    let shared = Arc::new(Shared { vote_dir });

    // GATED engine-drive on its OWN thread: the responder answers while it (would) crank. It NEVER
    // fires here — process_launch=0, auto_fire=false, spawner-emit only (off in this build).
    engine_drive::spawn_gated();

    let listener = TcpListener::bind(&bind)?;
    eprintln!(
        "[council-serve] {bind} thread-per-conn json=0 | engine=gated_own_thread process_launch=0 auto_fire=false | read-only, no cutover"
    );
    let active = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming().flatten() {
        if active.load(Ordering::SeqCst) >= MAX_CONN {
            let _ = http::write_text(stream, 503, "COUNCILSERVE|ok=0|error=busy|json=0\n");
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
