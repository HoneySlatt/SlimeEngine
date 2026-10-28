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
      devShells.${system}.default = pkgs.mkShell {
        packages = [
          # The toolchain pinned in rust-toolchain.toml, the same one rustup users get.
          (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
          pkgs.cargo-deny
          pkgs.lychee
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

        # winit (in slime-platform) and wgpu (in slime-gfx) open the windowing system's libraries and
        # the Vulkan loader at run time instead of linking them, fontique (in slime-ui)
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

        # The Vulkan driver of Mesa that renders on the CPU (lavapipe). scripts/check.sh points the
        # Vulkan loader at it (VK_DRIVER_FILES) so that GPU tests give the same results on every
        # machine and in CI, which has no GPU.
        SLIME_LAVAPIPE_ICD = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";
      };
    };
}
