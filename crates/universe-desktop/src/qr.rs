use gtk::prelude::*;
use gtk::{gdk, glib};

// Four modules of white around the code: what a phone's reader needs to find it.
const QUIET: usize = 4;

/// `text` as a QR code, each module `scale` pixels, dark on white whatever the theme.
pub fn texture(text: &str, scale: usize) -> Option<gdk::Texture> {
    let code = qrcode::QrCode::with_error_correction_level(text.as_bytes(), qrcode::EcLevel::L).ok()?;
    let width = code.width();
    let dark = code.to_colors();
    let side = (width + 2 * QUIET) * scale;
    let mut pixels = vec![255u8; side * side];
    for (i, color) in dark.iter().enumerate() {
        if *color != qrcode::Color::Dark {
            continue;
        }
        let (x, y) = ((i % width + QUIET) * scale, (i / width + QUIET) * scale);
        for row in y..y + scale {
            pixels[row * side + x..row * side + x + scale].fill(0);
        }
    }
    let bytes = glib::Bytes::from_owned(pixels);
    Some(gdk::MemoryTexture::new(side as i32, side as i32, gdk::MemoryFormat::G8, &bytes, side).upcast())
}
