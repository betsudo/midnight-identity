//! Ledger-neutral DID resolution adapter for VC bindings.

use midnight_did_domain::{
    BoundIdField, CurveType, DidResolutionResult, KeyType, VerificationMethodRelation, VerificationMethodType,
    decode_jubjub_jwk_coordinate, normalize_bound_fragment_id,
};
use midnight_did_method::{MidnightNetwork, MidnightSubjectId, parse_midnight_did, parse_midnight_did_string};

/// VC Core verification relationship supported by the DID binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationRelationship {
    /// Credential issuance authorization.
    AssertionMethod,
    /// Holder presentation authentication.
    Authentication,
    /// Capability invocation authorization.
    CapabilityInvocation,
}

/// A resolved, subject-bound native Jubjub verification method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidnightDidMethodBinding {
    /// On-chain DID contract address.
    pub controller_address: [u8; 32],
    /// SHA-256 of the canonical `#fragment`, matching the upstream adapter.
    pub method_id: [u8; 32],
    /// Canonical big-endian Jubjub x coordinate from the DID JWK.
    pub public_key_x: [u8; 32],
    /// Canonical big-endian Jubjub y coordinate from the DID JWK.
    pub public_key_y: [u8; 32],
    /// Positive resolved DID state version.
    pub did_state_version: u64,
    /// Relationship under which the method was resolved.
    pub verification_relationship: VerificationRelationship,
}

/// Explicit holder binding backed by an authentication method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidnightDidHolderBinding {
    /// Bound method.
    pub method: MidnightDidMethodBinding,
}

/// Refusals surfaced while mapping a DID resolution result.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BindingError {
    /// DID is malformed or is not an on-chain Midnight DID.
    #[error("an on-chain did:midnight subject is required")]
    InvalidDid,
    /// Resolution did not contain the requested subject document.
    #[error("resolved DID document subject does not match the request")]
    SubjectMismatch,
    /// Resolution reports a deactivated subject.
    #[error("resolved Midnight DID is deactivated")]
    Deactivated,
    /// Resolution omitted a positive uint64 state version.
    #[error("Midnight DID resolution must provide a positive uint64 versionId")]
    InvalidStateVersion,
    /// Requested method is absent.
    #[error("verification method is absent from the DID document")]
    MethodMissing,
    /// Method fragment is not a non-empty canonical `#fragment`.
    #[error("Midnight DID method identifier must be a non-empty canonical fragment")]
    InvalidMethodId,
    /// Method is not authorized for the requested relationship.
    #[error("verification method is not authorized for the requested relationship")]
    RelationshipMissing,
    /// Method is not a subject-controlled EC/Jubjub JSON Web Key.
    #[error("Midnight VC proofs require a subject-controlled EC/Jubjub JsonWebKey")]
    InvalidMethodProfile,
    /// Canonical JWK coordinate decode failed.
    #[error("Midnight DID Jubjub coordinates are not canonical")]
    InvalidCoordinates,
}

/// Raw SHA-256 of a canonical Midnight DID method fragment.
///
/// This intentionally does not reuse the older off-chain holder-binding
/// domain tag: the 1.0 upstream adapter specifies `sha256(UTF8(fragment))`.
pub fn midnight_did_method_id(canonical_fragment: &str) -> Result<[u8; 32], BindingError> {
    if !canonical_fragment.starts_with('#') || canonical_fragment.len() == 1 || canonical_fragment[1..].contains('#') {
        return Err(BindingError::InvalidMethodId);
    }
    Ok(*identus_crypto::hash::sha256(canonical_fragment.as_bytes()).as_array())
}

