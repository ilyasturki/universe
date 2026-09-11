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
    enable = lib.mkEnableOption "Universe game launcher (user side: config.toml, universed service, packages)";
    package = lib.mkOption { type = lib.types.package; default = universePkg; };
    ui = lib.mkOption { type = lib.types.package; default = uiPkg; };
    settings = lib.mkOption {
      type = tomlFormat.type;
      default = { };
      example = { paths.recordings_root = "/mnt/recordings/games"; launch.proton = "proton-ge"; modules.capture.codec = "av1_10bit"; };
      description = "Contents of config.toml (see docs/api.md). Paths, launch defaults, [modules.<id>] settings, keys.";
    };
    modules.enabled = lib.mkOption { type = lib.types.listOf lib.types.str; default = [ "gog" "capture" "journal" "tracker-md" ]; };
    service.enable = lib.mkOption { type = lib.types.bool; default = true; description = "Run universed as a systemd --user service in the graphical session."; };
  };
  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package cfg.ui ];
    xdg.configFile."universe/config.toml".source = tomlFormat.generate "universe-config.toml" settings;
    xdg.dataFile."dbus-1/services/io.github.ilyasturki.Universe.service".text = ''
      [D-BUS Service]
      Name=io.github.ilyasturki.Universe
      Exec=${cfg.package}/bin/universed
      SystemdService=universed.service
    '';
    systemd.user.services.universed = lib.mkIf cfg.service.enable {
      Unit = { Description = "Universe game launcher daemon"; PartOf = [ "graphical-session.target" ]; After = [ "graphical-session.target" ]; };
      Service = { Type = "dbus"; BusName = "io.github.ilyasturki.Universe"; ExecStart = "${cfg.package}/bin/universed"; Restart = "on-failure"; };
      Install.WantedBy = [ "graphical-session.target" ];
    };
  };
}
