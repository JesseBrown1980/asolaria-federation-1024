//! Fischer evaluator kernel for Host-8.
//!
//! Port target for the live Node Fischer runtime observed on `:4794`.
//! Preserves the CPL evaluator shape from `fischer-kernel.mjs`: illegal tier,
//! refute tier, CPL penalties/gains, G4 GLSM modifiers, verdict glyphs,
//! candidate counts, best alternatives, HBP row output, and no self-auth.

#![forbid(unsafe_code)]

pub const PAYLOAD_BLOAT_BYTES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FischerVerdict {
    Proceed,
    Hold,
    Block,
    Refute,
    Analyze,
}

impl FischerVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proceed => "PROCEED",
            Self::Hold => "HOLD",
            Self::Block => "BLOCK",
            Self::Refute => "REFUTE",
            Self::Analyze => "ANALYZE",
        }
    }

    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Proceed => "FP",
            Self::Hold => "FH",
            Self::Block => "FB",
            Self::Refute => "FR",
            Self::Analyze => "FA",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FischerEnvelope {
    pub pid: Option<String>,
    pub actor: String,
    pub verb: String,
    pub target: String,
    pub payload: String,
    pub payload_json_true: bool,
    pub hbp_path: Option<String>,
    pub sidecar_plan: Option<String>,
    pub ledger_path: Option<String>,
    pub tuple: Option<String>,
    pub cube_47d: Option<String>,
    pub cosign: Option<String>,
    pub halt_path: Option<String>,
    pub authority_jump: bool,
    pub recursive_consent: bool,
    pub operator_witness_required: bool,
    pub operator_witness: bool,
}

