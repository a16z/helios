{
  description = "Helios - A trustless, efficient, and portable multichain light client written in Rust";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };
  outputs = {
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {inherit system overlays;};
        rustToolchain = pkgs.rust-bin.stable.latest.default;
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          openssl
          perl
        ];

        # Create a package for Helios
        helios = pkgs.rustPlatform.buildRustPackage {
          inherit nativeBuildInputs;
          pname = "helios";
          version = "0.8.8"; # From workspace.package.version in Cargo.toml
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "ethereum_hashing-0.7.0" = "sha256-v0fY93t0tFZ/Tb02xKgTI0Z5gMNrXhmKwj3sLW7knpE=";
            };
          };
          meta = with pkgs.lib; {
            description = "A trustless, efficient, and portable multichain light client";
            homepage = "https://github.com/a16z/helios";
            license = licenses.mit;
          };
        };
      in {
        packages = {
          inherit helios;
          default = helios;
        };
        apps = {
          default = flake-utils.lib.mkApp {drv = helios;};
        };
        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs;
        };
      }
    );
}
