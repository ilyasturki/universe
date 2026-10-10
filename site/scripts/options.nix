{ root }:
let
  lock = builtins.fromJSON (builtins.readFile (root + "/flake.lock"));
  pkgs = import (builtins.fetchTree lock.nodes.nixpkgs.locked) { system = "x86_64-linux"; };
  lib = pkgs.lib;
  # The modules take the flake's packages as their defaults; their names are all the page shows.
  package = name: {
    type = "derivation";
    inherit name;
    outPath = "/nix/store/${name}";
  };
  options =
    module:
    lib.pipe
      (lib.evalModules {
        modules = [
          module
          { _module.check = false; }
        ];
        specialArgs = { inherit pkgs; };
      }).options
      [
        lib.optionAttrSetToDocList
        (lib.filter (o: o.visible && !o.internal && lib.hasPrefix "programs.universe." o.name))
        (map (o: {
          inherit (o) name type;
          description = o.description or null;
          default = o.default or null;
          example = o.example or null;
        }))
      ];
in
{
  home-manager = options (
    import (root + "/nix/home-manager.nix") {
      universePkg = package "universe";
      uiPkg = package "universe-ui";
      desktopPkg = package "universe-desktop";
      extensionPkg = package "universe-shell-extension";
    }
  );
  nixos = options (
    import (root + "/nix/nixos.nix") {
      gsrPkg = package "gpu-screen-recorder";
      uiPkg = package "universe-ui";
    }
  );
}
