//! Controlled composition of the verified Mac sequencer, bank and mixer.
//! This is not yet a real-time audio backend.
use crate::mac_midi::{
    MacMidiDriverEvent, MacMidiPrograms, MacMidiSequencer, MacMidiVoice, mac_midi_release_voices,
    mac_midi_select_voice,
};
use crate::mac_midi_bank::{MacMidiBank, MacPreparedVoice};
use crate::mac_midi_mix::{
    MacEndingVoice, MacOutputMode, MacPendingTail, MacRefillVoice, MacTerminalRefillVoice,
    mac_capture_replacement_tail, mac_mix_terminal_refill_configured,
};
use anyhow::{Context, Result, ensure};

struct OwnedTail {
    flag: u16,
    cursor: MacEndingVoice,
    samples: Vec<u8>,
}
struct Slot {
    identity: MacMidiVoice,
    prepared: Option<MacPreparedVoice>,
    cursor: MacEndingVoice,
    cache: u32,
    pending: Option<OwnedTail>,
}

pub(crate) struct MacMidiEngine {
    bank: MacMidiBank,
    sequencer: MacMidiSequencer,
    programs: MacMidiPrograms,
    slots: Vec<Slot>,
    flags: u16,
    active_limit: u16,
    divisor: u16,
    track_base: u16,
    high_water: u16,
    output_mode: MacOutputMode,
    inactive_secondary: usize,
}

impl MacMidiEngine {
    pub(crate) fn active(&self) -> bool {
        self.sequencer.active()
    }

    /// Continue mixing after the tracks end while a voice still needs service.
    pub(crate) fn has_audio_work(&self) -> bool {
        self.active() || self.slots.iter().any(|slot| slot.identity.status >= 0)
    }
    pub(crate) fn new(
        bank: MacMidiBank,
        midi: &[u8],
        voice_count: usize,
        flags: u16,
        active_limit: u16,
        divisor: u16,
        track_base: u16,
    ) -> Result<Self> {
        Self::new_configured(
            bank,
            midi,
            voice_count,
            flags,
            active_limit,
            divisor,
            track_base,
            0,
            MacOutputMode::Interpolated,
        )
    }

    pub(crate) fn new_configured(
        bank: MacMidiBank,
        midi: &[u8],
        voice_count: usize,
        flags: u16,
        active_limit: u16,
        divisor: u16,
        track_base: u16,
        inactive_secondary: usize,
        output_mode: MacOutputMode,
    ) -> Result<Self> {
        ensure!(
            (1..=16).contains(&voice_count),
            "unsupported Mac MIDI voice count"
        );
        ensure!(
            divisor > 0 && usize::from(divisor) <= voice_count,
            "unsupported Mac MIDI mixer divisor"
        );
        ensure!(
            voice_count + inactive_secondary <= 16,
            "too many Mac mixer slots"
        );
        let sequencer = MacMidiSequencer::new(midi).map_err(anyhow::Error::msg)?;
        let slots = (0..voice_count)
            .map(|_| Slot {
                identity: MacMidiVoice {
                    status: -1,
                    track_tag: 0,
                    channel: 0,
                    key: 0,
                    instrument: 0,
                    release_flags: 0,
                },
                prepared: None,
                cursor: MacEndingVoice {
                    position: 0,
                    fraction: 0,
                    integer_step: 0,
                    fractional_step: 0,
                },
                cache: 0,
                pending: None,
            })
            .collect();
        Ok(Self {
            bank,
            sequencer,
            programs: MacMidiPrograms::default(),
            slots,
            flags,
            active_limit,
            divisor,
            track_base,
            high_water: 0,
            output_mode,
            inactive_secondary,
        })
    }

