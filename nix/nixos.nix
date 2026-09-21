{ gsrPkg }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.universe;
in
{
  options.programs.universe = {
    enable = lib.mkEnableOption "Universe game launcher (system side: gsr-kms-server, uinput, InputPlumber, gamescope)";
    capture.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Enable gpu-screen-recorder with its setcap KMS helper for the capture module, at the version this flake's nixpkgs ships (the helper and the recorder must match).";
    };
    controller.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "What the controller macros need from the system: /dev/uinput (key macros type through it; add your user to the uinput group) and the game-devices udev rules that make pads readable by the logged-in user.";
    };
    inputplumber.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "The InputPlumber daemon, which the emulators' `inputplumber` option uses to hand the pads to the emulator as one composite device for the session.";
    };
    gamescope.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "gamescope with its CAP_SYS_NICE wrapper: every game runs inside it (launch.gamescope), one window that is black until the game draws.";
    };
  };
  config = lib.mkIf cfg.enable {
    programs.gpu-screen-recorder = lib.mkIf cfg.capture.enable {
      enable = true;
      package = lib.mkDefault gsrPkg;
    };
    assertions = lib.mkIf cfg.capture.enable [
      {
        assertion = lib.versionAtLeast config.programs.gpu-screen-recorder.package.version "6.1";
        message = "programs.universe: the capture module drives gpu-screen-recorder over its ipc socket (gsr-cli), which needs 6.1 or later; ${config.programs.gpu-screen-recorder.package.version} is installed. Pin a newer nixpkgs for programs.gpu-screen-recorder.package or set programs.universe.capture.enable = false.";
      }
    ];
    hardware.uinput.enable = lib.mkIf cfg.controller.enable true;
    services.inputplumber.enable = lib.mkIf cfg.inputplumber.enable true;
    services.udev.packages = lib.mkIf cfg.controller.enable [ pkgs.game-devices-udev-rules ];
    programs.gamescope = lib.mkIf cfg.gamescope.enable {
      enable = true;
      capSysNice = lib.mkDefault true;
      # gamescope passes -1 to vkAllocateMemory when no host memory type is both cached and coherent (ANV on Meteor Lake): the patch settles for coherent.
      package = lib.mkDefault (
        pkgs.gamescope.overrideAttrs (o: {
          patches = (o.patches or [ ]) ++ [ ./gamescope-mappable-fallback.patch ];
        })
      );
    };
  };
}
