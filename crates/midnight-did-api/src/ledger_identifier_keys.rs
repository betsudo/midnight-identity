// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");

//! Canonical DID URL identity and backwards-compatible physical ledger keys.
//!
//! Ledger 9 writes complete canonical DID URLs. Older deployments stored a
//! current-subject fragment as `#fragment`; only that exact URL shape has a
//! safe legacy alias. Path, query, foreign-DID, and external URL identities
//! must never be collapsed to a fragment.

use midnight_did_runtime::{Backend, Contract};

use crate::contract::DidLedgerSnapshot;
use crate::error::ApiError;
use crate::subject::get_did_subject;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LedgerIdentifier {
    pub(crate) canonical: String,
    pub(crate) legacy: Option<String>,
}

impl LedgerIdentifier {
    fn candidates(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.canonical.as_str()).chain(self.legacy.as_deref())
    }
}

pub(crate) fn ledger_identifier<B: Backend>(
    contract: &Contract<B>,
    canonical: impl Into<String>,
) -> Result<LedgerIdentifier, ApiError> {
    let canonical = canonical.into();
    let subject = get_did_subject(contract)?;
    let legacy = canonical
        .strip_prefix(&subject)
        .filter(|suffix| suffix.starts_with('#'))
        .map(ToOwned::to_owned);
    Ok(LedgerIdentifier { canonical, legacy })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VerificationMethodLedgerKind {
    Opaque,
    SchnorrJubjub,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExistingVerificationMethodLedgerIdentifier {
    pub(crate) id: String,
    pub(crate) kind: VerificationMethodLedgerKind,
}

pub(crate) fn find_existing_service_ledger_id(
    state: &DidLedgerSnapshot,
    identifier: &LedgerIdentifier,
) -> Result<Option<String>, ApiError> {
    let matches = identifier
        .candidates()
        .filter(|id| state.services.contains_key(*id))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(ApiError::invalid_argument(format!(
            "ambiguous service identifier '{}': canonical and legacy ledger keys both exist",
            identifier.canonical
        )));
    }
    Ok(matches.into_iter().next())
}

pub(crate) fn require_existing_service_ledger_id(
    state: &DidLedgerSnapshot,
    identifier: &LedgerIdentifier,
) -> Result<String, ApiError> {
    find_existing_service_ledger_id(state, identifier)?
        .ok_or_else(|| ApiError::invalid_argument(format!("service {} does not exist", identifier.canonical)))
}

pub(crate) fn find_existing_verification_method_ledger_identifier(
    state: &DidLedgerSnapshot,
    identifier: &LedgerIdentifier,
) -> Result<Option<ExistingVerificationMethodLedgerIdentifier>, ApiError> {
    let mut matches = Vec::new();
    for id in identifier.candidates() {
        if state.verification_methods.contains_key(id) {
            matches.push(ExistingVerificationMethodLedgerIdentifier {
                id: id.to_owned(),
                kind: VerificationMethodLedgerKind::Opaque,
            });
        }
        if state.schnorr_jubjub_verification_methods.contains_key(id) {
            matches.push(ExistingVerificationMethodLedgerIdentifier {
                id: id.to_owned(),
                kind: VerificationMethodLedgerKind::SchnorrJubjub,
            });
        }
    }
    if matches.len() > 1 {
        return Err(ApiError::invalid_argument(format!(
            "ambiguous verification method identifier '{}': multiple canonical or legacy ledger records exist",
            identifier.canonical
        )));
    }
    Ok(matches.into_iter().next())
}

pub(crate) fn require_existing_verification_method_ledger_id(
    state: &DidLedgerSnapshot,
    identifier: &LedgerIdentifier,
    expected_kind: Option<VerificationMethodLedgerKind>,
) -> Result<ExistingVerificationMethodLedgerIdentifier, ApiError> {
    let existing = find_existing_verification_method_ledger_identifier(state, identifier)?.ok_or_else(|| {
        crate::error::VerificationError::MethodMissing {
            method_id: identifier.canonical.clone(),
        }
    })?;
    if expected_kind.is_some_and(|kind| existing.kind != kind) {
        return Err(crate::error::VerificationError::MethodMissing {
            method_id: identifier.canonical.clone(),
        }
        .into());
    }
    Ok(existing)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use midnight_did_method::midnight_did::{MidnightNetwork, parse_contract_address};
    use midnight_did_runtime::{LedgerService, RecordingBackend};

    use super::*;

    const ADDRESS: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn contract() -> Contract<RecordingBackend> {
        Contract::new(
            RecordingBackend::new(),
            parse_contract_address(ADDRESS).expect("address"),
            MidnightNetwork::Testnet,
        )
    }

    #[test]
    fn only_plain_subject_fragments_receive_legacy_aliases() {
        let contract = contract();
        let subject = get_did_subject(&contract).expect("subject");
        assert_eq!(
            ledger_identifier(&contract, format!("{subject}#key"))
                .expect("identifier")
                .legacy,
            Some("#key".into())
        );
        assert_eq!(
            ledger_identifier(&contract, format!("{subject}/keys#key"))
                .expect("identifier")
                .legacy,
            None
        );
        assert_eq!(
            ledger_identifier(&contract, format!("{subject}?version=1#key"))
                .expect("identifier")
                .legacy,
            None
        );
    }

    #[test]
    fn canonical_and_legacy_service_records_are_ambiguous() {
        let contract = contract();
        let subject = get_did_subject(&contract).expect("subject");
        let canonical = format!("{subject}#svc");
        let service = LedgerService {
            id: canonical.clone(),
            typ: "LinkedDomains".into(),
            service_endpoint: "\"https://example.com\"".into(),
        };
        let state = DidLedgerSnapshot {
            services: BTreeMap::from([(canonical.clone(), service.clone()), ("#svc".into(), service)]),
            ..DidLedgerSnapshot::default()
        };
        let identifier = ledger_identifier(&contract, canonical).expect("identifier");
        assert!(matches!(
            find_existing_service_ledger_id(&state, &identifier),
            Err(ApiError::InvalidArgument(message)) if message.contains("ambiguous service identifier")
        ));
    }
}
