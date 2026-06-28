//! lane_event — claw-code 2.0's lane-event provenance/ordering, absorbed into the Host-8 frame as the
//! BEHCS envelope (Rust, json=0). claw-code stamps every worker-lane event with provenance + ordering
//! so an interleaved multi-lane stream can be deterministically replayed, gaps (lost events) surfaced,
//! and duplicates (replays / double-logs) collapsed. Our frame: each event carries a BEHCS envelope —
//! `lane` (source seat), `host_handle8` (the 8-byte FNV1a64 Host-8 address of the lane), `seq`
//! (per-lane monotonic counter), `lamport` (cross-lane logical clock), `hash` (content integrity).
//!
//! This generalizes the lane_health `count_unique_terminal` dedupe: there it deduped re-logged
//! terminals; here EVERY event is ordered, gap-checked, dedup-checked, and provenance-checked.
//!
//! CLAIMS-GATE: an event stream is reported INTACT (replay-trustworthy) only if every lane is
//! contiguous (no missing seq), carries no duplicate (lane,seq), and no event has a host_handle8 that
//! contradicts FNV1a64(lane). A gap means events were LOST — never report "complete" over a gap; a
//! provenance MISMATCH means forged/corrupt addressing — never count it clean. Pure + read-only: it
//! ANALYZES — it never reorders the live stream, drops events, or fires.

use std::collections::BTreeMap;

/// The 8-byte Host-8 address of a lane: FNV-1a 64-bit over the lane id (the canonical handle the
/// federation kernel uses). Deterministic — the same lane always hashes to the same handle.
pub fn host_handle8(lane: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV offset basis
    for b in lane.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
    }
    h
}

/// Whether an event's claimed provenance handle matches its lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// host_handle8 was present AND equals FNV1a64(lane) — independently attested.
    Verified,
    /// host_handle8 was absent (0) — we derive it from the lane; usable but not independently attested.
    Derived,
    /// host_handle8 was present but DOES NOT equal FNV1a64(lane) — forged/corrupt addressing.
    Mismatch,
}

impl Provenance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provenance::Verified => "verified",
            Provenance::Derived => "derived",
            Provenance::Mismatch => "mismatch",
        }
    }
    /// Severity for picking a lane's worst status: mismatch > derived > verified.
    fn severity(&self) -> u8 {
        match self {
            Provenance::Mismatch => 2,
            Provenance::Derived => 1,
            Provenance::Verified => 0,
        }
    }
}

/// One lane event carrying its BEHCS envelope.
#[derive(Debug, Clone)]
pub struct Event {
    pub lane: String,
    /// Claimed Host-8 handle; 0 = absent (provenance will be Derived, not Verified).
    pub host_handle8: u64,
    pub seq: u64,
    pub lamport: u64,
    pub hash: String,
}

/// Classify one event's provenance against its lane.
pub fn provenance(e: &Event) -> Provenance {
    if e.host_handle8 == 0 {
        Provenance::Derived
    } else if e.host_handle8 == host_handle8(&e.lane) {
        Provenance::Verified
    } else {
        Provenance::Mismatch
    }
}

/// The deterministic canonical replay order: by lamport, then lane, then seq. Returns indices into
/// `events` (does not move the live stream). Total + stable — same input always yields the same order.
pub fn canonical_order(events: &[Event]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..events.len()).collect();
    idx.sort_by(|&a, &b| {
        events[a]
            .lamport
            .cmp(&events[b].lamport)
            .then_with(|| events[a].lane.cmp(&events[b].lane))
            .then_with(|| events[a].seq.cmp(&events[b].seq))
    });
    idx
}

/// The COUNT of missing seq numbers in a lane (the gaps = LOST events) between its min and max.
/// Computed ARITHMETICALLY — O(distinct), never materializing the dense `min..=max` range — so a
/// corrupt/huge seq span can never OOM or hang the endpoint (the adversarial-review DoS fix). Empty /
/// single / contiguous -> 0.
pub fn gap_count(seqs: &[u64]) -> u64 {
    if seqs.is_empty() {
        return 0;
    }
    let mut s: Vec<u64> = seqs.to_vec();
    s.sort_unstable();
    s.dedup();
    let span = s[s.len() - 1] - s[0]; // min >= 0 -> no underflow
    span - (s.len() as u64 - 1) // distinct-1 <= span -> no underflow; == (span+1) - distinct
}

