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
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgsFor.${system};
          # Stable toolchain; edition 2024 requires rustc >= 1.85.
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-analyzer"
              "rust-src"
            ];
          };
          # Native libraries the bar links against at build time (via pkg-config)
          # and dlopens at run time.
          runtimeLibs = with pkgs; [
            wayland
            libxkbcommon
            libpulseaudio
            fontconfig
            freetype
            seatd
            vulkan-loader
          ];
        in
        {
          default = pkgs.mkShell {
            packages = [ rustToolchain ];
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = runtimeLibs;

            # Reproduces the old run.sh runtime wiring WITHOUT hardcoded /nix/store
            # hashes. The system GPU driver dir goes first so wgpu's Vulkan loader
            # finds the system ICDs and the driver's own libwayland.
            shellHook = ''
              export LD_LIBRARY_PATH="/run/opengl-driver/lib:${lib.makeLibraryPath runtimeLibs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
              if [ -z "''${VK_ICD_FILENAMES:-}" ] && [ -d /run/opengl-driver/share/vulkan/icd.d ]; then
                VK_ICD_FILENAMES="$(printf '%s:' /run/opengl-driver/share/vulkan/icd.d/*.json)"
                export VK_ICD_FILENAMES="''${VK_ICD_FILENAMES%:}"
              fi
            '';
          };
        }
      );

      formatter = forAllSystems (system: nixpkgsFor.${system}.nixfmt-rfc-style);
    };
}
