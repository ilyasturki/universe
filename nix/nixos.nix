{ universePkg, uiPkg }:
{ config, lib, pkgs, ... }:
let cfg = config.programs.universe;
in {
  options.programs.universe = {
    enable = lib.mkEnableOption "Universe game launcher (system side: gsr-kms-server, D-Bus service, packages)";
    package = lib.mkOption { type = lib.types.package; default = universePkg; };
    ui = lib.mkOption { type = lib.types.package; default = uiPkg; };
    capture.enable = lib.mkOption { type = lib.types.bool; default = true; description = "Enable gpu-screen-recorder with its setcap KMS helper for the capture module."; };
  };
  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package cfg.ui ];
    services.dbus.packages = [ cfg.package ];
    programs.gpu-screen-recorder.enable = lib.mkIf cfg.capture.enable true;
  };
}
