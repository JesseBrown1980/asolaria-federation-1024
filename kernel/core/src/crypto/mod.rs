//! Crypto substrate · Phase-2 Step 28 · ed25519 envelope signing
//!
//! Wraps `ed25519-dalek` 2.1 no_std. Sign + verify on the BEHCS-1024 envelope wire format
//! (canonical bytes per IX-700 schema in `kernel/docs/USERSPACE_ABI.md`, Step 30).
//!
//! Key storage: per-vantage trust roots embedded at build time via `KERNEL_TRUST_ROOTS.json`;
//! private key lives in TPM or `/sealed/` partition per Phase-9 tier security.
//!
//! v0.1: API surface + canonical-bytes helper. Real `ed25519-dalek` binding lands in v0.2
//! once Cargo.toml workspace builds cleanly.

use alloc::vec::Vec;
use sha2::{Digest, Sha256};

/// ed25519 signature length in bytes.
pub const SIGNATURE_LEN: usize = 64;

/// ed25519 public key length in bytes.
pub const PUBLIC_KEY_LEN: usize = 32;

/// ed25519 private key length in bytes (32-byte seed; full keypair derived).
pub const PRIVATE_KEY_LEN: usize = 32;

/// Crypto operation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CryptoErr {
    /// Signature did not verify against the provided public key.
    SignatureInvalid,
    /// Phase-1 sha-derived verify: recomputed expected bytes did not
    /// constant-time-match the provided 64-byte signature.
    /// Phase-2 will collapse this into `SignatureInvalid` once ed25519-dalek lands.
    VerificationFailed,
    /// Provided key bytes do not parse as a valid ed25519 key.
    KeyMalformed,
    /// Canonical envelope bytes failed to produce — input not well-formed.
    CanonicalizationFailed,
    /// Stub not yet implemented (v0.2).
    Unimplemented,
}

/// ed25519 signature wrapped for type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature(pub [u8; SIGNATURE_LEN]);

/// ed25519 public key wrapped for type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicKey(pub [u8; PUBLIC_KEY_LEN]);

/// ed25519 private key seed (zeroized on drop in v0.2 via the `zeroize` crate).
#[derive(Debug, Clone)]
pub struct PrivateKey(pub [u8; PRIVATE_KEY_LEN]);

/// Produces the canonical signing bytes for an envelope.
/// v0.1: simple deterministic byte concatenation. v0.2 will use CBOR canonical form per IX-700.
pub fn canonical_envelope_bytes(
    from_pid: u64,
    to_pid: u64,
    type_tag: &[u8],
    verb: &[u8],
    id: &[u8],
    ts_ns: u64,
    payload: &[u8],
) -> Result<Vec<u8>, CryptoErr> {
    if type_tag.is_empty() || id.is_empty() {
        return Err(CryptoErr::CanonicalizationFailed);
    }
    let mut buf: Vec<u8> = Vec::with_capacity(
        8 + 8 + type_tag.len() + verb.len() + id.len() + 8 + payload.len() + 6 * 1,
    );
    buf.extend_from_slice(&from_pid.to_be_bytes());
    buf.push(b'|');
    buf.extend_from_slice(&to_pid.to_be_bytes());
    buf.push(b'|');
    buf.extend_from_slice(type_tag);
    buf.push(b'|');
    buf.extend_from_slice(verb);
    buf.push(b'|');
    buf.extend_from_slice(id);
    buf.push(b'|');
    buf.extend_from_slice(&ts_ns.to_be_bytes());
    buf.push(b'|');
    buf.extend_from_slice(payload);
    Ok(buf)
}

/// Phase-1 expected-signature recomputation from (public, msg).
///
/// Layout (64 bytes):
///   `expected[0..32]  = sha256(public || ":sign:" || msg)`
///   `expected[32..64] = sha256(expected[0..32])`
///
/// Used by both `sign` (after deriving public) and `verify` to ensure byte-parity.
/// Phase-2 replaces this with ed25519-dalek curve signatures.
fn expected_sig_bytes(public: &PublicKey, msg: &[u8]) -> [u8; SIGNATURE_LEN] {
    let mut h1 = Sha256::new();
    h1.update(public.0.as_slice());
    h1.update(b":sign:");
    h1.update(msg);
    let half_a = h1.finalize();

    let mut h2 = Sha256::new();
    h2.update(half_a.as_slice());
    let half_b = h2.finalize();

    let mut out = [0u8; SIGNATURE_LEN];
    out[..32].copy_from_slice(&half_a);
    out[32..].copy_from_slice(&half_b);
    out
}

