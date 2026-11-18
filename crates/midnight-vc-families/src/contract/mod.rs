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

//! Generated Compact bindings for the Midnight VC credential families.
//!
//! One module per family entry point, emitted by the flake-pinned
//! `compactc --rust --skip-ts` and gated behind the family's cargo feature.
//! **Do not edit these files by hand** — run `just codegen-vc` after bumping
//! a family package pin, the compact flake input, or the pinned
//! `@midnight-ntwrk/credential-compact` core package, and `just
//! codegen-vc-check` to assert the committed output is reproducible.
//!
//! | module | feature | entry point |
//! |---|---|---|
//! | `digital_passport` | `digital-passport` | `@midnight-ntwrk/midnight-vc-passport` release tarball (`midnightntwrk/midnight-vc-passport`, tag `v0.1.0-rc5`): `src/digital-passport-credential.compact` |
//!
//! The modules are re-exported as modules rather than glob-re-exported:
//! each family entry point includes the VC core contract
//! (`credentials.compact` — staged from the pinned npm core package for the
//! digital-passport family), so its generated output redeclares every core
//! credentials type and a flat re-export would collide (the same
//! reason `midnight-vc-runtime` exposes `credentials` as a module). Consumers must not mix types across
//! family modules.
//!
//! Onboarding a further family is purely additive: a new cargo feature, a
//! new `#[cfg(feature)]`-gated module line here, and a new entry in the
//! `just codegen-vc` list — no existing family's feature, module, or
//! generated file changes.

#![allow(missing_docs)] // generated modules carry their own header docs

#[cfg(feature = "digital-passport")]
/// Compatibility namespace for the generated DigitalPassport surface.
pub mod digital_passport {
    pub use crate::runtime;
    pub use crate::types::*;

    /// Backwards-compatible snake-case names around Ledger 9 generated calls.
    pub mod pure_circuits {
        pub use crate::pure_circuits::*;
        pub use crate::pure_circuits::{
            assertValidDigitalPassportAgePredicate as assert_valid_digital_passport_age_predicate,
            assertValidDigitalPassportCredential as assert_valid_digital_passport_credential,
            assertValidDigitalPassportPresentation as assert_valid_digital_passport_presentation,
            assertValidIssuanceContextProof as assert_valid_issuance_context_proof,
            dateOfBirthCommitment as date_of_birth_commitment, digitalPassportClaimRoot as digital_passport_claim_root,
            digitalPassportCredentialBodyRoot as digital_passport_credential_body_root,
            digitalPassportPresentationBodyRoot as digital_passport_presentation_body_root,
            documentNumberCommitment as document_number_commitment,
            documentNumberNullCommitment as document_number_null_commitment,
            firstNameCommitment as first_name_commitment, issuanceProofChallenge as issuance_proof_challenge,
            issuingStateCommitment as issuing_state_commitment, lastNameCommitment as last_name_commitment,
            presentationProofChallenge as presentation_proof_challenge,
        };
    }
}
