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
          # A compositor without a screen, in which scripts/check.sh runs the windowed sample.
          pkgs.weston
        ];

        # winit (in slime-platform) and wgpu (in slime-gfx) open the windowing system's libraries and
        # the Vulkan loader at run time instead of linking them.
        # NixOS has no global library directory, so the dynamic loader has to be told where they
        # are.
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
          pkgs.wayland
          pkgs.libxkbcommon
          pkgs.libx11
          pkgs.libxcursor
          pkgs.libxi
          pkgs.libxcb
          pkgs.vulkan-loader
        ];

        # The Vulkan driver of Mesa that renders on the CPU (lavapipe). scripts/check.sh points the
        # Vulkan loader at it (VK_DRIVER_FILES) so that GPU tests give the same results on every
        # machine and in CI, which has no GPU.
        SLIME_LAVAPIPE_ICD = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";
      };
    };
}