/// Constant-time equality compare over two 64-byte signatures (XOR-fold).
///
/// Branches and indexing pattern are independent of byte values: every byte
/// position contributes to a single `acc` accumulator that is checked exactly
/// once at the end. No early-exit on mismatch.
#[inline]
fn ct_eq(a: &[u8; SIGNATURE_LEN], b: &[u8; SIGNATURE_LEN]) -> bool {
    let mut acc: u8 = 0;
    let mut i = 0;
    while i < SIGNATURE_LEN {
        acc |= a[i] ^ b[i];
        i += 1;
    }
    acc == 0
}

/// Sign canonical envelope bytes with a private key.
///
/// Phase-1 sha-derived placeholder. Phase-2 ed25519-dalek deferred.
/// Internally derives the public key, then emits the Phase-1 expected-sig layout
/// so `verify(public, msg, sig)` is byte-parity exact.
pub fn sign(canonical_bytes: &[u8], priv_key: &PrivateKey) -> Result<Signature, CryptoErr> {
    let public = derive_public(priv_key)?;
    Ok(Signature(expected_sig_bytes(&public, canonical_bytes)))
}

/// Verify a Phase-1 sha-derived signature against `(public, msg)`.
///
/// Recomputes the 64-byte expected signature from `(public, msg)` and
/// constant-time-compares against the provided `sig`.
/// Returns `Ok(())` on match, `Err(CryptoErr::VerificationFailed)` on mismatch.
pub fn verify(
    canonical_bytes: &[u8],
    sig: &Signature,
    pub_key: &PublicKey,
) -> Result<(), CryptoErr> {
    let expected = expected_sig_bytes(pub_key, canonical_bytes);
    if ct_eq(&expected, &sig.0) {
        Ok(())
    } else {
        Err(CryptoErr::VerificationFailed)
    }
}

/// Derives public key from a private-key seed.
///
/// Phase-1 sha-only placeholder: `PublicKey = sha256(secret || ":derive:")`.
/// Phase-2 will replace with ed25519-dalek curve-point derivation.
pub fn derive_public(secret: &PrivateKey) -> Result<PublicKey, CryptoErr> {
    let mut hasher = Sha256::new();
    hasher.update(secret.0.as_slice());
    hasher.update(b":derive:");
    let bytes = hasher.finalize();
    let mut pk = [0u8; PUBLIC_KEY_LEN];
    pk.copy_from_slice(&bytes);
    Ok(PublicKey(pk))
}

