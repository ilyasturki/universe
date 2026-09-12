{ universePkg, uiPkg }:
{ config, lib, pkgs, ... }:
let
  cfg = config.programs.universe;
  tomlFormat = pkgs.formats.toml { };
  settings = lib.recursiveUpdate {
    schema = 1;
    modules.enabled = cfg.modules.enabled;
  } cfg.settings;
in {
  options.programs.universe = {
    enable = lib.mkEnableOption "Universe game launcher (user side: config.toml, enabled modules, packages)";
    package = lib.mkOption { type = lib.types.package; default = universePkg; };
    ui = lib.mkOption { type = lib.types.package; default = uiPkg; };
    settings = lib.mkOption {
      type = lib.types.nullOr tomlFormat.type;
      default = { };
      example = { paths.recordings_root = "/mnt/recordings/games"; launch.proton = "proton-ge"; modules.capture.codec = "av1_10bit"; };
      description = "Contents of config.toml (see docs/api.md). Paths, launch defaults, [modules.<id>] settings, keys. null leaves config.toml to the user: the core writes it in place (`universe config set`, module enable, the UI settings), which a store symlink refuses.";
    };
    modules.enabled = lib.mkOption { type = lib.types.listOf lib.types.str; default = [ "gog" "capture" "journal" "tracker-md" ]; };
  };
  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package cfg.ui ];
    xdg.configFile."universe/config.toml" = lib.mkIf (cfg.settings != null) { source = tomlFormat.generate "universe-config.toml" settings; };
  };
}
