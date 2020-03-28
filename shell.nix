with import <nixpkgs> { config.allowUnsupportedSystem = true; config.android_sdk.accept_license = true; };
{
  aarchPkgs ? import <nixpkgs> { crossSystem = pkgs.lib.systems.examples.aarch64-android-prebuilt; },
  armPkgs ? import <nixpkgs> { crossSystem = pkgs.lib.systems.examples.armv7a-android-prebuilt; },
  i686Pkgs ? import <nixpkgs> { crossSystem = pkgs.platforms.pc32; },
  winPkgs ? import <nixos-unstable> { crossSystem = pkgs.lib.systems.examples.mingwW64; }
}:

stdenv.mkDerivation { # pkgs.androidenv.buildApp {
  name = "env";

  buildInputs = with pkgs; [
    libudev pkg-config opencv4 gdb aarchPkgs.stdenv.cc armPkgs.stdenv.cc winPkgs.stdenv.cc
  ];
}
