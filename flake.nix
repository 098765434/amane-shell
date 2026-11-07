{
  description = "amane-shell — a Wayland desktop bar for niri, built on the amane library fork.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      inherit (nixpkgs) lib;
      # All Linux systems the nixpkgs flake exposes.
      systems = lib.intersectLists lib.systems.flakeExposed lib.platforms.linux;
      forAllSystems = lib.genAttrs systems;
      nixpkgsFor = forAllSystems (
        system:
        import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        }
      );

      # Stable toolchain; edition 2024 requires rustc >= 1.85.
      toolchainFor =
        pkgs:
        pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-analyzer"
            "rust-src"
          ];
        };

      # Native libraries the bar links against at build time (via pkg-config)
      # and dlopens at run time. Shared by the devShell and the package.
      runtimeLibsFor =
        pkgs: with pkgs; [
          wayland
          libxkbcommon
          libpulseaudio
          fontconfig
          freetype
          seatd
          vulkan-loader
          pam # amane links -lpam (session/seat); host-provided in the devShell
          ffmpeg # amane-mirror decodes the phone's screen and sound (VAAPI on the GPU)
        ];

      # The programs amane-drop runs: curl for uploads, secret-tool (libsecret)
      # for the keyring, wl-copy for links.
      dropToolsFor =
        pkgs: with pkgs; [
          curl
          libsecret
          wl-clipboard
        ];

      # scrcpy's server, alone: amane-mirror pushes it to the phone. Copied out
      # of the scrcpy package so the closure doesn't keep scrcpy's own client
      # and its libraries; its version is the protocol amane-mirror speaks.
      scrcpyServerFor =
        pkgs:
        pkgs.runCommand "scrcpy-server-${pkgs.scrcpy.version}" { } ''
          install -Dm644 ${pkgs.scrcpy}/share/scrcpy/scrcpy-server $out/share/scrcpy/scrcpy-server
        '';

      scrcpyEnvFor = pkgs: {
        SCRCPY_SERVER = "${scrcpyServerFor pkgs}/share/scrcpy/scrcpy-server";
        SCRCPY_VERSION = pkgs.scrcpy.version;
      };

      # Cactus 4.0.0 for amane-assistant (the launcher's assistant): the prebuilt
      # static library (Apache-2.0) and its header from Cactus's release, and
      # the Needle 3 (commands) and Whistle (speech) weights, every file pinned
      # to a Hugging Face commit and its hash; nothing is fetched at run time.
      # Only x86_64-linux here; elsewhere the daemon builds without an engine.
      # The library is linked statically, with the LLVM libc++ it was built
      # against, so the daemon needs nothing more. See docs/shell/assistant.md.
      cactusFor =
        pkgs:
        let
          hf = repo: rev: file: "https://huggingface.co/Cactus-Compute/${repo}/resolve/${rev}/${file}";
          release = "a30ea9d3d8619ec5728c66fb7cde6c48ba681b55";
          fetch = repo: rev: file: hash: pkgs.fetchurl { url = hf repo rev file; inherit hash; };
        in
        pkgs.runCommand "cactus-4.0.0" { } ''
          install -Dm644 ${fetch "cactus" release "releases/4.0.0/linux-x86_64/libcactus.a" "sha256-nl56vhsL9mdbaaAKXF6HXoD95JbU55966G+urlyrj9Y="} $out/lib/libcactus.a
          install -Dm644 ${fetch "cactus" release "releases/4.0.0/linux-x86_64/cactus.h" "sha256-+s78oXoNXnFb2SqLDqJywaw0ogp5gAOx5pHI+6Ws5uI="} $out/include/cactus.h
          install -Dm644 ${fetch "cactus" release "releases/4.0.0/LICENSE" "sha256-z8d0m5b2O9McPEK1xHG/dWgUBT6EfBDz6wA0F7xSPTA="} $out/share/licenses/cactus/LICENSE
          install -Dm644 ${fetch "needle3" "c95d25b4efa1e9601e785d92438e4ade0dbddbf8" "needle3.cact" "sha256-ydkV7KKC7ULRoJsUO1kq20zGdE/+LSlK31z8VUgXDDg="} $out/share/amane-assistant/models/needle3.cact
          install -Dm644 ${fetch "whistle" "ad73f1468dd5203791e0cc81d31544c553563645" "whistle.cact" "sha256-tuAvBIVorF0BogQlVsZYBh5pmsvAqioUOfUvPUYd/+s="} $out/share/amane-assistant/models/whistle.cact
        '';

      hasCactus = pkgs: pkgs.stdenv.hostPlatform.system == "x86_64-linux";

      # What the build reads to link Cactus into amane-assistant, and where its
      # models are; empty where there's no Cactus.
      cactusEnvFor =
        pkgs:
        lib.optionalAttrs (hasCactus pkgs) {
          CACTUS_LIB_DIR = "${cactusFor pkgs}/lib";
          LIBCXX_DIR = "${pkgs.llvmPackages.libcxx}/lib";
          AMANE_ASSISTANT_MODELS = "${cactusFor pkgs}/share/amane-assistant/models";
        };

      # The system GPU driver dir goes first so wgpu's Vulkan loader finds the
      # system ICDs and the driver's own libwayland.
      # Passwords: the rbw the shell runs and the pinentry amane-pinentry falls back to, by
      # store path, so no PATH can put another program in their place.
      vaultEnvFor = pkgs: {
        AMANE_RBW = "${pkgs.rbw}/bin/rbw";
        AMANE_PINENTRY_FALLBACK = "${pkgs.pinentry-gnome3}/bin/pinentry-gnome3";
      };

      ldLibraryPath = pkgs: "/run/opengl-driver/lib:${lib.makeLibraryPath (runtimeLibsFor pkgs)}";

      # Without VK_ICD_FILENAMES, point the Vulkan loader at the system's
      # drivers. Shared by the devShell and the installed binary's wrapper.
      # Loading every driver in the dir (13 on NixOS, lavapipe among them)
      # costs about 6.5 MB of RSS per amane process, so on a machine with a
      # Radeon only its driver is named; others get the whole dir. The backend
      # is pinned to Vulkan so wgpu doesn't probe GL as well.
      # See docs/development/memory.md.
      vulkanIcdFallback = ''
        if [ -z "''${VK_ICD_FILENAMES:-}" ] && [ -d /run/opengl-driver/share/vulkan/icd.d ]; then
          if [ -e /run/opengl-driver/share/vulkan/icd.d/radeon_icd.x86_64.json ]; then
            export VK_ICD_FILENAMES=/run/opengl-driver/share/vulkan/icd.d/radeon_icd.x86_64.json
          else
            VK_ICD_FILENAMES="$(printf '%s:' /run/opengl-driver/share/vulkan/icd.d/*.json)"
            export VK_ICD_FILENAMES="''${VK_ICD_FILENAMES%:}"
          fi
        fi
        export WGPU_BACKEND="''${WGPU_BACKEND:-vulkan}"
      '';
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgsFor.${system};
          rustToolchain = toolchainFor pkgs;
          runtimeLibs = runtimeLibsFor pkgs;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          amane-shell = rustPlatform.buildRustPackage ({
            pname = "amane-shell";
            inherit ((lib.importTOML ./Cargo.toml).workspace.package) version;

            # only what the build and its tests read, so a docs change
            # doesn't rebuild
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./rustfmt.toml
                ./src
                ./kit
                ./apps
                ./design-tokens
                ./tests
                # the IPC test checks the verbs against it
                ./docs/integration/ipc.md
                ./niri-layer-rule.kdl
                ./gen-layer-rules.sh
              ];
            };

            # amane and amane's vello fork (vello_encoding, see Cargo.toml's
            # [patch.crates-io]) are git-dependencies; Nix needs their content
            # hashes to fetch them reproducibly. After a rev bump, update the
            # entry — the first build prints the expected `sha256-…` on mismatch.
            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                "amane-0.1.2" = "sha256-4LPnyx3Fn91Z+CYz8M5Iwl+ep05F+obtLdqjhUxvKGo=";
                # one entry covers every crate from the same git rev
                "vello-0.11.0" = "sha256-4bzBYT9BUIb0oMjB1rKJpxXhdT20lVQ4HTCTWsar/cc=";
              };
            };

            # the shell, and the apps beside it (Shell Settings, Messages, Phone Screen,
            # Shader Lab, Drop)
            cargoBuildFlags = [ "--workspace" ];
            cargoTestFlags = [ "--workspace" ];

            nativeBuildInputs = [
              pkgs.pkg-config
              pkgs.makeWrapper
              # ffmpeg's bindings are generated against its headers
              rustPlatform.bindgenHook
            ];
            buildInputs = runtimeLibs;

            # Reproduce the devShell's runtime wiring in the installed binary
            # (LD_LIBRARY_PATH + a Vulkan ICD fallback), with no hardcoded
            # /nix/store hashes beyond what makeLibraryPath computes.
            postInstall = ''
              install -Dm644 apps/settings/amane-settings.desktop -t $out/share/applications
              install -Dm644 apps/messages/amane-messages.desktop -t $out/share/applications
              install -Dm644 apps/settings/amane-settings.png -t $out/share/icons/hicolor/512x512/apps
              install -Dm644 apps/messages/amane-messages.png -t $out/share/icons/hicolor/512x512/apps
              install -Dm644 apps/mirror/amane-mirror.desktop -t $out/share/applications
              install -Dm644 apps/shader-lab/amane-shader-lab.desktop -t $out/share/applications
              install -Dm644 apps/drop/amane-drop.desktop -t $out/share/applications
            '';

            postFixup = ''
              for program in amane-shell amane-settings amane-messages amane-mirror amane-shader-lab amane-drop; do
                wrapProgram $out/bin/$program \
                  --prefix LD_LIBRARY_PATH : "${ldLibraryPath pkgs}" \
                  --run ${lib.escapeShellArg vulkanIcdFallback}
              done
              # Drop uploads with curl, keeps passwords with secret-tool and
              # copies links with wl-copy; Shell Settings keeps its passwords
              # too. After PATH, so the system's own copies win.
              for program in amane-drop amane-settings; do
                wrapProgram $out/bin/$program \
                  --suffix PATH : "${lib.makeBinPath (dropToolsFor pkgs)}"
              done
              # the assistant's daemon: no GPU, its models from the store, and
              # Cactus told not to report anything (it has no network code; this
              # is belt and braces, with the unit's RestrictAddressFamilies)
              wrapProgram $out/bin/amane-assistant \
                --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath [ pkgs.libpulseaudio ]}" \
                ${lib.optionalString (hasCactus pkgs) "--set-default AMANE_ASSISTANT_MODELS ${cactusFor pkgs}/share/amane-assistant/models"} \
                --set CACTUS_TELEMETRY 0 \
                --set DO_NOT_TRACK 1
            '';

            meta = {
              description = "A Wayland desktop bar for niri, built on the amane library fork";
              mainProgram = "amane-shell";
              license = lib.licenses.mit;
              platforms = lib.platforms.linux;
            };
          } // scrcpyEnvFor pkgs // cactusEnvFor pkgs // vaultEnvFor pkgs);
        in
        {
          default = amane-shell;
          amane-shell = amane-shell;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgsFor.${system};
          rustToolchain = toolchainFor pkgs;
          runtimeLibs = runtimeLibsFor pkgs;
        in
        {
          default = pkgs.mkShell ({
            packages = [ rustToolchain ];
            nativeBuildInputs = [
              pkgs.pkg-config
              pkgs.rustPlatform.bindgenHook
            ];
            buildInputs = runtimeLibs;

            # The runtime wiring, without hardcoded /nix/store hashes. The system
            # GPU driver dir goes first so wgpu's Vulkan loader
            # finds the system ICDs and the driver's own libwayland.
            shellHook = ''
              export LD_LIBRARY_PATH="${ldLibraryPath pkgs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
              ${vulkanIcdFallback}
            '';
          } // scrcpyEnvFor pkgs // cactusEnvFor pkgs // vaultEnvFor pkgs);
        }
      );

      formatter = forAllSystems (system: nixpkgsFor.${system}.nixfmt-rfc-style);
    };
}
