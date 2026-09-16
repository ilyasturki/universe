{ universePkg, uiPkg, extensionPkg }:
{ config, lib, pkgs, ... }:
let
  cfg = config.programs.universe;
  tomlFormat = pkgs.formats.toml { };
in {
  options.programs.universe = {
    enable = lib.mkEnableOption "Universe game launcher (user side: packages, config.toml)";
    package = lib.mkOption { type = lib.types.package; default = universePkg; };
    ui = lib.mkOption { type = lib.types.package; default = uiPkg; };
    shellExtension = lib.mkOption {
      type = lib.types.package;
      default = extensionPkg;
      description = "The 'universe@ilyasturki.github.io' GNOME Shell extension: the capture module lists windows through it for window-only recording and takes its screenshots in it, the controller macros show the shell's OSD through it. GNOME loads it after the next logout.";
    };
    settings = lib.mkOption {
      type = lib.types.nullOr tomlFormat.type;
      default = { };
      example = { paths.recordings_root = "/mnt/recordings/games"; launch.proton = "proton-ge"; modules.enabled = [ "gog" "capture" ]; modules.capture.codec = "av1_10bit"; };
      description = "Contents of config.toml (see docs/api.md): paths, launch defaults, the enabled modules and their [modules.<id>] settings, keys. null leaves config.toml to the user: the core writes it in place (`universe config set`, module enable, the UI settings), which a store symlink refuses.";
    };
  };
  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package cfg.ui cfg.shellExtension ];
    xdg.configFile."universe/config.toml" = lib.mkIf (cfg.settings != null) { source = tomlFormat.generate "universe-config.toml" ({ schema = 1; } // cfg.settings); };
  };
}
