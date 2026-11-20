//! MDRV per-voice service for the terminal envelope used by this build.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MacVoiceTail {
    pub position: u32,
    pub step: u32,
    pub fraction_words: u32,
    pub volume_words: u32,
    pub volume_table: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MacTerminalVoiceService {
    pub status: i16,
    pub release_timer: u16,
    pub tail_flag: u16,
    pub position: u32,
    pub last: u32,
    pub step: u32,
    pub fraction_words: u32,
    pub volume_words: u32,
    pub volume_table: u32,
    pub cached_distance: u32,
    pub tail: MacVoiceTail,
}

/// MDRV 0x324e..0x329c, immediately before replacing a selected voice.
pub(crate) fn mac_prepare_voice_replacement(
    voice: &mut MacTerminalVoiceService,
    driver_flags: u16,
) {
    if driver_flags & 0x0200 != 0 {
        return;
    }
    voice.tail_flag = 1;
    let remaining = voice.last.wrapping_sub(voice.position);
    if voice.status >= 0
        && voice.cached_distance != 0
        && (remaining.wrapping_sub(voice.cached_distance) as i32) > 0
    {
        voice.tail_flag = 0xff01;
        voice.tail = MacVoiceTail {
            position: voice.position,
            step: voice.step,
            fraction_words: voice.fraction_words,
            volume_words: voice.volume_words,
            volume_table: voice.volume_table,
        };
    }
}

/// MDRV 0x1eb4..0x1f5c with the terminal 0x8000 envelope word.
/// Only nonnegative-status slots are admitted by the caller's scan.
pub(crate) fn mac_service_terminal_voice(
    voice: &mut MacTerminalVoiceService,
    driver_flags: u16,
) -> Result<(), &'static str> {
    if voice.status < 0 {
        return Err("inactive voice cannot be serviced");
    }
    if voice.status == 0 && voice.tail_flag == 0 {
        if voice.release_timer != 0 {
            voice.release_timer = voice.release_timer.wrapping_sub(1);
            return Ok(());
        }
        voice.status = -1;
        let remaining = voice.last.wrapping_sub(voice.position);
        if driver_flags & 0x0200 == 0
            && voice.cached_distance != 0
            && (remaining.wrapping_sub(voice.cached_distance) as i32) > 0
        {
            voice.tail_flag = 0xff00 | (voice.tail_flag & 0xff);
            voice.tail = MacVoiceTail {
                position: voice.position,
                step: voice.step,
                fraction_words: voice.fraction_words,
                volume_words: voice.volume_words,
                volume_table: voice.volume_table,
            };
        }
    }
    if voice.tail_flag == 0 {
        voice.status = voice.status.wrapping_sub(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_timer_and_tail_capture_follow_service_order() {
        let mut voice = MacTerminalVoiceService {
            status: 0,
            release_timer: 1,
            tail_flag: 0,
            position: 10,
            last: 100,
            step: 0x10000,
            fraction_words: 0,
            volume_words: 0,
            volume_table: 0,
            cached_distance: 20,
            tail: MacVoiceTail::default(),
        };
        mac_service_terminal_voice(&mut voice, 0).unwrap();
        assert_eq!(
            (voice.status, voice.release_timer, voice.tail_flag),
            (0, 0, 0)
        );
        mac_service_terminal_voice(&mut voice, 0).unwrap();
        assert_eq!(
            (voice.status, voice.tail_flag, voice.tail.position),
            (-1, 0xff00, 10)
        );
    }

    #[test]
    #[ignore = "requires original-instruction voice replacement fixture"]
    fn replacement_tail_matches_original_instructions() {
        let data = std::fs::read(std::env::var_os("CITYGAME_MAC_VOICE_REPLACE").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&data).unwrap();
        assert_eq!(rows.len(), 144);
        for row in rows {
            let n = |k: &str| row[k].as_u64().unwrap() as u32;
            let mut v = MacTerminalVoiceService {
                status: row["status"].as_i64().unwrap() as i16,
                release_timer: 0,
                tail_flag: n("tail") as u16,
                position: 0x40000,
                last: 0x40000 + n("last"),
                step: 0x12345678,
                fraction_words: 0x13572468,
                volume_words: 0x24681357,
                volume_table: 0x3456789a,
                cached_distance: n("cache"),
                tail: MacVoiceTail::default(),
            };
            mac_prepare_voice_replacement(&mut v, n("flags") as u16);
            let expected: Vec<u32> = row["shadow"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as u32)
                .collect();
            assert_eq!(v.tail_flag, n("new_tail") as u16, "{row}");
            assert_eq!(
                vec![
                    v.tail.position,
                    v.tail.step,
                    v.tail.fraction_words,
                    v.tail.volume_words,
                    v.tail.volume_table
                ],
                expected,
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires original-instruction terminal voice service fixture"]
    fn terminal_service_matches_original_instructions() {
        let data =
            std::fs::read(std::env::var_os("CITYGAME_MAC_TERMINAL_SERVICE").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&data).unwrap();
        assert_eq!(rows.len(), 192);
        for row in rows {
            let n = |k: &str| row[k].as_u64().unwrap() as u32;
            let mut v = MacTerminalVoiceService {
                status: n("status") as i16,
                release_timer: n("timer") as u16,
                tail_flag: n("tail") as u16,
                position: 0x40000,
                last: 0x40032,
                step: 0x12345678,
                fraction_words: 0x13572468,
                volume_words: 0x24681357,
                volume_table: 0x3456789a,
                cached_distance: n("cache"),
                tail: MacVoiceTail::default(),
            };
            mac_service_terminal_voice(&mut v, n("flags") as u16).unwrap();
            let t = row["shadow"].as_array().unwrap();
            let expected: Vec<u32> = t.iter().map(|x| x.as_u64().unwrap() as u32).collect();
            assert_eq!(
                (v.status as u16, v.release_timer, v.tail_flag),
                (
                    n("new_status") as u16,
                    n("new_timer") as u16,
                    n("new_tail") as u16
                ),
                "{row}"
            );
            assert_eq!(
                vec![
                    v.tail.position,
                    v.tail.step,
                    v.tail.fraction_words,
                    v.tail.volume_words,
                    v.tail.volume_table
                ],
                expected,
                "{row}"
            );
            assert_eq!(n("envelope_pointer"), 0, "{row}");
        }
    }
}
