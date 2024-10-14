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
      cargo-config = builtins.fromTOML (builtins.readFile ./zeronsd/Cargo.toml);
      rust-version = "1.81.0";
    in {
    } // flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        devInputs = with pkgs; [
          rust-bin.stable.${rust-version}.complete
          pkg-config
          openssl
        ];

        zeronsd-bin = pkgs.rustPlatform.buildRustPackage rec {
          inherit (cargo-config.package) name version;

          src = ./.;
          buildAndTestSubdir = "zeronsd";

          nativeBuildInputs = devInputs;
          buildInputs = devInputs;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = devInputs;
          nativeBuildInputs = [ pkgs.just ];
        };

        packages = {
          zeronsd = zeronsd-bin;
          default = zeronsd-bin;

          container = pkgs.dockerTools.buildImage {
            inherit (cargo-config.package) name;
            tag = "latest";

            copyToRoot = pkgs.buildEnv {
              name = "image-root";
              paths = [ zeronsd-bin pkgs.dockerTools.caCertificates ];
              pathsToLink = [ "/bin" "/etc" ];
            };

            created = "now";

            runAsRoot = ''
              #{pkgs.runtimeShell}
              mkdir -p /var/lib/zeronsd
            '';

            config = {
              Cmd = [ "/bin/zeronsd" ];
              WorkingDir = "/var/lib/zeronsd";
            };
          };
        };

        overlays = {
          default = final: prev: { zeronsd = zeronsd-bin; };
        };
      });
}
