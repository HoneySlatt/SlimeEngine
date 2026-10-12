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
      # Linux on x86_64 is the only platform the project builds and tests on for now.
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

        # winit (in slime-platform) and softbuffer (in the examples) open the windowing system's
        # libraries at run time instead of linking them. NixOS has no global library directory,
        # so the dynamic loader has to be told where they are.
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
          pkgs.wayland
          pkgs.libxkbcommon
          pkgs.libx11
          pkgs.libxcursor
          pkgs.libxi
          pkgs.libxcb
        ];
      };
    };
}
