// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! DID URL reference resolution compatible with the TypeScript domain SDK.

use thiserror::Error;

/// Options controlling which absolute references may escape the current DID
/// subject.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DidUrlResolutionOptions {
    /// Allow an absolute DID URL belonging to another DID subject.
    pub allow_external_did: bool,
    /// Allow an absolute URL that does not use the `did` scheme.
    pub allow_external_url: bool,
    /// Accept method-specific case variations in the DID subject.
    pub case_insensitive_did_subject: bool,
}

/// Failure returned while resolving a DID URL reference.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DidUrlResolutionError {
    /// The reference is empty after trimming.
    #[error("DID URL reference must not be empty")]
    Empty,
    /// RFC 3986 network-path references are not valid DID URL references.
    #[error("DID URL reference must not be a network-path reference")]
    NetworkPath,
    /// An absolute DID URL names a different subject.
    #[error("DID URL subject must match the current DID ({did})")]
    SubjectMismatch {
        /// Expected bare DID subject.
        did: String,
    },
    /// A non-DID absolute URL was supplied where subject binding is required.
    #[error("DID URL reference must be bound to the current DID")]
    ExternalUrl,
    /// A permitted external URL was not syntactically absolute.
    #[error("DID URL reference must be an absolute URL")]
    InvalidAbsoluteUrl,
}

pub(crate) fn has_uri_scheme_for_ledger_utils(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    chars
        .take_while(|character| *character != ':')
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '+' | '.' | '-'))
        && value.find(':').is_some()
}

fn split_did_subject(value: &str) -> &str {
    value.find(['/', '?', '#']).map_or(value, |boundary| &value[..boundary])
}

fn split_path_and_suffix(reference: &str) -> (&str, &str) {
    reference.find(['?', '#']).map_or((reference, ""), |boundary| {
        (&reference[..boundary], &reference[boundary..])
    })
}

fn remove_last_segment(output: &mut String) {
    if let Some(boundary) = output.rfind('/') {
        output.truncate(boundary);
    } else {
        output.clear();
    }
}

fn remove_dot_segments(path: &str) -> String {
    let mut input = path.to_owned();
    let mut output = String::new();
    while !input.is_empty() {
        if input.starts_with("../") {
            input.drain(..3);
        } else if input.starts_with("./") {
            input.drain(..2);
        } else if input.starts_with("/./") {
            input.replace_range(..3, "/");
        } else if input == "/." {
            input = "/".into();
        } else if input.starts_with("/../") {
            input.replace_range(..4, "/");
            remove_last_segment(&mut output);
        } else if input == "/.." {
            input = "/".into();
            remove_last_segment(&mut output);
        } else if matches!(input.as_str(), "." | "..") {
            input.clear();
        } else {
            let segment_end = if let Some(stripped) = input.strip_prefix('/') {
                stripped.find('/').map_or(input.len(), |index| index + 1)
            } else {
                input.find('/').unwrap_or(input.len())
            };
            output.push_str(&input[..segment_end]);
            input.drain(..segment_end);
        }
    }
    if output.is_empty() { "/".into() } else { output }
}

fn resolve_relative_did_url(did: &str, reference: &str) -> String {
    if reference.starts_with('#') || reference.starts_with('?') {
        return format!("{did}{reference}");
    }
    let (path, suffix) = split_path_and_suffix(reference);
    let rooted = if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    };
    format!("{did}{}{suffix}", remove_dot_segments(&rooted))
}

/// Resolve a DID URL or relative DID URL against a bare DID subject.
///
/// This mirrors `resolveDIDURLReference` from the pinned TypeScript domain
/// package, including path/query/fragment handling and foreign-reference
/// policy.
pub fn resolve_did_url_reference(
    reference: &str,
    did: &str,
    options: DidUrlResolutionOptions,
) -> Result<String, DidUrlResolutionError> {
    let value = reference.trim();
    if value.is_empty() {
        return Err(DidUrlResolutionError::Empty);
    }
    if value.starts_with("//") {
        return Err(DidUrlResolutionError::NetworkPath);
    }

    if has_uri_scheme_for_ledger_utils(value) {
        if value.get(..4).is_some_and(|prefix| prefix.eq_ignore_ascii_case("did:")) {
            let subject = split_did_subject(value);
            let offchain = subject.to_ascii_lowercase().starts_with("did:midnight:offchain:");
            let subjects_match = if offchain || !options.case_insensitive_did_subject {
                subject == did
            } else {
                subject.eq_ignore_ascii_case(did)
            };
            if !options.allow_external_did && !subjects_match {
                return Err(DidUrlResolutionError::SubjectMismatch { did: did.to_owned() });
            }
            let canonical_subject = if subjects_match { did } else { subject };
            let suffix = &value[subject.len()..];
            return Ok(if suffix.is_empty() {
                canonical_subject.to_owned()
            } else {
                resolve_relative_did_url(canonical_subject, suffix)
            });
        }

        if !options.allow_external_url {
            return Err(DidUrlResolutionError::ExternalUrl);
        }
        return url::Url::parse(value)
            .map(|url| url.to_string())
            .map_err(|_| DidUrlResolutionError::InvalidAbsoluteUrl);
    }

    Ok(resolve_relative_did_url(did, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DID: &str = "did:midnight:devnet:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn resolves_every_relative_reference_form_like_typescript() {
        assert_eq!(
            resolve_did_url_reference("#key-1", DID, Default::default()).unwrap(),
            format!("{DID}#key-1")
        );
        assert_eq!(
            resolve_did_url_reference("?v=1", DID, Default::default()).unwrap(),
            format!("{DID}?v=1")
        );
        assert_eq!(
            resolve_did_url_reference("/keys/a#key-1", DID, Default::default()).unwrap(),
            format!("{DID}/keys/a#key-1")
        );
        assert_eq!(
            resolve_did_url_reference("./keys/../keys/a", DID, Default::default()).unwrap(),
            format!("{DID}/keys/a")
        );
    }

    #[test]
    fn enforces_subject_and_external_url_policy() {
        assert!(matches!(
            resolve_did_url_reference("did:example:other#key", DID, Default::default()),
            Err(DidUrlResolutionError::SubjectMismatch { .. })
        ));
        assert_eq!(
            resolve_did_url_reference(
                "did:example:other#key",
                DID,
                DidUrlResolutionOptions {
                    allow_external_did: true,
                    ..Default::default()
                }
            )
            .unwrap(),
            "did:example:other#key"
        );
        assert_eq!(
            resolve_did_url_reference(
                "https://example.com",
                DID,
                DidUrlResolutionOptions {
                    allow_external_url: true,
                    ..Default::default()
                }
            )
            .unwrap(),
            "https://example.com/"
        );
        assert_eq!(
            resolve_did_url_reference(
                "mailto:holder@example.com",
                DID,
                DidUrlResolutionOptions {
                    allow_external_url: true,
                    ..Default::default()
                }
            )
            .unwrap(),
            "mailto:holder@example.com"
        );
    }
}