/// Resolve a native Jubjub method from an already obtained DID resolution result.
pub fn resolve_midnight_did_method_binding(
    result: &DidResolutionResult,
    did: &str,
    verification_method_id: &str,
    relationship: VerificationRelationship,
) -> Result<MidnightDidMethodBinding, BindingError> {
    let parsed = parse_midnight_did_string(did).map_err(|_| BindingError::InvalidDid)?;
    let (network, subject) = parse_midnight_did(&parsed).map_err(|_| BindingError::InvalidDid)?;
    let MidnightSubjectId::Contract(address) = subject else {
        return Err(BindingError::InvalidDid);
    };
    if network == MidnightNetwork::Offchain {
        return Err(BindingError::InvalidDid);
    }
    let document = result.did_document.as_ref().ok_or(BindingError::SubjectMismatch)?;
    if document.id.as_str() != did {
        return Err(BindingError::SubjectMismatch);
    }
    if result.did_document_metadata.deactivated == Some(true) {
        return Err(BindingError::Deactivated);
    }
    let canonical_method_id =
        normalize_bound_fragment_id(verification_method_id, BoundIdField::VerificationMethodId, did)
            .map_err(|_| BindingError::MethodMissing)?;
    // Credential DID binding v1 hashes a current-subject `#fragment`, not an
    // arbitrary DID URL. Refuse path/query identities instead of collapsing
    // them to the same fragment and creating an authorization alias.
    let fragment = canonical_method_id
        .strip_prefix(did)
        .filter(|suffix| suffix.starts_with('#'))
        .ok_or(BindingError::InvalidMethodId)?;
    let method = document
        .verification_method
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|method| {
            normalize_bound_fragment_id(method.id().as_str(), BoundIdField::VerificationMethodId, did)
                .is_ok_and(|candidate| candidate == canonical_method_id)
        })
        .ok_or(BindingError::MethodMissing)?;
    if method.controller().as_str() != did
        || method.type_() != VerificationMethodType::JsonWebKey
        || method.public_key_jwk().kty() != KeyType::EC
        || method.public_key_jwk().crv() != CurveType::Jubjub
    {
        return Err(BindingError::InvalidMethodProfile);
    }
    let relation_entries = match relationship {
        VerificationRelationship::AssertionMethod => document.assertion_method.as_deref(),
        VerificationRelationship::Authentication => document.authentication.as_deref(),
        VerificationRelationship::CapabilityInvocation => document.capability_invocation.as_deref(),
    }
    .unwrap_or_default();
    if !relation_entries.iter().any(|entry| {
        normalize_bound_fragment_id(entry.as_str(), BoundIdField::VerificationMethodId, did)
            .is_ok_and(|candidate| candidate == canonical_method_id)
    }) {
        return Err(BindingError::RelationshipMissing);
    }
    let jwk = method.public_key_jwk();
    let x = decode_jubjub_jwk_coordinate(jwk.x(), "publicKeyJwk.x").map_err(|_| BindingError::InvalidCoordinates)?;
    let y = decode_jubjub_jwk_coordinate(jwk.y().ok_or(BindingError::InvalidMethodProfile)?, "publicKeyJwk.y")
        .map_err(|_| BindingError::InvalidCoordinates)?;
    // Match the upstream adapter's refusal order: method/profile/relationship
    // validation precedes the observed state-version check.
    let version = result
        .did_document_metadata
        .version_id
        .as_deref()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .ok_or(BindingError::InvalidStateVersion)?;
    Ok(MidnightDidMethodBinding {
        controller_address: address.0,
        method_id: midnight_did_method_id(fragment)?,
        public_key_x: x,
        public_key_y: y,
        did_state_version: version,
        verification_relationship: relationship,
    })
}

/// Create an explicit holder binding, refusing non-authentication methods.
pub fn create_midnight_did_holder_binding(
    method: MidnightDidMethodBinding,
) -> Result<MidnightDidHolderBinding, BindingError> {
    if method.verification_relationship != VerificationRelationship::Authentication {
        return Err(BindingError::RelationshipMissing);
    }
    Ok(MidnightDidHolderBinding { method })
}

