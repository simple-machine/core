with import <nixpkgs> {};
stdenv.mkDerivation {
  name = "env";
  buildInputs = [
    libudev pkg-config opencv4 gdb
  ];
}

