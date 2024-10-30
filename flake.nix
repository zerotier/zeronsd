{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, flake-utils, rust-overlay, ... }:
    let
      cargo-package = (builtins.fromTOML (builtins.readFile ./zeronsd/Cargo.toml)).package;
      rust-version = "1.81.0";
    in flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
          config.allowUnfree = true;
        };

        devInputs = with pkgs; [
          rust-bin.stable.${rust-version}.complete
          pkg-config
          openssl
          toml-cli
        ] ++ lib.optionals pkgs.stdenv.isDarwin [
          pkgs.libiconv
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
        ] ;

        zeronsd-bin = pkgs.rustPlatform.buildRustPackage rec {
          inherit (cargo-package) name version;

          src = ./.;
          buildAndTestSubdir = "zeronsd";

          nativeBuildInputs = devInputs;
          buildInputs = devInputs;

          cargoLock.lockFile = ./Cargo.lock;
        };
      in rec {
        devShells.default = pkgs.mkShell {
          buildInputs = devInputs;
          nativeBuildInputs = [ pkgs.just ];
        };

        packages = {
          zeronsd = zeronsd-bin;
          default = zeronsd-bin;
        };

        overlays = {
          default = final: prev: { zeronsd = zeronsd-bin; };
        };
      });
}
