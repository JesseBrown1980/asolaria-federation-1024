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

/// Sign canonical envelope bytes with a private key.
/// v0.1 stub.
pub fn sign(_canonical_bytes: &[u8], _priv: &PrivateKey) -> Result<Signature, CryptoErr> {
    Err(CryptoErr::Unimplemented)
}

/// Verify a signature against canonical envelope bytes + public key.
/// v0.1 stub.
pub fn verify(
    _canonical_bytes: &[u8],
    _sig: &Signature,
    _pub_key: &PublicKey,
) -> Result<(), CryptoErr> {
    Err(CryptoErr::Unimplemented)
}

/// Derives public key from a private-key seed.
/// v0.1 stub.
pub fn derive_public(_priv: &PrivateKey) -> Result<PublicKey, CryptoErr> {
    Err(CryptoErr::Unimplemented)
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
    fn stub_sign_returns_unimplemented() {
        let priv_key = PrivateKey([0u8; 32]);
        assert_eq!(sign(b"x", &priv_key), Err(CryptoErr::Unimplemented));
    }
}
