// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

//! Ledger 9 controller/recovery authorization signing for `did:midnight`.
//!
//! This module preserves the TypeScript suite's deterministic nonce and
//! 248-bit challenge reduction while using the Ledger 9 curve types. The
//! Ledger 8 helper remains separate because Rust must not mix curve types from
//! different Ledger revisions.

use midnight_transient_crypto::curve::{EmbeddedFr, EmbeddedGroupAffine, Fr};
use midnight_transient_crypto::hash::transient_hash;
use sha2::{Digest, Sha256};

/// Nonce domain shared with the official TypeScript DID suite.
pub const NONCE_DOMAIN_V1: &str = "midnight-did:jubjub-schnorr:v1";

/// A Ledger 9 authorization signature accepted by the generated DID contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizationSignature {
    /// Announcement point `R = k·G`.
    pub announcement: EmbeddedGroupAffine,
    /// Response scalar `s = k + c·sk (mod r)`.
    pub response: EmbeddedFr,
}

/// Errors produced while constructing a Ledger 9 authorization signature.
#[derive(Debug, thiserror::Error)]
pub enum SigningError {
    /// A derived point was the identity and therefore had no affine coordinates.
    #[error("identity point cannot be encoded")]
    IdentityPoint,
}

/// Derive the Ledger 9 Jubjub public key for a controller or recovery seed.
pub fn derive_public_key_from_seed(seed: &[u8]) -> EmbeddedGroupAffine {
    EmbeddedGroupAffine::generator() * seed_to_secret_scalar(seed)
}

/// Sign the four full-field elements emitted by a generated DID authorization
/// digest, retaining all digest bits during deterministic nonce derivation.
pub fn sign_authorization_digest_from_seed(
    seed: &[u8; 32],
    digest: &[Fr; 4],
) -> Result<AuthorizationSignature, SigningError> {
    let secret = seed_to_secret_scalar(seed);
    let public_key = EmbeddedGroupAffine::generator() * secret;
    let mut nonce_seed = Vec::with_capacity(NONCE_DOMAIN_V1.len() + seed.len() + digest.len() * 32);
    nonce_seed.extend_from_slice(NONCE_DOMAIN_V1.as_bytes());
    nonce_seed.extend_from_slice(seed);
    for field in digest {
        nonce_seed.extend_from_slice(&field_to_bytes32_be(field));
    }
    let nonce = hash_to_scalar(&Sha256::digest(nonce_seed));
    let announcement = EmbeddedGroupAffine::generator() * nonce;
    let challenge = authorization_challenge(&announcement, &public_key, digest)?;
    Ok(AuthorizationSignature {
        announcement,
        response: nonce + challenge * secret,
    })
}

/// Convert a Jubjub scalar into the Ledger 9 base field used by Compact.
pub fn field_from_scalar(scalar: &EmbeddedFr) -> Fr {
    Fr::from_le_bytes(&scalar.as_le_bytes()).expect("every Jubjub scalar fits in the base field")
}

fn seed_to_secret_scalar(seed: &[u8]) -> EmbeddedFr {
    let mut normalized = [0u8; 32];
    let length = seed.len().min(32);
    normalized[..length].copy_from_slice(&seed[..length]);
    hash_to_scalar(&Sha256::digest(normalized))
}

fn hash_to_scalar(digest: &[u8]) -> EmbeddedFr {
    let mut little_endian = [0u8; 32];
    for (slot, byte) in little_endian.iter_mut().zip(digest.iter().take(32).rev()) {
        *slot = *byte;
    }
    EmbeddedFr::from_le_bytes_wide(&little_endian).expect("32 bytes fit the wide scalar reduction input")
}

fn authorization_challenge(
    announcement: &EmbeddedGroupAffine,
    public_key: &EmbeddedGroupAffine,
    digest: &[Fr; 4],
) -> Result<EmbeddedFr, SigningError> {
    let (announcement_x, announcement_y) = coordinates(announcement)?;
    let (public_key_x, public_key_y) = coordinates(public_key)?;
    let challenge = transient_hash(&[
        announcement_x,
        announcement_y,
        public_key_x,
        public_key_y,
        digest[0],
        digest[1],
        digest[2],
        digest[3],
    ]);
    let mut reduced = field_to_bytes32_le(&challenge);
    reduced[31] = 0;
    Ok(EmbeddedFr::from_le_bytes(&reduced).expect("the 248-bit authorization challenge fits the Jubjub scalar field"))
}

fn coordinates(point: &EmbeddedGroupAffine) -> Result<(Fr, Fr), SigningError> {
    match (point.x(), point.y()) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(SigningError::IdentityPoint),
    }
}

fn field_to_bytes32_le(field: &Fr) -> [u8; 32] {
    let mut result = [0u8; 32];
    let bytes = field.as_le_bytes();
    let length = bytes.len().min(result.len());
    result[..length].copy_from_slice(&bytes[..length]);
    result
}

fn field_to_bytes32_be(field: &Fr) -> [u8; 32] {
    let mut result = field_to_bytes32_le(field);
    result.reverse();
    result
}
