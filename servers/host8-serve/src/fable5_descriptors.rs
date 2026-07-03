use std::fs;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const ASOLARIA_TASK_MANAGER_PID: &str = "9af4238c6780ffa8";

const TASK_MANAGER_PORTS: &[(&str, u16, &str)] = &[
    ("maps-atlas", 4790, "atlas"),
    ("recall-atlas", 4791, "recall"),
    ("gnn-oracle", 4792, "python"),
    ("gnn-gsl", 4793, "python"),
    ("fischer-live", 4794, "node"),
    ("recall-serve-rust", 4796, "rust"),
    ("graphify-60d", 4815, "python"),
    ("agent-keyboard", 4913, "node-executor-gated"),
    ("liris-bus", 4944, "dashboard-mirror"),
    ("behcs-bus", 4947, "bus"),
    ("super-dashboard", 4949, "dashboard"),
    ("omnidispatcher", 4950, "node"),
    ("vote-quorum", 4952, "python"),
    ("cosign-chain", 4953, "python"),
    ("host8-serve-rust", 5088, "rust-host8"),
    ("ai-memory", 49374, "memory"),
];

const UNIFIED_PIPE_PIECES: &[(&str, &str, &str)] = &[
    (
        "fischer-stub-room-converters",
        "EXISTS-WIRE",
        "host8 rooms as instant-call HBP function-converter descriptors",
    ),
    (
        "pipe-into-unified-dashboard",
        "GATED",
        "owning GAC graphify dashboard route, no clobber",
    ),
    (
        "remote-control-n-nest",
        "GATED",
        "3-level nested remote-control descriptors, operator and hookwall gated",
    ),
    (
        "pixel-first-engines",
        "HOST8-DESCRIPTOR",
        "pixels-first HBI/HBP render engines, gpu-less",
    ),
    (
        "jesse-pixel-backend",
        "HOST8-DESCRIPTOR",
        "operator-facing HBI/HBP backend projection",
    ),
    (
        "hrm-frozen-brain",
        "HOST8-DESCRIPTOR",
        "HRM frozen-slice understanding descriptor, not live consciousness proof",
    ),
    (
        "geospatial-abilities",
        "HOST8-DESCRIPTOR",
        "60D plus GAC coordinate abilities descriptor",
    ),
    (
        "asolaria-task-manager",
        "HOST8-ROUTE",
        "view-only live-system tracker, no kill",
    ),
    (
        "omnicpu",
        "MINT-NEW",
        "proposed CPU telemetry kernel lane, not found as existing live surface",
    ),
    (
        "omnigpu",
        "MINT-NEW",
        "proposed GPU telemetry lane; current build remains gpu-less",
    ),
];

const FISCHER_CONVERTERS: &[(&str, &str, &str)] = &[
    (
        "fischer-score-gate",
        "EXISTS-WIRE",
        "score-gated function converter descriptor; fire remains gated",
    ),
    (
        "stub-room-call",
        "EXISTS-WIRE",
        "Host8 room stub callable as HBP function descriptor",
    ),
    (
        "behcs-translate",
        "GATED",
        "BEHCS translator descriptor; measured rung remains 256-1024",
    ),
    (
        "hookwall-classify",
        "GATED",
        "classify edge and verb before function conversion",
    ),
    (
        "gnn-edge-score",
        "GATED",
        "GNN watched-edge score before route materialization",
    ),
];

fn port_is_running(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(140),
    )
    .is_ok()
}

fn listen_inode_for_port(port: u16) -> Option<String> {
    let wanted = format!(":{:04X}", port);
    for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let body = fs::read_to_string(table).ok()?;
        for line in body.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() > 9 && cols[1].ends_with(&wanted) && cols[3] == "0A" {
                return Some(cols[9].to_string());
            }
        }
    }
    None
}

fn process_for_inode(inode: &str) -> (String, String, u64) {
    let proc_dir = match fs::read_dir("/proc") {
        Ok(dir) => dir,
        Err(_) => return ("-".to_string(), "-".to_string(), 0),
    };
    let needle = format!("socket:[{}]", inode);
    for entry in proc_dir.filter_map(|e| e.ok()) {
        let pid = entry.file_name().to_string_lossy().to_string();
        if pid.parse::<u32>().is_err() {
            continue;
        }
        let fd_dir = entry.path().join("fd");
        let fds = match fs::read_dir(fd_dir) {
            Ok(fds) => fds,
            Err(_) => continue,
        };
        if !fds.filter_map(|e| e.ok()).any(|fd| {
            fs::read_link(fd.path())
                .map(|target| target.to_string_lossy() == needle)
                .unwrap_or(false)
        }) {
            continue;
        }
        let comm = fs::read_to_string(entry.path().join("comm"))
            .unwrap_or_default()
            .trim()
            .to_string();
        let rss_kb = fs::read_to_string(entry.path().join("status"))
            .ok()
            .and_then(|body| {
                body.lines().find_map(|line| {
                    line.strip_prefix("VmRSS:").and_then(|rest| {
                        rest.split_whitespace()
                            .next()
                            .and_then(|v| v.parse::<u64>().ok())
                    })
                })
            })
            .unwrap_or(0);
        return (pid, comm, rss_kb);
    }
    ("-".to_string(), "-".to_string(), 0)
}

