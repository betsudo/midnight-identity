{ rust-bin, rust-overlay }:

let
  stableVersion = "1.99.0";
  rustOverrideArgs = {
    extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" "llvm-tools" ];
    targets    = [ "wasm32-unknown-unknown" "aarch64-apple-ios-sim" ];
  };
in
{
  rust =
    rust-bin.stable.${stableVersion}.default.override rustOverrideArgs;
}
