#!/usr/bin/env bash
# Wrapper that sets the Nix pkg-config / shared-library paths needed to
# build and run amane-shell outside a nix-shell.
# Usage:
#   ./run.sh            # cargo run  (default: dark)
#   ./run.sh build      # cargo build
#   ./run.sh light      # AMANE_THEME=light cargo run
#   ./run.sh dark       # AMANE_THEME=dark  cargo run

set -euo pipefail

cd "$(dirname "$0")"

CARGO_BIN=/nix/store/4skfxmk429bn3cb81q1nv0jm36idnijv-cargo-1.95.0/bin
RUSTC_BIN=/nix/store/agq2dfmwfmxh913syj2plgfracn4j1fv-rustc-bootstrap-1.95.0/bin
PKGCFG_BIN=/nix/store/viqf3122g0bn0ahqgns1yg52axx9p531-pkg-config-wrapper-0.29.2/bin

export PATH="$CARGO_BIN:$RUSTC_BIN:$PKGCFG_BIN:$PATH"

# -dev outputs: headers + .pc files, for the build.
export PKG_CONFIG_PATH="\
/nix/store/qvvd80p8wab2bqg33xcd0p62al4c7ddl-fontconfig-2.17.1-dev/lib/pkgconfig:\
/nix/store/j60c51vgjq9hm15igqbnpl435a5pd43s-freetype-2.14.3-dev/lib/pkgconfig:\
/nix/store/dmww2702yyd5zbdmrnl21cfirvdksvmg-libxkbcommon-1.13.1-dev/lib/pkgconfig:\
/nix/store/p9nhq3wp3gc0arxapzqhq1ivraglxkx9-libpulseaudio-17.0-dev/lib/pkgconfig:\
/nix/store/1sh5ilplhxvm1bba2y2m697nv5fcch3s-wayland-1.25.0-dev/lib/pkgconfig:\
/nix/store/fh4a0fr1d5dkb68ncax1g3m1cmpkqhhk-seatd-0.9.3-dev/lib/pkgconfig"

# non-dev outputs: the actual .so files needed at run time, plus the system
# OpenGL/Vulkan driver dir so wgpu can dlopen libvulkan + the AMD/mesa ICD.
# System driver dir goes FIRST so the GPU driver loads its own libwayland.
export LD_LIBRARY_PATH="\
/run/opengl-driver/lib:\
/nix/store/9lkc4la5w66ajdqi1jfvdb12ifl26629-vulkan-loader-1.4.341.0/lib:\
/nix/store/ni6q8ya15pmg5ai8igcnlwq3frifb20m-libxkbcommon-1.13.1/lib:\
/nix/store/np8lckjq0w95qp9bl99kxd8awpc336jz-wayland-1.25.0/lib:\
/nix/store/qic504b4hvmfhq6v7hhvg4k0kkxnjkxl-fontconfig-2.17.1-lib/lib:\
/nix/store/q4bzwshhgc42rjxp74z2dwhz0cwi5ji8-freetype-2.14.3/lib:\
/nix/store/xz2pyk53pi8dn3vrc14c84cwhjjmnf4g-libpulseaudio-17.0/lib:\
/nix/store/a772bchd8qlp1rq7mxgd99j1fvf91gvd-seatd-0.9.3/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# Point the Vulkan loader at the system ICDs (radeon, intel, lvp, ...).
export VK_ICD_FILENAMES="${VK_ICD_FILENAMES:-$(printf '%s:' /run/opengl-driver/share/vulkan/icd.d/*.json)}"
export VK_ICD_FILENAMES="${VK_ICD_FILENAMES%:}"

case "${1:-run}" in
    build)  cargo build ;;
    light)  AMANE_THEME=light cargo run ;;
    dark)   AMANE_THEME=dark  cargo run ;;
    run|*)  cargo run ;;
esac
