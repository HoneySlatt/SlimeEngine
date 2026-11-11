#!/usr/bin/env bash
# Runs the canvas prototype on one configuration, on Mesa's drivers that draw on the CPU and never
# on a GPU: lavapipe for Vulkan, llvmpipe for GL. Run it inside `nix develop`, from the
# repository's root, after `cargo build --release -p canvas-prototype`:
#
#   crates/canvas-prototype/run-config.sh <vulkan|vulkan-webgl2|gl|gl-es> [--frames N] [--warm-up N]
#
# `gl` takes the context wgpu chooses, desktop GL; `gl-es` makes Mesa refuse desktop GL 3.3 and offer
# OpenGL ES 3.0 alone, whose shaders, GLSL ES 3.00, are WebGL2's. Run `vulkan` first: the others
# compare their images with the images of S that it writes.
set -euo pipefail

[ $# -ge 1 ] || { sed -n 2,10p "$0" >&2; exit 2; }
config=$1
shift
[ -n "${SLIME_LAVAPIPE_ICD:-}" ] || { echo "run-config.sh: run it inside nix develop" >&2; exit 2; }
binary=target/release/canvas-prototype
[ -x "$binary" ] || { echo "run-config.sh: build $binary first, from the repository's root" >&2; exit 2; }

case $config in
    vulkan | vulkan-webgl2)
        exec env -u VK_LOADER_DRIVERS_SELECT -u VK_LOADER_DRIVERS_DISABLE \
            VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" "$binary" "$config" "$@"
        ;;
    gl | gl-es)
        # wgpu opens libEGL at run time: glvnd's, from the flake's nixpkgs, which the default shell
        # lacks; it loads Mesa's EGL, the same Mesa as lavapipe's.
        glvnd=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#libglvnd)
        mesa=${SLIME_LAVAPIPE_ICD%/share/vulkan/*}
        es=()
        if [ "$config" = gl-es ]; then
            es=(MESA_GL_VERSION_OVERRIDE=3.2 MESA_GLES_VERSION_OVERRIDE=3.0)
        fi
        exec env -u WAYLAND_DISPLAY -u DISPLAY \
            LD_LIBRARY_PATH="$glvnd/lib:${LD_LIBRARY_PATH:-}" \
            __EGL_VENDOR_LIBRARY_FILENAMES="$mesa/share/glvnd/egl_vendor.d/50_mesa.json" \
            LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe "${es[@]}" \
            "$binary" gl "$@"
        ;;
    *)
        echo "run-config.sh: unknown configuration $config" >&2
        exit 2
        ;;
esac
