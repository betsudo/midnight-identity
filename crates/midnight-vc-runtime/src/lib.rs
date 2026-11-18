// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Codegen target for Midnight Verifiable Credentials (issue #13, ADR 0009
//! split rule 1).
//!
//! The VC-layer counterpart of `midnight-did-runtime`: it holds the Rust
//! bindings the flake-pinned `compactc --rust --skip-ts` emits for the VC
//! Compact contracts, and nothing else yet. It is a separate crate from
//! [`midnight_vc_domain`] because `midnight-compact-runtime` drags in the halo2 /
//! arkworks stack, which does not build for `wasm32-unknown-unknown` (ADR 0006,
//! ADR 0009 rule 1).
//!
//! [`midnight_vc_domain`]: https://docs.rs/midnight-vc-domain
//!
//! See [`contract`] for the per-contract modules, the exact upstream entry
//! points, and the regeneration workflow.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]
#![allow(missing_docs)] // generated crate-root items carry upstream names

/// Crate version reported by the build.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// The Ledger 9 Rust AST backend emits absolute `crate::types` and
// `crate::pure_circuits` references. Mount the generated unit at crate root,
// then preserve the historical `contract::credentials::*` path with re-exports
// in `contract/mod.rs`.
include!("contract/credentials.rs");

pub mod contract;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        ContractAddress, ExplicitHolderBinding, RegistryBoundStatusBinding, StatusRegistryRef, VerificationMethodRef,
    };

    fn verification_method_ref(controller: u8, method: u8) -> VerificationMethodRef {
        VerificationMethodRef {
            controllerAddress: ContractAddress {
                bytes: runtime::FixedBytes([controller; 32]),
            },
            methodId: runtime::FixedBytes([method; 32]),
        }
    }

    #[test]
    fn version_is_reported() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn explicit_holder_binding_requires_the_same_controller_and_method() {
        let binding = ExplicitHolderBinding {
            holderVerificationMethodRef: verification_method_ref(1, 2),
        };
        assert!(pure_circuits::assertValidExplicitHolderBinding(binding.clone()).is_ok());
        assert!(pure_circuits::assertMatchingExplicitHolderBindings(binding.clone(), binding.clone()).is_ok());

        let different_controller = ExplicitHolderBinding {
            holderVerificationMethodRef: verification_method_ref(3, 2),
        };
        assert!(pure_circuits::assertMatchingExplicitHolderBindings(binding.clone(), different_controller).is_err());

        let different_method = ExplicitHolderBinding {
            holderVerificationMethodRef: verification_method_ref(1, 4),
        };
        assert!(pure_circuits::assertMatchingExplicitHolderBindings(binding, different_method).is_err());
    }

    #[test]
    fn registry_status_binding_refuses_unset_ids_and_commits_every_field() {
        let binding = RegistryBoundStatusBinding {
            registryRef: StatusRegistryRef {
                registryId: runtime::FixedBytes([5; 32]),
                authorityVerificationMethodRef: verification_method_ref(6, 7),
            },
            statusHandleCommitment: runtime::FixedBytes([8; 32]),
        };
        let root = pure_circuits::registryBoundStatusBindingRoot(binding.clone()).unwrap();
        assert_eq!(
            root,
            pure_circuits::registryBoundStatusBindingRoot(binding.clone()).unwrap()
        );

        let mut changed = binding.clone();
        changed.statusHandleCommitment = runtime::FixedBytes([9; 32]);
        assert_ne!(root, pure_circuits::registryBoundStatusBindingRoot(changed).unwrap());

        let mut missing_registry = binding.clone();
        missing_registry.registryRef.registryId = runtime::FixedBytes([0; 32]);
        assert!(pure_circuits::assertValidRegistryBoundStatusBinding(missing_registry).is_err());

        let mut missing_handle = binding;
        missing_handle.statusHandleCommitment = runtime::FixedBytes([0; 32]);
        assert!(pure_circuits::assertValidRegistryBoundStatusBinding(missing_handle).is_err());
    }
}
