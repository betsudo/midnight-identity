#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { createPrivateKey, createPublicKey, sign, verify } from "node:crypto";
import { readFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const typescriptRoot = path.join(repositoryRoot, "third_party/midnight-did");
const fixturePath = path.join(
  repositoryRoot,
  "crates/midnight-did-api/tests/fixtures/ledger9-cross-language-conformance.json",
);
const fixture = JSON.parse(await readFile(fixturePath, "utf8"));

const [{ LedgerToDomain }, midnight, contract, jubjub] = await Promise.all([
  import(pathToFileURL(path.join(typescriptRoot, "packages/did/dist/ledger-to-domain.js"))),
  import(pathToFileURL(path.join(typescriptRoot, "packages/did/dist/midnight.js"))),
  import(pathToFileURL(path.join(typescriptRoot, "packages/contract/dist/index.js"))),
  import(pathToFileURL(path.join(typescriptRoot, "packages/jubjub-schnorr/dist/index.js"))),
]);

class CompactMap extends Map {
  isEmpty() {
    return this.size === 0;
  }
}

class CompactSet extends Set {
  isEmpty() {
    return this.size === 0;
  }
}

const littleEndianHexToBigInt = (hex) => {
  const bytes = Buffer.from(hex, "hex");
  return BigInt(`0x${Buffer.from(bytes).reverse().toString("hex")}`);
};

const snapshot = fixture.ledgerSnapshot;
const verificationMethods = new CompactMap(
  Object.entries(snapshot.verification_methods).map(([id, method]) => [
    id,
    {
      id: method.id,
      typ: contract.DIDContract.VerificationMethodType[method.typ],
      publicKeyJwk: {
        kty: contract.DIDContract.KeyType[method.public_key_jwk.kty],
        crv: contract.DIDContract.CurveType[
          method.public_key_jwk.crv === "P-256" ? "P256" : method.public_key_jwk.crv
        ],
        x: method.public_key_jwk.x,
        y: method.public_key_jwk.y,
      },
    },
  ]),
);
const schnorrJubjubVerificationMethods = new CompactMap(
  Object.entries(snapshot.schnorr_jubjub_verification_methods).map(([id, method]) => [
    id,
    {
      id: method.id,
      publicKey: {
        x: littleEndianHexToBigInt(method.public_key.x),
        y: littleEndianHexToBigInt(method.public_key.y),
      },
    },
  ]),
);
const services = new CompactMap(
  Object.entries(snapshot.services).map(([id, service]) => [
    id,
    {
      id: service.id,
      typ: service.typ,
      serviceEndpoint: service.service_endpoint,
    },
  ]),
);
const ledger = {
  id: { bytes: Uint8Array.from(Buffer.from(snapshot.id_hex.padEnd(64, "0"), "hex")) },
  active: snapshot.active,
  deactivated: snapshot.deactivated,
  controllerPublicKey: { x: 1n, y: 1n },
  version: BigInt(snapshot.version),
  operationCount: BigInt(snapshot.operation_count),
  contractVersion: BigInt(snapshot.contract_version),
  created: BigInt(snapshot.created_ms),
  updated: BigInt(snapshot.updated_ms),
  alsoKnownAs: new CompactSet(snapshot.also_known_as),
  verificationMethods,
  schnorrJubjubVerificationMethods,
  authenticationRelation: new CompactSet(snapshot.authentication_relation),
  assertionMethodRelation: new CompactSet(snapshot.assertion_method_relation),
  keyAgreementRelation: new CompactSet(snapshot.key_agreement_relation),
  capabilityInvocationRelation: new CompactSet(snapshot.capability_invocation_relation),
  capabilityDelegationRelation: new CompactSet(snapshot.capability_delegation_relation),
  services,
};

const document = LedgerToDomain.ledgerStateToDIDDocument(
  ledger,
  midnight.MidnightNetwork.Undeployed,
  midnight.parseContractAddress(fixture.contractAddress),
);
const metadata = LedgerToDomain.ledgerStateToMetadata(ledger);
assert.deepEqual(
  JSON.parse(JSON.stringify({ didDocument: document, didDocumentMetadata: metadata })),
  fixture.expected,
  "TypeScript resolver output diverged from the shared Rust/TS fixture",
);

const ed = fixture.ed25519;
const edPrivate = createPrivateKey({
  key: {
    kty: "OKP",
    crv: "Ed25519",
    d: Buffer.from(ed.seedHex, "hex").toString("base64url"),
    x: Buffer.from(ed.publicKeyHex, "hex").toString("base64url"),
  },
  format: "jwk",
});
const edPublic = createPublicKey(edPrivate);
const edMessage = Buffer.from(ed.messageHex, "hex");
const edSignature = sign(null, edMessage, edPrivate);
assert.equal(edSignature.toString("hex"), ed.signatureHex);
assert.equal(verify(null, edMessage, edPublic, Buffer.from(ed.signatureHex, "hex")), true);

const jj = fixture.jubjub;
const jjSeed = Uint8Array.from(Buffer.from(jj.seedHex, "hex"));
const jjPayload = new TextEncoder().encode(jj.payloadUtf8);
const jjSignature = jubjub.signJubjubPayloadFromSeed(jjSeed, jjPayload);
assert.equal(Buffer.from(jubjub.encodeJubjubSignature(jjSignature)).toString("hex"), jj.signatureHex);
const jjPublic = jubjub.deriveJubjubPublicKeyFromSeed(jjSeed);
assert.equal(jubjub.verifyJubjubPayload(jjPublic, jjPayload, jjSignature), true);
assert.equal(jubjub.verifyJubjubPayload(jjPublic, new TextEncoder().encode("tampered"), jjSignature), false);

process.stdout.write(
  `cross-language DID conformance passed (TS ${fixture.sourceRevisions.typescript}, Rust baseline ${fixture.sourceRevisions.rust})\n`,
);
