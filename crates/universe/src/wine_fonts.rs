use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::LazyLock;

use regex::bytes::Regex;

const FONT_KEYS: [&[u8]; 3] = [
    br"[Software\\Wine\\Fonts\\External Fonts]",
    br"[Software\\Microsoft\\Windows NT\\CurrentVersion\\Fonts]",
    br"[Software\\Microsoft\\Windows\\CurrentVersion\\Fonts]",
];

static VALUE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^"(?:[^"\\]|\\.)*"="((?:[^"\\]|\\.)*)"$"#).unwrap());

/// Only `\\` and `\"`: a value with any other escape stays undecoded, so kept.
fn unescape(s: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len());
    let mut bytes = s.iter();
    while let Some(&b) = bytes.next() {
        if b == b'\\' {
            match *bytes.next()? {
                c @ (b'\\' | b'"') => out.push(c),
                _ => return None,
            }
        } else {
            out.push(b);
        }
    }
    Some(out)
}

/// /usr/share/fonts is the Steam runtime container's, unseen from here: only a store file is known gone.
fn dead_store_font(line: &[u8]) -> bool {
    let Some(value) = VALUE.captures(line.strip_suffix(b"\n").unwrap_or(line)).and_then(|c| unescape(&c[1])) else {
        return false;
    };
    let Some(rest) = value.strip_prefix(b"Z:").or_else(|| value.strip_prefix(b"z:")).filter(|r| r.starts_with(br"\nix\store\")) else {
        return false;
    };
    let path: Vec<u8> = rest.iter().map(|&b| if b == b'\\' { b'/' } else { b }).collect();
    matches!(Path::new(std::ffi::OsStr::from_bytes(&path)).try_exists(), Ok(false))
}

fn prune(text: &[u8]) -> (Vec<u8>, usize) {
    let mut out = Vec::with_capacity(text.len());
    let (mut in_fonts, mut dropped) = (false, 0);
    for line in text.split_inclusive(|&b| b == b'\n') {
        if line.starts_with(b"[") {
            in_fonts = FONT_KEYS.iter().any(|k| line.starts_with(k));
        } else if in_fonts && dead_store_font(line) {
            dropped += 1;
            continue;
        }
        out.extend_from_slice(line);
    }
    (out, dropped)
}

/// Each registered font gone missing has every Wine process list its dir case-insensitively at start: /nix/store's, for seconds.
pub fn prune_dead(prefix: &Path) -> std::io::Result<usize> {
    // A running wineserver writes its own copy of the registry back over the files.
    if crate::data::wine_running(prefix) {
        return Ok(0);
    }
    let pfx = crate::saves::wine_prefix(prefix);
    let mut total = 0;
    for name in ["user.reg", "system.reg"] {
        let path = pfx.join(name);
        let mut file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        let read = file.metadata()?;
        let mut text = Vec::new();
        file.read_to_end(&mut text)?;
        let (kept, dropped) = prune(&text);
        if dropped > 0 {
            crate::config::replace(&path, &kept, Some(&read))?;
            total += dropped;
        }
    }
    Ok(total)
}
