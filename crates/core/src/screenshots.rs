//! Images in MCP tool results, such as a browser's screenshots, kept with the tool call in the
//! timeline (`browser-screenshots-in-timeline` design decisions 2, 3 and 5). Only `mcp__*` tools:
//! Claude Code's own tools, such as `Read` of an image file, can return files from anywhere on the
//! host's computer.

use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::{DynamicImage, ImageFormat, ImageReader, Limits, codecs::jpeg::JpegEncoder};
use serde_json::Value;
use specdeck_protocol::timeline::{Image, LeftOut, ToolOutcome};

/// Images whose base64 is at most this long are kept as they came, which covers SpecDeck's own
/// screenshots (150 KB of JPEG); larger ones are made smaller until they fit.
pub const MAX_SCREENSHOT_BASE64: usize = 200 * 1024;
/// The widths and JPEG qualities a large image is made smaller with, in order.
const SMALLER: [(u32, u8); 2] = [(1280, 70), (960, 60)];
/// An image claiming more pixels on a side than this is not decoded.
const MAX_SIDE: u32 = 8192;
/// Most memory decoding one image may take.
const MAX_DECODE_BYTES: u64 = 64 * 1024 * 1024;
const MCP_TOOLS: &str = "mcp__";
/// SpecDeck's own browser answers a screenshot with the page's address first.
const OWN_SCREENSHOT: &str = "mcp__specdeck__screenshot";

/// The screenshot outcome for a `PostToolUse` of `tool_name`: `None` unless it is an MCP tool
/// whose result holds an image. With `shown` false (the host turned screenshots off) the outcome
/// says a screenshot was taken, without the image.
pub fn outcome(tool_name: &str, response: &Value, shown: bool) -> Option<ToolOutcome> {
    if !tool_name.starts_with(MCP_TOOLS) {
        return None;
    }
    let blocks = response.as_array().or_else(|| response.get("content")?.as_array())?;
    let (media_type, data) = blocks.iter().find_map(|block| {
        let source = block.get("source")?;
        (block.get("type")? == "image" && source.get("type")? == "base64")
            .then(|| Some((source.get("media_type")?.as_str()?, source.get("data")?.as_str()?)))
            .flatten()
    })?;
    let address = (tool_name == OWN_SCREENSHOT).then(|| address(blocks)).flatten();
    let (image, left_out) = if shown {
        match fit(media_type, data) {
            Ok(image) => (Some(image), None),
            Err(left_out) => (None, Some(left_out)),
        }
    } else {
        (None, Some(LeftOut::TurnedOff))
    };
    Some(ToolOutcome::Screenshot { image, address, left_out })
}

/// The address at the start of the first text block, without its scheme.
fn address(blocks: &[Value]) -> Option<String> {
    let text = blocks.iter().find(|b| b.get("type").is_some_and(|t| t == "text"))?;
    let line = text.get("text")?.as_str()?.lines().next()?;
    let rest = line.strip_prefix("https://").or_else(|| line.strip_prefix("http://"))?;
    (!rest.is_empty()).then(|| rest.to_owned())
}

/// The image as it is sent: as it came when small enough, otherwise made smaller.
fn fit(media_type: &str, data: &str) -> Result<Image, LeftOut> {
    let bytes = STANDARD.decode(data).map_err(|_| LeftOut::Unreadable)?;
    let reader = || {
        let mut reader = ImageReader::new(Cursor::new(&bytes));
        reader.set_format(format(media_type).ok_or(LeftOut::Unreadable)?);
        Ok::<_, LeftOut>(reader)
    };
    let (width, height) = reader()?.into_dimensions().map_err(|_| LeftOut::Unreadable)?;
    if data.len() <= MAX_SCREENSHOT_BASE64 {
        return Ok(Image {
            media_type: media_type.to_owned(),
            data: data.to_owned(),
            width,
            height,
        });
    }
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(LeftOut::TooLarge);
    }
    let mut reader = reader()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|error| match error {
        image::ImageError::Limits(_) => LeftOut::TooLarge,
        _ => LeftOut::Unreadable,
    })?;
    SMALLER
        .iter()
        .find_map(|&(width, quality)| smaller(&decoded, width, quality))
        .ok_or(LeftOut::TooLarge)
}

fn format(media_type: &str) -> Option<ImageFormat> {
    match media_type {
        "image/jpeg" => Some(ImageFormat::Jpeg),
        "image/png" => Some(ImageFormat::Png),
        _ => None,
    }
}

