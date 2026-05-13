//! BUS-AND-KICK primitive · BEHCS-256 absorb (partial Rust port)
//!
//! Source: `C:/asolaria-behcs-256/packages/bus-and-kick/src/primitive.mjs`
//! sha16=`e2232ed5a3ea4405`, 5603B · `PROF-BUS-AND-KICK-SUPERVISOR`, Hilbert room 29
//!
//! Per Vanguard scout #2 verdict: port envelope composition + verb semantics; HTTP I/O stays Node.js.
//!
//! Jesse rule (canonized 2026-04-20):
//!   - **bus = content** · heartbeat is bus-only, no kick (LAW-001: do not steal user attention)
//!   - **keyboard = wake-kick** · only for action-required directives
//!
//! Three canonical actions:
//!   - `postToBus(env)` — content delivery, bus-only
//!   - `kickPeer(peer, text)` — physical wake-kick via supervised type, **sparingly**
//!   - `postAndKick(peer, env, kickText)` — combo for action-required cross-peer

use alloc::format;
use alloc::string::{String, ToString};

/// Canonical Hilbert-room number for bus-and-kick supervisor (per liris source line 1).
pub const BUS_AND_KICK_HILBERT_ROOM: u32 = 29;

/// PROF supervisor name (per JS source).
pub const BUS_AND_KICK_PROF: &str = "PROF-BUS-AND-KICK-SUPERVISOR";

/// Action class — distinguishes content delivery from attention-stealing wake-kick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionClass {
    /// Bus-only — content delivery, no wake-kick.
    BusOnly,
    /// Wake-kick only — physical keyboard input, no bus envelope.
    KickOnly,
    /// Combo — bus content + wake-kick (action-required).
    PostAndKick,
    /// Heartbeat — bus-only cadence ping per Jesse rule.
    Heartbeat,
}

/// Classify an envelope verb into the correct action class per Jesse rule.
pub fn classify_verb(verb: &str) -> ActionClass {
    let v = verb.to_uppercase();
    if v.contains("HEARTBEAT") || v.contains("PULSE_LOOP") || v.contains("STEADY") {
        ActionClass::Heartbeat
    } else if v.contains("ACTIONABLE")
        || v.contains("DIRECTIVE")
        || v.contains("WAKE")
        || v.contains("ALERT")
        || v.contains("KICK_REQUIRED")
    {
        ActionClass::PostAndKick
    } else {
        ActionClass::BusOnly
    }
}

/// Bus-and-kick errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BatkErr {
    /// Peer name unknown.
    UnknownPeer,
    /// Bearer token missing (peer-tokens.json not loadable).
    BearerMissing,
    /// Supervisor nonce mint failed.
    NonceMintFailed,
    /// Stub not yet wired (HTTP I/O lives in userspace per microkernel discipline).
    Unimplemented,
}

/// An envelope ready for postToBus, with the JS-original field set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedEnvelope {
    pub id: String,
    pub from: String,
    pub to: String,
    pub mode: String,
    pub actor: String,
    pub target: String,
    pub ts_iso: String,
    pub verb: String,
    pub payload: String,
}

impl ComposedEnvelope {
    /// Build with defaults matching JS `postToBus()` default-field-filling at lines 46-55.
    pub fn compose(
        envelope_id_hint: Option<&str>,
        from: &str,
        to: &str,
        actor: &str,
        target: &str,
        ts_iso: &str,
        verb: &str,
        payload: &str,
    ) -> Self {
        let id = match envelope_id_hint {
            Some(s) => s.to_string(),
            None => format!("{}-batk-{}", from, ts_iso),
        };
        Self {
            id,
            from: from.to_string(),
            to: to.to_string(),
            mode: "real".to_string(),
            actor: actor.to_string(),
            target: target.to_string(),
            ts_iso: ts_iso.to_string(),
            verb: verb.to_string(),
            payload: payload.to_string(),
        }
    }
}

/// Heartbeat-envelope verb canon (per JS line 111-117).
pub fn heartbeat_verb_for(host: &str) -> String {
    format!("EVT-{}-HEARTBEAT", host.to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hilbert_room_is_29() {
        assert_eq!(BUS_AND_KICK_HILBERT_ROOM, 29);
    }

    #[test]
    fn classify_heartbeat_is_bus_only() {
        assert_eq!(classify_verb("EVT-ACER-HEARTBEAT"), ActionClass::Heartbeat);
        assert_eq!(
            classify_verb("LIRIS_HEARTBEAT_STEADY"),
            ActionClass::Heartbeat
        );
    }

    #[test]
    fn classify_directive_is_post_and_kick() {
        assert_eq!(
            classify_verb("EVT-LIRIS-DIRECTIVE-RESPAWN"),
            ActionClass::PostAndKick
        );
        assert_eq!(classify_verb("WAKE_LIRIS_NOW"), ActionClass::PostAndKick);
    }

    #[test]
    fn classify_misc_is_bus_only() {
        assert_eq!(
            classify_verb("EVT-ACER-PHASE-2-CARGO-SCAFFOLD-LANDED"),
            ActionClass::BusOnly
        );
        assert_eq!(
            classify_verb("ACER_COSIGN_LIRIS_BUS_BIAS_RECEIPTS"),
            ActionClass::BusOnly
        );
    }

    #[test]
    fn compose_with_id_hint() {
        let e = ComposedEnvelope::compose(
            Some("custom-id"),
            "acer",
            "liris",
            "test-actor",
            "liris",
            "2026-05-11T18:00:00Z",
            "EVT-TEST",
            "payload-string",
        );
        assert_eq!(e.id, "custom-id");
        assert_eq!(e.mode, "real");
        assert_eq!(e.from, "acer");
        assert_eq!(e.to, "liris");
    }

    #[test]
    fn compose_without_id_generates_one() {
        let e = ComposedEnvelope::compose(
            None,
            "acer",
            "liris",
            "test-actor",
            "liris",
            "2026-05-11T18:00:00Z",
            "EVT-TEST",
            "p",
        );
        assert!(e.id.starts_with("acer-batk-"));
    }

    #[test]
    fn heartbeat_verb_uppercases() {
        assert_eq!(heartbeat_verb_for("acer"), "EVT-ACER-HEARTBEAT");
        assert_eq!(heartbeat_verb_for("falcon"), "EVT-FALCON-HEARTBEAT");
    }
}
