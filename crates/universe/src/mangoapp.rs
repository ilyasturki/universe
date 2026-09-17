//! MangoHud told directly: mangoapp over its SysV control queue (what `mangohudctl` speaks), the in-game layer over its control socket.

use std::io::Write;

/// mangoapp_ctrl_msgid1_v1, packed: msg_type i64, ctrl_msg_type u32, version u32, no_display u8, log_session u8, log_session_name [u8; 64], reload_config u8.
const CTRL_LEN: usize = 83;
const NO_DISPLAY_AT: usize = 16;
const RELOAD_AT: usize = 82;

/// The `control=` name in a game's layer conf: an abstract unix socket, bound by the first Vulkan instance of that game alone.
pub fn layer_control(game_id: &str) -> String {
    format!("universe-mangohud-{game_id}")
}

fn ctrl_message(shown: bool, reload: bool) -> [u8; 8 + CTRL_LEN] {
    let mut msg = [0u8; 8 + CTRL_LEN];
    msg[..8].copy_from_slice(&2i64.to_ne_bytes());
    msg[8..12].copy_from_slice(&1u32.to_ne_bytes());
    msg[12..16].copy_from_slice(&1u32.to_ne_bytes());
    msg[NO_DISPLAY_AT] = if shown { 2 } else { 1 };
    msg[RELOAD_AT] = u8::from(reload);
    msg
}

/// Shows or hides every mangoapp that reads the queue; `reload` makes it reread its conf first.
pub fn set_shown(shown: bool, reload: bool) -> std::io::Result<()> {
    let msg = ctrl_message(shown, reload);
    // ftok of a relative "mangoapp" fails without such a file in the cwd, so mangoapp, mangohudctl and this all share key -1.
    let path = std::ffi::CString::new("mangoapp").unwrap();
    // SAFETY: the C strings and the buffer outlive the calls; msgsnd reads CTRL_LEN bytes past the 8-byte type, all inside `msg`, the length mangohudctl sends.
    unsafe {
        let key = libc::ftok(path.as_ptr(), 65);
        let id = libc::msgget(key, 0o666 | libc::IPC_CREAT);
        if id < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if libc::msgsnd(id, msg.as_ptr().cast(), CTRL_LEN, libc::IPC_NOWAIT) < 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Flips the in-game layer's HUD; a frozen game reads it once it thaws.
pub fn layer_toggle(game_id: &str) -> std::io::Result<()> {
    layer_send(&layer_control(game_id), b":hud;")
}

fn layer_send(name: &str, command: &[u8]) -> std::io::Result<()> {
    use std::os::linux::net::SocketAddrExt;
    let addr = std::os::unix::net::SocketAddr::from_abstract_name(name)?;
    let mut s = std::os::unix::net::UnixStream::connect_addr(&addr)?;
    s.write_all(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `mangohudctl set no_display false` puts on the queue, byte for byte: mtype 2, ctrl type 1, version 1, no_display 2.
    #[test]
    fn the_control_message_is_mangohudctls() {
        let msg = ctrl_message(true, false);
        let mut want = vec![0u8; 8 + CTRL_LEN];
        want[..8].copy_from_slice(&2u64.to_ne_bytes());
        want[8] = 1;
        want[12] = 1;
        want[16] = 2;
        assert_eq!(msg.to_vec(), want);
        let hide = ctrl_message(false, true);
        assert_eq!((hide[16], hide[17], &hide[18..82], hide[82]), (1, 0, &[0u8; 64][..], 1), "log_session untouched, its name empty, reload last");
    }

    #[test]
    fn a_control_command_lands_whole_before_the_close() {
        use std::io::Read;
        use std::os::linux::net::SocketAddrExt;
        let name = format!("universe-mangohud-test-{}", std::process::id());
        let addr = std::os::unix::net::SocketAddr::from_abstract_name(&name).unwrap();
        let listener = std::os::unix::net::UnixListener::bind_addr(&addr).unwrap();
        layer_send(&name, b":hud;").unwrap();
        let (mut client, _) = listener.accept().unwrap();
        let mut got = String::new();
        client.read_to_string(&mut got).unwrap();
        assert_eq!(got, ":hud;");
    }
}
