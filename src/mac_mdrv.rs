//! Decoder for user-supplied SoundMusicSys MDRV resources.
//!
//! Adapted from resource_dasm's SoundMusicSys decoder, copyright Martin
//! Michelsen, MIT license: see third_party/resource_dasm-MIT.txt.

use sha2::{Digest, Sha256};

const MAX_DRIVER_SIZE: usize = 16 * 1024 * 1024;
const VERIFIED_DRIVER_SHA256: [u8; 32] = [
    0xe8, 0xdd, 0xed, 0xd9, 0xd8, 0xbb, 0x38, 0x25, 0x62, 0xf1, 0xa2, 0xe0, 0xa0, 0x71, 0xc1, 0x82,
    0xfc, 0x59, 0x6b, 0x9c, 0xa9, 0xc1, 0xc3, 0x7d, 0x43, 0x9e, 0x1d, 0x8f, 0xea, 0xd8, 0xd2, 0x82,
];

pub(crate) fn decode_mdrv(data: &[u8]) -> Result<Vec<u8>, &'static str> {
    if data.len() < 7 {
        return Err("short MDRV resource");
    }
    let size = (u32::from_be_bytes(data[..4].try_into().unwrap()) & 0x00ff_ffff) as usize;
    if size > MAX_DRIVER_SIZE {
        return Err("MDRV resource too large");
    }
    let mut decoded = data.to_vec();
    let mut key = 56549_u32;
    for byte in &mut decoded[7..] {
        let cipher = *byte;
        *byte ^= (key >> 8) as u8;
        key = (u32::from(cipher).wrapping_add(key))
            .wrapping_mul(52845)
            .wrapping_add(22719);
    }
    let mut output = Vec::with_capacity(size);
    let mut at = 4;
    while at < decoded.len() {
        let control = decoded[at];
        at += 1;
        for bit in 0..8 {
            if at >= decoded.len() {
                break;
            }
            if control & (1 << bit) != 0 {
                output.push(decoded[at]);
                at += 1;
            } else {
                if at + 1 >= decoded.len() {
                    at = decoded.len();
                    break;
                }
                let params = u16::from_be_bytes([decoded[at], decoded[at + 1]]);
                at += 2;
                let distance = 0x1000 - usize::from(params & 0x0fff);
                let mut start = output
                    .len()
                    .checked_sub(distance)
                    .ok_or("invalid MDRV backreference")?;
                for _ in 0..(usize::from(params >> 12) + 3) {
                    let byte = *output.get(start).ok_or("invalid MDRV backreference")?;
                    output.push(byte);
                    start += 1;
                }
            }
            if output.len() > size {
                return Err("MDRV output exceeds declared size");
            }
        }
    }
    if output.len() != size {
        return Err("MDRV output length mismatch");
    }
    Ok(output)
}

/// The table offset is valid only for the exact, privately verified driver.
pub(crate) fn verified_pitch_table(driver: &[u8]) -> Option<&[u8]> {
    if Sha256::digest(driver).as_slice() != VERIFIED_DRIVER_SHA256 {
        return None;
    }
    driver.get(0x64a4..0x64a4 + 132 * 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_drivers_are_rejected() {
        assert!(decode_mdrv(&[0; 6]).is_err());
        assert!(decode_mdrv(&[0, 0, 0, 3, 0, 0, 0]).is_err());
        assert!(verified_pitch_table(&[0; 32]).is_none());
    }

    #[test]
    fn decrypts_and_expands_overlapping_backreferences() {
        // Three literals followed by a six-byte copy at distance three.
        let mut encoded = vec![0, 0, 0, 9, 0x07, b'A', b'B', b'C', 0x3f, 0xfd];
        let mut key = 56549_u32;
        for byte in &mut encoded[7..] {
            let cipher = *byte ^ (key >> 8) as u8;
            *byte = cipher;
            key = (u32::from(cipher).wrapping_add(key))
                .wrapping_mul(52845)
                .wrapping_add(22719);
        }
        assert_eq!(decode_mdrv(&encoded).unwrap(), b"ABCABCABC");
    }

    #[test]
    #[ignore = "requires original Mac MDRV resource and decoded reference"]
    fn original_driver_decodes_byte_for_byte() {
        let compressed = std::fs::read(std::env::var_os("CITYGAME_MAC_MDRV").unwrap()).unwrap();
        let reference =
            std::fs::read(std::env::var_os("CITYGAME_MAC_MDRV_DECODED").unwrap()).unwrap();
        let decoded = decode_mdrv(&compressed).unwrap();
        assert_eq!(decoded, reference);
        assert_eq!(
            verified_pitch_table(&decoded).unwrap(),
            &reference[0x64a4..0x64a4 + 132 * 4]
        );
    }
}