impl From<VerificationRelationship> for VerificationMethodRelation {
    fn from(value: VerificationRelationship) -> Self {
        match value {
            VerificationRelationship::AssertionMethod => Self::AssertionMethod,
            VerificationRelationship::Authentication => Self::Authentication,
            VerificationRelationship::CapabilityInvocation => Self::CapabilityInvocation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DID: &str = "did:midnight:testnet:1111111111111111111111111111111111111111111111111111111111111111";

    fn resolution() -> DidResolutionResult {
        serde_json::from_value(serde_json::json!({
            "didDocument": {
                "@context": ["https://www.w3.org/ns/did/v1", "https://w3id.org/security/jwk/v1"],
                "id": DID,
                "alsoKnownAs": null,
                "verificationMethod": [{
                    "id": format!("{DID}#issuer-key"),
                    "type": "JsonWebKey",
                    "controller": DID,
                    "publicKeyJwk": {
                        "kty": "EC", "crv": "Jubjub",
                        "x": "EMyWcM8XCxkJTyn8MDXM4q6wVLMfqCxYCu0MwT0hHPQ",
                        "y": "H0wYFnDb0GGRQPznl3NU9G1sohdsMMrVdZpEQyiZrd0"
                    }
                }],
                "authentication": ["#issuer-key"],
                "assertionMethod": ["#issuer-key"],
                "service": null
            },
            "didDocumentMetadata": { "versionId": "12" },
            "didResolutionMetadata": {}
        }))
        .expect("valid resolution fixture")
    }

    #[test]
    fn maps_upstream_07_vector_and_refuses_wrong_relationship() {
        let binding = resolve_midnight_did_method_binding(
            &resolution(),
            DID,
            "#issuer-key",
            VerificationRelationship::AssertionMethod,
        )
        .unwrap();
        assert_eq!(binding.controller_address, [0x11; 32]);
        assert_eq!(binding.did_state_version, 12);
        assert_eq!(binding.method_id, midnight_did_method_id("#issuer-key").unwrap());
        assert_eq!(
            create_midnight_did_holder_binding(binding).unwrap_err(),
            BindingError::RelationshipMissing,
        );
    }

    #[test]
    fn rejects_deactivated_and_zero_version_results() {
        let mut result = resolution();
        result.did_document_metadata.deactivated = Some(true);
        assert_eq!(
            resolve_midnight_did_method_binding(&result, DID, "#issuer-key", VerificationRelationship::AssertionMethod)
                .unwrap_err(),
            BindingError::Deactivated,
        );
        result.did_document_metadata.deactivated = Some(false);
        result.did_document_metadata.version_id = Some("0".into());
        assert_eq!(
            resolve_midnight_did_method_binding(&result, DID, "#issuer-key", VerificationRelationship::AssertionMethod)
                .unwrap_err(),
            BindingError::InvalidStateVersion,
        );
    }

    #[test]
    fn creates_an_authentication_holder_binding() {
        let method = resolve_midnight_did_method_binding(
            &resolution(),
            DID,
            "#issuer-key",
            VerificationRelationship::Authentication,
        )
        .unwrap();
        let holder = create_midnight_did_holder_binding(method).unwrap();
        assert_eq!(
            holder.method.verification_relationship,
            VerificationRelationship::Authentication
        );
        assert_eq!(holder.method.controller_address, [0x11; 32]);
    }

    #[test]
    fn rejects_offchain_and_cross_network_subjects() {
        let offchain = format!("did:midnight:offchain:{}", "ab".repeat(32));
        assert_eq!(
            resolve_midnight_did_method_binding(
                &resolution(),
                &offchain,
                "#issuer-key",
                VerificationRelationship::Authentication,
            )
            .unwrap_err(),
            BindingError::InvalidDid,
        );

        let mainnet = format!("did:midnight:mainnet:{}", "11".repeat(32));
        assert_eq!(
            resolve_midnight_did_method_binding(
                &resolution(),
                &mainnet,
                "#issuer-key",
                VerificationRelationship::Authentication,
            )
            .unwrap_err(),
            BindingError::SubjectMismatch,
        );
    }

    #[test]
    fn method_identifier_refuses_noncanonical_fragments() {
        for invalid in ["", "issuer-key", "#", "#issuer#key"] {
            assert_eq!(
                midnight_did_method_id(invalid).unwrap_err(),
                BindingError::InvalidMethodId
            );
        }
        assert_eq!(
            midnight_did_method_id("#issuer-key").unwrap(),
            midnight_did_method_id("#issuer-key").unwrap(),
        );
    }

    #[test]
    fn binding_refuses_path_identity_instead_of_aliasing_its_fragment() {
        let mut result = resolution();
        let document = result.did_document.as_mut().unwrap();
        let method = document.verification_method.as_ref().unwrap()[0].clone();
        document.verification_method = Some(vec![
            midnight_did_domain::VerificationMethod::new(midnight_did_domain::NewVerificationMethod {
                id: format!("{DID}/keys#issuer-key"),
                type_: method.type_(),
                controller: method.controller().as_str().to_owned(),
                public_key_jwk: method.public_key_jwk().clone(),
            })
            .unwrap(),
        ]);
        document.assertion_method = Some(vec![
            midnight_did_domain::DidKeyId::parse(format!("{DID}/keys#issuer-key")).unwrap(),
        ]);

        assert_eq!(
            resolve_midnight_did_method_binding(
                &result,
                DID,
                &format!("{DID}/keys#issuer-key"),
                VerificationRelationship::AssertionMethod,
            )
            .unwrap_err(),
            BindingError::InvalidMethodId,
        );
    }

    #[test]
    fn rejects_missing_methods_relationships_and_foreign_controllers() {
        let mut missing_method = resolution();
        missing_method.did_document.as_mut().unwrap().verification_method = None;
        assert_eq!(
            resolve_midnight_did_method_binding(
                &missing_method,
                DID,
                "#issuer-key",
                VerificationRelationship::AssertionMethod,
            )
            .unwrap_err(),
            BindingError::MethodMissing,
        );

        let mut missing_relationship = resolution();
        missing_relationship.did_document.as_mut().unwrap().assertion_method = None;
        assert_eq!(
            resolve_midnight_did_method_binding(
                &missing_relationship,
                DID,
                "#issuer-key",
                VerificationRelationship::AssertionMethod,
            )
            .unwrap_err(),
            BindingError::RelationshipMissing,
        );

        let mut foreign_controller = resolution();
        let document = foreign_controller.did_document.as_mut().unwrap();
        let method = document.verification_method.as_ref().unwrap()[0].clone();
        document.verification_method = Some(vec![
            midnight_did_domain::VerificationMethod::new(midnight_did_domain::NewVerificationMethod {
                id: method.id().as_str().to_owned(),
                type_: method.type_(),
                controller: format!("did:midnight:testnet:{}", "22".repeat(32)),
                public_key_jwk: method.public_key_jwk().clone(),
            })
            .unwrap(),
        ]);
        assert_eq!(
            resolve_midnight_did_method_binding(
                &foreign_controller,
                DID,
                "#issuer-key",
                VerificationRelationship::AssertionMethod,
            )
            .unwrap_err(),
            BindingError::InvalidMethodProfile,
        );
    }
}
