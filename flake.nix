{
  description = "Xollo dev environment";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        tex = (pkgs.texliveSmall.withPackages (ps: with ps; [
          # https://github.com/James-Yu/LaTeX-Workshop/wiki/Install#installation
          latexmk # making files from latex
          chktex # linting
          latexindent # formatting
        ]));
      in
      {
        devShells.default = pkgs.mkShellNoCC {
          buildInputs = with pkgs; [
            # for go development
            go
            gotools
            gopls

            # LaTeX
            tex
          ];
        };
      }
    );
}
