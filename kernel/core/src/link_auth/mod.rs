//! Cross-host / cross-colony LINK AUTH — the shared-secret + owner-human-PID consent gate.
//!
//! The operator model: every computer has a login; an owner-human-PID owns its machine (OP-JESSE
//! owns acer, OP-RAYSSA owns liris); a host answers another host's back-end query (dashboard /
//! search / fabric) ONLY IF the request both (a) carries a valid HMAC proving the SHARED SECRET
//! KEY the two owners hold, and (b) names an owner-PID this host has CONSENTED to link with. Like
//! two people on a social network with their own logins — your machine is yours, and you choose
//! who may cross-search it.
//!
//! Because the link is KEY-GATED, it is PRIVATE between the key holders: PII may flow between
//! consented owners' OWN machines (operator-authorized — Jesse + Rayssa share everything by mutual
//! agreement), while the public can never query it (no key → DenyBadHmac). The key is the carve-out.
//!
//! SECURITY INVARIANTS:
//!  - The shared key VALUE is supplied at runtime (loaded from a local secret file / env) and is
//!    NEVER hardcoded here, logged, or committed. This module only verifies a key it is handed.
//!  - The verb is bound into the signed message, so a "search" grant cannot be replayed as a
//!    "fabric-write".
//!  - A timestamp + nonce bound the request against replay (freshness window).
//!  - HMAC comparison runs over all 32 bytes without early-out (constant-time-ish).
//!
//! `no_std`, alloc-free: feeds request parts straight into the HMAC, no concatenation buffer.

use sha2::{Digest, Sha256};

/// SHA-256 block size (HMAC key-padding block).
const HMAC_BLOCK: usize = 64;

/// HMAC-SHA256 over a sequence of byte parts (the canonical message is the parts in order).
/// Alloc-free: the parts are hashed in place. Matches RFC 2104 / RFC 4231.
pub fn hmac_sha256_parts(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    // Key normalization: keys longer than the block are hashed first; then zero-padded to block.
    let mut k0 = [0u8; HMAC_BLOCK];
    if key.len() > HMAC_BLOCK {
        let kh = Sha256::digest(key);
        k0[..32].copy_from_slice(&kh);
    } else {
        k0[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0u8; HMAC_BLOCK];
    let mut opad = [0u8; HMAC_BLOCK];
    let mut i = 0;
    while i < HMAC_BLOCK {
        ipad[i] = k0[i] ^ 0x36;
        opad[i] = k0[i] ^ 0x5c;
        i += 1;
    }
    // inner = H(ipad || msg-parts)
    let mut inner = Sha256::new();
    inner.update(ipad);
    for p in parts {
        inner.update(p);
    }
    let inner_digest = inner.finalize();
    // outer = H(opad || inner)
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_digest);
    let out = outer.finalize();
    let mut mac = [0u8; 32];
    mac.copy_from_slice(&out);
    mac
}

/// A cross-host link request presented to this host's back-end gate.
#[derive(Debug, Clone, Copy)]
pub struct LinkRequest<'a> {
    /// Requesting owner-human PID (the OP / SPECIAL-OP, e.g. "OP-JESSE" / "OP-RAYSSA").
    pub owner_pid: &'a str,
    /// Requesting host / colony (e.g. "acer" / "liris").
    pub host: &'a str,
    /// Verb / scope being requested (e.g. "search", "dashboard", "fabric-query") — bound into the sig.
    pub verb: &'a str,
    /// Per-request nonce (anti-replay).
    pub nonce: &'a str,
    /// Request unix timestamp (seconds) — freshness checked against the host clock.
    pub ts_unix_s: u64,
    /// Lowercase-hex HMAC-SHA256 the requester computed over the canonical message with the shared key.
    pub hmac_hex: &'a str,
}

