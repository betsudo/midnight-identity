{ inputs, ... }:
{
  perSystem =
    { pkgs, system, ... }:
    {
      # Code generation only needs the compiler.  The regular
      # `compactc` derivation pulls the entire JavaScript runtime (and its
      # on-chain Rust vendor tree) into the shell even though `--skip-ts` never
      # uses it.  Besides being needlessly large, that closure makes drift
      # checks depend on hundreds of live crates.io downloads.  The upstream
      # no-runtime package is the same compiler derivation with that unused
      # closure removed, keeping the codegen shell small and deterministic.
      _module.args = {
        compactPkg = inputs.compact.packages.${system}.compactc-no-runtime;
        compactDidPkg = pkgs.writeShellScriptBin "compactc-did" ''
          export COMPACT_RUST_RUNTIME_DIR=${inputs.compact-runtime-did}
          exec ${inputs.compact-did.packages.${system}.compactc-no-runtime}/bin/compactc "$@"
        '';
      };
    };
}
