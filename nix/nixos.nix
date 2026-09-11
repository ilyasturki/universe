{ universePkg, uiPkg, gsrPkg }:
{ config, lib, pkgs, ... }:
let cfg = config.programs.universe;
in {
  options.programs.universe = {
    enable = lib.mkEnableOption "Universe game launcher (system side: gsr-kms-server, packages)";
    package = lib.mkOption { type = lib.types.package; default = universePkg; };
    ui = lib.mkOption { type = lib.types.package; default = uiPkg; };
    capture.enable = lib.mkOption { type = lib.types.bool; default = true; description = "Enable gpu-screen-recorder with its setcap KMS helper for the capture module, at the version this flake's nixpkgs ships (the helper and the recorder must match)."; };
  };
  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package cfg.ui ];
    programs.gpu-screen-recorder = lib.mkIf cfg.capture.enable { enable = true; package = lib.mkDefault gsrPkg; };
  };
}
