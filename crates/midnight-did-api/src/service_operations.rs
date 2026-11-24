// This file is part of Compact.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//  	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Service endpoint operations.
//!
//! Rust port of `packages/api/src/service-operations.ts`.

use midnight_did_domain::did_document::Service;
use midnight_did_domain::ledger_utils::BoundIdField;
use midnight_did_runtime::{Backend, Contract};

use crate::contract::{FinalizedTxData, MapMutation};
use crate::error::{ApiError, ContractError};
use crate::ledger_identifier_keys::{
    find_existing_service_ledger_id, ledger_identifier, require_existing_service_ledger_id,
};
use crate::ledger_mappers::service_to_ledger;
use crate::subject::normalize_bound_did_url_for;

/// `addService`.
pub async fn add_service<B: Backend>(contract: &Contract<B>, service: &Service) -> Result<FinalizedTxData, ApiError> {
    let ledger = service_to_ledger(contract, service)?;
    let state = contract
        .read_snapshot()
        .await
        .map_err(|e| ApiError::Contract(ContractError::Failed(e.to_string())))?;
    let identifier = ledger_identifier(contract, ledger.id.clone())?;
    if find_existing_service_ledger_id(&state, &identifier)?.is_some() {
        return Err(ApiError::invalid_argument(format!(
            "service {} already exists",
            ledger.id
        )));
    }
    contract
        .set_service(ledger, MapMutation::Insert)
        .await
        .map_err(|e| ApiError::Contract(ContractError::Failed(e.to_string())))
}

/// `updateService`.
pub async fn update_service<B: Backend>(
    contract: &Contract<B>,
    service: &Service,
) -> Result<FinalizedTxData, ApiError> {
    let mut ledger = service_to_ledger(contract, service)?;
    let state = contract
        .read_snapshot()
        .await
        .map_err(|e| ApiError::Contract(ContractError::Failed(e.to_string())))?;
    let identifier = ledger_identifier(contract, ledger.id.clone())?;
    ledger.id = require_existing_service_ledger_id(&state, &identifier)?;
    contract
        .set_service(ledger, MapMutation::Update)
        .await
        .map_err(|e| ApiError::Contract(ContractError::Failed(e.to_string())))
}

/// `removeService`.
pub async fn remove_service<B: Backend>(contract: &Contract<B>, service_id: &str) -> Result<FinalizedTxData, ApiError> {
    let canonical = normalize_bound_did_url_for(contract, service_id, BoundIdField::ShortServiceId)?;
    let state = contract
        .read_snapshot()
        .await
        .map_err(|e| ApiError::Contract(ContractError::Failed(e.to_string())))?;
    let identifier = ledger_identifier(contract, canonical)?;
    let physical_id = require_existing_service_ledger_id(&state, &identifier)?;
    contract
        .remove_service(physical_id)
        .await
        .map_err(|e| ApiError::Contract(ContractError::Failed(e.to_string())))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use midnight_did_domain::did_document::{NewService, ServiceEndpoint, ServiceType};
    use midnight_did_method::midnight_did::{MidnightNetwork, parse_contract_address};
    use midnight_did_runtime::{DidContractCall, DidLedgerSnapshot, LedgerService, RecordingBackend};

    use super::*;

    const ADDR: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn contract() -> Contract<RecordingBackend> {
        Contract::new(
            RecordingBackend::new(),
            parse_contract_address(ADDR).unwrap(),
            MidnightNetwork::Testnet,
        )
    }

    fn contract_with_service() -> Contract<RecordingBackend> {
        let snapshot = DidLedgerSnapshot {
            services: BTreeMap::from([(
                "#svc-1".into(),
                LedgerService {
                    id: "#svc-1".into(),
                    typ: "LinkedDomains".into(),
                    service_endpoint: "\"https://example.com\"".into(),
                },
            )]),
            ..DidLedgerSnapshot::default()
        };
        Contract::new(
            RecordingBackend::with_snapshot(snapshot),
            parse_contract_address(ADDR).unwrap(),
            MidnightNetwork::Testnet,
        )
    }

    #[tokio::test]
    async fn adds_a_service() {
        let contract = contract();
        let svc = Service::new(NewService {
            id: "svc-1".into(),
            type_: ServiceType::One("LinkedDomains".into()),
            service_endpoint: ServiceEndpoint::Uri("https://example.com".into()),
        })
        .expect("valid service");
        add_service(&contract, &svc).await.unwrap();
        let calls = contract.backend.recorded_calls();
        assert!(matches!(
            &calls[..],
            [DidContractCall::ReadLedger, DidContractCall::SetService { service, mutation: MapMutation::Insert }]
                if service.id == format!("did:midnight:testnet:{ADDR}/svc-1")
        ));
    }

    #[tokio::test]
    async fn removes_a_service() {
        let contract = contract_with_service();
        remove_service(&contract, "#svc-1").await.unwrap();
        let calls = contract.backend.recorded_calls();
        assert!(matches!(
            &calls[..],
            [DidContractCall::ReadLedger, DidContractCall::RemoveService { service_id }]
                if service_id == "#svc-1"
        ));
    }
}
