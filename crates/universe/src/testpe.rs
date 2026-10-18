//! A Windows program reduced to its version resource, written the way a linker lays one out.

use std::os::unix::fs::FileExt;
use std::path::Path;

#[derive(Default)]
pub struct Exe<'a> {
    pub strings: &'a [(&'a str, &'a str)],
    pub fixed: Option<[u16; 4]>,
    pub pe32: bool,
    /// Bytes between the headers and the resources: a sparse hole, so a test can stand in for a multi-gigabyte program.
    pub gap: u32,
}

const HEADERS: u32 = 0x200;
const RSRC_RVA: u32 = 0x1000;
const RT_VERSION: u32 = 16;
const SUBDIR: u32 = 0x8000_0000;

impl Exe<'_> {
    pub fn write(&self, path: &Path) {
        let rsrc = self.resources();
        let at = HEADERS + self.gap;
        let f = std::fs::File::create(path).unwrap();
        f.write_all_at(&self.headers(rsrc.len() as u32, at), 0).unwrap();
        f.write_all_at(&rsrc, at as u64).unwrap();
    }

    fn headers(&self, rsrc_len: u32, rsrc_at: u32) -> Vec<u8> {
        let mut b = vec![0u8; HEADERS as usize];
        b[..2].copy_from_slice(b"MZ");
        put32(&mut b, 0x3c, 0x40);
        b[0x40..0x44].copy_from_slice(b"PE\0\0");
        let coff = 0x44;
        let (magic, dirs) = if self.pe32 { (0x10b, 96) } else { (0x20b, 112) };
        let opt_size = dirs + 16 * 8;
        put16(&mut b, coff, if self.pe32 { 0x14c } else { 0x8664 });
        put16(&mut b, coff + 2, 1);
        put16(&mut b, coff + 16, opt_size as u16);
        let opt = coff + 20;
        put16(&mut b, opt, magic);
        put32(&mut b, opt + dirs + 2 * 8, RSRC_RVA);
        put32(&mut b, opt + dirs + 2 * 8 + 4, rsrc_len);
        let section = opt + opt_size;
        b[section..section + 5].copy_from_slice(b".rsrc");
        put32(&mut b, section + 8, rsrc_len);
        put32(&mut b, section + 12, RSRC_RVA);
        put32(&mut b, section + 16, rsrc_len);
        put32(&mut b, section + 20, rsrc_at);
        b
    }

    /// The type, name and language directories, one entry each, then the data entry and the blob.
    fn resources(&self) -> Vec<u8> {
        let mut b = Vec::new();
        for (id, next) in [(RT_VERSION, SUBDIR | 24), (1, SUBDIR | 48), (0x409, 72)] {
            b.extend_from_slice(&[0; 14]);
            b.extend_from_slice(&1u16.to_le_bytes());
            b.extend_from_slice(&id.to_le_bytes());
            b.extend_from_slice(&next.to_le_bytes());
        }
        let info = self.version_info();
        b.extend_from_slice(&(RSRC_RVA + 88).to_le_bytes());
        b.extend_from_slice(&(info.len() as u32).to_le_bytes());
        b.extend_from_slice(&[0; 8]);
        b.extend_from_slice(&info);
        b
    }

    fn version_info(&self) -> Vec<u8> {
        let fixed = self.fixed.map(|[a, b, c, d]| {
            let (ms, ls) = ((a as u32) << 16 | b as u32, (c as u32) << 16 | d as u32);
            [0xFEEF_04BD, 0x0001_0000, ms, ls, ms, ls, 0x3f, 0, 0x0004_0004, 1, 0, 0, 0].iter().flat_map(|w: &u32| w.to_le_bytes()).collect::<Vec<u8>>()
        });
        let strings = self.strings.iter().map(|(k, v)| node(k, Value::Text(v), vec![])).collect();
        let table = node("040904B0", Value::None, strings);
        node("VS_VERSION_INFO", fixed.map_or(Value::None, Value::Binary), vec![node("StringFileInfo", Value::None, vec![table])])
    }
}

enum Value<'a> {
    None,
    Binary(Vec<u8>),
    Text(&'a str),
}

/// `wLength`, `wValueLength` (in words for text), `wType`, the key, then the value and the children, each 4-aligned.
fn node(key: &str, value: Value, children: Vec<Vec<u8>>) -> Vec<u8> {
    let (value, value_len, text) = match value {
        Value::None => (vec![], 0, true),
        Value::Binary(b) => (b.clone(), b.len(), false),
        Value::Text(t) => (utf16z(t), t.encode_utf16().count() + 1, true),
    };
    let mut b = vec![0; 6];
    b.extend(utf16z(key));
    pad(&mut b);
    b.extend(value);
    for child in children {
        pad(&mut b);
        b.extend(child);
    }
    let len = b.len() as u16;
    put16(&mut b, 0, len);
    put16(&mut b, 2, value_len as u16);
    put16(&mut b, 4, text as u16);
    b
}

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
}

fn pad(b: &mut Vec<u8>) {
    b.resize(b.len().next_multiple_of(4), 0);
}

fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