/// Link-gate verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkVerdict {
    /// Key proven + owner consented + fresh → cross-query allowed.
    Allow,
    /// HMAC did not verify against the shared key (no key / wrong key / tampered).
    DenyBadHmac,
    /// Owner-PID is not in this host's consent allowlist (owner did not allow the link).
    DenyOwnerNotGranted,
    /// Request timestamp outside the freshness window (stale / replay).
    DenyStale,
    /// Required field empty or hmac not 64 hex chars.
    DenyMalformed,
}

/// Verify a cross-host link request. `shared_key` is the secret both owners hold (loaded at runtime,
/// never hardcoded). `granted_owner_pids` is THIS host's consent allowlist — the owner-PIDs whose
/// links the owner permits. `now_unix_s` is the host clock; `max_skew_s` the freshness window.
///
/// Canonical signed message (both sides MUST build it identically): the parts
/// `"LINK|" , owner_pid , "|" , host , "|" , verb , "|" , nonce , "|" , ts_unix_s.to_be_bytes()`.
pub fn verify_link(
    req: &LinkRequest<'_>,
    shared_key: &[u8],
    granted_owner_pids: &[&str],
    now_unix_s: u64,
    max_skew_s: u64,
) -> LinkVerdict {
    if req.owner_pid.is_empty()
        || req.host.is_empty()
        || req.verb.is_empty()
        || req.nonce.is_empty()
        || req.hmac_hex.len() != 64
    {
        return LinkVerdict::DenyMalformed;
    }
    // Freshness (anti-replay) FIRST so a stale request never reaches the key comparison.
    let diff = if now_unix_s >= req.ts_unix_s {
        now_unix_s - req.ts_unix_s
    } else {
        req.ts_unix_s - now_unix_s
    };
    if diff > max_skew_s {
        return LinkVerdict::DenyStale;
    }
    // Owner consent — this host must have granted the requesting owner-PID.
    if !granted_owner_pids.iter().any(|g| *g == req.owner_pid) {
        return LinkVerdict::DenyOwnerNotGranted;
    }
    // HMAC over the canonical parts (verb bound in).
    let ts_be = req.ts_unix_s.to_be_bytes();
    let mac = hmac_sha256_parts(
        shared_key,
        &[
            b"LINK|",
            req.owner_pid.as_bytes(),
            b"|",
            req.host.as_bytes(),
            b"|",
            req.verb.as_bytes(),
            b"|",
            req.nonce.as_bytes(),
            b"|",
            &ts_be,
        ],
    );
    if hmac_hex_matches(&mac, req.hmac_hex) {
        LinkVerdict::Allow
    } else {
        LinkVerdict::DenyBadHmac
    }
}

