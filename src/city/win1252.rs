//! Windows-1252 single-byte labels for the Western Win95 city format.
//!
//! Other Windows ANSI code pages depend on the original system locale and are
//! not yet identified by the save file.

const HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

pub(crate) fn decode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| match byte {
            0x80..=0x9f => HIGH[usize::from(byte - 0x80)],
            _ => char::from(byte),
        })
        .collect()
}

fn encode_char(ch: char) -> Option<u8> {
    match ch as u32 {
        0x00..=0x7f | 0xa0..=0xff => Some(ch as u8),
        _ => HIGH
            .iter()
            .position(|&value| value == ch)
            .map(|i| 0x80 + i as u8),
    }
}

pub(crate) fn can_encode(text: &str) -> bool {
    text.chars()
        .take_while(|&ch| ch != '\0')
        .all(|ch| encode_char(ch).is_some())
}

pub(crate) fn encode(text: &str, limit: usize) -> Vec<u8> {
    text.chars()
        .take_while(|&ch| ch != '\0')
        .take(limit)
        .map(|ch| encode_char(ch).unwrap_or(b'?'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_single_bytes_round_trip_and_unsupported_characters_fall_back() {
        let bytes: Vec<u8> = (1..=255).collect();
        assert_eq!(encode(&decode(&bytes), 255), bytes);
        assert_eq!(encode("Café – € ☃", 23), b"Caf\xe9 \x96 \x80 ?");
        assert_eq!(encode("ab\0cd", 23), b"ab");
    }
}
