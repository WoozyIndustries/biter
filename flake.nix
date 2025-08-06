{
  description = "biter development environment";

  # I have chosen not to manage Rust toolchain/dependencies via Nix, because I am quite
  # fond of rustup, and the entire Rust toolchain. This flake will be for external deps.

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = { self, nixpkgs }: 
  let
    pkgs = nixpkgs.legacyPackages.x86_64-linux;
    macpkgs = nixpkgs.legacyPackages.aarch64-darwin;
  in
  {
    devShells.x86_64-linux.default = pkgs.mkShell {
      buildInputs = with pkgs; [ clang ];
    };

    devShells.aarch64-darwin.default = macpkgs.mkShell {
      buildInputs = with macpkgs; [ clang ];
    };
  };
}