    pub(crate) fn handle_event(&mut self, event: MacMidiDriverEvent) -> Result<()> {
        let Some(request) =
            self.programs
                .handle_event(event, self.flags, &self.bank.song, self.track_base)
        else {
            return Ok(());
        };
        if !request.on {
            let mut ids: Vec<_> = self.slots.iter().map(|v| v.identity).collect();
            mac_midi_release_voices(&mut ids, &request, self.flags);
            for (slot, id) in self.slots.iter_mut().zip(ids) {
                slot.identity = id;
            }
            return Ok(());
        }
        let prepared = self.bank.prepare_voice(&request)?;
        ensure!(
            prepared.terminal_envelope,
            "unsupported Mac instrument envelope"
        );
        let ids: Vec<_> = self.slots.iter().map(|v| v.identity).collect();
        let Some(index) = mac_midi_select_voice(
            &ids,
            &request,
            prepared.instrument_flags,
            self.flags,
            self.active_limit,
        ) else {
            return Ok(());
        };
        let slot = &mut self.slots[index];
        let pending = if self.flags & 0x0200 != 0 {
            slot.pending.take()
        } else {
            let old = MacRefillVoice {
                status: slot.identity.status,
                cursor: slot.cursor,
                cached_distance: slot.cache,
                samples: slot.prepared.as_ref().map_or(&[], |p| p.samples.as_slice()),
                loop_range: slot.prepared.as_ref().and_then(|p| p.loop_range.clone()),
            };
            mac_capture_replacement_tail(&old, self.flags)
                .map_err(anyhow::Error::msg)?
                .map(|tail| OwnedTail {
                    flag: tail.flag,
                    cursor: tail.cursor,
                    samples: tail.samples.to_vec(),
                })
        };
        slot.identity = MacMidiVoice {
            status: request.priority.unwrap_or(0) as i16,
            track_tag: request.track_tag,
            channel: u16::from(request.channel),
            key: request.key,
            instrument: request.instrument,
            release_flags: self.flags & 0x3f,
        };
        slot.cursor = MacEndingVoice {
            position: 0,
            fraction: 0,
            integer_step: (prepared.pitch.step >> 16) as u16,
            fractional_step: prepared.pitch.step as u16,
        };
        slot.prepared = Some(prepared);
        slot.cache = 0;
        slot.pending = pending;
        Ok(())
    }

