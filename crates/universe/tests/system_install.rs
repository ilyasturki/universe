use std::process::Command;

fn helper(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_universe-system-install")).args(args).env("DBUS_SYSTEM_BUS_ADDRESS", "unix:path=/dev/null/bus").output().unwrap()
}

#[test]
fn the_install_helper_refuses_what_universe_does_not_list_before_reaching_packagekit() {
    for args in [&[][..], &["bash"], &["lib32-mangohud"], &["/usr/bin/gamescope"], &["mangohud", "gamescope"], &["mangohud", "--", "sh"]] {
        let out = helper(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        assert!(out.stdout.is_empty(), "{args:?} reports no progress");
    }
}

#[test]
fn a_listed_tool_goes_on_to_packagekit() {
    let out = helper(&["mangohud"]);
    assert_eq!(out.status.code(), Some(1), "past the refusal: {}", String::from_utf8_lossy(&out.stderr));
}
