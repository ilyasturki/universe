{ gsrPkg, uiPkg }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.universe;
  # uaccess only takes from a rule ahead of 73-seat-late.rules; services.udev.extraRules lands in 99-local.rules.
  rules = pkgs.writeTextDir "lib/udev/rules.d/70-universe.rules" (
    builtins.readFile ../packaging/system/70-universe.rules
  );
  ui = "${cfg.session.package}/bin/universe-ui";
  # providedSessions names the file less .desktop: NixOS checks it, and defaultSession picks by it.
  session =
    (pkgs.writeTextDir "share/wayland-sessions/universe.desktop" (
      lib.replaceStrings [ "Exec=universe-ui" ] [ "Exec=${ui}" ] (
        builtins.readFile ../packaging/system/universe.desktop
      )
    )).overrideAttrs
      { passthru.providedSessions = [ "universe" ]; };
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
      description = "What the controller macros and the pads module need from the system: /dev/uinput opened to the logged-in user (key macros type through it), the game-devices udev rules that make pads readable by the logged-in user, and /dev/uhid with the virtual pads' hidraw nodes opened to that user (the pads module creates one virtual pad per player there).";
    };
    gamescope.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "gamescope with its CAP_SYS_NICE wrapper: every game runs inside it (launch.gamescope), one window that is black until the game draws.";
    };
    session = {
      enable = lib.mkEnableOption "the Universe session in the display manager: gamescope drives the screen with the launcher as its only client, like SteamOS's Game Mode, and quitting logs out. `services.displayManager.defaultSession = \"universe\"` with an autologin boots straight into it";
      package = lib.mkOption {
        type = lib.types.package;
        default = uiPkg;
        defaultText = lib.literalMD "this flake's universe-ui";
        description = "The universe-ui the session runs: the one Home Manager installs, so both read the same library.";
      };
    };
  };
  config = lib.mkMerge [
    {
      assertions = [
        {
          assertion = cfg.session.enable -> cfg.enable;
          message = "programs.universe.session needs programs.universe.enable: the session's gamescope comes with it.";
        }
      ];
    }
    (lib.mkIf cfg.enable {
      services.displayManager.sessionPackages = lib.mkIf cfg.session.enable [ session ];
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
        rules
      ];
      # /dev/uhid is a static node: a user's open does not load the module, and the rule applies once it is loaded.
      boot.kernelModules = lib.mkIf cfg.controller.enable [ "uhid" ];
      programs.gamescope = lib.mkIf cfg.gamescope.enable {
        enable = true;
        capSysNice = lib.mkDefault true;
      };
    })
  ];
}