/// `image` at most `width` wide, as JPEG of `quality`, when its base64 fits.
fn smaller(image: &DynamicImage, width: u32, quality: u8) -> Option<Image> {
    // The height bound is the image's own, so only the width scales it, and never up.
    let scaled = image.resize(width, image.height(), image::imageops::FilterType::Triangle);
    let mut jpeg = Vec::new();
    scaled.to_rgb8().write_with_encoder(JpegEncoder::new_with_quality(&mut jpeg, quality)).ok()?;
    let data = STANDARD.encode(jpeg);
    (data.len() <= MAX_SCREENSHOT_BASE64).then(|| Image {
        media_type: "image/jpeg".into(),
        data,
        width: scaled.width(),
        height: scaled.height(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn encoded(image: &DynamicImage, format: ImageFormat) -> String {
        let mut bytes = Vec::new();
        image.write_to(&mut Cursor::new(&mut bytes), format).unwrap();
        STANDARD.encode(bytes)
    }

    fn response(media_type: &str, data: &str) -> Value {
        json!([
            { "type": "text", "text": "http://localhost:5173/login\nSign in" },
            { "type": "image", "source": { "type": "base64", "media_type": media_type, "data": data } },
            { "type": "text", "text": "[Image: source: /home/user/.claude/shot.jpg]" }
        ])
    }

    /// Noise, which JPEG cannot make small.
    fn noise(width: u32, height: u32) -> DynamicImage {
        let mut state = 0x2545_f491_u32;
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |_, _| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            image::Rgb([state as u8, (state >> 8) as u8, (state >> 16) as u8])
        }))
    }

    #[test]
    fn a_small_screenshot_is_kept_as_it_came() {
        let data = encoded(&noise(64, 40), ImageFormat::Jpeg);
        let outcome = outcome(OWN_SCREENSHOT, &response("image/jpeg", &data), true);
        assert_eq!(
            outcome,
            Some(ToolOutcome::Screenshot {
                image: Some(Image { media_type: "image/jpeg".into(), data, width: 64, height: 40 }),
                address: Some("localhost:5173/login".into()),
                left_out: None,
            })
        );
    }

    #[test]
    fn other_servers_get_no_address() {
        let data = encoded(&noise(8, 8), ImageFormat::Png);
        let tool = "mcp__playwright__browser_take_screenshot";
        let Some(ToolOutcome::Screenshot { image, address, .. }) =
            outcome(tool, &response("image/png", &data), true)
        else {
            panic!()
        };
        assert_eq!(image.unwrap().media_type, "image/png");
        assert_eq!(address, None);
    }

    #[test]
    fn only_mcp_tools_with_an_image() {
        let data = encoded(&noise(8, 8), ImageFormat::Png);
        assert_eq!(outcome("Read", &response("image/png", &data), true), None);
        assert_eq!(
            outcome("Task", &json!({ "content": response("image/png", &data) }), true),
            None
        );
        let text_only = json!([{ "type": "text", "text": "Opened http://localhost:5173/" }]);
        assert_eq!(outcome("mcp__specdeck__navigate", &text_only, true), None);
        // The other shape: an object with a `content` array.
        let wrapped = json!({ "content": response("image/png", &data) });
        assert!(outcome("mcp__other__shot", &wrapped, true).is_some());
    }

    #[test]
    fn turned_off_keeps_no_image() {
        let data = encoded(&noise(8, 8), ImageFormat::Jpeg);
        assert_eq!(
            outcome(OWN_SCREENSHOT, &response("image/jpeg", &data), false),
            Some(ToolOutcome::Screenshot {
                image: None,
                address: Some("localhost:5173/login".into()),
                left_out: Some(LeftOut::TurnedOff),
            })
        );
    }

    /// A gradient with a little noise, like a page: large as PNG, small as JPEG.
    fn page(width: u32, height: u32) -> DynamicImage {
        let grain = noise(width, height).to_rgb8();
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| {
            let shade = (x * 200 / width + y * 55 / height) as u8;
            image::Rgb([shade.saturating_add(grain.get_pixel(x, y)[0] % 8), shade, 255 - shade])
        }))
    }

    #[test]
    fn a_large_screenshot_is_made_smaller() {
        let data = encoded(&page(1600, 1000), ImageFormat::Png);
        assert!(data.len() > 480 * 1024);
        let Ok(image) = fit("image/png", &data) else { panic!() };
        assert_eq!(image.media_type, "image/jpeg");
        assert!(image.data.len() <= MAX_SCREENSHOT_BASE64);
        assert!(image.width <= 1280);
        assert_eq!(image.width * 1000, image.height * 1600);
    }

    #[test]
    fn too_large_even_made_smaller() {
        let data = encoded(&noise(1600, 1000), ImageFormat::Png);
        assert_eq!(fit("image/png", &data).err(), Some(LeftOut::TooLarge));
    }

    #[test]
    fn unreadable_images_are_left_out() {
        let data = encoded(&noise(1600, 1000), ImageFormat::Png);
        let cut = &data[..data.len() / 2];
        assert_eq!(fit("image/png", cut).err(), Some(LeftOut::Unreadable));
        assert_eq!(fit("image/png", "not base64!").err(), Some(LeftOut::Unreadable));
        assert_eq!(fit("image/webp", &data).err(), Some(LeftOut::Unreadable));
    }

    #[test]
    fn huge_images_are_not_decoded() {
        // A PNG header claiming 100000×100000 pixels, padded past the size kept as it came.
        let mut png = encoded(&noise(1, 1), ImageFormat::Png).into_bytes();
        let mut bytes = STANDARD.decode(&png).unwrap();
        bytes[16..20].copy_from_slice(&100_000u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&100_000u32.to_be_bytes());
        // The IHDR chunk's CRC covers its type and data, bytes 12..29.
        let crc = crc32(&bytes[12..29]);
        bytes[29..33].copy_from_slice(&crc.to_be_bytes());
        bytes.resize(MAX_SCREENSHOT_BASE64, 0);
        png = STANDARD.encode(bytes).into_bytes();
        let data = String::from_utf8(png).unwrap();
        assert_eq!(fit("image/png", &data).err(), Some(LeftOut::TooLarge));
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }
}
