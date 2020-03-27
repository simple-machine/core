with import <nixpkgs> { config.allowUnsupportedSystem = true; config.android_sdk.accept_license = true; };
{ armPkgs ? import <nixpkgs> { crossSystem = pkgs.lib.systems.examples.aarch64-android-prebuilt; } }:

stdenv.mkDerivation { # pkgs.androidenv.buildApp {
  name = "env";

  buildInputs = with pkgs; [
    libudev pkg-config opencv4 gdb armPkgs.stdenv.cc
  ];
}
