with import <nixpkgs> {};

let
  moz_overlay = import (builtins.fetchTarball
    "https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz");
  nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
  rustStable = nixpkgs.latest.rustChannels.stable.rust.override {
    extensions =
      [ "rust-src" "rls-preview" "rustfmt-preview" "clippy-preview" ];
  };
in with nixpkgs;
stdenv.mkDerivation rec {
  name = "moz_overlay_shell";
  buildInputs = [
    rustStable
    dbus
    pkgconfig
    gtk3
  ];
  RUST_BACKTRACE = 1;
  LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
}
