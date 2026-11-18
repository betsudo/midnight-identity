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

//! Generated Compact bindings for the Midnight VC core contract.
//!
//! Emitted by the flake-pinned `compactc --rust --skip-ts`. **Do not edit
//! these files by hand** — run `just codegen-vc` after bumping the
//! `vc_core_version` pin in the `justfile` or the compact flake input, and
//! `just codegen-vc-check` to assert the committed output is reproducible.
//!
//! | module | entry point |
//! |---|---|
//! | [`credentials`] | `dist/credentials.compact` in the `@midnight-ntwrk/credential-compact` npm package |
//!
//! VC Core 1.0.0-rc1 adds explicit holder/status bindings, signer
//! authorization, and context-separated issuance/presentation proof circuits.

#![allow(missing_docs)] // generated modules carry their own header docs

/// Compatibility namespace for the generated VC Core surface.
pub mod credentials {
    pub use crate::runtime;
    pub use crate::types::*;

    /// Backwards-compatible snake-case names around Ledger 9 generated calls.
    pub mod pure_circuits {
        pub use crate::pure_circuits::issuanceProofChallenge as issuance_proof_challenge;
        pub use crate::pure_circuits::*;
    }
}