pub(crate) fn render_task_manager() -> String {
    let mut running = 0usize;
    let mut rust_running = 0usize;
    let mut total_rss_kb = 0u64;
    let mut rows = String::new();
    for (service, port, runtime) in TASK_MANAGER_PORTS {
        let is_running = port_is_running(*port);
        running += usize::from(is_running);
        let (pid, comm, rss_kb) = listen_inode_for_port(*port)
            .map(|inode| process_for_inode(&inode))
            .unwrap_or_else(|| ("-".to_string(), "-".to_string(), 0));
        rust_running += usize::from(is_running && runtime.contains("rust"));
        total_rss_kb = total_rss_kb.saturating_add(rss_kb);
        rows.push_str(&format!(
            "TASKPROC|port={}|service={}|runtime={}|os_pid={}|comm={}|rss_kb={}|state={}|view_only=1|no_kill=1|fire=0|process_launch=0|json=0\n",
            port,
            super::hbp_escape(service),
            super::hbp_escape(runtime),
            super::hbp_escape(pid),
            super::hbp_escape(comm),
            rss_kb,
            if is_running { "RUNNING" } else { "OFF" }
        ));
    }
    format!(
        "TASKHDR|schema=ASOLARIA-TASK-MANAGER-HOST8|pid={}|services={}|running={}|rust_running={}|rss_kb={}|vantage=linux_host8_loopback|view_only=1|no_kill=1|fire=0|process_launch=0|gpu_less=1|json=0\n{}",
        super::hbp_escape(ASOLARIA_TASK_MANAGER_PID),
        TASK_MANAGER_PORTS.len(),
        running,
        rust_running,
        total_rss_kb,
        rows
    )
}

pub(crate) fn render_unified_pipe() -> String {
    let mut out = format!(
        "UNIFIEDPIPEHDR|schema=FABLE5-UNIFIED-DASHBOARD-PIPE-HOST8|pieces={}|route=GAC+graphify+dashboard|no_clobber_4949=1|operator_veto=1|view_only=1|fire=0|process_launch=0|gpu_less=1|json=0\n",
        UNIFIED_PIPE_PIECES.len()
    );
    for (name, status, meaning) in UNIFIED_PIPE_PIECES {
        out.push_str(&format!(
            "UNIFIEDPIPE|name={}|status={}|meaning={}|host8_descriptor=1|fire=0|dispatch=0|json=0\n",
            super::hbp_escape(name),
            super::hbp_escape(status),
            super::hbp_escape(meaning)
        ));
    }
    out
}

pub(crate) fn render_fischer_converters(
    room_id: &str,
    room_stub: &str,
    host_handle8: &str,
) -> String {
    let mut out = format!(
        "FISCHERCONVERTERHDR|schema=FISCHER-STUB-ROOM-CONVERTERS-HOST8|room_id={}|room_stub={}|host_handle8={}|converters={}|instant_call=descriptor_only|view_only=1|fire=0|process_launch=0|json=0\n",
        super::hbp_escape(room_id),
        super::hbp_escape(room_stub),
        super::hbp_escape(host_handle8),
        FISCHER_CONVERTERS.len()
    );
    for (name, status, meaning) in FISCHER_CONVERTERS {
        let pid = super::sha16(&format!("fischer-converter|{}|{}", host_handle8, name));
        out.push_str(&format!(
            "FISCHERCONVERTER|name={}|pid={}|status={}|meaning={}|room_stub={}|executor=rust-host8-descriptor|hookwall=required|gnn=required|sidecar=sha16|fire=0|process_launch=0|json=0\n",
            super::hbp_escape(name),
            super::hbp_escape(pid),
            super::hbp_escape(status),
            super::hbp_escape(meaning),
            super::hbp_escape(room_stub)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn task_manager_route_is_view_only_hbp() {
        let out = super::render_task_manager();
        assert!(out.contains("TASKHDR|"));
        assert!(out.contains("view_only=1"));
        assert!(out.contains("no_kill=1"));
        assert!(out.contains("fire=0"));
        assert!(out.contains("process_launch=0"));
        assert!(out.contains("json=0"));
        assert!(!out.contains('{'));
        assert!(!out.contains('}'));
    }

    #[test]
    fn unified_pipe_and_fischer_converters_are_hbp_descriptors() {
        let pipe = super::render_unified_pipe();
        assert!(pipe.contains("UNIFIEDPIPEHDR|"));
        assert!(pipe.contains("pixel-first-engines"));
        assert!(pipe.contains("fire=0"));
        assert!(pipe.contains("json=0"));
        assert!(!pipe.contains('{'));
        assert!(!pipe.contains('}'));

        let converters = super::render_fischer_converters(
            "gnn-dispatch-bridge",
            "rooms/task-manager/bridge/gnn-dispatch-bridge",
            "22b0500ba10a8de6",
        );
        assert!(converters.contains("FISCHERCONVERTERHDR|"));
        assert!(converters.contains("process_launch=0"));
        assert!(converters.contains("hookwall=required"));
        assert!(converters.contains("json=0"));
        assert!(!converters.contains('{'));
        assert!(!converters.contains('}'));
    }
}
