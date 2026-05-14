{
  description = "shh — macOS Keychain-backed env-var secrets CLI";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" ];
      forSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forSystems (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustc
              cargo
              rustfmt
              clippy
              rust-analyzer
              just
              pkg-config
              libiconv
              darwin.apple_sdk.frameworks.Security
              darwin.apple_sdk.frameworks.CoreFoundation
            ];

            shellHook = ''
              echo "shh dev shell — rustc $(rustc --version | awk '{print $2}'), just $(just --version | awk '{print $2}')"
            '';
          };
        });
    };
}
