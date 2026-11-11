#!/usr/bin/env bash
# Runs the canvas prototype on one configuration, on Mesa's drivers that draw on the CPU and never
# on a GPU: lavapipe for Vulkan, llvmpipe for GL. Run it inside `nix develop`, from the
# repository's root, after `cargo build --release -p canvas-prototype`:
#
#   crates/canvas-prototype/run-config.sh <vulkan|vulkan-webgl2|gl|gl-es|vulkan-gpu|vulkan-webgl2-gpu|gl-es-gpu> [--frames N] [--warm-up N]
#       [--effect transform|group|blur|path|grow|all]...
#
# `gl` takes the context wgpu chooses, desktop GL; `gl-es` makes Mesa refuse desktop GL 3.3 and offer
# OpenGL ES 3.0 alone, whose shaders, GLSL ES 3.00, are WebGL2's. Run `vulkan` first: the others
# compare their images with the images of S that it writes.
set -euo pipefail

[ $# -ge 1 ] || { sed -n 2,11p "$0" >&2; exit 2; }
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
    vulkan-gpu | vulkan-webgl2-gpu)
        # The owner's GPU, through radv, only with the owner's go for a series of measures.
        exec env -u VK_LOADER_DRIVERS_SELECT -u VK_LOADER_DRIVERS_DISABLE \
            VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/radeon_icd.x86_64.json \
            "$binary" "${config%-gpu}" --any-adapter "$@"
        ;;
    gl-es-gpu)
        # The owner's GPU, through radeonsi's OpenGL ES 3.0, only with the owner's go.
        glvnd=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#libglvnd)
        exec env -u WAYLAND_DISPLAY -u DISPLAY \
            LD_LIBRARY_PATH="$glvnd/lib:/run/opengl-driver/lib:${LD_LIBRARY_PATH:-}" \
            __EGL_VENDOR_LIBRARY_FILENAMES=/run/opengl-driver/share/glvnd/egl_vendor.d/50_mesa.json \
            MESA_GL_VERSION_OVERRIDE=3.2 MESA_GLES_VERSION_OVERRIDE=3.0 \
            "$binary" gl --any-adapter "$@"
        ;;
    *)
        echo "run-config.sh: unknown configuration $config" >&2
        exit 2
        ;;
esac
