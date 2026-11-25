//! Profile-image normalization under the owner-approved contract (2026-11-25).
//!
//! Untrusted bytes are sniffed, checked against the declared type, decoded
//! within strict dimension and memory limits, oriented, flattened onto white,
//! downscaled to at most 512 px and re-encoded as a fresh baseline JPEG. The
//! output carries no EXIF, GPS, ICC, XMP or other source metadata, and only
//! this re-encoded output is ever stored or served.
use deypay_application::profile::{
    ImageNormalizer, ImageRejection, JPEG_QUALITY, MAX_INPUT_SIDE, MAX_OUTPUT_SIDE,
    MAX_STORED_BYTES, NormalizedImage, UploadFormat,
};
use image::{
    DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits, RgbImage,
    codecs::jpeg::JpegEncoder, imageops::FilterType,
};
use std::io::Cursor;

/// Decoder memory budget per image, including the decoded output buffer.
pub const MAX_DECODE_BYTES: u64 = 96 * 1024 * 1024;
/// Quality steps tried, in order, when a complex image exceeds the size cap.
const QUALITY_STEPS: [u8; 4] = [JPEG_QUALITY, 70, 55, 40];

pub struct JpegNormalizer;

fn rejection(error: &ImageError) -> ImageRejection {
    match error {
        ImageError::Limits(_) => ImageRejection::TooManyPixels,
        _ => ImageRejection::Undecodable,
    }
}

fn flatten_onto_white(image: DynamicImage) -> RgbImage {
    if !image.color().has_alpha() {
        return image.into_rgb8();
    }
    let rgba = image.into_rgba8();
    let mut rgb = RgbImage::new(rgba.width(), rgba.height());
    for (target, source) in rgb.pixels_mut().zip(rgba.pixels()) {
        let alpha = u16::from(source[3]);
        for channel in 0..3 {
            let value = u16::from(source[channel]) * alpha + 255 * (255 - alpha);
            target[channel] = ((value + 127) / 255) as u8;
        }
    }
    rgb
}

impl ImageNormalizer for JpegNormalizer {
    fn normalize(
        &self,
        declared: UploadFormat,
        bytes: &[u8],
    ) -> Result<NormalizedImage, ImageRejection> {
        let expected = match declared {
            UploadFormat::Jpeg => ImageFormat::Jpeg,
            UploadFormat::Png => ImageFormat::Png,
            UploadFormat::WebP => ImageFormat::WebP,
        };
        // The content, not the client's label, decides the decoder.
        if image::guess_format(bytes).ok() != Some(expected) {
            return Err(ImageRejection::TypeMismatch);
        }
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_INPUT_SIDE);
        limits.max_image_height = Some(MAX_INPUT_SIDE);
        limits.max_alloc = Some(MAX_DECODE_BYTES);
        let mut reader = ImageReader::with_format(Cursor::new(bytes), expected);
        reader.limits(limits);
        let mut decoder = reader.into_decoder().map_err(|e| rejection(&e))?;
        if decoder.total_bytes() > MAX_DECODE_BYTES {
            return Err(ImageRejection::TooManyPixels);
        }
        let orientation = decoder.orientation().map_err(|e| rejection(&e))?;
        let mut image = DynamicImage::from_decoder(decoder).map_err(|e| rejection(&e))?;
        image.apply_orientation(orientation);
        if image.width().max(image.height()) > MAX_OUTPUT_SIDE {
            image = image.resize(MAX_OUTPUT_SIDE, MAX_OUTPUT_SIDE, FilterType::Lanczos3);
        }
        let rgb = flatten_onto_white(image);
        let (width, height) = rgb.dimensions();
        for quality in QUALITY_STEPS {
            let mut jpeg = Vec::new();
            JpegEncoder::new_with_quality(&mut jpeg, quality)
                .encode_image(&rgb)
                .map_err(|_| ImageRejection::Undecodable)?;
            if jpeg.len() <= MAX_STORED_BYTES {
                return NormalizedImage::new(jpeg, width, height)
                    .ok_or(ImageRejection::Undecodable);
            }
        }
        Err(ImageRejection::TooComplex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb, Rgba};

