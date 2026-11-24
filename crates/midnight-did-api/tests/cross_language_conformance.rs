// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

//! Language-neutral Ledger 9 conformance vectors shared with the pinned
//! TypeScript `midnight-did` implementation.

use std::path::PathBuf;

use ed25519_zebra::{Signature as Ed25519Signature, SigningKey, VerificationKey};
use midnight_did_api::contract::DidLedgerSnapshot;
use midnight_did_api::resolution::{ledger_state_to_did_document, ledger_state_to_metadata};
use midnight_did_jubjub_schnorr::{
    decode_signature, derive_public_key_from_seed, encode_signature, sign_payload_from_seed, verify_payload,
};
use midnight_did_method::midnight_did::MidnightNetwork;
use serde_json::Value;

const FIXTURE: &str = "ledger9-cross-language-conformance.json";

fn fixture() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(FIXTURE);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("parse {path:?}: {error}"))
}

fn decode_hex<const N: usize>(value: &str) -> [u8; N] {
    let bytes = hex::decode(value).expect("fixture hex");
    bytes
        .try_into()
        .unwrap_or_else(|bytes: Vec<u8>| panic!("expected {N} fixture bytes, got {}", bytes.len()))
}

#[test]
fn rust_resolver_matches_the_pinned_typescript_document() {
    let fixture = fixture();
    assert_eq!(
        fixture["sourceRevisions"]["typescript"],
        "750268664278c162e9171406983dc45f24bb5be7"
    );
    let snapshot: DidLedgerSnapshot = serde_json::from_value(fixture["ledgerSnapshot"].clone()).expect("snapshot");
    let address = fixture["contractAddress"].as_str().expect("contract address");
    let document = ledger_state_to_did_document(&snapshot, MidnightNetwork::Undeployed, address).expect("document");
    let actual = serde_json::json!({
        "didDocument": document,
        "didDocumentMetadata": ledger_state_to_metadata(&snapshot),
    });
    assert_eq!(actual, fixture["expected"]);
}

#[test]
fn ed25519_signature_is_identical_and_verifies_in_both_languages() {
    let fixture = fixture();
    let vector = &fixture["ed25519"];
    let seed = decode_hex::<32>(vector["seedHex"].as_str().expect("seed"));
    let public_key = decode_hex::<32>(vector["publicKeyHex"].as_str().expect("public key"));
    let message = hex::decode(vector["messageHex"].as_str().expect("message")).expect("message hex");
    let expected_signature = decode_hex::<64>(vector["signatureHex"].as_str().expect("signature"));

    let signing_key = SigningKey::from(seed);
    assert_eq!(<[u8; 32]>::from(signing_key.verification_key()), public_key);
    let rust_signature = signing_key.sign(&message);
    assert_eq!(rust_signature.to_bytes(), expected_signature);

    let verification_key = VerificationKey::try_from(public_key).expect("verification key");
    let typescript_signature = Ed25519Signature::from_bytes(&expected_signature);
    verification_key
        .verify(&typescript_signature, &message)
        .expect("TypeScript/RFC 8032 signature verifies in Rust");
}

#[test]
fn jubjub_signature_is_identical_and_verifies_in_both_languages() {
    let fixture = fixture();
    let vector = &fixture["jubjub"];
    let seed = decode_hex::<32>(vector["seedHex"].as_str().expect("seed"));
    let payload = vector["payloadUtf8"].as_str().expect("payload").as_bytes();
    let expected_signature = hex::decode(vector["signatureHex"].as_str().expect("signature")).expect("signature hex");

    let rust_signature = sign_payload_from_seed(&seed, payload).expect("Rust signature");
    assert_eq!(
        encode_signature(&rust_signature).expect("encode").as_slice(),
        expected_signature.as_slice()
    );

    let typescript_signature = decode_signature(&expected_signature).expect("TypeScript signature");
    let public_key = derive_public_key_from_seed(&seed);
    assert!(verify_payload(&public_key, payload, &typescript_signature));
    assert!(!verify_payload(&public_key, b"tampered", &typescript_signature));
}
