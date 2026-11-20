//! Checked preparation of voices from one imported Mac music pack.
use crate::mac_midi::{
    MacInstrumentMetadata, MacMidiPitch, MacMidiVoiceRequest, MacSongMetadata,
    mac_midi_initial_pitch, mac_midi_pitch_table, mac_midi_sample_bounds,
    mac_midi_sample_data_offset,
};
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub(crate) struct MacMidiBank {
    root: std::path::PathBuf,
    pub song: MacSongMetadata,
    pitch: Vec<u32>,
}

#[derive(Debug)]
pub(crate) struct MacPreparedVoice {
    pub instrument: u16,
    pub sound_id: u16,
    pub instrument_flags: u16,
    pub adjusted_key: u16,
    pub pitch: MacMidiPitch,
    pub terminal_envelope: bool,
    pub samples: Vec<u8>,
    pub loop_range: Option<std::ops::Range<usize>>,
}

impl MacMidiBank {
    pub(crate) fn load(root: &Path, song_id: u16) -> Result<Self> {
        let song_data =
            std::fs::read(root.join(format!("SONG/{song_id}.bin"))).context("reading Mac SONG")?;
        let song = MacSongMetadata::parse(&song_data).map_err(anyhow::Error::msg)?;
        let pitch_data = std::fs::read(root.join("MDRV/11-pitch.bin"))
            .context("reading Mac MDRV pitch table")?;
        let pitch = mac_midi_pitch_table(&pitch_data).map_err(anyhow::Error::msg)?;
        Ok(Self {
            root: root.to_owned(),
            song,
            pitch,
        })
    }

    pub(crate) fn prepare_voice(&self, request: &MacMidiVoiceRequest) -> Result<MacPreparedVoice> {
        ensure!(request.on, "cannot prepare a note-off as a voice");
        let inst_data = std::fs::read(self.root.join(format!("INST/{}.bin", request.instrument)))
            .context("reading Mac INST")?;
        let inst = MacInstrumentMetadata::parse(&inst_data).map_err(anyhow::Error::msg)?;
        ensure!(
            inst.region_count == 0,
            "Mac instrument regions are not yet supported"
        );
        ensure!(
            inst.flags & 0x0200 == 0,
            "Mac random pitch modulation is not yet supported"
        );
        let sound = std::fs::read(self.root.join(format!("snd/{}.bin", inst.sound_id)))
            .context("reading Mac instrument sound")?;
        // The verified instrument bank uses a format-1 sound with one modifier,
        // one buffer command and the standard uncompressed sound header.
        ensure!(
            sound.get(..4) == Some(&[0, 1, 0, 1]) && sound.get(10..14) == Some(&[0, 1, 0x80, 0x51]),
            "unsupported Mac instrument sound layout"
        );
        let first = mac_midi_sample_data_offset(&sound).map_err(anyhow::Error::msg)?;
        let h = first.checked_sub(22).context("invalid Mac sound header")?;
        ensure!(
            sound.get(h + 20) == Some(&0),
            "unsupported Mac instrument sound encoding"
        );
        let long = |at: usize| -> u32 { u32::from_be_bytes(sound[at..at + 4].try_into().unwrap()) };
        let length = long(h + 4);
        ensure!(length > 0, "empty Mac instrument sample");
        let end = first
            .checked_add(length as usize)
            .context("Mac sample length overflow")?;
        ensure!(end <= sound.len(), "short Mac instrument sample");
        let bounds = mac_midi_sample_bounds(length, long(h + 12), long(h + 16), inst.flags as u8);
        let loop_range = match (bounds.loop_start, bounds.loop_end) {
            (Some(start), Some(end)) => {
                ensure!(
                    start < end && end <= length,
                    "Mac instrument loop outside sample"
                );
                Some(start as usize..end as usize)
            }
            (None, None) => None,
            _ => anyhow::bail!("invalid Mac instrument loop"),
        };
        let adjusted_key = inst.adjusted_key(request.key);
        let pitch_word = u16::from_be_bytes([sound[h + 20], sound[h + 21]]);
        let pitch = mac_midi_initial_pitch(adjusted_key, pitch_word, inst.flags, &self.pitch, 0)
            .context("Mac pitch outside imported table")?;
        Ok(MacPreparedVoice {
            instrument: request.instrument,
            sound_id: inst.sound_id,
            instrument_flags: inst.flags,
            adjusted_key,
            pitch,
            terminal_envelope: inst_data.get(16..18) == Some(&[0x80, 0x00]),
            samples: sound[first..end].to_vec(),
            loop_range,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_truncated_and_wrong_sounds() {
        let root =
            std::env::temp_dir().join(format!("citygame-bank-invalid-{}", std::process::id()));
        for kind in ["SONG", "INST", "snd", "MDRV"] {
            std::fs::create_dir_all(root.join(kind)).unwrap();
        }
        std::fs::write(root.join("SONG/10018.bin"), [0_u8; 18]).unwrap();
        std::fs::write(root.join("MDRV/11-pitch.bin"), [0_u8; 132 * 4]).unwrap();
        let mut inst = [0_u8; 22];
        inst[1] = 1;
        std::fs::write(root.join("INST/1.bin"), inst).unwrap();
        std::fs::write(root.join("snd/1.bin"), [0_u8; 42]).unwrap();
        let bank = MacMidiBank::load(&root, 10018).unwrap();
        let request = MacMidiVoiceRequest {
            on: true,
            instrument: 1,
            key: 60,
            velocity: 80,
            channel: 0,
            track_tag: 0,
            priority: Some(10_000),
        };
        assert!(bank.prepare_voice(&request).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires user Mac resources and original-instruction bank fixture"]
    fn imported_bank_prepares_original_instrument_voices() {
        let source =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_RESOURCE_ROOT").unwrap());
        let pack =
            std::env::temp_dir().join(format!("citygame-bank-original-{}", std::process::id()));
        crate::audio_import::import_mac(&source, &pack).unwrap();
        let bank = MacMidiBank::load(&pack.join("catalog/mac-audio"), 10018).unwrap();
        assert_eq!(bank.song.midi_id, 10018);
        let fixture =
            std::fs::read(std::env::var_os("CITYGAME_MAC_BANK_FIXTURE").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&fixture).unwrap();
        assert_eq!(rows.len(), 51);
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as u16;
            let request = MacMidiVoiceRequest {
                on: true,
                instrument: n("instrument"),
                key: n("key"),
                velocity: 80,
                channel: 0,
                track_tag: 0,
                priority: Some(10_000),
            };
            let voice = bank.prepare_voice(&request).unwrap();
            assert!(voice.terminal_envelope, "{row}");
            assert_eq!(
                (
                    voice.instrument,
                    voice.sound_id,
                    voice.adjusted_key,
                    voice.pitch.step,
                    voice.samples.len() - 1
                ),
                (
                    n("instrument"),
                    n("sound"),
                    n("adjusted"),
                    row["step"].as_u64().unwrap() as u32,
                    row["last"].as_u64().unwrap() as usize
                ),
                "{row}"
            );
            let expected = row["loop_start"]
                .as_u64()
                .zip(row["loop_end"].as_u64())
                .map(|(s, e)| s as usize..e as usize);
            assert_eq!(voice.loop_range, expected, "{row}");
        }
        std::fs::remove_dir_all(pack).unwrap();
    }
}
