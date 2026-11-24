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

//! Helpers that shape ledger reads/writes for the Midnight DID contract.
//!
//! Port of `ledger-utils.ts`. The helpers focus on string-level pre/post-
//! processing — fragment-id normalization, encoding the polymorphic service
//! `type` and `serviceEndpoint` fields into the on-chain string flavour, and
//! aliasing absolute-URI checks.

use thiserror::Error;

use crate::did_document::{ServiceEndpoint, ServiceType, normalize_service_endpoint};
use crate::did_url::{DidUrlResolutionError, DidUrlResolutionOptions, resolve_did_url_reference};

/// Which ledger field a value is bound to. Used solely for error messages so
/// callers can distinguish failures between, e.g., `verificationMethod.id`
/// and `service.id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundIdField {
    /// `verificationMethod.id`.
    VerificationMethodId,
    /// `schnorrJubjubVerificationMethod.id`.
    SchnorrJubjubVerificationMethodId,
    /// `service.id`.
    ServiceId,
    /// `methodId` (compact field name).
    MethodId,
    /// `serviceId` (compact field name).
    ShortServiceId,
}

impl BoundIdField {
    fn label(self) -> &'static str {
        match self {
            BoundIdField::VerificationMethodId => "verificationMethod.id",
            BoundIdField::SchnorrJubjubVerificationMethodId => "schnorrJubjubVerificationMethod.id",
            BoundIdField::ServiceId => "service.id",
            BoundIdField::MethodId => "methodId",
            BoundIdField::ShortServiceId => "serviceId",
        }
    }
}

/// Errors surfaced by the ledger helpers.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LedgerUtilsError {
    /// Value was empty (after trimming).
    #[error("{field} must not be empty")]
    Empty {
        /// Field name where the empty value was provided.
        field: String,
    },
    /// Value did not look like a DID URL or a relative reference.
    #[error("{field} must be a DID URL or relative reference")]
    NotARelativeOrDidUrl {
        /// Field name.
        field: String,
    },
    /// Embedded DID URL fragment was empty.
    #[error("{field} DID URL must include a non-empty fragment identifier")]
    EmptyFragment {
        /// Field name.
        field: String,
    },
    /// DID URL subject did not match the current DID.
    #[error("{field} DID URL subject must match the current DID ({expected})")]
    SubjectMismatch {
        /// Field name.
        field: String,
        /// Expected DID subject.
        expected: String,
    },
    /// A service id resolved to the bare DID rather than a service resource.
    #[error("{field} must identify a service")]
    DoesNotIdentifyService {
        /// Field name.
        field: String,
    },
    /// Service-type entries were not unique.
    #[error("service type entries must be unique")]
    DuplicateServiceTypeEntries,
    /// Service-type entries were empty.
    #[error("service type entries must not be empty")]
    EmptyServiceTypeEntries,
    /// `service type` property was empty.
    #[error("service type must not be empty")]
    EmptyServiceType,
    /// `service type` property was malformed.
    #[error("service type property must be a non-empty string set")]
    InvalidServiceType,
    /// Absolute-URI assertion failed.
    #[error("{field} must be a valid absolute URI (RFC3986)")]
    NotAbsoluteUri {
        /// Field name.
        field: String,
    },
}

/// Normalize any fragment-id-shaped string to the leading-`#` form. Bare
/// fragments are prefixed, DID URLs are reduced to their fragment portion.
#[must_use]
pub fn normalize_fragment_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('#') {
        return trimmed.to_owned();
    }
    if let Some(idx) = trimmed.rfind('#') {
        return format!("#{}", &trimmed[idx + 1..]);
    }
    format!("#{trimmed}")
}

