//! Read-only json=0 routes (increment-1 of the :4949 super-asolaria-os-dashboard Host-8 port).
//! `/health` and `/api/canon-index` at data-parity with the Node, default HBP (json=0); `?format=json`
//! gives the cold-egress JSON. No append/write/fire/cutover route exists in this build.

use std::collections::HashSet;
use std::fs;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::Shared;

const SERVICE: &str = "super-asolaria-os-dashboard";
const PORT: u16 = 4949;
const APEX: &str = "COL-ASOLARIA";
const COHORT: &str = "ACER-PID-H740C";

fn json_opt_in(query: &str) -> bool {
    query
        .split('&')
        .any(|kv| kv == "format=json" || kv == "cold=json")
}

fn hbp_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '|' | '\r' | '\n' | '\t' => '_',
            _ => c,
        })
        .collect()
}

fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// Process-relative uptime (set on first request). Not a parity field (live-varying, like ts).
fn uptime_s() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs()
}

/// UTC ISO-8601 from the wall clock (civil-from-days; stdlib-only, no chrono).
fn iso_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y0 = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y0 + 1 } else { y0 };
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

pub fn route(
    shared: &Arc<Shared>,
    method: &str,
    path: &str,
    query: &str,
) -> (u16, &'static str, String) {
    if method == "OPTIONS" {
        return (200, "text/plain; charset=utf-8", String::new());
    }
    if method != "GET" {
        return (
            404,
            "text/plain; charset=utf-8",
            format!(
                "DASHSERVE|ok=0|error=method_not_allowed|method={}|json=0\n",
                hbp_escape(method)
            ),
        );
    }
    let json = json_opt_in(query);
    match path {
        "/health" => health(json),
        "/api/canon-index" => canon_index(shared, json),
        "/" | "/super-os" => (200, "text/plain; charset=utf-8", banner()),
        _ => (
            404,
            "text/plain; charset=utf-8",
            format!(
                "DASHSERVE|ok=0|error=unknown_route|path={}|served=/health,/api/canon-index|increment=1|json=0\n",
                hbp_escape(path)
            ),
        ),
    }
}

fn banner() -> String {
    format!(
        "DASHSERVE|service={SERVICE}|port={PORT}|host8=dashboard-serve|increment=1|served=/health,/api/canon-index|cold_egress=?format=json|staged=true|cutover=false|json=0\n"
    )
}

fn health(json: bool) -> (u16, &'static str, String) {
    let up = uptime_s();
    let ts = iso_now();
    if json {
        (
            200,
            "application/json",
            format!(
                "{{\"ok\":true,\"service\":\"{SERVICE}\",\"port\":{PORT},\"apex\":\"{APEX}\",\"operator_pair\":[\"OP-JESSE-PID\",\"OP-RAYSSA-PID\"],\"uptime_s\":{up},\"cohort_anchor\":\"{COHORT}\",\"ts\":\"{ts}\",\"host8\":\"dashboard-serve\",\"staged\":true}}"
            ),
        )
    } else {
        (
            200,
            "text/plain; charset=utf-8",
            format!(
                "DASHHEALTH|ok=1|service={SERVICE}|port={PORT}|apex={APEX}|operator_pair=OP-JESSE-PID,OP-RAYSSA-PID|uptime_s={up}|cohort_anchor={COHORT}|ts={ts}|host8=dashboard-serve|staged=true|json=0\n"
            ),
        )
    }
}

/// Extract distinct `](X.md)` markdown-link basenames from MEMORY.md (the "indexed" set).
fn extract_md_links(text: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let mut rest = text;
    while let Some(i) = rest.find("](") {
        let after = &rest[i + 2..];
        if let Some(j) = after.find(')') {
            let link = &after[..j];
            let base = link.rsplit(['/', '\\']).next().unwrap_or(link);
            if base.to_lowercase().ends_with(".md") {
                set.insert(base.to_string());
            }
            rest = &after[j + 1..];
        } else {
            break;
        }
    }
    set
}

