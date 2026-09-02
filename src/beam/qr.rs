//! Renders high-contrast terminal QR codes with the required quiet zone.

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
