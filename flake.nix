{
  description = "biter development environment";

  # I have chosen not to manage Rust toolchain/dependencies via Nix, because I am quite
  # fond of rustup, and the entire Rust toolchain. This flake will be for external deps.
  
  # TODO: find a way to elimenate the duplicate package list declarations for different
  # platforms (but please find a way that isn't super 🚌ed 🆙 like what you saw online).

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
      buildInputs = with pkgs; [ clang pkg-config ];
    };

    devShells.aarch64-darwin.default = macpkgs.mkShell {
      buildInputs = with macpkgs; [ clang pkg-config ];
    };
  };
}