/// Compile-time enforcement on key lengths (catches future ed25519-curve substitutions).
const _: () = {
    assert!(SIGNATURE_LEN == 64);
    assert!(PUBLIC_KEY_LEN == 32);
    assert!(PRIVATE_KEY_LEN == 32);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_match_ed25519_canon() {
        assert_eq!(SIGNATURE_LEN, 64);
        assert_eq!(PUBLIC_KEY_LEN, 32);
        assert_eq!(PRIVATE_KEY_LEN, 32);
    }

    #[test]
    fn canonical_bytes_deterministic() {
        let a = canonical_envelope_bytes(1, 2, b"TEST_TYPE", b"EVT-TEST", b"id-0", 123, b"payload")
            .unwrap();
        let b = canonical_envelope_bytes(1, 2, b"TEST_TYPE", b"EVT-TEST", b"id-0", 123, b"payload")
            .unwrap();
        assert_eq!(a, b);
        assert!(a.len() > 10);
    }

    #[test]
    fn empty_type_rejected() {
        let r = canonical_envelope_bytes(1, 2, b"", b"EVT", b"id", 0, b"");
        assert_eq!(r, Err(CryptoErr::CanonicalizationFailed));
    }

    #[test]
    fn empty_id_rejected() {
        let r = canonical_envelope_bytes(1, 2, b"T", b"EVT", b"", 0, b"");
        assert_eq!(r, Err(CryptoErr::CanonicalizationFailed));
    }

    #[test]
    fn sign_deterministic_same_inputs() {
        let priv_key = PrivateKey([7u8; 32]);
        let msg = b"hello-canonical";
        let a = sign(msg, &priv_key).unwrap();
        let b = sign(msg, &priv_key).unwrap();
        assert_eq!(a, b);
        // halves must differ (sha256 vs sha256(sha256))
        assert_ne!(&a.0[..32], &a.0[32..]);
    }

    #[test]
    fn sign_differs_across_secrets() {
        let k1 = PrivateKey([1u8; 32]);
        let k2 = PrivateKey([2u8; 32]);
        let msg = b"same-msg";
        let s1 = sign(msg, &k1).unwrap();
        let s2 = sign(msg, &k2).unwrap();
        assert_ne!(s1, s2);
    }

    #[test]
    fn derive_public_deterministic() {
        let sk = PrivateKey([0x11u8; 32]);
        let p1 = derive_public(&sk).unwrap();
        let p2 = derive_public(&sk).unwrap();
        assert_eq!(p1, p2);
        assert_eq!(p1.0.len(), PUBLIC_KEY_LEN);
    }

    #[test]
    fn derive_public_distinct_across_secrets() {
        let p1 = derive_public(&PrivateKey([0x01u8; 32])).unwrap();
        let p2 = derive_public(&PrivateKey([0x02u8; 32])).unwrap();
        assert_ne!(p1, p2);
        // Distinct seed → distinct public (sha-collision-resistance proxy).
        assert_ne!(p1.0, p2.0);
    }

    #[test]
    fn derive_sign_verify_loop_structural() {
        // End-to-end loop: secret → derive_public → sign → verify pair.
        // Phase-1 verify now implemented (sha-recompute + ct_eq). Roundtrip MUST be Ok.
        let sk = PrivateKey([0x42u8; 32]);
        let msg = b"loop-closure-msg";
        let pk = derive_public(&sk).expect("derive_public must succeed Phase-1");
        let sig = sign(msg, &sk).expect("sign must succeed Phase-1");
        assert_eq!(sig.0.len(), SIGNATURE_LEN);
        assert_eq!(pk.0.len(), PUBLIC_KEY_LEN);
        verify(msg, &sig, &pk).expect("Phase-1 verify must accept genuine sign output");
        // Re-derive must collide (loop stable across calls).
        let pk2 = derive_public(&sk).unwrap();
        assert_eq!(pk, pk2);
    }

    // ---- W5-A3 close-out: verify Phase-1 explicit tests ----

    #[test]
    fn verify_roundtrip_succeeds() {
        // sign+verify roundtrip on a single (secret, msg) pair: Ok(()).
        let sk = PrivateKey([0xA7u8; 32]);
        let msg = b"sign-verify-roundtrip-canonical-bytes";
        let pk = derive_public(&sk).unwrap();
        let sig = sign(msg, &sk).unwrap();
        assert_eq!(verify(msg, &sig, &pk), Ok(()));
    }

    #[test]
    fn verify_tampered_signature_fails() {
        // Flip a single bit anywhere in the 64-byte sig: VerificationFailed.
        let sk = PrivateKey([0x5Cu8; 32]);
        let msg = b"tamper-detection-msg";
        let pk = derive_public(&sk).unwrap();
        let good = sign(msg, &sk).unwrap();

        // Tamper at byte 0 (first half — sha256 region).
        let mut bad_a = good.0;
        bad_a[0] ^= 0x01;
        assert_eq!(
            verify(msg, &Signature(bad_a), &pk),
            Err(CryptoErr::VerificationFailed)
        );

        // Tamper at byte 33 (second half — sha256(sha256) region).
        let mut bad_b = good.0;
        bad_b[33] ^= 0x80;
        assert_eq!(
            verify(msg, &Signature(bad_b), &pk),
            Err(CryptoErr::VerificationFailed)
        );

        // Tamper at last byte.
        let mut bad_c = good.0;
        bad_c[SIGNATURE_LEN - 1] ^= 0xFF;
        assert_eq!(
            verify(msg, &Signature(bad_c), &pk),
            Err(CryptoErr::VerificationFailed)
        );
    }

    #[test]
    fn verify_wrong_public_key_fails() {
        // Sign with sk_a, verify with public key from sk_b: VerificationFailed.
        let sk_a = PrivateKey([0x11u8; 32]);
        let sk_b = PrivateKey([0x22u8; 32]);
        let pk_b = derive_public(&sk_b).unwrap();
        let msg = b"wrong-public-key-msg";
        let sig_a = sign(msg, &sk_a).unwrap();
        assert_eq!(
            verify(msg, &sig_a, &pk_b),
            Err(CryptoErr::VerificationFailed)
        );

        // Symmetric: sign with sk_b, verify with pk_a → also VerificationFailed.
        let pk_a = derive_public(&sk_a).unwrap();
        let sig_b = sign(msg, &sk_b).unwrap();
        assert_eq!(
            verify(msg, &sig_b, &pk_a),
            Err(CryptoErr::VerificationFailed)
        );
    }

    #[test]
    fn ct_eq_xor_fold_basics() {
        // Sanity-check the constant-time helper directly.
        let a = [0u8; SIGNATURE_LEN];
        let b = [0u8; SIGNATURE_LEN];
        assert!(ct_eq(&a, &b));

        let mut c = [0u8; SIGNATURE_LEN];
        c[17] = 1;
        assert!(!ct_eq(&a, &c));

        let mut d = [0u8; SIGNATURE_LEN];
        d[SIGNATURE_LEN - 1] = 0xFF;
        assert!(!ct_eq(&a, &d));
    }
}
