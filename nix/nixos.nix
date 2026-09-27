{ gsrPkg }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.universe;
  # uaccess only takes from a rule ahead of 73-seat-late.rules; services.udev.extraRules lands in 99-local.rules.
  padsRules = pkgs.writeTextDir "lib/udev/rules.d/70-universe-pads.rules" ''
    KERNEL=="uhid", TAG+="uaccess"
    KERNEL=="hidraw*", KERNELS=="*:0079:555[0-7].*", TAG+="uaccess"
  '';
in
{
  imports = [
    (lib.mkRemovedOptionModule [ "programs" "universe" "inputplumber" "enable" ]
      "Universe no longer drives InputPlumber: the controls module sets each emulator's pads up itself."
    )
  ];
  options.programs.universe = {
    enable = lib.mkEnableOption "Universe game launcher (system side: gsr-kms-server, uinput, uhid, gamescope)";
    capture.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Enable gpu-screen-recorder with its setcap KMS helper for the capture module, at the version this flake's nixpkgs ships (the helper and the recorder must match).";
    };
    controller.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "What the controller macros and the pads module need from the system: /dev/uinput (key macros type through it; add your user to the uinput group), the game-devices udev rules that make pads readable by the logged-in user, and /dev/uhid with the virtual pads' hidraw nodes opened to that user (the pads module creates one virtual pad per player there).";
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
    services.udev.packages = lib.mkIf cfg.controller.enable [
      pkgs.game-devices-udev-rules
      padsRules
    ];
    # /dev/uhid is a static node: a user's open does not load the module, and the rule applies once it is loaded.
    boot.kernelModules = lib.mkIf cfg.controller.enable [ "uhid" ];
    programs.gamescope = lib.mkIf cfg.gamescope.enable {
      enable = true;
      capSysNice = lib.mkDefault true;
    };
  };
}