impl Default for FischerEnvelope {
    fn default() -> Self {
        Self {
            pid: Some("BH.FISCHER.HOST8.DEFAULT".to_string()),
            actor: "fischer-eval-host8".to_string(),
            verb: "evaluate".to_string(),
            target: "fischer-kernel".to_string(),
            payload: String::new(),
            payload_json_true: false,
            hbp_path: Some("fischer-eval-host8-ledger.hbp".to_string()),
            sidecar_plan: Some("fischer-eval-host8".to_string()),
            ledger_path: Some("fischer-eval-host8-ledger.hbp".to_string()),
            tuple: None,
            cube_47d: None,
            cosign: None,
            halt_path: Some("/halt".to_string()),
            authority_jump: false,
            recursive_consent: false,
            operator_witness_required: false,
            operator_witness: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FischerScore {
    pub composite: String,
    pub l0_real: bool,
    /// Shannon signal as q in 0..=1000 (per-mille; integer only).
    pub shannon_q: u16,
    pub g4_state: String,
}

impl Default for FischerScore {
    fn default() -> Self {
        Self {
            composite: "0".to_string(),
            l0_real: false,
            shannon_q: 0,
            g4_state: "UNKNOWN".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisScores {
    /// All three axes are q in 0..=1000 (per-mille; integer only).
    pub king_safety_q: u16,
    pub center_gain_q: u16,
    pub proof_gain_q: u16,
    pub authority_debt: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FischerEval {
    pub pid: String,
    pub move_name: String,
    pub verdict: FischerVerdict,
    pub cpl: i32,
    pub flags: Vec<&'static str>,
    pub axes: AxisScores,
    pub best_alt: &'static str,
    pub candidate_count: u8,
    pub glyph: &'static str,
    pub voxel_coord: String,
    pub g4_state: String,
    pub pass: bool,
    pub promoted: bool,
    pub l0_real: bool,
    pub composite: String,
}

struct HardEvalSpec {
    pid: String,
    move_name: String,
    verdict: FischerVerdict,
    cpl: i32,
    flags: Vec<&'static str>,
    best_alt: &'static str,
    candidate_count: u8,
    voxel_coord: String,
    g4_state: String,
}

pub fn evaluate(envelope: &FischerEnvelope, score: &FischerScore, strict: bool) -> FischerEval {
    let pid = envelope
        .pid
        .as_deref()
        .filter(|pid| !pid.is_empty())
        .unwrap_or("UNKNOWN")
        .to_string();
    let move_name = if envelope.verb.is_empty() {
        "unknown".to_string()
    } else {
        envelope.verb.clone()
    };
    let voxel_coord = pid_to_voxel_coord(&pid);
    let g4_state = if score.g4_state.is_empty() {
        "UNKNOWN".to_string()
    } else {
        score.g4_state.clone()
    };

    if g4_state == "MISTAKE_FLAGGED" {
        return hard_eval(
            HardEvalSpec {
                pid,
                move_name,
                verdict: FischerVerdict::Block,
                cpl: 999,
                flags: vec!["glsm_mistake_flagged"],
                best_alt: "analyze_in_white_room",
                candidate_count: derive_candidate_count(&envelope.verb),
                voxel_coord,
                g4_state,
            },
            score,
        );
    }

    if envelope.pid.as_deref().unwrap_or("").is_empty() {
        return hard_eval(
            HardEvalSpec {
                pid,
                move_name,
                verdict: FischerVerdict::Block,
                cpl: 500,
                flags: vec!["missing_pid"],
                best_alt: derive_best_alt(&["missing_pid"], &envelope.verb, &g4_state),
                candidate_count: derive_candidate_count(&envelope.verb),
                voxel_coord,
                g4_state,
            },
            score,
        );
    }
    if envelope.verb.is_empty() {
        return hard_eval(
            HardEvalSpec {
                pid,
                move_name,
                verdict: FischerVerdict::Block,
                cpl: 500,
                flags: vec!["missing_verb"],
                best_alt: derive_best_alt(&["missing_verb"], &envelope.verb, &g4_state),
                candidate_count: derive_candidate_count(&envelope.verb),
                voxel_coord,
                g4_state,
            },
            score,
        );
    }

    if check_refuted(envelope) {
        return hard_eval(
            HardEvalSpec {
                pid,
                move_name,
                verdict: FischerVerdict::Refute,
                cpl: 999,
                flags: vec!["refuted_pattern"],
                best_alt: "halt_and_request_human_apex",
                candidate_count: derive_candidate_count(&envelope.verb),
                voxel_coord,
                g4_state,
            },
            score,
        );
    }

    let (raw_cpl, flags) = compute_cpl(envelope, score);
    let cpl = (raw_cpl + glsm_cpl_mod(&g4_state)).max(0);
    let needs_analyze = !strict
        && (is_write_verb(&envelope.verb) || is_spawn_verb(&envelope.verb))
        && flags.contains(&"no_replay_path");
    let verdict = if cpl >= 500 {
        FischerVerdict::Block
    } else if cpl >= 150 {
        if needs_analyze {
            FischerVerdict::Analyze
        } else {
            FischerVerdict::Hold
        }
    } else {
        FischerVerdict::Proceed
    };
    let axes = axis_scores(envelope, score);
    let best_alt = derive_best_alt(&flags, &envelope.verb, &g4_state);
    FischerEval {
        pid,
        move_name,
        verdict,
        cpl,
        flags,
        axes,
        best_alt,
        candidate_count: derive_candidate_count(&envelope.verb),
        glyph: verdict.glyph(),
        voxel_coord,
        g4_state,
        pass: matches!(verdict, FischerVerdict::Proceed | FischerVerdict::Analyze),
        promoted: verdict == FischerVerdict::Proceed && cpl < 50,
        l0_real: score.l0_real,
        composite: score.composite.clone(),
    }
}

fn hard_eval(spec: HardEvalSpec, score: &FischerScore) -> FischerEval {
    FischerEval {
        pid: spec.pid,
        move_name: spec.move_name,
        verdict: spec.verdict,
        cpl: spec.cpl,
        flags: spec.flags,
        axes: AxisScores {
            king_safety_q: 0.0,
            center_gain_q: 0.0,
            proof_gain_q: 0.0,
            authority_debt: 2,
        },
        best_alt: spec.best_alt,
        candidate_count: spec.candidate_count,
        glyph: spec.verdict.glyph(),
        voxel_coord: spec.voxel_coord,
        g4_state: spec.g4_state,
        pass: false,
        promoted: false,
        l0_real: score.l0_real,
        composite: score.composite.clone(),
    }
}

fn compute_cpl(envelope: &FischerEnvelope, score: &FischerScore) -> (i32, Vec<&'static str>) {
    let mut cpl = 0;
    let mut hard_floor = 0;
    let mut flags = Vec::new();
    let verb = envelope.verb.as_str();
    let has_proof_path = has_any(&[
        envelope.hbp_path.as_deref(),
        envelope.sidecar_plan.as_deref(),
        envelope.ledger_path.as_deref(),
    ]);

    if is_cosign_required(verb) && envelope.cosign.as_deref().unwrap_or("").is_empty() {
        cpl += 250;
        hard_floor = hard_floor.max(150);
        flags.push("missing_cosign");
    }
    if envelope.authority_jump {
        cpl += 400;
        hard_floor = hard_floor.max(400);
        flags.push("authority_jump");
    }

    if is_spawn_verb(verb) && envelope.halt_path.as_deref().unwrap_or("").is_empty() {
        cpl += 350;
        hard_floor = hard_floor.max(200);
        flags.push("missing_halt_path");
    }
    if envelope.recursive_consent {
        cpl += 400;
        hard_floor = hard_floor.max(400);
        flags.push("recursive_consent");
    }
    if envelope.operator_witness_required && !envelope.operator_witness {
        cpl += 300;
        hard_floor = hard_floor.max(150);
        flags.push("missing_operator_witness");
    }

    if !has_proof_path {
        cpl += 180;
        hard_floor = hard_floor.max(100);
        flags.push("no_replay_path");
    } else {
        cpl -= 160;
        flags.push("proof_gain_q");
    }
    if envelope.target.is_empty() {
        cpl += 80;
        flags.push("route_ambiguity");
    }
    if score.l0_real {
        cpl -= 100;
        flags.push("gnn_observable");
    }

    if is_write_verb(verb) && !is_compact_verb(verb) && !has_proof_path {
        cpl += 220;
        flags.push("unsealed_write");
    }
    if envelope.payload.len() > PAYLOAD_BLOAT_BYTES {
        cpl += 100;
        flags.push("bloat_delta");
    }
    if score.shannon_q > 0.92 && !matches!(verb, "write" | "export" | "generate") {
        cpl += 120;
        flags.push("entropy_delta");
    }

    if envelope.pid.as_deref().unwrap_or("").starts_with("BH.") {
        cpl -= 140;
        flags.push("reproducible");
    }
    if has_any(&[envelope.cube_47d.as_deref(), envelope.tuple.as_deref()]) {
        cpl -= 80;
        flags.push("cube_local");
    }
    if is_compact_verb(verb) {
        cpl -= 120;
        flags.push("compaction_gain");
    }

    (0.max(cpl.max(hard_floor)), flags)
}

fn axis_scores(envelope: &FischerEnvelope, score: &FischerScore) -> AxisScores {
    let verb = envelope.verb.as_str();
    let has_halt = if is_spawn_verb(verb) {
        !envelope.halt_path.as_deref().unwrap_or("").is_empty()
    } else {
        true
    };
    let has_proof = has_any(&[
        envelope.hbp_path.as_deref(),
        envelope.sidecar_plan.as_deref(),
        envelope.ledger_path.as_deref(),
    ]);
    let has_cosign =
        !is_cosign_required(verb) || !envelope.cosign.as_deref().unwrap_or("").is_empty();
    let has_target = !envelope.target.is_empty();
    let has_cube = has_any(&[envelope.cube_47d.as_deref(), envelope.tuple.as_deref()]);
    let center_gain_q = (if has_proof { 0.4 } else { 0.0 })
        + (if has_target { 0.3 } else { 0.0 })
        + (if score.l0_real { 0.2 } else { 0.0 })
        + (if has_cube { 0.1 } else { 0.0 });

    AxisScores {
        king_safety_q: if has_halt && !envelope.authority_jump && !envelope.recursive_consent {
            1.0
        } else {
            0.0
        },
        center_gain_q: (center_gain_q * 1000.0_f64).round() / 1000.0,
        proof_gain_q: if has_proof { 1.0 } else { 0.0 },
        authority_debt: u8::from(envelope.authority_jump) + u8::from(!has_cosign),
    }
}

pub fn render_row(eval: &FischerEval, prev_hash: &str, ts: &str) -> String {
    let flags = if eval.flags.is_empty() {
        String::new()
    } else {
        eval.flags.join(",")
    };
    let mut fields = vec![
        "FISCHERv1".to_string(),
        format!("pid={}", hbp_escape(&eval.pid)),
        format!("move={}", hbp_escape(&eval.move_name)),
        format!("verdict={}", eval.verdict.as_str()),
        format!("cpl={}", eval.cpl),
        format!("candidate_count={}", eval.candidate_count),
        format!("best_alt={}", eval.best_alt),
        format!("king_safety_q={:.3}", eval.axes.king_safety_q),
        format!("center_gain_q={:.3}", eval.axes.center_gain_q),
        format!("proof_gain_q={:.3}", eval.axes.proof_gain_q),
        format!("authority_debt={}", eval.axes.authority_debt),
        format!("g4_state={}", hbp_escape(&eval.g4_state)),
        format!("voxel_coord={}", hbp_escape(&eval.voxel_coord)),
        format!("glyph={}", eval.glyph),
        format!("flags={}", hbp_escape(&flags)),
        format!("l0_real={}", bit(eval.l0_real)),
        format!("composite={}", hbp_escape(&eval.composite)),
        format!("prev_hash={}", hbp_escape(prev_hash)),
        format!("ts={}", hbp_escape(ts)),
        "json=0".to_string(),
        "runtime=0".to_string(),
    ];
    let row_hash = sha8(&fields.join("|"));
    fields.push(format!("row_hash={}", row_hash));
    fields.join("|") + "\n"
}

pub fn render_hbi(eval: &FischerEval) -> String {
    let flags = if eval.flags.is_empty() {
        "clean".to_string()
    } else {
        eval.flags.join(" · ")
    };
    format!(
        "FISCHER HBI\npid        : {}\nmove       : {}\nverdict    : {}  (cpl={})  glyph={}\ng4_state   : {}\nvoxel_coord: {}\nbest_alt   : {}\ncandidates : {}\nking_safe  : {:.3}  center: {:.3}  proof: {:.3}\nauth_debt  : {}\nflags      : {}\n",
        eval.pid,
        eval.move_name,
        eval.verdict.as_str(),
        eval.cpl,
        eval.glyph,
        eval.g4_state,
        eval.voxel_coord,
        eval.best_alt,
        eval.candidate_count,
        eval.axes.king_safety_q,
        eval.axes.center_gain_q,
        eval.axes.proof_gain_q,
        eval.axes.authority_debt,
        flags
    )
}

pub fn parse_bool(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true" | "TRUE" | "yes" | "YES"))
}

/// Parse a q value (per-mille integer, 0..=1000) from a string field. Integer only:
/// replaces the previous `parse_q`. Values outside 0..=1000 fall back to `default`.
pub fn parse_q(value: Option<&str>, default: u16) -> u16 {
    value
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|q| *q <= 1000)
        .unwrap_or(default)
}

pub fn hbp_escape(s: &str) -> String {
    s.replace(['|', '\r', '\n', '\t'], "_")
}

pub fn sha8(input: &str) -> String {
    let digest = sha256(input.as_bytes());
    let mut out = String::with_capacity(8);
    for byte in digest.iter().take(4) {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn sha16(input: &str) -> String {
    let digest = sha256(input.as_bytes());
    let mut out = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut msg = Vec::with_capacity(input.len() + 72);
    msg.extend_from_slice(input);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let j = i * 4;
            *word = u32::from_be_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn pid_to_voxel_coord(pid: &str) -> String {
    let h = sha16(pid);
    let x = u16::from_str_radix(&h[0..4], 16).unwrap_or(0) % 16;
    let y = u16::from_str_radix(&h[4..8], 16).unwrap_or(0) % 16;
    let z = u16::from_str_radix(&h[8..12], 16).unwrap_or(0) % 16;
    format!("BH3D:{}-{}-{}", x, y, z)
}

fn check_refuted(envelope: &FischerEnvelope) -> bool {
    is_refuted_verb(&envelope.verb)
        || (!envelope.actor.is_empty()
            && envelope.actor == envelope.target
            && is_promotion_verb(&envelope.verb))
        || envelope.payload_json_true
        || (envelope.authority_jump && envelope.cosign.as_deref().unwrap_or("").is_empty())
}

fn derive_best_alt(flags: &[&str], verb: &str, glsm_state: &str) -> &'static str {
    if glsm_state == "MISTAKE_FLAGGED" {
        return "analyze_in_white_room";
    }
    if flags.contains(&"authority_jump") {
        return "escalate_via_cosign_ring";
    }
    if flags.contains(&"recursive_consent") {
        return "halt_and_request_human_apex";
    }
    if flags.contains(&"missing_cosign") {
        return "hold_for_cosign";
    }
    if flags.contains(&"missing_halt_path") {
        return "declare_halt_path_first";
    }
    if flags.contains(&"unsealed_write") {
        return "seal_with_hbp_before_write";
    }
    if flags.contains(&"no_replay_path") {
        return "declare_ledger_path_first";
    }
    if flags.contains(&"missing_operator_witness") {
        return "request_operator_witness";
    }
    if flags.contains(&"bloat_delta") {
        return "compact_payload_to_tuple";
    }
    if is_spawn_verb(verb) {
        return "send_to_white_room_first";
    }
    if is_write_verb(verb) {
        return "compact_then_seal";
    }
    "hold_for_analysis"
}

fn derive_candidate_count(verb: &str) -> u8 {
    if is_spawn_verb(verb) {
        4
    } else if is_write_verb(verb) || is_cosign_required(verb) {
        3
    } else {
        2
    }
}

fn glsm_cpl_mod(state: &str) -> i32 {
    match state {
        "EDGE_MINED" => -40,
        "PATH_FOUND" => -80,
        "CONVERGED" => -200,
        "MISTAKE_FLAGGED" => 999,
        _ => 0,
    }
}

fn has_any(values: &[Option<&str>]) -> bool {
    values
        .iter()
        .any(|v| v.map(|s| !s.is_empty()).unwrap_or(false))
}

fn bit(value: bool) -> u8 {
    if value {
        1
    } else {
        0
    }
}

fn is_write_verb(v: &str) -> bool {
    matches!(
        v,
        "write" | "delete" | "compact" | "seal" | "upsert" | "drop" | "migrate"
    )
}

fn is_spawn_verb(v: &str) -> bool {
    matches!(
        v,
        "spawn" | "scale" | "mint" | "fork" | "launch" | "bootstrap"
    )
}

fn is_compact_verb(v: &str) -> bool {
    matches!(v, "compact" | "gc" | "drain" | "prune" | "archive")
}

fn is_cosign_required(v: &str) -> bool {
    matches!(
        v,
        "promote" | "mint" | "seal" | "authorize" | "cosign" | "scale"
    )
}

fn is_refuted_verb(v: &str) -> bool {
    matches!(
        v,
        "self_authorize"
            | "bypass_hookwall"
            | "recursive_consent"
            | "force_promote"
            | "skip_cosign"
            | "delete_evidence"
            | "disable_halt"
            | "grant_authority"
    )
}

fn is_promotion_verb(v: &str) -> bool {
    matches!(v, "promote" | "mint" | "seal" | "authorize" | "cosign")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PID: &str = "BH.HOOKWALL.TEST000000000001";

    fn clean(overrides: impl FnOnce(&mut FischerEnvelope)) -> FischerEnvelope {
        let mut env = FischerEnvelope {
            pid: Some(PID.to_string()),
            actor: "helm".to_string(),
            verb: "tick".to_string(),
            target: "fabric".to_string(),
            payload: "heartbeat".to_string(),
            hbp_path: Some("D:/bigpickle-rebuild/ledger/test.hbp".to_string()),
            halt_path: Some("/halt".to_string()),
            ..FischerEnvelope::default()
        };
        overrides(&mut env);
        env
    }

    fn clean_score() -> FischerScore {
        FischerScore {
            composite: "0.85".to_string(),
            l0_real: true,
            shannon_q: 0.55,
            g4_state: "UNKNOWN".to_string(),
        }
    }

    #[test]
    fn proceed_on_clean_envelope_with_proof_path_and_pid() {
        let r = evaluate(&clean(|_| {}), &clean_score(), false);
        assert_eq!(r.verdict, FischerVerdict::Proceed);
        assert!(r.cpl < 150);
        assert!(r.pass);
    }

    #[test]
    fn block_on_missing_pid() {
        let r = evaluate(&clean(|env| env.pid = None), &clean_score(), false);
        assert_eq!(r.cpl, 500);
        assert_eq!(r.verdict, FischerVerdict::Block);
        assert!(r.flags.contains(&"missing_pid"));
    }

    #[test]
    fn refute_on_self_authorize() {
        let r = evaluate(
            &clean(|env| env.verb = "self_authorize".to_string()),
            &clean_score(),
            false,
        );
        assert_eq!(r.verdict, FischerVerdict::Refute);
        assert_eq!(r.cpl, 999);
        assert!(!r.pass);
    }

    #[test]
    fn missing_cosign_does_not_proceed() {
        let r = evaluate(
            &clean(|env| {
                env.verb = "promote".to_string();
                env.cosign = None;
            }),
            &clean_score(),
            false,
        );
        assert!(r.cpl >= 150);
        assert_ne!(r.verdict, FischerVerdict::Proceed);
    }

    #[test]
    fn spawn_without_halt_path_cannot_proceed() {
        let r = evaluate(
            &clean(|env| {
                env.verb = "spawn".to_string();
                env.halt_path = None;
            }),
            &clean_score(),
            false,
        );
        assert!(r.flags.contains(&"missing_halt_path"));
        assert!(r.cpl >= 200);
        assert_ne!(r.verdict, FischerVerdict::Proceed);
    }

    #[test]
    fn write_without_proof_analyzes_or_blocks() {
        let r = evaluate(
            &clean(|env| {
                env.verb = "write".to_string();
                env.hbp_path = None;
                env.sidecar_plan = None;
                env.ledger_path = None;
            }),
            &FischerScore {
                composite: "0.40".to_string(),
                l0_real: false,
                shannon_q: 0.55,
                g4_state: "UNKNOWN".to_string(),
            },
            false,
        );
        assert!(matches!(
            r.verdict,
            FischerVerdict::Analyze | FischerVerdict::Block | FischerVerdict::Hold
        ));
        assert!(r.flags.contains(&"unsealed_write") || r.flags.contains(&"no_replay_path"));
    }

    #[test]
    fn glsm_mistake_flagged_forces_block() {
        let r = evaluate(
            &clean(|_| {}),
            &FischerScore {
                g4_state: "MISTAKE_FLAGGED".to_string(),
                ..clean_score()
            },
            false,
        );
        assert_eq!(r.verdict, FischerVerdict::Block);
        assert_eq!(r.cpl, 999);
        assert_eq!(r.best_alt, "analyze_in_white_room");
    }

    #[test]
    fn row_is_hbp_json0_and_uses_move_not_verb() {
        let r = evaluate(&clean(|_| {}), &clean_score(), false);
        let row = render_row(&r, "0000000000000000", "2026-06-24T00:00:00Z");
        assert!(row.starts_with("FISCHERv1|"));
        assert!(row.contains("|move=tick|"));
        assert!(row.contains("|json=0|"));
        assert!(row.contains("|runtime=0|"));
        assert!(row.contains("|row_hash="));
        assert!(!row.contains("|verb="));
        assert!(!row.contains('{'));
    }

    #[test]
    fn meta_shape_matches_node_contract() {
        let r = evaluate(&clean(|_| {}), &clean_score(), false);
        assert!(r.voxel_coord.starts_with("BH3D:"));
        assert_eq!(r.candidate_count, 2);
        assert!(!render_hbi(&r).contains('{'));
        assert_eq!(sha8("abc"), "ba7816bf");
    }
}
