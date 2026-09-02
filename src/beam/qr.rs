//! Renders high-contrast terminal QR codes with the required quiet zone.

use qrcode::QrCode;
use qrcode::render::unicode::Dense1x2;

pub(super) fn render(text: &str) -> Result<String, String> {
    if text.is_empty() {
        return Err(
            "there is no text to share; pipe text into omabeam or copy some text first".into(),
        );
    }

    let code = QrCode::new(text.as_bytes()).map_err(|_| {
        "text is too long for a QR code; shorten it or share a file instead".to_owned()
    })?;
    let image = code
        .render::<Dense1x2>()
        .quiet_zone(true)
        .module_dimensions(1, 1)
        .build();

    Ok(image)
}
