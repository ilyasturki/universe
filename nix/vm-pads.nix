# Every recorded pad of tests/pads made real in a VM whose kernel drivers read it (hid-playstation, hid-nintendo, hid-microsoft,
# xpad), under udev's game-devices and 70-universe rules and a user session: tools/pad-check --via kernel. `just test vm`.
{
  pkgs,
  module,
  universe,
  ui,
  src,
}:
pkgs.testers.runNixOSTest {
  name = "universe-vm-pads";
  nodes.machine =
    { pkgs, ... }:
    {
      imports = [ module ];
      programs.universe = {
        enable = true;
        capture.enable = false;
        gamescope.enable = false;
      };
      # The Edge's Fn and back buttons reach evdev from Linux 7.2.
      boot.kernelPackages = pkgs.linuxPackages_latest;
      # dummy_hcd is the bus the USB pads appear on for xpad; FunctionFS hands their endpoints to tools/padsim.
      boot.kernelModules = [
        "dummy_hcd"
        "libcomposite"
        "usb_f_fs"
      ];
      users.users.alice.isNormalUser = true;
      services.getty.autologinUser = "alice";
      environment.systemPackages = [
        universe
        ui
        pkgs.python3
      ];
      virtualisation = {
        memorySize = 3072;
        cores = 2;
      };
    };
  testScript = ''
    machine.wait_for_unit("multi-user.target")
    machine.wait_until_succeeds("loginctl show-user alice --property=State | grep -q active")
    machine.succeed("test -c /dev/uhid && ls /sys/class/udc | grep -q dummy_udc")
    print(machine.succeed(
        "python3 ${src}/tools/pad-check --via kernel --as alice --record"
        " --universe ${universe}/bin/universe --ui ${ui}/bin/universe-ui"
        " --modules ${universe}/share/universe/modules 2>&1"
    ))
  '';
}