/// Resolve an identifier against a specific DID subject without dropping its
/// path, query, or fragment components.
///
/// This is the Rust equivalent of TypeScript `normalizeBoundDIDURL`.
/// Service fields may use non-DID absolute URLs but foreign DID subjects are
/// rejected.
///
/// # Errors
///
/// Returns a field-specific [`LedgerUtilsError`] for malformed, external, or
/// empty references.
pub fn normalize_bound_did_url(
    value: &str,
    field: BoundIdField,
    expected_did_subject: &str,
) -> Result<String, LedgerUtilsError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(LedgerUtilsError::Empty {
            field: field.label().into(),
        });
    }
    if trimmed.ends_with('#') {
        return Err(LedgerUtilsError::EmptyFragment {
            field: field.label().into(),
        });
    }
    let service_field = matches!(field, BoundIdField::ServiceId | BoundIdField::ShortServiceId);
    let resolved = resolve_did_url_reference(
        trimmed,
        expected_did_subject,
        DidUrlResolutionOptions {
            allow_external_url: service_field,
            case_insensitive_did_subject: true,
            ..DidUrlResolutionOptions::default()
        },
    )
    .map_err(|error| match error {
        DidUrlResolutionError::Empty => LedgerUtilsError::Empty {
            field: field.label().into(),
        },
        DidUrlResolutionError::SubjectMismatch { .. } => LedgerUtilsError::SubjectMismatch {
            field: field.label().into(),
            expected: expected_did_subject.into(),
        },
        DidUrlResolutionError::NetworkPath
        | DidUrlResolutionError::ExternalUrl
        | DidUrlResolutionError::InvalidAbsoluteUrl => LedgerUtilsError::NotARelativeOrDidUrl {
            field: field.label().into(),
        },
    })?;
    if service_field && resolved == expected_did_subject {
        return Err(LedgerUtilsError::DoesNotIdentifyService {
            field: field.label().into(),
        });
    }
    Ok(resolved)
}

/// Normalize a fragment-bearing identifier that is bound to a specific DID
/// subject while preserving its complete canonical absolute DID URL.
///
/// The historical function name is retained for compatibility with the
/// TypeScript API. It no longer reduces identifiers to `#fragment`.
///
/// # Errors
///
/// Returns [`LedgerUtilsError::Empty`] for blank input,
/// [`LedgerUtilsError::NotARelativeOrDidUrl`] when the value cannot be
/// parsed as a relative reference or DID URL,
/// [`LedgerUtilsError::EmptyFragment`] when the DID URL has no fragment
/// portion, or [`LedgerUtilsError::SubjectMismatch`] when the embedded
/// DID does not match `expected_did_subject`.
pub fn normalize_bound_fragment_id(
    value: &str,
    field: BoundIdField,
    expected_did_subject: &str,
) -> Result<String, LedgerUtilsError> {
    let trimmed = value.trim();
    let bare_label = !trimmed.contains('#')
        && !trimmed.starts_with('/')
        && !trimmed.starts_with('.')
        && !trimmed.starts_with('?')
        && !super::did_url::has_uri_scheme_for_ledger_utils(trimmed);
    let reference = if bare_label {
        format!("#{trimmed}")
    } else {
        trimmed.to_owned()
    };
    let resolved = normalize_bound_did_url(&reference, field, expected_did_subject)?;
    if !resolved.contains('#') {
        return Err(LedgerUtilsError::EmptyFragment {
            field: field.label().into(),
        });
    }
    if !resolved.starts_with(&format!("{expected_did_subject}#"))
        && !resolved.starts_with(&format!("{expected_did_subject}/"))
        && !resolved.starts_with(&format!("{expected_did_subject}?"))
    {
        return Err(LedgerUtilsError::SubjectMismatch {
            field: field.label().into(),
            expected: expected_did_subject.into(),
        });
    }
    Ok(resolved)
}

