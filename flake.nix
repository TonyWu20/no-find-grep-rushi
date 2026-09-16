{
  description = "no-find-grep — tool.before hook that blocks bare find/grep and rg flag misuse in pending bash tool calls";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, fenix }:
    let
      # Explicit supported-systems list + genAttrs, the same shape as the
      # kernel flake (packages.<system>.<name>): the transposed layout
      # eachDefaultSystem would produce, without its breakage
      # (docs/reference/nix/ext-flake-authoring.md §3).
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      pkgLib = nixpkgs.lib;

      buildFor = system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ fenix.overlays.default ];
          };
          rustToolchain = fenix.packages.${system}.stable.withComponents [
            "cargo" "clippy" "rust-src" "rustc" "rustfmt" "rust-analyzer"
          ];

          # Build one standalone cargo crate from a subpath of this
          # flake's source tree. The crate at the repo root has no
          # intra-repo path deps; the empty [workspace] table in
          # Cargo.toml keeps cargo off the parent rushi-exts tree.
          # A trailing-slash string source ("${self}/.") fails the
          # source unpacker, so the root crate uses a path literal.
          buildCrate = { crateDir, crateName }:
            let
              # Root crate: path literals (relative to the flake source).
              # Sub-crate: string interpolation against the flake source.
              crateSrc  = if crateDir == "." then ./.           else "${self}/${crateDir}";
              crateLock = if crateDir == "." then ./Cargo.lock  else "${self}/${crateDir}/Cargo.lock";
            in
            pkgs.rustPlatform.buildRustPackage {
              pname = crateName;
              version = "0.1.0";
              src = crateSrc;
              nativeBuildInputs = [ rustToolchain ];
              cargoLock = { lockFile = crateLock; };
              doCheck = false;
            };

          # Base hook package (guide §4.2): a bare buildRustPackage result
          # IS the hook source — $out/bin/harness-hook-no-find-grep is
          # exactly what mk-rushi's hook copy step expects. Bound here (a
          # `let` binding) because `let … in` is not a legal attr value
          # inside the rec {} below; the rec attr then merges meta.rushi
          # onto it.
          hookBase = buildCrate {
            crateDir = ".";
            crateName = "hook-no-find-grep";
          };
        in
        # ── Hook (guide §4.2): a bare buildRustPackage result IS the
        #    hook source — $out/bin/harness-hook-no-find-grep is exactly
        #    what mk-rushi's hook copy step expects. No wrapper needed.
        #
        #    meta.rushi.bin (rushi#13): the runtime binary name, so a
        #    consumer can derive hook commands without a second typed
        #    copy. The merge preserves any meta buildRustPackage set.
        rec {
          hook-no-find-grep = hookBase // {
            meta = (hookBase.meta or { }) // {
              rushi = { bin = "harness-hook-no-find-grep"; };
            };
          };

          default = hook-no-find-grep;
        };
    in
    {
      packages = pkgLib.genAttrs supportedSystems (system: buildFor system);
    };
}
