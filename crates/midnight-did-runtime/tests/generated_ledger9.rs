// SPDX-License-Identifier: Apache-2.0

//! Native Rust AST contract qualification for the Ledger 9 DID constructor
//! and one representative replayable state transition.

use midnight_compact_runtime as runtime;
use midnight_did_runtime::ledger9_signing::{
    derive_public_key_from_seed, field_from_scalar, sign_authorization_digest_from_seed,
};
use midnight_did_runtime::{ledger_contract, pure_circuits, types};

const CIRCUITS: &[&str] = &[
    "deactivate",
    "recoverControllerKey",
    "removeSchnorrJubjubVerificationMethod",
    "removeService",
    "removeVerificationMethod",
    "rotateControllerKey",
    "setAlsoKnownAs",
    "setSchnorrJubjubVerificationMethod",
    "setService",
    "setVerificationMethod",
    "setVerificationMethodRelation",
    "verifySchnorrJubjubDigestSignature",
];

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
        let quotient = runtime::BoundedUint::new(bytes[31] as u128).expect("field quotient is at most 127");
        bytes[31] = 0;
        let remainder = runtime::WideUint::from_le_bytes(&bytes).expect("low 248 challenge bits fit");
        (context.private_state.clone(), (quotient, remainder))
    }

    fn localControllerPublicKey(
        &self,
        context: runtime::context::WitnessContext<'_, PrivateState, ledger_contract::LedgerView<'_>>,
    ) -> (PrivateState, runtime::JubjubPoint) {
        let state = context.private_state.clone();
        let point = derive_public_key_from_seed(&state.controller_seed).into();
        (state, point)
    }

    fn localRecoveryAuthorityPublicKey(
        &self,
        context: runtime::context::WitnessContext<'_, PrivateState, ledger_contract::LedgerView<'_>>,
    ) -> (PrivateState, runtime::JubjubPoint) {
        let state = context.private_state.clone();
        let point = derive_public_key_from_seed(&state.recovery_seed).into();
        (state, point)
    }

    fn currentTimestamp(
        &self,
        context: runtime::context::WitnessContext<'_, PrivateState, ledger_contract::LedgerView<'_>>,
    ) -> (PrivateState, runtime::BoundedUint<18446744073709551615>) {
        let state = context.private_state.clone();
        let timestamp = runtime::BoundedUint::new(state.timestamp as u128).unwrap();
        (state, timestamp)
    }
}

fn private_state() -> PrivateState {
    PrivateState {
        controller_seed: [0x21; 32],
        recovery_seed: [0x22; 32],
        timestamp: 1_700_000_000_000,
    }
}

fn generated_signature(seed: &[u8; 32], digest: &[runtime::Field; 4]) -> types::SchnorrSignature {
    let signature = sign_authorization_digest_from_seed(seed, digest).expect("authorization signature");
    types::SchnorrSignature {
        announcement: signature.announcement.into(),
        response: field_from_scalar(&signature.response),
    }
}

#[test]
fn generated_zkir_v3_artifacts_parse_with_the_pinned_runtime_cohort() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/keys");
    for circuit in CIRCUITS {
        let path = root.join(format!("{circuit}.zkir"));
        let file = std::fs::File::open(&path).unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
        midnight_zkir::IrSource::load(std::io::BufReader::new(file))
            .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    }
}

