{
  description = "Wyoming wake word server for LiveKit WakeWord models";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    nix2container.url = "github:nlewo/nix2container";
    nix2container.inputs.nixpkgs.follows = "nixpkgs";
    hey-jarvis = {
      url = "https://github.com/livekit/livekit-wakeword/releases/download/v2.1.1/hey_jarvis.onnx";
      flake = false;
    };
  };

  outputs = {
    self,
    nixpkgs,
    utils,
    crane,
    nix2container,
    hey-jarvis,
    ...
  }:
    utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;

        common = {
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              craneLib.filterCargoSources path type
              || pkgs.lib.hasSuffix "/VERSION" path;
          };
          strictDeps = true;
        };

        wyoming-livekit-wakeword = craneLib.buildPackage (common
          // {
            cargoArtifacts = craneLib.buildDepsOnly common;
            meta.mainProgram = "wyoming-livekit-wakeword";
          });

        # Symlinks to every input's source. Nothing the image needs references
        # them, so CI's store GC would delete nixpkgs and re-download it next run.
        # CI builds this with an out-link, which makes it a GC root.
        flake-inputs = pkgs.linkFarm "flake-inputs" self.inputs;

        models = pkgs.linkFarm "livekit-wakeword-models" {
          "hey_jarvis.onnx" = hey-jarvis;
        };

        # Use `nix run .#image.copyToDockerDaemon` to load it locally.
        image = nix2container.packages.${system}.nix2container.buildImage {
          name = "wyoming-livekit-wakeword";
          tag = "latest";
          maxLayers = 20;
          config = {
            entrypoint = ["${wyoming-livekit-wakeword}/bin/wyoming-livekit-wakeword"];
            exposedports."10400/tcp" = {};
            env = ["MODEL_DIR=${models}"];
          };
        };
      in {
        packages = {
          default = image;
          inherit image models wyoming-livekit-wakeword flake-inputs;
        };

        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.clippy
            pkgs.rustfmt
            pkgs.rust-analyzer
          ];

          RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          MODEL_DIR = models;
          RUST_LOG = "info,wyoming_livekit_wakeword=debug";
        };
      }
    );
}
