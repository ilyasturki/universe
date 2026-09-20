{
  universePkg,
  uiPkg,
  extensionPkg,
}:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.universe;
  tomlFormat = pkgs.formats.toml { };
in
{
  options.programs.universe = {
    enable = lib.mkEnableOption "Universe game launcher (user side: packages, config.toml)";
    package = lib.mkOption {
      type = lib.types.package;
      default = universePkg;
    };
    ui = lib.mkOption {
      type = lib.types.package;
      default = uiPkg;
    };
    shellExtension = lib.mkOption {
      type = lib.types.package;
      default = extensionPkg;
      description = "The 'universe@ilyasturki.github.io' GNOME Shell extension: the capture module lists windows through it for window-only recording and takes its screenshots in it, the controller macros show the shell's OSD through it. GNOME loads it after the next logout.";
    };
    settings = lib.mkOption {
      type = lib.types.nullOr tomlFormat.type;
      default = null;
      example = {
        paths.recordings_root = "/mnt/recordings/games";
        launch.proton = "proton-ge";
        modules.enabled = [ "capture" ];
        modules.capture.codec = "av1_10bit";
        sources.enabled = [ "gog" ];
        sources.gog.platform = "linux";
      };
      description = "Contents of config.toml (see docs/api.md): paths, launch defaults, the enabled modules and sources with their [modules.<id>] and [sources.<id>] settings, keys. Set, config.toml is a store symlink the core cannot write: `universe config set`, enabling a module or source and the UI's settings pages are refused, so every setting comes from here. null (the default) leaves config.toml to the user and the core writes it in place.";
    };
  };
  config = lib.mkIf cfg.enable {
    home.packages = [
      cfg.package
      cfg.ui
      cfg.shellExtension
    ];
    xdg.configFile."universe/config.toml" = lib.mkIf (cfg.settings != null) {
      source = tomlFormat.generate "universe-config.toml" ({ schema = 1; } // cfg.settings);
    };
  };
}