    fn encode(image: DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, format).unwrap();
        out.into_inner()
    }
    fn gradient(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(width, height, |x, y| {
            Rgb([(x % 256) as u8, (y % 256) as u8, 96])
        }))
    }
    fn decode(jpeg: &[u8]) -> DynamicImage {
        image::load_from_memory_with_format(jpeg, ImageFormat::Jpeg).unwrap()
    }
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffff_u32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(&body);
        out.extend(crc32(&body).to_be_bytes());
        out
    }
    /// A PNG that only declares its size: no pixel data follows.
    fn png_header(width: u32, height: u32, bit_depth: u8, color_type: u8) -> Vec<u8> {
        let mut ihdr = width.to_be_bytes().to_vec();
        ihdr.extend(height.to_be_bytes());
        ihdr.extend([bit_depth, color_type, 0, 0, 0]);
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(chunk(b"IHDR", &ihdr));
        png.extend(chunk(b"IDAT", &[]));
        png.extend(chunk(b"IEND", &[]));
        png
    }
    /// Inserts an EXIF orientation and a comment carrying a private marker.
    fn with_metadata(jpeg: &[u8], orientation: u8) -> Vec<u8> {
        let tiff = [
            b'M',
            b'M',
            0,
            42,
            0,
            0,
            0,
            8,
            0,
            1,
            0x01,
            0x12,
            0,
            3,
            0,
            0,
            0,
            1,
            0,
            orientation,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let mut exif = b"Exif\0\0".to_vec();
        exif.extend_from_slice(&tiff);
        let mut out = jpeg[..2].to_vec();
        out.extend([0xff, 0xe1]);
        out.extend(((exif.len() + 2) as u16).to_be_bytes());
        out.extend(exif);
        let comment = b"GPS 6.5244N 3.3792E private-marker";
        out.extend([0xff, 0xfe]);
        out.extend(((comment.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(comment);
        out.extend_from_slice(&jpeg[2..]);
        out
    }
    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn photos_are_oriented_downscaled_and_stripped_of_metadata() {
        let source = with_metadata(&encode(gradient(1200, 600), ImageFormat::Jpeg), 6);
        assert!(contains(&source, b"Exif") && contains(&source, b"private-marker"));
        let image = JpegNormalizer
            .normalize(UploadFormat::Jpeg, &source)
            .unwrap();
        // Orientation 6 rotates the landscape source to portrait before scaling.
        assert_eq!((image.width(), image.height()), (256, 512));
        assert!(image.jpeg().len() <= MAX_STORED_BYTES);
        for marker in [&b"Exif"[..], b"private-marker", b"GPS"] {
            assert!(!contains(image.jpeg(), marker));
        }
        let decoded = decode(image.jpeg());
        assert_eq!((decoded.width(), decoded.height()), (256, 512));
    }

    #[test]
    fn small_images_are_not_enlarged_and_transparency_becomes_white() {
        let transparent = DynamicImage::ImageRgba8(ImageBuffer::from_fn(40, 20, |x, _| {
            if x < 20 {
                Rgba([0, 0, 0, 0])
            } else {
                Rgba([0, 0, 0, 255])
            }
        }));
        for format in [ImageFormat::Png, ImageFormat::WebP] {
            let declared = if format == ImageFormat::Png {
                UploadFormat::Png
            } else {
                UploadFormat::WebP
            };
            let image = JpegNormalizer
                .normalize(declared, &encode(transparent.clone(), format))
                .unwrap();
            assert_eq!((image.width(), image.height()), (40, 20));
            let decoded = decode(image.jpeg()).into_rgb8();
            assert!(
                decoded.get_pixel(5, 10).0.iter().all(|&c| c >= 245),
                "{format:?}"
            );
            assert!(
                decoded.get_pixel(35, 10).0.iter().all(|&c| c <= 10),
                "{format:?}"
            );
        }
    }

    #[test]
    fn the_declared_type_must_match_the_content() {
        let png = encode(gradient(8, 8), ImageFormat::Png);
        let jpeg = encode(gradient(8, 8), ImageFormat::Jpeg);
        for (declared, bytes) in [
            (UploadFormat::Jpeg, png.as_slice()),
            (UploadFormat::WebP, png.as_slice()),
            (UploadFormat::Png, jpeg.as_slice()),
            (
                UploadFormat::Png,
                b"<svg xmlns='http://www.w3.org/2000/svg'/>".as_slice(),
            ),
            (
                UploadFormat::Jpeg,
                b"<html><script>alert(1)</script></html>".as_slice(),
            ),
            (UploadFormat::Jpeg, b"".as_slice()),
        ] {
            assert_eq!(
                JpegNormalizer.normalize(declared, bytes),
                Err(ImageRejection::TypeMismatch)
            );
        }
    }

    #[test]
    fn oversized_and_bomb_headers_are_refused_before_decoding() {
        for (width, height, depth, color) in [
            (MAX_INPUT_SIDE + 1, 1, 8, 2),
            (1, MAX_INPUT_SIDE + 1, 8, 2),
            (100_000, 100_000, 8, 6),
            // Within the side limit, but 128 MiB of 16-bit RGBA exceeds the budget.
            (MAX_INPUT_SIDE, MAX_INPUT_SIDE, 16, 6),
        ] {
            assert_eq!(
                JpegNormalizer
                    .normalize(UploadFormat::Png, &png_header(width, height, depth, color)),
                Err(ImageRejection::TooManyPixels),
                "{width}x{height}"
            );
        }
        // The largest permitted side is accepted.
        let edge = encode(gradient(MAX_INPUT_SIDE, 2), ImageFormat::Png);
        let image = JpegNormalizer.normalize(UploadFormat::Png, &edge).unwrap();
        assert_eq!((image.width(), image.height()), (512, 1));
    }

    #[test]
    fn malformed_content_is_undecodable() {
        let png = encode(gradient(64, 64), ImageFormat::Png);
        let mut corrupt = png.clone();
        let middle = corrupt.len() / 2;
        corrupt[middle..].iter_mut().for_each(|b| *b ^= 0x5a);
        for bytes in [&png[..png.len() / 2], &corrupt[..], &png_header(4, 4, 8, 2)] {
            assert_eq!(
                JpegNormalizer.normalize(UploadFormat::Png, bytes),
                Err(ImageRejection::Undecodable)
            );
        }
        let garbage = [0xff, 0xd8, 0xff, 0xe0, 0x00, 0x02, 0x13, 0x37];
        assert_eq!(
            JpegNormalizer.normalize(UploadFormat::Jpeg, &garbage),
            Err(ImageRejection::Undecodable)
        );
    }

    #[test]
    fn noisy_images_meet_the_size_cap_or_are_refused() {
        let mut state = 0x2545_f491_u32;
        let noise = DynamicImage::ImageRgb8(ImageBuffer::from_fn(512, 512, |_, _| {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            };
            Rgb([next(), next(), next()])
        }));
        let png = encode(noise, ImageFormat::Png);
        match JpegNormalizer.normalize(UploadFormat::Png, &png) {
            Ok(image) => assert!(image.jpeg().len() <= MAX_STORED_BYTES),
            Err(rejection) => assert_eq!(rejection, ImageRejection::TooComplex),
        }
    }
}
