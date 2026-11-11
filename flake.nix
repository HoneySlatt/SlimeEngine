{
  description = "SlimeEngine development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      # The development shell is for Linux on x86_64; on Windows, contributors use rustup
      # (CONTRIBUTING.md), and CI builds and tests there without Nix.
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };
    in
    {
      devShells.${system} = rec {
        default = pkgs.mkShell {
          packages = [
            # The toolchain pinned in rust-toolchain.toml, the same one rustup users get.
            (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
            pkgs.cargo-deny
            pkgs.lychee
            # Links the programs built for Linux, faster than rustc's own lld (ADR-0221).
            pkgs.mold
            # Times a program's start, from its process's launch to its exit after its first image
            # (ADR-0189).
            pkgs.hyperfine
            # Compresses a build for the web as servers send it, to weigh it against uwui's budget
            # (ADR-0181, ADR-0241).
            pkgs.brotli
            # Records a program's heap over its run, for scripts/resources.sh (ADR-0175, ADR-0193).
            pkgs.heaptrack
            # A compositor without a screen that takes virtual input, and the tool that gives it, in
            # which scripts/input-to-present.sh gives the editor and the game a person's input
            # (ADR-0191).
            pkgs.sway
            pkgs.wlrctl
            # A compositor without a screen, in which scripts/check.sh runs the windowed samples.
            pkgs.weston
            # The service of screen readers on Linux, AT-SPI, on a session bus of D-Bus of its own,
            # and a client of it in Python, through which scripts/check.sh reads and uses the
            # `interface` example as a screen reader does (ADR-0146).
            pkgs.at-spi2-core
            pkgs.dbus
            # gdbus, which turns AT-SPI on.
            pkgs.glib
            (pkgs.python3.withPackages (python: [ python.pygobject3 ]))
          ];

          # Where the client of AT-SPI finds the types of AT-SPI, GLib and D-Bus, and the AT-SPI's
          # bus and registry, which are not on the path.
          GI_TYPELIB_PATH = pkgs.lib.makeSearchPath "lib/girepository-1.0" [
            pkgs.at-spi2-core
            pkgs.glib
            pkgs.gobject-introspection
          ];
          SLIME_AT_SPI = "${pkgs.at-spi2-core}";

          # winit (in slime-platform) and wgpu (in slime-gfx) open the windowing system's libraries
          # and the Vulkan loader at run time instead of linking them, fontique (in uwui)
          # fontconfig, which finds the system's fonts for a program that asks for them, and rfd (in
          # slime-platform) D-Bus's library, through which it asks the desktop's portal for a dialog
          # (ADR-0157). NixOS has no global library directory, so the dynamic loader has to be told
          # where they are.
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
            pkgs.wayland
            pkgs.libxkbcommon
            pkgs.libx11
            pkgs.libxcursor
            pkgs.libxi
            pkgs.libxcb
            pkgs.vulkan-loader
            pkgs.fontconfig
            pkgs.dbus.lib
          ];

          # The Vulkan driver of Mesa that renders on the CPU (lavapipe). scripts/check.sh points
          # the Vulkan loader at it (VK_DRIVER_FILES) so that GPU tests give the same results on
          # every machine and in CI, which has no GPU.
          SLIME_LAVAPIPE_ICD = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";

          # mold links what is built for Linux, in this shell only, so that a contributor without
          # Nix still builds with rustc's own linker (ADR-0221). A `RUSTFLAGS` of one's own replaces
          # it.
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C link-arg=-fuse-ld=mold";
        };

        # The default shell and the series of headless compositors that scripts/compositors.sh runs
        # the editor and the game in, kept apart so that the default shell does not grow (ADR-0195,
        # ADR-0240): `nix develop .#compositors`. CI's `compositors` workflow runs in it.
        compositors = default.overrideAttrs (shell: {
          nativeBuildInputs = shell.nativeBuildInputs ++ [
            # KDE's compositor and GNOME's, each run with a virtual output of its own.
            pkgs.kdePackages.kwin
            pkgs.mutter
            # An X server without a screen, and the tool that gives it input through XTest.
            pkgs.xvfb
            pkgs.xdotool
            # Lists the protocols that each Wayland compositor gives.
            pkgs.wayland-utils
            # libei's C library, through which scripts/compositor-input.py gives KWin and Mutter
            # their input.
            pkgs.libei
          ];
          LD_LIBRARY_PATH = "${shell.LD_LIBRARY_PATH}:${pkgs.lib.makeLibraryPath [ pkgs.libei ]}";
          # Tells scripts/compositors.sh that it runs in this shell.
          SLIME_COMPOSITORS = "1";
          # Mesa's EGL, which KWin and Mutter draw with: outside NixOS, as on CI's runners, the
          # loader of EGL finds no driver of nixpkgs' own. scripts/compositors.sh names it to them.
          SLIME_MESA_EGL = "${pkgs.mesa}/share/glvnd/egl_vendor.d/50_mesa.json";
        });
      };
    };
}
