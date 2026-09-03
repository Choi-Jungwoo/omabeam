//! Renders high-contrast terminal QR codes with the required quiet zone.

use std::path::Path;

use qrcode::QrCode;
use qrcode::render::unicode::Dense1x2;
pub(super) use qrcode::types::QrError;

pub(super) fn render(text: &str) -> Result<String, QrError> {
    let code = QrCode::new(text.as_bytes())?;
    let image = code
        .render::<Dense1x2>()
        .quiet_zone(true)
        .module_dimensions(1, 1)
        .build();

    Ok(image)
}

pub(super) fn render_png(text: &str, path: &Path) -> Result<(), String> {
    let code = QrCode::new(text.as_bytes()).map_err(|e| format!("could not encode QR: {e}"))?;
    let image = code
        .render::<image::Luma<u8>>()
        .quiet_zone(true)
        .module_dimensions(8, 8)
        .build();
    image
        .save(path)
        .map_err(|e| format!("could not write QR image: {e}"))?;
    Ok(())
}