/// Constant-time-ish compare of a 32-byte MAC to a 64-char lowercase-hex string (no early-out).
fn hmac_hex_matches(mac: &[u8; 32], expected_hex: &str) -> bool {
    if expected_hex.len() != 64 {
        return false;
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let eb = expected_hex.as_bytes();
    let mut equal = true;
    let mut i = 0;
    while i < 32 {
        let hi = HEX[(mac[i] >> 4) as usize];
        let lo = HEX[(mac[i] & 0x0f) as usize];
        if eb[2 * i] != hi || eb[2 * i + 1] != lo {
            equal = false;
        }
        i += 1;
    }
    equal
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Render a 32-byte MAC to lowercase hex into `out` (test helper).
    fn hex32(mac: &[u8; 32]) -> [u8; 64] {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = [0u8; 64];
        let mut i = 0;
        while i < 32 {
            out[2 * i] = HEX[(mac[i] >> 4) as usize];
            out[2 * i + 1] = HEX[(mac[i] & 0x0f) as usize];
            i += 1;
        }
        out
    }

    #[test]
    fn hmac_matches_rfc_known_vector() {
        // Well-known HMAC-SHA256("key", "The quick brown fox jumps over the lazy dog").
        let mac = hmac_sha256_parts(b"key", &[b"The quick brown fox jumps over the lazy dog"]);
        let hex = hex32(&mac);
        let s = core::str::from_utf8(&hex).unwrap();
        assert_eq!(s, "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8");
    }

    fn signed_request<'a>(
        key: &[u8],
        owner: &'a str,
        host: &'a str,
        verb: &'a str,
        nonce: &'a str,
        ts: u64,
        hex_buf: &'a mut [u8; 64],
    ) -> LinkRequest<'a> {
        let ts_be = ts.to_be_bytes();
        let mac = hmac_sha256_parts(
            key,
            &[
                b"LINK|",
                owner.as_bytes(),
                b"|",
                host.as_bytes(),
                b"|",
                verb.as_bytes(),
                b"|",
                nonce.as_bytes(),
                b"|",
                &ts_be,
            ],
        );
        *hex_buf = hex32(&mac);
        LinkRequest {
            owner_pid: owner,
            host,
            verb,
            nonce,
            ts_unix_s: ts,
            hmac_hex: core::str::from_utf8(hex_buf).unwrap(),
        }
    }

    // Test fixture only — NOT a real secret. The live shared key is a generated 256-bit value in a
    // local secret file (~/.asolaria/recall.key), never in code/commits.
    const KEY: &[u8] = b"test-link-key-fixture-not-a-real-secret";
    const GRANTS: &[&str] = &["OP-JESSE", "OP-RAYSSA"];

    #[test]
    fn valid_keyed_consented_fresh_request_is_allowed() {
        let mut hb = [0u8; 64];
        let req = signed_request(KEY, "OP-RAYSSA", "liris", "search", "n1", 1000, &mut hb);
        assert_eq!(verify_link(&req, KEY, GRANTS, 1005, 120), LinkVerdict::Allow);
    }

    #[test]
    fn wrong_key_is_denied_bad_hmac() {
        let mut hb = [0u8; 64];
        let req = signed_request(KEY, "OP-RAYSSA", "liris", "search", "n1", 1000, &mut hb);
        // Verifier holds a DIFFERENT key -> the public / an impostor cannot pass.
        assert_eq!(
            verify_link(&req, b"not-the-shared-key", GRANTS, 1005, 120),
            LinkVerdict::DenyBadHmac
        );
    }

    #[test]
    fn ungranted_owner_is_denied() {
        let mut hb = [0u8; 64];
        let req = signed_request(KEY, "OP-STRANGER", "evil", "search", "n1", 1000, &mut hb);
        assert_eq!(
            verify_link(&req, KEY, GRANTS, 1005, 120),
            LinkVerdict::DenyOwnerNotGranted
        );
    }

    #[test]
    fn stale_request_is_denied() {
        let mut hb = [0u8; 64];
        let req = signed_request(KEY, "OP-JESSE", "acer", "search", "n1", 1000, &mut hb);
        // 1000 + 9999 skew, window 120 -> stale.
        assert_eq!(verify_link(&req, KEY, GRANTS, 1000 + 9999, 120), LinkVerdict::DenyStale);
    }

    #[test]
    fn verb_is_bound_into_the_signature() {
        // A request signed for "search" must NOT verify if the presented verb is swapped to a
        // higher-privilege verb (tamper) — the recomputed HMAC won't match.
        let mut hb = [0u8; 64];
        let mut req = signed_request(KEY, "OP-RAYSSA", "liris", "search", "n1", 1000, &mut hb);
        req.verb = "fabric-write";
        assert_eq!(verify_link(&req, KEY, GRANTS, 1005, 120), LinkVerdict::DenyBadHmac);
    }

    #[test]
    fn malformed_request_is_denied() {
        let req = LinkRequest {
            owner_pid: "OP-JESSE",
            host: "acer",
            verb: "search",
            nonce: "n1",
            ts_unix_s: 1000,
            hmac_hex: "tooshort",
        };
        assert_eq!(verify_link(&req, KEY, GRANTS, 1005, 120), LinkVerdict::DenyMalformed);
    }
}
