//! Small, bounded KTX2 fixtures exercise Bevy's actual Zstd image decoder.
//! The malformed case expands only 1025 bytes, never a multi-gigabyte bomb.
use bevy::image::{CompressedImageFormats, TextureError, ktx2_buffer_to_image};

fn rle_ktx2(decoded_size: u32, truncated: bool) -> Vec<u8> {
    assert!((1024..=1025).contains(&decoded_size));
    // Zstandard frame: no content-size field, 1 KiB window, one last RLE block.
    // RFC 8878 requires a decoded block <= min(window, 128 KiB).
    let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0, 0];
    let block = ((decoded_size << 3) | 3).to_le_bytes();
    frame.extend_from_slice(&block[..3]);
    if !truncated {
        frame.push(0x7f);
    }

    let mut file = b"\xabKTX 20\xbb\r\n\x1a\n".to_vec();
    // R8_UNORM; one row/mip/face; Zstandard supercompression; empty DFD.
    for word in [9, 1, decoded_size, 1, 0, 0, 1, 1, 2, 104, 4, 0, 0] {
        file.extend_from_slice(&word.to_le_bytes());
    }
    file.extend_from_slice(&0u64.to_le_bytes()); // SGD offset
    file.extend_from_slice(&0u64.to_le_bytes()); // SGD length
    assert_eq!(file.len(), 80);
    for word in [112u64, frame.len() as u64, u64::from(decoded_size)] {
        file.extend_from_slice(&word.to_le_bytes());
    }
    file.extend_from_slice(&4u32.to_le_bytes()); // DFD total length
    file.extend_from_slice(&[0; 4]); // alignment before level data
    assert_eq!(file.len(), 112);
    file.extend_from_slice(&frame);
    file
}

#[test]
fn zstd_image_valid_block_at_window_limit_decodes_exact_pixels() {
    let image = ktx2_buffer_to_image(&rle_ktx2(1024, false), CompressedImageFormats::NONE, false)
        .expect("valid boundary block must remain supported");
    assert_eq!(image.texture_descriptor.size.width, 1024);
    assert_eq!(image.texture_descriptor.size.height, 1);
    assert_eq!(image.data.as_deref(), Some([0x7f; 1024].as_slice()));
}

#[test]
fn zstd_image_rejects_decoded_block_larger_than_frame_window() {
    let result = ktx2_buffer_to_image(&rle_ktx2(1025, false), CompressedImageFormats::NONE, false);
    let message = match result {
        Err(TextureError::SuperDecompressionError(message)) => message,
        Ok(image) => panic!(
            "malformed block was accepted with {} decoded bytes",
            image.data.as_ref().map_or(0, Vec::len)
        ),
        Err(other) => panic!("expected decompression rejection, got {other:?}"),
    };
    assert!(
        message.contains("1025") && message.contains("1024"),
        "{message}"
    );
}

#[test]
fn zstd_image_truncated_block_remains_an_error() {
    assert!(matches!(
        ktx2_buffer_to_image(&rle_ktx2(1024, true), CompressedImageFormats::NONE, false),
        Err(TextureError::SuperDecompressionError(_))
    ));
}
