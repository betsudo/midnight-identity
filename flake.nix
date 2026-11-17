{
  description = "Midnight DID — native Rust implementation";

  nixConfig = {
    extra-substituters     = [ "https://cache.iog.io" ];
    extra-trusted-public-keys = [ "hydra.iohk.io:f/Ea+s+dFdN+3Y/G+FDgSq+a5NEWhJGzdjvKNGv0/EQ=" ];
  };

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-parts.url  = "github:hercules-ci/flake-parts";
    # Retained Ledger 8 source for the independently versioned Passport Vault
    # evidence leaf. The DID runtime no longer consumes this input.
    midnight-ledger = {
      type  = "github";
      owner = "MediaNoxLabs";
      repo  = "midnight-ledger";
      rev   = "8655615e7c4cbf3a1187b3203bf72e69dc09b8dc";
      flake = false;
    };
    # Retained Ledger 8 proof source for the same Passport Vault evidence leaf.
    midnight-zk = {
      type  = "github";
      owner = "MediaNoxLabs";
      repo  = "midnight-zk";
      rev   = "532629b044a88473a7175f4a96c2511c91156136";
      flake = false;
    };
    # Canonical patched Ledger 9.1.0.2 source used by the DID runtime and its
    # Compact runtime. VC/Passport retain the separate Ledger 8 input above.
    midnight-ledger-v9 = {
      type  = "github";
      owner = "betsudo";
      repo  = "midnight-ledger";
      rev   = "6fbc2e3900845a858f70eebf8a0deab1d4d3c0d6";
      flake = false;
    };
    compact = {
      type  = "github";
      owner = "MediaNoxLabs";
      repo  = "compact";
      # Authenticated compiler/toolchain revision used by generated VC and
      # Passport artifacts. Runtime upgrades must not rewrite that provenance.
      rev   = "55ba765f5484a47d9184824a619f93893448c740";
      inputs.nixpkgs.url = "github:NixOS/nixpkgs/bcc4a9d9533c033d806a46b37dc444f9b0da49dd";
    };
    compact-runtime = {
      type  = "github";
      owner = "MediaNoxLabs";
      repo  = "compact";
      # Retained Ledger 8 runtime identity for Passport Vault evidence.
      rev   = "32e314770da10cc62dab30dcfeb340ec4c6bcb64";
      flake = false;
    };
    compact-runtime-did = {
      type  = "github";
      owner = "betsudo";
      repo  = "compact";
      # Native Rust AST runtime from the signed Ledger 9 compatibility
      # checkpoint. Cargo.toml pins this same revision.
      rev   = "d6513a24debac14d43a8199460d9dc2c6d9af2b1";
      flake = false;
    };
    compact-did = {
      type  = "github";
      owner = "betsudo";
      repo  = "compact";
      # DID-only Rust AST compiler. Keep the existing `compact` input for
      # VC/Passport generation until those packages publish Ledger 9 artifacts.
      rev   = "d6513a24debac14d43a8199460d9dc2c6d9af2b1";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, flake-parts, ... }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-darwin"
      ];

      imports = [
        ./nix/devShells.nix
        ./nix/overlays.nix
        ./nix/compact.nix
      ];

      perSystem =
        { system, ... }:
        {
          _module.args = {
            inherit rust-overlay;
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ (import rust-overlay) ];
            };
            midnightDidRsLib = {
              rustTools = import ./nix/rustTools.nix {
                rust-bin     = (import nixpkgs { inherit system; overlays = [ (import rust-overlay) ]; }).rust-bin;
                rust-overlay = rust-overlay;
              };
              sources = {
                midnight-ledger = inputs.midnight-ledger;
                midnight-zk     = inputs.midnight-zk;
              };
            };
          };
        };
    };
}