/// Scan the memory dir for *.md, classify each as indexed (linked from MEMORY.md) or orphan.
/// Data-parity with the Node :4949 /api/canon-index (md_file_count / indexed_entry_count /
/// orphan_count / orphans[]). indexed + orphan == md_file_count by construction.
fn canon_index(shared: &Arc<Shared>, json: bool) -> (u16, &'static str, String) {
    let dir = &shared.memory_dir;
    let mut md_files: Vec<String> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                // The index file itself (MEMORY.md) is not a canon entry — exclude it (Node parity).
                // MEMORY-ARCHIVE.md is kept: it is linked from MEMORY.md, so it lands in `indexed`.
                if name.to_lowercase().ends_with(".md") && !name.eq_ignore_ascii_case("MEMORY.md") {
                    Some(name)
                } else {
                    None
                }
            })
            .collect(),
        Err(e) => {
            let kind = hbp_escape(&e.kind().to_string());
            return if json {
                (
                    500,
                    "application/json",
                    format!(
                        "{{\"ok\":false,\"error\":\"memory_dir_unreadable\",\"kind\":\"{kind}\"}}"
                    ),
                )
            } else {
                (
                    500,
                    "text/plain; charset=utf-8",
                    format!("DASHCANON|ok=0|error=memory_dir_unreadable|kind={kind}|json=0\n"),
                )
            };
        }
    };
    md_files.sort();
    let links = extract_md_links(&fs::read_to_string(dir.join("MEMORY.md")).unwrap_or_default());
    let orphans: Vec<&String> = md_files.iter().filter(|f| !links.contains(*f)).collect();
    let indexed_entry_count = md_files.len() - orphans.len();
    let md_file_count = md_files.len();
    let orphan_count = orphans.len();
    let dir_disp = dir.display().to_string();

    if json {
        let mut body = format!(
            "{{\"memory_dir\":\"{}\",\"md_file_count\":{md_file_count},\"indexed_entry_count\":{indexed_entry_count},\"orphan_count\":{orphan_count},\"orphans\":[",
            json_escape(&dir_disp)
        );
        for (i, o) in orphans.iter().enumerate() {
            if i > 0 {
                body.push(',');
            }
            body.push('"');
            body.push_str(&json_escape(o));
            body.push('"');
        }
        body.push_str("]}");
        (200, "application/json", body)
    } else {
        let mut body = format!(
            "DASHCANON|ok=1|memory_dir={}|md_file_count={md_file_count}|indexed_entry_count={indexed_entry_count}|orphan_count={orphan_count}|json=0\n",
            hbp_escape(&dir_disp)
        );
        for o in &orphans {
            body.push_str(&format!("DASHORPHAN|name={}|json=0\n", hbp_escape(o)));
        }
        (200, "text/plain; charset=utf-8", body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sh(dir: &str) -> Arc<Shared> {
        Arc::new(Shared {
            memory_dir: PathBuf::from(dir),
        })
    }

    #[test]
    fn health_default_is_json0_hbp() {
        let (code, ct, body) = route(&sh("."), "GET", "/health", "");
        assert_eq!(code, 200);
        assert_eq!(ct, "text/plain; charset=utf-8");
        assert!(body.contains("json=0"));
        assert!(body.contains("service=super-asolaria-os-dashboard"));
        assert!(!body.contains('{'));
    }

    #[test]
    fn health_json_opt_in_is_application_json() {
        let (code, ct, body) = route(&sh("."), "GET", "/health", "format=json");
        assert_eq!(code, 200);
        assert_eq!(ct, "application/json");
        assert!(body.starts_with("{\"ok\":true"));
        assert!(body.contains("\"service\":\"super-asolaria-os-dashboard\""));
        assert!(body.contains("\"port\":4949"));
    }

    #[test]
    fn unknown_route_is_404_json0() {
        let (code, _ct, body) = route(&sh("."), "GET", "/nope", "");
        assert_eq!(code, 404);
        assert!(body.contains("error=unknown_route"));
        assert!(body.ends_with("json=0\n"));
    }

    #[test]
    fn options_is_200_empty() {
        let (code, _ct, body) = route(&sh("."), "OPTIONS", "/health", "");
        assert_eq!(code, 200);
        assert!(body.is_empty());
    }

    #[test]
    fn no_post_or_fire_route() {
        for p in [
            "/health",
            "/api/canon-index",
            "/summon",
            "/fire",
            "/super-os",
        ] {
            assert_eq!(
                route(&sh("."), "POST", p, "fire=1").0,
                404,
                "no write/fire: {p}"
            );
        }
    }

    #[test]
    fn canon_index_counts_sum_to_total() {
        let dir = std::env::temp_dir().join(format!("asolaria-dash-canon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // MEMORY.md is the index (excluded from the scan, Node parity). Canon entries a.md + b.md;
        // MEMORY.md links a.md -> indexed=1 (a.md), orphan=1 (b.md), md_file_count=2.
        std::fs::write(dir.join("a.md"), "x").unwrap();
        std::fs::write(dir.join("b.md"), "y").unwrap();
        std::fs::write(dir.join("MEMORY.md"), "- [A](a.md)\n").unwrap();
        let (code, ct, body) = route(
            &sh(dir.to_str().unwrap()),
            "GET",
            "/api/canon-index",
            "format=json",
        );
        assert_eq!(code, 200);
        assert_eq!(ct, "application/json");
        assert!(body.contains("\"md_file_count\":2"));
        assert!(body.contains("\"indexed_entry_count\":1"));
        assert!(body.contains("\"orphan_count\":1"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn canon_index_missing_dir_is_500_not_clean_zero() {
        let (code, _ct, body) = route(
            &sh("__asolaria_missing_memory_dir__"),
            "GET",
            "/api/canon-index",
            "",
        );
        assert_eq!(code, 500);
        assert!(body.contains("error=memory_dir_unreadable"));
        assert!(!body.contains("md_file_count=0"));
    }
}
