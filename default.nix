{ pkgs ? import <nixpkgs> {} }: with pkgs;

stdenv.mkDerivation {
  name = "gli";

  buildInputs = [ latest.rustChannels.nightly.rust openssl cmake zlib pkgconfig ];
}