/// Per-lane integrity summary.
#[derive(Debug, Clone)]
pub struct LaneSummary {
    pub lane: String,
    pub handle8: u64,
    pub count: usize,
    pub seq_min: u64,
    pub seq_max: u64,
    pub gap_count: usize,
    /// Re-logged (lane,seq) slots (a slot appearing more than once) — replays / double-logs.
    pub dup_count: usize,
    /// (lane,seq) slots that carry MORE THAN ONE distinct hash — forked/conflicting events claiming
    /// the same sequence slot (corruption), strictly worse than a benign same-hash replay.
    pub conflict_count: usize,
    pub provenance: Provenance,
    /// No gaps AND no dups AND no conflicts AND provenance != mismatch — this lane is replay-trustworthy.
    pub contiguous: bool,
}

/// Analyze a stream into per-lane summaries, lanes in deterministic (name) order. Pure: never mutates
/// or drops the live stream.
pub fn analyze(events: &[Event]) -> Vec<LaneSummary> {
    let mut by_lane: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
    for e in events {
        by_lane.entry(e.lane.as_str()).or_default().push(e);
    }
    by_lane
        .into_iter()
        .map(|(lane, evs)| {
            let seqs: Vec<u64> = evs.iter().map(|e| e.seq).collect();
            let gaps = gap_count(&seqs);
            // group the distinct hashes seen at each seq slot: 1 hash repeated = benign replay;
            // >1 distinct hash at a slot = a forked/conflicting event (corruption).
            let mut by_seq: BTreeMap<u64, std::collections::BTreeSet<&str>> = BTreeMap::new();
            for e in &evs {
                by_seq.entry(e.seq).or_default().insert(e.hash.as_str());
            }
            let dup_count = seqs.len() - by_seq.len(); // re-logged (lane,seq) slots
            let conflict_count = by_seq.values().filter(|hashes| hashes.len() > 1).count();
            let prov = evs
                .iter()
                .map(|e| provenance(e))
                .max_by_key(|p| p.severity())
                .unwrap_or(Provenance::Derived);
            let seq_min = seqs.iter().copied().min().unwrap_or(0);
            let seq_max = seqs.iter().copied().max().unwrap_or(0);
            LaneSummary {
                lane: lane.to_string(),
                handle8: host_handle8(lane),
                count: evs.len(),
                seq_min,
                seq_max,
                gap_count: gaps as usize,
                dup_count,
                conflict_count,
                provenance: prov,
                contiguous: gaps == 0
                    && dup_count == 0
                    && conflict_count == 0
                    && prov != Provenance::Mismatch,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(lane: &str, handle: u64, seq: u64, lamport: u64) -> Event {
        Event {
            lane: lane.to_string(),
            host_handle8: handle,
            seq,
            lamport,
            hash: format!("h{seq}"),
        }
    }

    #[test]
    fn host_handle8_is_deterministic_fnv1a64() {
        assert_eq!(host_handle8("x"), host_handle8("x"));
        assert_ne!(host_handle8("a"), host_handle8("b"));
        // FNV-1a of the empty string is the offset basis.
        assert_eq!(host_handle8(""), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn provenance_verified_derived_mismatch() {
        let lane = "fabric";
        let good = host_handle8(lane);
        assert_eq!(provenance(&ev(lane, good, 1, 1)), Provenance::Verified);
        assert_eq!(provenance(&ev(lane, 0, 1, 1)), Provenance::Derived); // absent -> derived
        assert_eq!(
            provenance(&ev(lane, good ^ 0xdead, 1, 1)),
            Provenance::Mismatch // present but wrong -> forged/corrupt
        );
    }

    #[test]
    fn canonical_order_is_lamport_then_lane_then_seq() {
        let evs = vec![
            ev("b", 0, 1, 5),
            ev("a", 0, 2, 5),
            ev("a", 0, 1, 5),
            ev("z", 0, 9, 1),
        ];
        let order = canonical_order(&evs);
        // lamport 1 first (z,9), then lamport 5 ordered by lane then seq: a/1, a/2, b/1
        let seq_lane: Vec<(&str, u64)> = order
            .iter()
            .map(|&i| (evs[i].lane.as_str(), evs[i].seq))
            .collect();
        assert_eq!(seq_lane, vec![("z", 9), ("a", 1), ("a", 2), ("b", 1)]);
    }

    #[test]
    fn canonical_order_is_deterministic() {
        let evs = vec![ev("a", 0, 3, 2), ev("a", 0, 1, 2), ev("a", 0, 2, 2)];
        assert_eq!(canonical_order(&evs), canonical_order(&evs));
    }

    #[test]
    fn gap_count_counts_missing_seqs() {
        assert_eq!(gap_count(&[1, 2, 3]), 0); // contiguous
        assert_eq!(gap_count(&[1, 2, 4, 7]), 3); // 3, 5, 6 missing
        assert_eq!(gap_count(&[]), 0);
        assert_eq!(gap_count(&[5]), 0); // single = no gap
        assert_eq!(gap_count(&[3, 1, 2]), 0); // unsorted input handled
        assert_eq!(gap_count(&[1, 1, 3]), 1); // dup doesn't create a phantom gap; only 2 missing
    }

    #[test]
    fn gap_count_huge_span_is_instant_no_dos() {
        // ADVERSARIAL-REVIEW FIX (blocker): a corrupt/huge seq span must NOT materialize a dense
        // range. This is pure arithmetic -> returns instantly, proving no O(max-min) allocation.
        assert_eq!(gap_count(&[0, u64::MAX]), u64::MAX - 1);
        assert_eq!(gap_count(&[1, 1_000_000_000]), 999_999_998);
    }

    #[test]
    fn analyze_clean_lane_is_contiguous() {
        let evs = vec![
            ev("fab", host_handle8("fab"), 1, 1),
            ev("fab", host_handle8("fab"), 2, 2),
            ev("fab", host_handle8("fab"), 3, 3),
        ];
        let a = analyze(&evs);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].lane, "fab");
        assert_eq!(a[0].count, 3);
        assert_eq!(a[0].gap_count, 0);
        assert_eq!(a[0].dup_count, 0);
        assert_eq!(a[0].provenance, Provenance::Verified);
        assert!(a[0].contiguous);
    }

    #[test]
    fn analyze_gap_breaks_contiguous() {
        // CLAIMS-GATE: a lost event (gap) must break contiguous — never report complete over a gap.
        let evs = vec![ev("x", 0, 1, 1), ev("x", 0, 2, 2), ev("x", 0, 5, 5)];
        let a = analyze(&evs);
        assert_eq!(a[0].gap_count, 2); // 3 and 4 lost
        assert!(!a[0].contiguous);
    }

    #[test]
    fn analyze_duplicate_breaks_contiguous_and_counts() {
        // generalizes the lane_health terminal-dedupe: a re-logged (lane,seq) is a duplicate.
        let evs = vec![ev("x", 0, 1, 1), ev("x", 0, 2, 2), ev("x", 0, 2, 3)];
        let a = analyze(&evs);
        assert_eq!(a[0].count, 3);
        assert_eq!(a[0].dup_count, 1); // seq 2 logged twice
        assert_eq!(a[0].gap_count, 0);
        assert!(!a[0].contiguous);
    }

    #[test]
    fn analyze_provenance_mismatch_breaks_contiguous() {
        // a forged handle on ANY event taints the lane and breaks trust, even if seqs are contiguous.
        let evs = vec![
            ev("x", host_handle8("x"), 1, 1),
            ev("x", 0xbad, 2, 2), // wrong handle
        ];
        let a = analyze(&evs);
        assert_eq!(a[0].provenance, Provenance::Mismatch);
        assert_eq!(a[0].gap_count, 0);
        assert!(!a[0].contiguous); // contiguous seqs but corrupt provenance -> not trustworthy
    }

    #[test]
    fn analyze_lanes_in_deterministic_name_order() {
        let evs = vec![
            ev("zeta", 0, 1, 1),
            ev("alpha", 0, 1, 1),
            ev("mid", 0, 1, 1),
        ];
        let a = analyze(&evs);
        let names: Vec<&str> = a.iter().map(|l| l.lane.as_str()).collect();
        assert_eq!(names, vec!["alpha", "mid", "zeta"]);
    }
}
