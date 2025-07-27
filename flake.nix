{
  description = "Development environment for rust-cktap";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };

      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [ "rust-src" ];
        targets = [ "x86_64-unknown-linux-musl" ];
      };

    in {
      devShells.default = pkgs.mkShell {
        packages = with pkgs; [
          bashInteractive
          
          # Rust with musl target
          rustToolchain
          cargo-watch
          clippy

          # For dependencies
          pkg-config
          pkgsStatic.stdenv.cc
          
          # Static libraries for musl build
          pkgsStatic.libusb1
          pkgsStatic.libudev-zero
        ];

        CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = "${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc";
        CC_x86_64_unknown_linux_musl = "${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc";
        
        # Point libusb to use the static libraries
        LIBUSB_STATIC = "1";
        LIBUSB1_SYS_STATIC = "1";
        PKG_CONFIG_ALL_STATIC = "1";
        
        # Ensure pkg-config can find the static libraries
        PKG_CONFIG_PATH = "${pkgs.pkgsStatic.libusb1}/lib/pkgconfig:${pkgs.pkgsStatic.libudev-zero}/lib/pkgconfig";
        
        RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
      };
    });
}
