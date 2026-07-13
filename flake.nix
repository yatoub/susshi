{
  description = "Susshi - terminal SSH connection manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "susshi";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;

          src = pkgs.lib.cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.openssl ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            ];

          meta = with pkgs.lib; {
            description = "Terminal SSH manager with YAML inventories, multi-hop jump hosts, Wallix bastion, tunnels, SCP, and Catppuccin TUI";
            homepage = "https://yatoub.github.io/susshi/";
            license = licenses.mit;
            mainProgram = "susshi";
          };
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ self.packages.${system}.default ];
          packages = [ pkgs.cargo-llvm-cov ];
        };
      });
}
