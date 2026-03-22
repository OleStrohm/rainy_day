{
  description = "Rust flake with nightly";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { flake-utils, rust-overlay, nixpkgs, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = (import nixpkgs) {
          inherit system overlays;
        };
        tracey = pkgs.rustPlatform.buildRustPackage rec {
          name = "tracey";
          version = "1.3.0";

          src = pkgs.fetchFromGitHub {
            owner = "bearcove";
            repo = "tracey";
            tag = "v${version}";
            hash = "sha256-QEtOCy1+tvWHWn8yrAJM7unWq0AIP/+k8wEOH+B2V6M=";
          };
          cargoHash = "sha256-/QNpD59wqVqnXl2tjAfK3Z9cswmUd9/VwIC7Xyd6v+A=";

          nativeBuildInputs = with pkgs; [
            nodejs
            pnpmConfigHook
            pnpm
          ];

          pnpmDeps = pkgs.fetchPnpmDeps {
            inherit name src;
            pname = name;
            sourceRoot = "${src.name}/crates/tracey/src/bridge/http/dashboard";
            fetcherVersion = 3;
            hash = "sha256-PtaMB8FSS4vNZMcRiGCqzm5tug9CFvzx3O8GLlv/xyk=";
          };
          pnpmRoot = "crates/tracey/src/bridge/http/dashboard";

          doCheck = false;
        };
      in
      with pkgs;
      {
        devShells.default = mkShell {
          nativeBuildInputs = [
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-analyzer" "clippy" "rust-src" ];
            })
            tracey
          ];
        };
      }
    );
}
