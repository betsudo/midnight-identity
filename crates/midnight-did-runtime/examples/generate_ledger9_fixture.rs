// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

//! Regenerate the Ledger 9 state capture used by indexer and resolver tests.

use std::path::Path;

use midnight_compact_runtime as runtime;
use midnight_compact_runtime::ledger::{ContractState, DefaultDB};
use midnight_did_runtime::ledger9_signing::{
    derive_public_key_from_seed, field_from_scalar, sign_authorization_digest_from_seed,
};
use midnight_did_runtime::{ledger_contract, pure_circuits, types};

#[derive(Clone)]
struct PrivateState {
    controller_seed: [u8; 32],
    recovery_seed: [u8; 32],
    timestamp: u64,
}

struct Witnesses;

impl ledger_contract::Witnesses<PrivateState> for Witnesses {
    fn getSchnorrReduction(
        &self,
        context: runtime::context::WitnessContext<'_, PrivateState, ledger_contract::LedgerView<'_>>,
        challenge_hash: runtime::Field,
    ) -> (
        PrivateState,
        (
            runtime::BoundedUint<127>,
            runtime::WideUint<1329227995784915872903807060280344575u128, 340282366920938463463374607431768211455u128>,
        ),
    ) {
        let mut bytes = challenge_hash.as_le_bytes();
        bytes.resize(32, 0);
        let quotient = runtime::BoundedUint::new(bytes[31] as u128).expect("field quotient");
        bytes[31] = 0;
        let remainder = runtime::WideUint::from_le_bytes(&bytes).expect("field remainder");
        (context.private_state.clone(), (quotient, remainder))
    }

    fn localControllerPublicKey(
        &self,
        context: runtime::context::WitnessContext<'_, PrivateState, ledger_contract::LedgerView<'_>>,
    ) -> (PrivateState, runtime::JubjubPoint) {
        let state = context.private_state.clone();
        (
            state,
            derive_public_key_from_seed(&context.private_state.controller_seed).into(),
        )
    }

    fn localRecoveryAuthorityPublicKey(
        &self,
        context: runtime::context::WitnessContext<'_, PrivateState, ledger_contract::LedgerView<'_>>,
    ) -> (PrivateState, runtime::JubjubPoint) {
        let state = context.private_state.clone();
        (
            state,
            derive_public_key_from_seed(&context.private_state.recovery_seed).into(),
        )
    }

    fn currentTimestamp(
        &self,
        context: runtime::context::WitnessContext<'_, PrivateState, ledger_contract::LedgerView<'_>>,
    ) -> (PrivateState, runtime::BoundedUint<18446744073709551615>) {
        let state = context.private_state.clone();
        (
            state,
            runtime::BoundedUint::new(context.private_state.timestamp as u128).unwrap(),
        )
    }
}

fn signature(seed: &[u8; 32], digest: &[runtime::Field; 4]) -> types::SchnorrSignature {
    let signature = sign_authorization_digest_from_seed(seed, digest).expect("authorization signature");
    types::SchnorrSignature {
        announcement: signature.announcement.into(),
        response: field_from_scalar(&signature.response),
    }
}

fn fixture_bytes() -> Vec<u8> {
    let private = PrivateState {
        controller_seed: [0x21; 32],
        recovery_seed: [0x22; 32],
        timestamp: 1_700_000_000_000,
    };
    let controller_seed = private.controller_seed;
    let initial = ledger_contract::initial_state(runtime::context::ConstructorContext::new(private), &Witnesses)
        .expect("constructor");
    let initial_view = ledger_contract::PublicStateView::from(&initial);
    let id = initial_view.id().unwrap();
    let version = initial_view.version().unwrap();
    let method_id = runtime::OpaqueString::from("#key-1");
    let method = types::VerificationMethod {
        id: method_id.clone(),
        typ: types::VerificationMethodType::JsonWebKey,
        publicKeyJwk: types::PublicKeyJwk {
            kty: types::KeyType::OKP,
            crv: types::CurveType::Ed25519,
            x: runtime::OpaqueString::from("11qYAYdk9JgcPdBA3mxrDhe4n2RZedmD3uQw1JY1N8w"),
            y: runtime::OpaqueString::from(""),
        },
    };
    let digest = pure_circuits::setVerificationMethodAuthorizationDigest(
        id.clone(),
        version,
        method.clone(),
        types::MapMutation::Insert,
    )
    .expect("verification method digest");
    let inserted = ledger_contract::recorded::setVerificationMethod(
        initial.into_circuit_context(runtime::ledger::ContractAddress::default()),
        &Witnesses,
        method,
        types::MapMutation::Insert,
        signature(&controller_seed, &digest.0),
        version,
    )
    .expect("insert verification method");

    let inserted_view = ledger_contract::PublicStateView::from(&inserted);
    let version = inserted_view.version().unwrap();
    let digest = pure_circuits::setVerificationMethodRelationAuthorizationDigest(
        id,
        version,
        types::VerificationMethodRelation::Authentication,
        method_id.clone(),
        types::SetMutation::Insert,
    )
    .expect("verification relation digest");
    let related = ledger_contract::recorded::setVerificationMethodRelation(
        inserted.execution.context,
        &Witnesses,
        types::VerificationMethodRelation::Authentication,
        method_id,
        types::SetMutation::Insert,
        signature(&controller_seed, &digest.0),
        version,
    )
    .expect("insert authentication relation");

    let contract_state = ContractState::<DefaultDB>::new(
        related.execution.context.query.state.get_ref().clone(),
        Default::default(),
        Default::default(),
    );
    let mut bytes = Vec::new();
    midnight_serialize::tagged_serialize(&contract_state, &mut bytes).expect("serialize contract state");
    bytes
}

fn write_fixture(root: &Path, package: &str, bytes: &[u8]) {
    let directory = root.join("crates").join(package).join("tests/fixtures");
    std::fs::write(directory.join("state-ledger9-enriched.hex"), hex::encode(bytes) + "\n")
        .expect("write state fixture");
    std::fs::write(directory.join("state-ledger9-enriched.addr"), "00".repeat(32) + "\n")
        .expect("write address fixture");
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let bytes = fixture_bytes();
    write_fixture(root, "midnight-did-indexer", &bytes);
    write_fixture(root, "midnight-did-resolver", &bytes);
}
