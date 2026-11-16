{ ... }:
{
  perSystem =
    {
      pkgs,
      midnightDidRsLib,
      midnightLedgerSrc,
      midnightZkSrc,
      compactRuntimeRsSrc,
      compactRuntimeRsMacrosSrc,
      compactDidRuntimeRsSrc,
      compactDidRuntimeRsMacrosSrc,
      compactPkg,
      compactDidPkg,
      ...
    }:
    let
      inherit (midnightDidRsLib.rustTools) rust;

      factoryPackages = with pkgs; [
        git
        gh
        nodejs_24
      ];

      rustPackages = with pkgs; [
        rust
        just
        taplo
        cargo-nextest
        cargo-llvm-cov
        jq
      ] ++ factoryPackages;

      factoryShellHook = ''
        export ROOT_DIR=$(${pkgs.git}/bin/git rev-parse --show-toplevel)
        cd "$ROOT_DIR"
      '';

      workspaceShellHook = ''
        export ROOT_DIR=$(${pkgs.git}/bin/git rev-parse --show-toplevel)
        cd "$ROOT_DIR"

        # Materialize third_party/midnight-ledger as a symlink to the nix-store path.
        TARGET="${midnightLedgerSrc}"
        LINK="$ROOT_DIR/third_party/midnight-ledger"
        mkdir -p "$ROOT_DIR/third_party"
        if [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$TARGET" ]; then
          :
        else
          rm -rf "$LINK"
          ln -s "$TARGET" "$LINK"
          echo "Linked $LINK -> $TARGET"
        fi

        # Retain the Ledger 8 proof source for the Passport Vault evidence
        # generator. The DID Cargo graph uses released Ledger 9 crates.
        TARGET="${midnightZkSrc}"
        LINK="$ROOT_DIR/third_party/midnight-zk"
        if [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$TARGET" ]; then
          :
        else
          rm -rf "$LINK"
          ln -s "$TARGET" "$LINK"
          echo "Linked $LINK -> $TARGET"
        fi

        # Materialise compact's runtime-rs + runtime-rs-macros subtrees inside
        # third_party/compact/, mirroring the in-repo layout so that the
        # relative path `../runtime-rs-macros` (in midnight-compact-runtime's Cargo.toml)
        # and `../runtime-rs` (in runtime-rs-macros' dev-deps) both resolve
        # correctly without aliasing.
        mkdir -p "$ROOT_DIR/third_party/compact"

        TARGET="${compactRuntimeRsSrc}"
        LINK="$ROOT_DIR/third_party/compact/runtime-rs"
        if [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$TARGET" ]; then
          :
        else
          rm -rf "$LINK"
          ln -s "$TARGET" "$LINK"
          echo "Linked $LINK -> $TARGET"
        fi

        TARGET="${compactRuntimeRsMacrosSrc}"
        LINK="$ROOT_DIR/third_party/compact/runtime-rs-macros"
        if [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$TARGET" ]; then
          :
        else
          rm -rf "$LINK"
          ln -s "$TARGET" "$LINK"
          echo "Linked $LINK -> $TARGET"
        fi

        # Materialise the DID-only Ledger 9 runtime separately. This prevents
        # the optional path override from changing VC/Passport's Ledger 8
        # runtime identity inside the same development shell.
        mkdir -p "$ROOT_DIR/third_party/compact-did"

        TARGET="${compactDidRuntimeRsSrc}"
        LINK="$ROOT_DIR/third_party/compact-did/runtime-rs"
        if [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$TARGET" ]; then
          :
        else
          rm -rf "$LINK"
          ln -s "$TARGET" "$LINK"
          echo "Linked $LINK -> $TARGET"
        fi

        TARGET="${compactDidRuntimeRsMacrosSrc}"
        LINK="$ROOT_DIR/third_party/compact-did/runtime-rs-macros"
        if [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$TARGET" ]; then
          :
        else
          rm -rf "$LINK"
          ln -s "$TARGET" "$LINK"
          echo "Linked $LINK -> $TARGET"
        fi

        echo "Entered midnight-identity devshell. Run 'just --list' for available commands."
      '';

      mkWorkspaceShell = packages: pkgs.mkShell {
        inherit packages;
        shellHook = workspaceShellHook;
        env.RUST_LOG = "info";
      };
    in
    {
      devShells = {
        # Fast policy shell used by bootstrap and repository-local Git hooks.
        factory = pkgs.mkShell {
          packages = factoryPackages;
          shellHook = factoryShellHook;
        };

        # Lightweight Pi shell for factory supervision. Keep Compact/Rust
        # materialization on the explicit default/rust shells so `--pi` can
        # start without constructing unrelated toolchains.
        pi = pkgs.mkShell {
          packages = [ pkgs.pi-coding-agent ] ++ factoryPackages;
          shellHook = factoryShellHook;
        };

        # Interactive factory shell: keep compactc and pi.dev available.
        default = mkWorkspaceShell (
          [
            compactPkg
            compactDidPkg
            pkgs.pi-coding-agent
          ]
          ++ rustPackages
        );

        # Ordinary Rust gates do not need to construct compactc and its
        # proving/Node dependency closure.
        rust = mkWorkspaceShell rustPackages;
      };
    };
}
