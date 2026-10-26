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
        ];

      # The system GPU driver dir goes first so wgpu's Vulkan loader finds the
      # system ICDs and the driver's own libwayland.
      ldLibraryPath = pkgs: "/run/opengl-driver/lib:${lib.makeLibraryPath (runtimeLibsFor pkgs)}";

      # Without VK_ICD_FILENAMES, point the Vulkan loader at the system's
      # drivers. Shared by the devShell and the installed binary's wrapper.
      vulkanIcdFallback = ''
        if [ -z "''${VK_ICD_FILENAMES:-}" ] && [ -d /run/opengl-driver/share/vulkan/icd.d ]; then
          VK_ICD_FILENAMES="$(printf '%s:' /run/opengl-driver/share/vulkan/icd.d/*.json)"
          export VK_ICD_FILENAMES="''${VK_ICD_FILENAMES%:}"
        fi
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

          amane-shell = rustPlatform.buildRustPackage {
            pname = "amane-shell";
            inherit ((lib.importTOML ./Cargo.toml).package) version;

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
                ./niri-layer-rule.kdl
                ./gen-layer-rules.sh
              ];
            };

            # amane is a git-dependency (see Cargo.toml); Nix needs its content
            # hash to fetch it reproducibly. After a rev bump, update this — the
            # first build prints the expected `sha256-…` on mismatch.
            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                "amane-0.1.2" = "sha256-zrsrlE9LB9DAkCws9lLbaUbbsMELG21JiuBwanhI1wQ=";
              };
            };

            # the shell, and the apps beside it (Shell Settings)
            cargoBuildFlags = [ "--workspace" ];
            cargoTestFlags = [ "--workspace" ];

            nativeBuildInputs = [
              pkgs.pkg-config
              pkgs.makeWrapper
            ];
            buildInputs = runtimeLibs;

            # Reproduce the devShell's runtime wiring in the installed binary
            # (LD_LIBRARY_PATH + a Vulkan ICD fallback), with no hardcoded
            # /nix/store hashes beyond what makeLibraryPath computes.
            postInstall = ''
              install -Dm644 apps/settings/amane-settings.desktop -t $out/share/applications
            '';

            postFixup = ''
              for program in amane-shell amane-settings; do
                wrapProgram $out/bin/$program \
                  --prefix LD_LIBRARY_PATH : "${ldLibraryPath pkgs}" \
                  --run ${lib.escapeShellArg vulkanIcdFallback}
              done
            '';

            meta = {
              description = "A Wayland desktop bar for niri, built on the amane library fork";
              mainProgram = "amane-shell";
              license = lib.licenses.mit;
              platforms = lib.platforms.linux;
            };
          };
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
          default = pkgs.mkShell {
            packages = [ rustToolchain ];
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = runtimeLibs;

            # The runtime wiring, without hardcoded /nix/store hashes. The system
            # GPU driver dir goes first so wgpu's Vulkan loader
            # finds the system ICDs and the driver's own libwayland.
            shellHook = ''
              export LD_LIBRARY_PATH="${ldLibraryPath pkgs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
              ${vulkanIcdFallback}
            '';
          };
        }
      );

      formatter = forAllSystems (system: nixpkgsFor.${system}.nixfmt-rfc-style);
    };
}