#[test]
fn generated_compatibility_receipt_pins_ledger9_and_runtime_abi() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!("../assets/rust-compatibility.json")).unwrap();
    let compatibility = &receipt["compatibility"];
    assert_eq!(compatibility["ledger_version"], "ledger-9.1.0.2");
    assert_eq!(compatibility["runtime_version"], "0.2.0");
    assert_eq!(compatibility["runtime_abi"], 54);
    assert_eq!(compatibility["ir_schema"], 20);
    assert_eq!(compatibility["rust_version"], "1.99");
    assert_eq!(compatibility["ledger_dependencies"]["midnight-ledger-v9"], "=1.0.2");
    assert_eq!(compatibility["ledger_dependencies"]["midnight-storage"], "=3.1.0");
    assert_eq!(receipt["compiler"]["compact"], "0.31.215");
    assert_eq!(receipt["compiler"]["language"], "0.28.0");
    assert_eq!(receipt["compiler"]["typescript_runtime"], "0.18.0");
    assert_eq!(
        compatibility["ledger_git_sources"]["midnight-ledger-v9"]["rev"],
        "f273996eaba509aaf5f6d935f2f9df5c8ec98bf2"
    );
    assert_eq!(
        compatibility["ledger_git_sources"]["midnight-proofs"]["rev"],
        "0f9d5c3879f92a8beae8bdb9e7305f39e03a3227"
    );
    assert_eq!(
        compatibility["ledger_git_sources"]["midnight-zkir-v2"]["version"],
        "=2.2.0"
    );
    assert_eq!(
        compatibility["ledger_git_sources"]["midnight-zkir-v3"]["rev"],
        "0196bcd68fce3bfcb69914520b9ddaed051d4757"
    );
    assert_eq!(runtime::LEDGER_VERSION, "ledger-9.1.0.2");
    assert_eq!(runtime::RUST_RUNTIME_ABI, 54);
}

#[test]
fn constructor_initializes_ledger9_state_and_preserves_key_separation() {
    let initial =
        ledger_contract::initial_state(runtime::context::ConstructorContext::new(private_state()), &Witnesses)
            .expect("generated constructor");
    let view = ledger_contract::PublicStateView::from(&initial);

    assert_eq!(view.contractVersion().unwrap().value(), 2);
    assert!(view.active().unwrap());
    assert!(!view.deactivated().unwrap());
    assert_eq!(view.version().unwrap().value(), 0);
    assert_eq!(view.operationCount().unwrap().value(), 0);
    assert_eq!(view.created().unwrap().value(), 1_700_000_000_000);
    assert_eq!(view.updated().unwrap().value(), 1_700_000_000_000);
    assert_ne!(
        view.controllerPublicKey().unwrap(),
        view.recoveryAuthorityPublicKey().unwrap()
    );
}

#[test]
fn constructor_rejects_equal_controller_and_recovery_keys() {
    let mut state = private_state();
    state.recovery_seed = state.controller_seed;
    let error = ledger_contract::initial_state(runtime::context::ConstructorContext::new(state), &Witnesses)
        .err()
        .expect("equal keys must be refused");
    assert!(
        matches!(error, runtime::CompactError::AssertionFailed(ref message) if message.contains("recovery authority")),
        "unexpected refusal: {error:?}"
    );
}

#[test]
fn recorded_stateful_call_replays_public_program_and_refuses_stale_replay() {
    let state = private_state();
    let controller_seed = state.controller_seed;
    let initial = ledger_contract::initial_state(runtime::context::ConstructorContext::new(state), &Witnesses)
        .expect("generated constructor");
    let initial_view = ledger_contract::PublicStateView::from(&initial);
    let id = initial_view.id().unwrap();
    let expected_version = initial_view.version().unwrap();
    let alias = runtime::OpaqueString::from("did:example:ledger9");
    let mutation = types::SetMutation::Insert;
    let digest = pure_circuits::setAlsoKnownAsAuthorizationDigest(id, expected_version, alias.clone(), mutation)
        .expect("authorization digest");
    let signature = generated_signature(&controller_seed, &digest.0);
    let context = initial.into_circuit_context(runtime::ledger::ContractAddress::default());

    let recorded = ledger_contract::recorded::setAlsoKnownAs(
        context,
        &Witnesses,
        alias.clone(),
        mutation,
        signature.clone(),
        expected_version,
    )
    .expect("recorded state transition");

    assert!(
        !recorded.public.verify_ops().is_empty(),
        "stateful call must record a public VM program"
    );
    let view = ledger_contract::PublicStateView::from(&recorded);
    assert_eq!(view.version().unwrap().value(), 1);
    assert_eq!(view.operationCount().unwrap().value(), 1);
    assert!(view.alsoKnownAs().unwrap().member(alias.clone()));

    let replay_error = ledger_contract::recorded::setAlsoKnownAs(
        recorded.execution.context,
        &Witnesses,
        alias,
        mutation,
        signature,
        expected_version,
    )
    .err()
    .expect("stale replay must be refused");
    assert!(
        matches!(replay_error, runtime::CompactError::AssertionFailed(ref message) if message.contains("stale")),
        "unexpected replay refusal: {replay_error:?}"
    );
}