/// Encode the polymorphic service `type` property into the single-string
/// shape the ledger stores. A single type becomes the value as-is; an array
/// is JSON-encoded so callers can recover the original list.
///
/// # Errors
///
/// Returns [`LedgerUtilsError::EmptyServiceType`] when the single-type
/// variant is blank, [`LedgerUtilsError::InvalidServiceType`] when the
/// multi-type array is empty, [`LedgerUtilsError::EmptyServiceTypeEntries`]
/// when any entry is blank, or
/// [`LedgerUtilsError::DuplicateServiceTypeEntries`] when entries are not
/// unique.
///
/// # Panics
///
/// Panics only on impossible-by-construction conditions: the
/// `normalized.into_iter().next().unwrap()` is guarded by
/// `if normalized.len() == 1`, and `serde_json::to_string` on a
/// `Vec<String>` is total. A panic here would indicate a logic bug in
/// this function, not a fallible runtime input.
pub fn service_type_to_ledger(service_type: &ServiceType) -> Result<String, LedgerUtilsError> {
    match service_type {
        ServiceType::One(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Err(LedgerUtilsError::EmptyServiceType);
            }
            Ok(trimmed.to_owned())
        }
        ServiceType::Many(types) => {
            if types.is_empty() {
                return Err(LedgerUtilsError::InvalidServiceType);
            }
            let normalized: Vec<String> = types.iter().map(|s| s.trim().to_owned()).collect();
            if normalized.iter().any(|s| s.is_empty()) {
                return Err(LedgerUtilsError::EmptyServiceTypeEntries);
            }
            let mut sorted = normalized.clone();
            sorted.sort();
            sorted.dedup();
            if sorted.len() != normalized.len() {
                return Err(LedgerUtilsError::DuplicateServiceTypeEntries);
            }
            if normalized.len() == 1 {
                Ok(normalized.into_iter().next().unwrap())
            } else {
                Ok(serde_json::to_string(&normalized).expect("serialize ServiceType"))
            }
        }
    }
}

/// Encode a service endpoint as the JSON-encoded canonical string the ledger
/// uses. Performs URI normalization on every nested string.
///
/// # Panics
///
/// `serde_json::to_string` on a normalised `ServiceEndpoint` is total —
/// every variant contains only strings or maps of strings. A panic here
/// would indicate a `serde` invariant break, not a runtime input failure.
#[must_use]
pub fn service_endpoint_to_ledger(endpoint: ServiceEndpoint) -> String {
    let normalized = normalize_service_endpoint(endpoint);
    serde_json::to_string(&normalized).expect("serialize ServiceEndpoint")
}

/// Assert that `value` is a syntactically-valid absolute URI (RFC 3986).
/// Returns the trimmed value on success. Default field label is `aliasUri`,
/// matching the TS port.
///
/// # Errors
///
/// Returns [`LedgerUtilsError::Empty`] when the trimmed value is empty,
/// or [`LedgerUtilsError::NotAbsoluteUri`] when the value does not parse
/// as an absolute URI.
pub fn assert_absolute_uri(value: &str, field: Option<&str>) -> Result<String, LedgerUtilsError> {
    let label = field.unwrap_or("aliasUri");
    let alias = value.trim();
    if alias.is_empty() {
        return Err(LedgerUtilsError::Empty { field: label.into() });
    }
    url::Url::parse(alias)
        .map(|_| ())
        .map_err(|_| LedgerUtilsError::NotAbsoluteUri { field: label.into() })?;
    // TS parity (`assertAbsoluteUri`): the TRIMMED alias is returned
    // verbatim — aliases are stored as given, not normalized.
    Ok(alias.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_bare_fragment() {
        assert_eq!(normalize_fragment_id("key-1"), "#key-1");
        assert_eq!(normalize_fragment_id("did:example:1#key-1"), "#key-1");
        assert_eq!(normalize_fragment_id("#key-1"), "#key-1");
    }

    #[test]
    fn binds_fragment_to_did() {
        let did = "did:midnight:devnet:abcd";
        let frag =
            normalize_bound_fragment_id(&format!("{did}#key-1"), BoundIdField::VerificationMethodId, did).unwrap();
        assert_eq!(frag, format!("{did}#key-1"));
    }

    #[test]
    fn rejects_mismatched_subject() {
        let did = "did:midnight:devnet:abcd";
        let err = normalize_bound_fragment_id(
            "did:midnight:devnet:xxxx#key-1",
            BoundIdField::VerificationMethodId,
            did,
        )
        .unwrap_err();
        assert!(matches!(err, LedgerUtilsError::SubjectMismatch { .. }));
    }
}