    /// Mirrors the observed callback order: mix the current voices, then
    /// service MIDI events for the next buffer. Callback timing is external.
    pub(crate) fn refill(&mut self) -> Result<Vec<u8>> {
        let mut borrowed: Vec<_> = self
            .slots
            .iter()
            .map(|slot| MacTerminalRefillVoice {
                voice: MacRefillVoice {
                    status: slot.identity.status,
                    cursor: slot.cursor,
                    cached_distance: slot.cache,
                    samples: slot.prepared.as_ref().map_or(&[], |p| p.samples.as_slice()),
                    loop_range: slot.prepared.as_ref().and_then(|p| p.loop_range.clone()),
                },
                release_timer: slot.identity.release_flags,
                pending_tail: slot.pending.as_ref().map(|p| MacPendingTail {
                    flag: p.flag,
                    cursor: p.cursor,
                    samples: &p.samples,
                }),
            })
            .collect();
        let output = mac_mix_terminal_refill_configured(
            &mut borrowed,
            self.flags,
            self.divisor,
            self.inactive_secondary,
            self.output_mode,
            &mut self.high_water,
        )
        .map_err(anyhow::Error::msg)?;
        let updates: Vec<_> = borrowed
            .iter()
            .map(|v| {
                (
                    v.voice.status,
                    v.voice.cursor,
                    v.voice.cached_distance,
                    v.release_timer,
                    v.pending_tail.is_none(),
                )
            })
            .collect();
        drop(borrowed);
        for (slot, (status, cursor, cache, timer, consumed)) in self.slots.iter_mut().zip(updates) {
            slot.identity.status = status;
            slot.identity.release_flags = timer;
            slot.cursor = cursor;
            slot.cache = cache;
            if consumed {
                slot.pending = None;
            }
        }
        let mut events = Vec::new();
        self.sequencer
            .service_with_driver_events(|event| events.push(event))
            .map_err(anyhow::Error::msg)
            .context("servicing Mac MIDI")?;
        for event in events {
            self.handle_event(event)?;
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mac_midi::{MacMidiDriverEvent, MacMidiEvent};

    #[test]
    fn program_note_release_and_replacement_drive_refills() {
        let root = std::env::temp_dir().join(format!("citygame-engine-{}", std::process::id()));
        for kind in ["SONG", "INST", "snd", "MDRV"] {
            std::fs::create_dir_all(root.join(kind)).unwrap();
        }
        let mut song = vec![0_u8; 22];
        song[1] = 1;
        song[17] = 1;
        song[19] = 3;
        song[21] = 1;
        std::fs::write(root.join("SONG/1.bin"), song).unwrap();
        let mut pitch = Vec::new();
        for _ in 0..132 {
            pitch.extend_from_slice(&0x10000_u32.to_be_bytes());
        }
        std::fs::write(root.join("MDRV/11-pitch.bin"), pitch).unwrap();
        let mut inst = [0_u8; 22];
        inst[0..2].copy_from_slice(&10001_u16.to_be_bytes());
        inst[16] = 0x80;
        std::fs::write(root.join("INST/1.bin"), inst).unwrap();
        let mut sound = vec![0_u8; 42 + 512];
        sound[1] = 1;
        sound[3] = 1;
        sound[11] = 1;
        sound[12] = 0x80;
        sound[13] = 0x51;
        sound[24..28].copy_from_slice(&512_u32.to_be_bytes());
        sound[41] = 60;
        for i in 0..512 {
            sound[42 + i] = i as u8;
        }
        std::fs::write(root.join("snd/10001.bin"), sound).unwrap();
        let bank = MacMidiBank::load(&root, 1).unwrap();
        let midi = b"MThd\0\0\0\x06\0\0\0\x01\0\x78MTrk\0\0\0\x04\0\xff\x2f\0";
        let mut engine = MacMidiEngine::new(bank, midi, 1, 0x0400, 1, 1, 0x4100).unwrap();
        engine
            .handle_event(MacMidiDriverEvent::ProgramChange {
                track: 0,
                channel: 0,
                program: 3,
            })
            .unwrap();
        let on = MacMidiDriverEvent::Note(MacMidiEvent::NoteOn {
            track: 0,
            channel: 0,
            key: 60,
            velocity: 80,
        });
        let off = MacMidiDriverEvent::Note(MacMidiEvent::NoteOff {
            track: 0,
            channel: 0,
            key: 60,
            velocity: 0,
        });
        engine.handle_event(on).unwrap();
        assert_eq!(engine.slots[0].identity.instrument, 1);
        assert!(engine.refill().unwrap().iter().all(|&x| x == 128));
        assert!(engine.refill().unwrap().iter().any(|&x| x != 128));
        engine.handle_event(off).unwrap();
        assert_eq!(engine.slots[0].identity.status, 0);
        engine.handle_event(on).unwrap();
        assert_eq!(engine.slots[0].pending.as_ref().unwrap().flag, 0xff01);
        let old = engine.refill().unwrap();
        assert!(old.iter().any(|&x| x != 128));
        assert_eq!(engine.slots[0].cursor.position, 0);
        assert!(engine.refill().unwrap().iter().any(|&x| x != 128));
        assert!(engine.slots[0].cursor.position > 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires original Mac resources"]
    fn original_theme_runs_through_controlled_engine() {
        let source =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_RESOURCE_ROOT").unwrap());
        let pack =
            std::env::temp_dir().join(format!("citygame-engine-original-{}", std::process::id()));
        crate::audio_import::import_mac(&source, &pack).unwrap();
        let bank = MacMidiBank::load(&pack.join("catalog/mac-audio"), 10018).unwrap();
        let midi = std::fs::read(pack.join("catalog/sounds/10018.MID")).unwrap();
        let mut engine = MacMidiEngine::new(bank, &midi, 16, 0x0400, 16, 16, 0x4100).unwrap();
        let mut calls = 0;
        let mut heard = false;
        while engine.sequencer.active() && calls < 6200 {
            let audio = engine.refill().unwrap();
            calls += 1;
            assert_eq!(audio.len(), 370);
            heard |= audio.iter().any(|&v| v != 128);
        }
        assert_eq!(calls, 6111);
        assert!(heard);
        assert!(!engine.sequencer.active());
        std::fs::remove_dir_all(pack).unwrap();
    }

    #[test]
    #[ignore = "requires original Mac resources and original-instruction audio fixture"]
    fn original_theme_initial_refills_match_driver() {
        let source =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_RESOURCE_ROOT").unwrap());
        let fixture = std::fs::read(std::env::var_os("CITYGAME_MAC_THEME_AUDIO").unwrap()).unwrap();
        let fixture: serde_json::Value = serde_json::from_slice(&fixture).unwrap();
        let pack =
            std::env::temp_dir().join(format!("citygame-engine-audio-{}", std::process::id()));
        crate::audio_import::import_mac(&source, &pack).unwrap();
        let bank = MacMidiBank::load(&pack.join("catalog/mac-audio"), 10018).unwrap();
        let midi = std::fs::read(pack.join("catalog/sounds/10018.MID")).unwrap();
        let mut engine = MacMidiEngine::new(bank, &midi, 16, 0x0400, 16, 16, 0x4100).unwrap();
        let buffers = fixture["buffers"].as_array().unwrap();
        assert_eq!(buffers.len(), fixture["calls"].as_u64().unwrap() as usize);
        assert!(!buffers.is_empty());
        for (i, row) in buffers.iter().enumerate() {
            if std::env::var_os("CITYGAME_MAC_ENGINE_TRACE").is_some() && (1597..1605).contains(&i)
            {
                let state:Vec<_>=engine.slots.iter().enumerate().filter(|(_,s)|s.identity.status>=0).map(|(slot,s)|serde_json::json!({"slot":slot,"status":s.identity.status,"track":s.identity.track_tag,"channel":s.identity.channel,"key":s.identity.key,"instrument":s.identity.instrument,"sound":s.prepared.as_ref().map(|p|p.sound_id),"pos":s.cursor.position,"step":(u32::from(s.cursor.integer_step)<<16)|u32::from(s.cursor.fractional_step),"fraction":s.cursor.fraction,"cache":s.cache,"timer":s.identity.release_flags,"tail":s.pending.as_ref().map_or(0,|p|p.flag)})).collect();
                eprintln!(
                    "before {} {}",
                    i + 1,
                    serde_json::to_string(&state).unwrap()
                );
            }
            let expected: Vec<u8> = row
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect();
            let actual = engine.refill().unwrap();
            if actual != expected {
                let index = actual
                    .iter()
                    .zip(&expected)
                    .position(|(a, b)| a != b)
                    .unwrap();
                panic!(
                    "refill {} sample {}: CityGame {}, original {}",
                    i + 1,
                    index,
                    actual[index],
                    expected[index]
                );
            }
        }
        std::fs::remove_dir_all(pack).unwrap();
    }

    #[test]
    #[ignore = "requires original Mac resources and captured-configuration audio fixture"]
    fn original_theme_captured_configuration_matches_driver() {
        let source =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_RESOURCE_ROOT").unwrap());
        let song_id: u16 = std::env::var("CITYGAME_MAC_THEME_ID")
            .unwrap_or_else(|_| "10018".into())
            .parse()
            .unwrap();
        let fixture =
            std::fs::read(std::env::var_os("CITYGAME_MAC_THEME_LIVECONFIG").unwrap()).unwrap();
        let fixture: serde_json::Value = serde_json::from_slice(&fixture).unwrap();
        let pack =
            std::env::temp_dir().join(format!("citygame-engine-liveconfig-{}", std::process::id()));
        crate::audio_import::import_mac(&source, &pack).unwrap();
        let bank = MacMidiBank::load(&pack.join("catalog/mac-audio"), song_id).unwrap();
        let midi = std::fs::read(pack.join(format!("catalog/sounds/{song_id}.MID"))).unwrap();
        let mut engine = MacMidiEngine::new_configured(
            bank,
            &midi,
            6,
            0x4028,
            6,
            6,
            0x4100,
            2,
            MacOutputMode::Direct,
        )
        .unwrap();
        let expected_end: Option<usize> = std::env::var("CITYGAME_MAC_EXPECT_END")
            .ok()
            .map(|value| value.parse().unwrap());
        let buffers = fixture["buffers"].as_array().unwrap();
        assert_eq!(buffers.len(), fixture["calls"].as_u64().unwrap() as usize);
        let statuses = fixture.get("status_after_service").map(|value| {
            let values = value.as_array().unwrap();
            assert_eq!(values.len(), buffers.len());
            values
        });
        if let Some(end) = expected_end {
            assert!(
                end > 0 && end < buffers.len(),
                "fixture must include post-end buffers"
            );
        }
        let mut worker_finished = false;
        for (i, row) in buffers.iter().enumerate() {
            let expected: Vec<u8> = row
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect();
            if !engine.has_audio_work() {
                worker_finished = true;
            }
            if worker_finished {
                assert!(
                    expected.iter().all(|&sample| sample == 128),
                    "worker would cut off original audio at refill {}",
                    i + 1
                );
            }
            let actual = engine.refill().unwrap();
            if let Some(statuses) = statuses {
                assert_eq!(
                    engine.active(),
                    statuses[i].as_u64().unwrap() != 0,
                    "original track-status mismatch after refill {}",
                    i + 1
                );
            }
            if let Some(end) = expected_end {
                if i + 1 == end {
                    assert!(
                        !engine.active(),
                        "sequencer still active after refill {end}"
                    );
                } else if i + 1 < end {
                    assert!(engine.active(), "sequencer ended early at refill {}", i + 1);
                }
            }
            if actual != expected {
                let sample = actual
                    .iter()
                    .zip(&expected)
                    .position(|(a, b)| a != b)
                    .unwrap();
                panic!(
                    "refill {} sample {}: CityGame {}, original {}",
                    i + 1,
                    sample,
                    actual[sample],
                    expected[sample]
                );
            }
        }
        assert!(worker_finished, "worker did not finish within the fixture");
        std::fs::remove_dir_all(pack).unwrap();
    }

    #[test]
    #[ignore = "requires original Mac resources"]
    fn original_city_cues_render_in_captured_configuration() {
        let source =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_RESOURCE_ROOT").unwrap());
        let pack =
            std::env::temp_dir().join(format!("citygame-engine-city-cues-{}", std::process::id()));
        crate::audio_import::import_mac(&source, &pack).unwrap();
        for id in [
            10000, 10001, 10003, 10004, 10007, 10008, 10011, 10012, 10013, 10018,
        ] {
            let bank = MacMidiBank::load(&pack.join("catalog/mac-audio"), id).unwrap();
            let midi = std::fs::read(pack.join(format!("catalog/sounds/{id}.MID"))).unwrap();
            let mut engine = MacMidiEngine::new_configured(
                bank,
                &midi,
                6,
                0x4028,
                6,
                6,
                0x4100,
                2,
                MacOutputMode::Direct,
            )
            .unwrap();
            let mut calls = 0;
            let mut track_end = None;
            while engine.has_audio_work() && calls < 30000 {
                engine
                    .refill()
                    .unwrap_or_else(|e| panic!("cue {id} refill {calls}: {e:#}"));
                calls += 1;
                if !engine.active() && track_end.is_none() {
                    track_end = Some(calls);
                }
            }
            assert!(!engine.has_audio_work(), "cue {id} exceeded refill limit");
            eprintln!(
                "cue {id}: tracks end at {:?}, voices end at {calls}",
                track_end
            );
        }
        std::fs::remove_dir_all(pack).unwrap();
    }
}
