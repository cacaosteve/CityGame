//! Mac CODE 9 / MDRV 11 descriptor-query behavior.
//!
//! These types describe recovered native state. The runtime does not yet
//! maintain the original descriptor lifecycle, so this is not wired to audio.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MacSoundRecord {
    pub active: bool,
    pub descriptor: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MacSoundVoice {
    pub descriptor: u16,
    pub state: i16,
}

/// Fields used by the recovered 8-bit, unlooped MDRV mixing path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MacOneShotVoice {
    pub state: i16,
    pub position: u32,
    pub end: u32,
    pub pitch: u32,
    pub phase: u16,
    pub cached_span: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MacSoundRefillStep {
    MixVoices,
    UpdateMusic,
    Convert { half_rate: bool },
}

/// Process a Sound Manager double-buffer refill request once delivered.
/// The Sound Manager owns delivery timing and clears the ready bit first.
pub(crate) fn mac_refill_sound_buffer(
    flags: &mut u32,
    channel_present: bool,
    half_rate: bool,
    mut run: impl FnMut(MacSoundRefillStep),
) {
    if !channel_present {
        *flags |= 4;
        return;
    }
    run(MacSoundRefillStep::MixVoices);
    run(MacSoundRefillStep::UpdateMusic);
    run(MacSoundRefillStep::Convert { half_rate });
    *flags |= 1;
}

/// The Sound Manager's two buffer descriptors. State 6/7 addresses buffer 0;
/// state 8/9 addresses buffer 1. Flags and lengths are the native LONG fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MacSoundDoubleBuffer {
    pub state: u16,
    pub flags: [u32; 2],
    pub frames: [u32; 2],
}

/// The seven LONGs copied by the Sound Manager's source component when it
/// accepts another audio block; field four is its signed remaining count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MacSoundSourceBlock {
    pub fields: [u32; 7],
}

/// Position and fixed-point state used at the System sound converter's
/// source-block boundary. Position is in bytes; excess is in source frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MacSoundConverterProgress {
    pub position: u32,
    pub excess: u32,
    pub phase: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MacSoundOutputSpan {
    pub pointer: u32,
    pub frames: u32,
}

/// The format and buffer properties retained by the System converter.
/// `description` contains the seven LONGs at converter offset +8.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MacSoundConverterSetup {
    pub description: [u32; 7],
    pub limit: u32,
    pub flags: u32,
    pub rate: u32,
    pub changed: u8,
}

/// Prepare the retained raw/twos source description and native allocation
/// size after the component's owner and allocator checks have succeeded.
pub(crate) fn mac_retain_sound_description(mut description: [u32; 7]) -> Option<([u32; 7], u32)> {
    let factor = match (description[1], description[2] as u16) {
        (0x7261_7720, 8) => 2,
        (0x7477_6f73, 16) => 8,
        _ => return None,
    };
    description[4] = description[4].min(0x7ffc);
    Some((description, description[4].wrapping_mul(factor)))
}

/// Normalize the source description's requested count to a whole number of
/// interleaved channels. The original assumes a nonzero channel count.
pub(crate) fn mac_normalize_sound_description(description: &mut [u32; 7], channels: u16) -> bool {
    if channels == 0 {
        return false;
    }
    let requested = if description[4] == 0 {
        if description[0] & 0x80 != 0 {
            0x7ffc
        } else {
            1024
        }
    } else {
        description[4]
    };
    let channels = i32::from(channels);
    description[4] = ((requested as i32 / channels).wrapping_mul(channels)) as u32;
    description[5] = 0;
    true
}

/// Apply the original converter's format setup. A false result corresponds
/// to its format-change error; the caller supplies the converter address to
/// the native component API when handling that result.
pub(crate) fn mac_setup_sound_converter(
    setup: &mut MacSoundConverterSetup,
    incoming: [u32; 7],
) -> bool {
    if incoming[1] != setup.description[1] || (incoming[2] as u16) != (setup.description[2] as u16)
    {
        setup.description[2] = (incoming[2] & 0xffff_0000) | (setup.description[2] & 0xffff);
        setup.description[3] = incoming[3];
        return false;
    }
    setup.description = incoming;
    setup.flags = incoming[0];
    setup.limit = incoming[4].min(1024);
    setup.rate = incoming[3];
    setup.changed = 1;
    true
}

/// Select a source span on the converter's unity-rate, non-null-source path.
/// The caller must have resolved any prior source-block exhaustion first.
pub(crate) fn mac_sound_unity_source_span(
    block: &MacSoundSourceBlock,
    progress: &mut MacSoundConverterProgress,
    limit: u32,
    shift: u16,
    flags: u32,
) -> Option<MacSoundOutputSpan> {
    let source = block.fields[5];
    if source == 0 {
        return None;
    }
    let shift = u32::from(shift & 63);
    let remaining = block.fields[4];
    let frames = if flags & 0x80 != 0 {
        remaining
    } else {
        remaining
            .wrapping_sub(progress.position.checked_shr(shift).unwrap_or(0))
            .min(limit)
    };
    let pointer = source.wrapping_add(progress.position);
    progress.position = progress
        .position
        .wrapping_add(frames.checked_shl(shift).unwrap_or(0));
    Some(MacSoundOutputSpan { pointer, frames })
}

/// Apply the converter's block-boundary accounting after it processes audio.
/// The shift translates between its byte position and source-frame count.
/// Returns the phase copied to the native converter's local stack frame.
pub(crate) fn mac_sound_finish_source_block(
    block: &mut MacSoundSourceBlock,
    progress: &mut MacSoundConverterProgress,
    shift: u16,
) -> u32 {
    // 68k register shifts use the low six bits, and shifts of 32 or more
    // clear a longword rather than wrapping the shift count modulo 32.
    let shift = u32::from(shift & 63);
    let consumed = progress.position.checked_shr(shift).unwrap_or(0);
    let remaining = block.fields[4];
    if consumed >= remaining {
        progress.excess = consumed.wrapping_sub(remaining);
        progress.position = progress.excess.checked_shl(shift).unwrap_or(0);
        block.fields[4] = 0;
    } else {
        progress.excess = 0;
    }
    progress.phase
}

/// Check whether the source component needs another block. A present callback
/// returns its seven fields on success, or None when the source has ended.
/// Returns whether the callback was invoked; consumption is modeled elsewhere.
pub(crate) fn mac_sound_source_request(
    block: &mut MacSoundSourceBlock,
    callback: Option<impl FnOnce() -> Option<[u32; 7]>>,
) -> bool {
    if (block.fields[4] as i32) > 0 {
        return false;
    }
    let Some(callback) = callback else {
        return false;
    };
    if let Some(fields) = callback() {
        block.fields = fields;
    } else {
        block.fields[4] = 0;
    }
    true
}

/// Run one Sound Manager dispatch after the sound channel requests more data.
/// `play` receives the selected buffer or None for the silence buffer;
/// `refill` executes the installed callback after the ready bit is cleared.
/// The caller owns the timing of channel requests and actual sample playback.
pub(crate) fn mac_dispatch_sound_double_buffer(
    buffers: &mut MacSoundDoubleBuffer,
    mut play: impl FnMut(Option<usize>),
    mut refill: impl FnMut(usize, &mut u32),
) -> bool {
    loop {
        match buffers.state {
            6 | 8 => {
                let index = if buffers.state == 6 { 0 } else { 1 };
                if buffers.flags[index] & 1 != 0 {
                    play((buffers.frames[index] != 0).then_some(index));
                    buffers.state += 1;
                } else {
                    play(None);
                }
                return true;
            }
            7 | 9 => {
                let index = if buffers.state == 7 { 0 } else { 1 };
                buffers.flags[index] &= !1;
                let last = buffers.flags[index] & 4 != 0;
                refill(index, &mut buffers.flags[index]);
                buffers.state = if last {
                    0
                } else if index == 0 {
                    8
                } else {
                    6
                };
            }
            _ => return false,
        }
    }
}

/// Visit the voices selected by one MDRV buffer-service pass. Unlike the
/// descriptor operations, this pass begins at slot zero and visits
/// `start + count` slots. A zero or out-of-range total is not a valid modeled
/// buffer configuration. The caller handles each active voice's sound mode.
pub(crate) fn mac_scan_sound_service(
    voices: &mut [MacOneShotVoice],
    start: u16,
    count: u16,
    mut service: impl FnMut(usize, &mut MacOneShotVoice),
) -> Option<()> {
    let total = start.wrapping_add(count) as usize;
    if total == 0 || total > voices.len() {
        return None;
    }
    for (index, voice) in voices[..total].iter_mut().enumerate() {
        if voice.state >= 0 {
            service(index, voice);
        }
    }
    Some(())
}

/// Advance one MDRV buffer for an active one-shot voice with no modulation,
/// transition or loop. The caller supplies the driver's two buffer counts and
/// rate selector; waveform mixing and all other voice modes remain separate.
pub(crate) fn mac_service_one_shot_voice(
    voice: &mut MacOneShotVoice,
    span_frames: u16,
    output_frames: u16,
    half_rate: bool,
) -> bool {
    if voice.state <= 0 {
        return false;
    }
    voice.state -= 1;
    if voice.cached_span == 0 {
        let frames = u32::from(span_frames);
        voice.cached_span = (voice.pitch >> 16)
            .wrapping_mul(frames)
            .wrapping_add(((voice.pitch & 0xffff) * frames) >> 16);
    }
    // The native range check uses the unhalved cached span even when the
    // driver's output mode halves the per-sample pitch.
    let difference = voice
        .end
        .wrapping_sub(voice.position)
        .wrapping_sub(voice.cached_span);
    let terminal = difference == 0 || (difference as i32) < 0;
    let pitch = if half_rate {
        voice.pitch >> 1
    } else {
        voice.pitch
    };
    if !terminal && pitch & 0xffff != 0 {
        // The fractional fast path accumulates a WORD offset and uses ADDA.W
        // once after the buffer. Its final offset is sign-extended.
        let sum = u64::from(voice.phase) + u64::from(pitch) * u64::from(output_frames);
        voice.phase = sum as u16;
        voice.position = voice
            .position
            .wrapping_add_signed(i32::from((sum >> 16) as u16 as i16));
    } else {
        for _ in 0..output_frames {
            if terminal && voice.position >= voice.end {
                break;
            }
            let sum = u32::from(voice.phase).wrapping_add(pitch);
            voice.phase = sum as u16;
            voice.position = voice
                .position
                .wrapping_add_signed(i32::from((sum >> 16) as u16 as i16));
        }
    }
    if terminal {
        voice.state = -1;
    }
    true
}

/// Advance the descriptor states and one-shot mixer states together on one
/// delivered MDRV buffer callback. Delivery timing remains the caller's job.
/// Reject unsupported zero-state transitions before mutating either table.
pub(crate) fn mac_service_one_shot_pool(
    voices: &mut [MacSoundVoice],
    mixers: &mut [MacOneShotVoice],
    start: u16,
    count: u16,
    span_frames: u16,
    output_frames: u16,
    half_rate: bool,
) -> Option<()> {
    let total = usize::from(start.wrapping_add(count));
    if total == 0 || total > voices.len() || total > mixers.len() {
        return None;
    }
    if voices[..total]
        .iter()
        .zip(&mixers[..total])
        .any(|(voice, mixer)| voice.state != mixer.state || voice.state == 0)
    {
        return None;
    }
    mac_scan_sound_service(mixers, start, count, |_, mixer| {
        mac_service_one_shot_voice(mixer, span_frames, output_frames, half_rate);
    })?;
    for (voice, mixer) in voices[..total].iter_mut().zip(&mixers[..total]) {
        voice.state = mixer.state;
    }
    Some(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MacManagedSoundRecord {
    pub active: bool,
    pub descriptor: u16,
    pub rank: i32,
}

fn mdrv_voice_is_idle(descriptor: u16, voices: &[MacSoundVoice]) -> bool {
    voices
        .iter()
        .find(|voice| voice.descriptor == descriptor)
        .is_none_or(|voice| voice.state < 0)
}

/// CODE 9's record allocator. The observer receives actual MDRV operation
/// numbers and descriptors in dispatch order. An empty slice is rejected;
/// the original helper assumes its caller allocated at least one record.
pub(crate) fn mac_reserve_sound_record(
    records: &mut [MacManagedSoundRecord],
    voices: &mut [MacSoundVoice],
    driver_inhibited: bool,
    driver_mode: u16,
    rank_watermark: i32,
    mut dispatched: impl FnMut(u16, u16),
) -> Option<usize> {
    if records.is_empty() {
        return None;
    }
    let mut selected = records.iter().position(|record| !record.active);
    let was_full = selected.is_none();
    if was_full {
        for (index, record) in records.iter().enumerate() {
            let idle = if driver_inhibited || driver_mode == 100 {
                false
            } else {
                dispatched(0x14, record.descriptor);
                mdrv_voice_is_idle(record.descriptor, voices)
            };
            if idle {
                selected = Some(index);
                break;
            }
        }
        if selected.is_none() {
            let mut lowest = rank_watermark;
            for (index, record) in records.iter().enumerate() {
                if record.rank < lowest {
                    lowest = record.rank;
                    selected = Some(index);
                }
            }
        }
    }
    let stop_selected = was_full && selected.is_some();
    let index = selected.unwrap_or(0);
    if stop_selected && !matches!(records[index].descriptor, 0x8001 | 0x8002) {
        if !driver_inhibited && driver_mode != 100 {
            dispatched(0x12, records[index].descriptor);
            mac_stop_sound_voices(voices, records[index].descriptor);
        }
        records[index] = MacManagedSoundRecord::default();
    }
    records[index].active = false;
    records[index].rank = 0;
    Some(index)
}

/// MDRV command 0x11's admission worker: the first inactive slot wins;
/// otherwise replace the last slot tied for the smallest nonnegative state.
/// The sample pointer and full mixer fields remain owned by the caller.
pub(crate) fn mac_start_sound_voice(
    voices: &mut [MacSoundVoice],
    descriptor: u16,
    sample_present: bool,
) -> Option<usize> {
    if !sample_present || voices.is_empty() {
        return None;
    }
    let mut selected = 0;
    let mut lowest = i16::MAX;
    for (i, voice) in voices.iter().enumerate() {
        if voice.state < 0 {
            selected = i;
            break;
        }
        if voice.state <= lowest {
            lowest = voice.state;
            selected = i;
        }
    }
    voices[selected] = MacSoundVoice {
        descriptor,
        state: 0x7ffe,
    };
    Some(selected)
}

/// MDRV command 0x12 marks every active matching descriptor inactive.
pub(crate) fn mac_stop_sound_voices(voices: &mut [MacSoundVoice], descriptor: u16) {
    for voice in voices {
        if voice.state >= 0 && voice.descriptor == descriptor {
            voice.state = -1;
        }
    }
}

/// MDRV command 0x16 clears the effect range, including one extra slot from
/// its native DBRA loop. The original assumes the whole range is allocated.
pub(crate) fn mac_stop_all_sound_voices(
    voices: &mut [MacSoundVoice],
    start: u16,
    count: u16,
) -> Option<()> {
    if count == 0 {
        return Some(());
    }
    // The native ASL.W / ADDA.W addressing is only modeled for nonnegative
    // signed byte offsets and a fully present voice table.
    if start >= 8192 {
        return None;
    }
    let start = usize::from(start);
    let end = start.checked_add(usize::from(count))?.checked_add(1)?;
    for voice in voices.get_mut(start..end)? {
        voice.state = -1;
    }
    Some(())
}

/// CODE 9 export 18 and the MDRV 11 command-0x14 handler. `records` and
/// `voices` are already restricted to the native configured ranges.
pub(crate) fn mac_sound_records_are_idle(
    driver_unavailable: bool,
    driver_inhibited: bool,
    driver_mode: u16,
    records: &[MacSoundRecord],
    voices: &[MacSoundVoice],
    mut queried: impl FnMut(u16),
) -> bool {
    if driver_unavailable {
        return true;
    }
    let mut result = true;
    for record in records.iter().filter(|record| record.active) {
        // The native export visits every active record even after a zero
        // result, so retain query side effects and their order.
        let idle = if driver_inhibited || driver_mode == 100 {
            false
        } else {
            queried(record.descriptor);
            // MDRV returns immediately on the first matching slot, including
            // when that slot is inactive and a later duplicate is active.
            mdrv_voice_is_idle(record.descriptor, voices)
        };
        if !idle {
            result = false;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_shot_pool_rejects_unmodeled_states_without_partial_service() {
        let mut voices = [
            MacSoundVoice {
                descriptor: 507,
                state: 2,
            },
            MacSoundVoice {
                descriptor: 504,
                state: 0,
            },
        ];
        let voice = |state| MacOneShotVoice {
            state,
            position: 0,
            end: 100,
            pitch: 0x10000,
            phase: 0,
            cached_span: 0,
        };
        let mut mixers = [voice(2), voice(0)];
        let before = (voices, mixers);
        assert_eq!(
            mac_service_one_shot_pool(&mut voices, &mut mixers, 0, 2, 10, 10, false),
            None
        );
        assert_eq!((voices, mixers), before);
    }

    #[test]
    fn one_shot_pool_services_from_slot_zero_through_configured_end() {
        let mut voices = [
            MacSoundVoice {
                descriptor: 504,
                state: 4,
            },
            MacSoundVoice {
                descriptor: 507,
                state: 4,
            },
            MacSoundVoice {
                descriptor: 511,
                state: -1,
            },
            MacSoundVoice {
                descriptor: 512,
                state: 4,
            },
        ];
        let voice = |state| MacOneShotVoice {
            state,
            position: 0,
            end: 100,
            pitch: 0x10000,
            phase: 0,
            cached_span: 0,
        };
        let mut mixers = [voice(4), voice(4), voice(-1), voice(4)];
        assert_eq!(
            mac_service_one_shot_pool(&mut voices, &mut mixers, 2, 1, 10, 10, false),
            Some(())
        );
        assert_eq!(voices.map(|voice| voice.state), [3, 3, -1, 4]);
        assert_eq!(mixers.map(|voice| voice.position), [10, 10, 0, 0]);
    }

    #[test]
    #[ignore = "requires private live Mac sound-service trace"]
    fn callback_service_drives_descriptor_query_until_live_voice_retires() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_SERVICE_LIVE").unwrap();
        let fixture: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let runs = fixture["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 7);
        let n = |row: &serde_json::Value, key: &str| row[key].as_u64().unwrap() as u32;
        for run in runs {
            let steps = run.as_array().unwrap();
            let first = &steps[0]["before"];
            let initial = MacOneShotVoice {
                state: n(first, "state") as u16 as i16,
                position: n(first, "position"),
                end: n(first, "end"),
                pitch: n(first, "pitch"),
                phase: 0,
                cached_span: n(first, "span"),
            };
            let mut voices = [MacSoundVoice {
                descriptor: 507,
                state: initial.state,
            }];
            let mut mixers = [initial];
            let records = [MacSoundRecord {
                active: true,
                descriptor: 507,
            }];
            for (index, step) in steps.iter().enumerate() {
                assert!(!mac_sound_records_are_idle(
                    false,
                    false,
                    1,
                    &records,
                    &voices,
                    |_| {}
                ));
                assert_eq!(
                    mac_service_one_shot_pool(
                        &mut voices,
                        &mut mixers,
                        0,
                        1,
                        n(&fixture, "span_frames") as u16,
                        n(&fixture, "output_frames") as u16,
                        fixture["half_rate"].as_bool().unwrap()
                    ),
                    Some(())
                );
                let after = &step["after"];
                assert_eq!(
                    (
                        voices[0].state as u16,
                        mixers[0].position,
                        mixers[0].cached_span
                    ),
                    (
                        n(after, "state") as u16,
                        n(after, "position"),
                        n(after, "span")
                    ),
                    "callback {index}"
                );
                assert_eq!(
                    mac_sound_records_are_idle(false, false, 1, &records, &voices, |_| {}),
                    index + 1 == steps.len()
                );
            }
        }
    }

    #[test]
    fn duplicate_inactive_voice_masks_later_active_voice() {
        let records = [MacSoundRecord {
            active: true,
            descriptor: 507,
        }];
        let voices = [
            MacSoundVoice {
                descriptor: 507,
                state: -1,
            },
            MacSoundVoice {
                descriptor: 507,
                state: 1,
            },
        ];
        let mut calls = Vec::new();
        assert!(mac_sound_records_are_idle(
            false,
            false,
            1,
            &records,
            &voices,
            |v| calls.push(v)
        ));
        assert_eq!(calls, [507]);
    }

    #[test]
    fn sound_scan_queries_later_records_after_a_busy_voice() {
        let records = [
            MacSoundRecord {
                active: true,
                descriptor: 504,
            },
            MacSoundRecord {
                active: true,
                descriptor: 507,
            },
        ];
        let voices = [MacSoundVoice {
            descriptor: 504,
            state: 0,
        }];
        let mut calls = Vec::new();
        assert!(!mac_sound_records_are_idle(
            false,
            false,
            1,
            &records,
            &voices,
            |v| calls.push(v)
        ));
        assert_eq!(calls, [504, 507]);
    }

    #[test]
    fn admission_prefers_first_free_then_last_lowest_and_stop_visits_all() {
        let mut voices = [
            MacSoundVoice {
                descriptor: 1,
                state: 4,
            },
            MacSoundVoice {
                descriptor: 2,
                state: 4,
            },
            MacSoundVoice {
                descriptor: 3,
                state: 5,
            },
        ];
        assert_eq!(mac_start_sound_voice(&mut voices, 507, true), Some(1));
        voices[0].state = -1;
        voices[2].state = -1;
        assert_eq!(mac_start_sound_voice(&mut voices, 507, true), Some(0));
        mac_stop_sound_voices(&mut voices, 507);
        assert_eq!(
            (voices[0].state, voices[1].state, voices[2].state),
            (-1, -1, -1)
        );
        assert_eq!(mac_start_sound_voice(&mut voices, 507, false), None);
    }

    #[test]
    fn stop_all_clears_the_native_extra_slot_only_for_nonzero_count() {
        let mut voices = [MacSoundVoice {
            descriptor: 507,
            state: 5,
        }; 5];
        assert_eq!(mac_stop_all_sound_voices(&mut voices, 1, 2), Some(()));
        assert_eq!(voices.map(|v| v.state), [5, -1, -1, -1, 5]);
        assert_eq!(mac_stop_all_sound_voices(&mut voices, 1, 0), Some(()));
        assert_eq!(mac_stop_all_sound_voices(&mut voices, 3, 2), None);
    }

    #[test]
    #[ignore = "requires private original-Mac stop-all fixtures"]
    fn stop_all_matches_original_mdrv_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_STOP_ALL_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 200);
        for row in rows {
            let mut voices: Vec<_> = row["states"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| MacSoundVoice {
                    descriptor: 507,
                    state: v.as_u64().unwrap() as u16 as i16,
                })
                .collect();
            assert_eq!(
                mac_stop_all_sound_voices(
                    &mut voices,
                    row["start"].as_u64().unwrap() as u16,
                    row["count"].as_u64().unwrap() as u16
                ),
                Some(())
            );
            let after: Vec<_> = voices.iter().map(|v| v.state as u16).collect();
            let expected: Vec<_> = row["after"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect();
            assert_eq!(after, expected, "{row}");
        }
    }

    #[test]
    fn full_record_table_falls_back_without_stopping_when_no_rank_is_lower() {
        let mut records = [MacManagedSoundRecord {
            active: true,
            descriptor: 507,
            rank: 5,
        }];
        let mut voices = [MacSoundVoice {
            descriptor: 507,
            state: 1,
        }];
        let mut calls = Vec::new();
        assert_eq!(
            mac_reserve_sound_record(&mut records, &mut voices, false, 1, 5, |op, id| calls
                .push((op, id))),
            Some(0)
        );
        assert_eq!(calls, [(0x14, 507)]);
        assert_eq!(voices[0].state, 1);
        assert_eq!(
            records[0],
            MacManagedSoundRecord {
                active: false,
                descriptor: 507,
                rank: 0
            }
        );
    }

    #[test]
    fn one_shot_retirement_uses_unhalved_native_span() {
        let mut voice = MacOneShotVoice {
            state: 2,
            position: 0,
            end: 140,
            pitch: 0x6660,
            phase: 0,
            cached_span: 0,
        };
        assert!(mac_service_one_shot_voice(&mut voice, 370, 370, true));
        assert_eq!(
            (voice.position, voice.cached_span, voice.state),
            (73, 147, -1)
        );
    }

    #[test]
    fn service_scan_starts_at_zero_and_includes_zero_state() {
        let voice = |state| MacOneShotVoice {
            state,
            position: 0,
            end: 0,
            pitch: 0,
            phase: 0,
            cached_span: 0,
        };
        let mut voices = [voice(1), voice(-1), voice(0), voice(1)];
        let mut visited = Vec::new();
        assert_eq!(
            mac_scan_sound_service(&mut voices, 2, 1, |i, _| visited.push(i)),
            Some(())
        );
        assert_eq!(visited, [0, 2]);
        assert_eq!(mac_scan_sound_service(&mut voices, 0, 0, |_, _| {}), None);
        assert_eq!(mac_scan_sound_service(&mut voices, 4, 1, |_, _| {}), None);
    }

    #[test]
    fn refill_sets_ready_after_all_workers_or_marks_missing_channel() {
        let mut flags = 0x20;
        let mut calls = Vec::new();
        mac_refill_sound_buffer(&mut flags, true, true, |step| calls.push(step));
        assert_eq!(
            calls,
            [
                MacSoundRefillStep::MixVoices,
                MacSoundRefillStep::UpdateMusic,
                MacSoundRefillStep::Convert { half_rate: true }
            ]
        );
        assert_eq!(flags, 0x21);
        calls.clear();
        mac_refill_sound_buffer(&mut flags, false, false, |step| calls.push(step));
        assert!(calls.is_empty());
        assert_eq!(flags, 0x25);
    }

    #[test]
    fn double_buffer_refills_consumed_then_plays_next_or_silence() {
        use std::cell::RefCell;
        let mut buffers = MacSoundDoubleBuffer {
            state: 7,
            flags: [1, 1],
            frames: [370, 370],
        };
        let calls = RefCell::new(Vec::new());
        assert!(mac_dispatch_sound_double_buffer(
            &mut buffers,
            |which| calls.borrow_mut().push(("play", which)),
            |which, flags| {
                calls.borrow_mut().push(("refill", Some(which)));
                *flags |= 1;
            }
        ));
        assert_eq!(*calls.borrow(), [("refill", Some(0)), ("play", Some(1))]);
        assert_eq!(buffers.state, 9);
        buffers.flags[1] |= 4;
        assert!(!mac_dispatch_sound_double_buffer(
            &mut buffers,
            |_| panic!(),
            |_, _| {}
        ));
        assert_eq!(buffers.state, 0);
    }

    #[test]
    fn source_requests_a_block_only_after_remaining_count_expires() {
        let mut block = MacSoundSourceBlock {
            fields: [0, 1, 2, 3, 1, 5, 6],
        };
        assert!(!mac_sound_source_request(&mut block, Some(|| panic!())));
        block.fields[4] = u32::MAX;
        assert!(mac_sound_source_request(&mut block, Some(|| Some([7; 7]))));
        assert_eq!(block.fields, [7; 7]);
        block.fields[4] = 0;
        assert!(mac_sound_source_request(&mut block, Some(|| None)));
        assert_eq!(block.fields, [7, 7, 7, 7, 0, 7, 7]);
    }

    #[test]
    fn source_block_boundary_retains_excess_for_next_block() {
        let mut block = MacSoundSourceBlock {
            fields: [0, 0, 0, 0, 370, 0, 0],
        };
        let mut progress = MacSoundConverterProgress {
            position: 375,
            excess: 99,
            phase: 0x8000,
        };
        assert_eq!(
            mac_sound_finish_source_block(&mut block, &mut progress, 0),
            0x8000
        );
        assert_eq!(
            (block.fields[4], progress.position, progress.excess),
            (0, 5, 5)
        );
    }

    #[test]
    fn unity_source_span_respects_limit_and_retains_partial_position() {
        let block = MacSoundSourceBlock {
            fields: [0, 0, 0, 0, 370, 0x1000, 0],
        };
        let mut progress = MacSoundConverterProgress {
            position: 300,
            excess: 0,
            phase: 0,
        };
        assert_eq!(
            mac_sound_unity_source_span(&block, &mut progress, 50, 0, 0),
            Some(MacSoundOutputSpan {
                pointer: 0x112c,
                frames: 50
            })
        );
        assert_eq!(progress.position, 350);
    }

    #[test]
    fn converter_setup_caps_the_requested_frames() {
        let mut setup = MacSoundConverterSetup {
            description: [0; 7],
            limit: 0,
            flags: 0,
            rate: 0,
            changed: 0,
        };
        assert!(mac_setup_sound_converter(
            &mut setup,
            [4, 0, 0, 0x10000, 1200, 0, 0]
        ));
        assert_eq!(
            (setup.limit, setup.flags, setup.rate, setup.changed),
            (1024, 4, 0x10000, 1)
        );
    }

    #[test]
    fn source_description_defaults_and_rounds_by_channels() {
        let mut description = [0, 0, 0, 0, 0, 42, 0];
        assert!(mac_normalize_sound_description(&mut description, 3));
        assert_eq!((description[4], description[5]), (1023, 0));
        description[0] = 0x80;
        description[4] = 0;
        assert!(mac_normalize_sound_description(&mut description, 2));
        assert_eq!(description[4], 0x7ffc);
        assert!(!mac_normalize_sound_description(&mut description, 0));
    }

    #[test]
    fn retained_description_caps_count_and_sizes_the_sample_buffer() {
        let input = [0, 0x7261_7720, 8, 0, 0x8000, 0, 0];
        let (retained, allocation) = mac_retain_sound_description(input).unwrap();
        assert_eq!((retained[4], allocation), (0x7ffc, 0xfff8));
    }

    #[test]
    #[ignore = "requires private original-Mac retained description fixtures"]
    fn retained_description_matches_original_component_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_RETAIN_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 150);
        for row in rows {
            let input = std::array::from_fn(|i| row["description"][i].as_u64().unwrap() as u32);
            let (retained, allocation) = mac_retain_sound_description(input).unwrap();
            let after = &row["after"];
            let expected: [u32; 7] =
                std::array::from_fn(|i| after["description"][i].as_u64().unwrap() as u32);
            assert_eq!(retained, expected, "{row}");
            assert_eq!(
                allocation as u64,
                after["allocation"].as_u64().unwrap(),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires private original-Mac source description fixtures"]
    fn source_description_matches_original_manager_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_DESCRIPTION_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 200);
        for row in rows {
            let n = |name: &str| row[name].as_u64().unwrap() as u32;
            let mut description = [n("flags"), 0, 0, 0, n("count"), n("old_extra"), 0];
            assert!(mac_normalize_sound_description(
                &mut description,
                n("channels") as u16
            ));
            assert_eq!(
                description[4] as u64,
                row["after"]["count"].as_u64().unwrap(),
                "{row}"
            );
            assert_eq!(
                description[5] as u64,
                row["after"]["extra"].as_u64().unwrap(),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires private original-Mac converter setup fixtures"]
    fn converter_setup_matches_original_component_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_SETUP_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 200);
        for row in rows {
            let n = |name: &str| row[name].as_u64().unwrap() as u32;
            let arr = |key: &str| std::array::from_fn(|i| row[key][i].as_u64().unwrap() as u32);
            let mut setup = MacSoundConverterSetup {
                description: arr("prior"),
                limit: n("old_limit"),
                flags: n("old_flags"),
                rate: n("old_rate"),
                changed: n("old_changed") as u8,
            };
            let accepted = mac_setup_sound_converter(&mut setup, arr("description"));
            let after = &row["after"];
            let expected: [u32; 7] =
                std::array::from_fn(|i| after["description"][i].as_u64().unwrap() as u32);
            assert_eq!(setup.description, expected, "{row}");
            assert_eq!(
                (setup.limit, setup.flags, setup.rate, setup.changed),
                (
                    after["limit"].as_u64().unwrap() as u32,
                    after["flags"].as_u64().unwrap() as u32,
                    after["rate"].as_u64().unwrap() as u32,
                    after["changed"].as_u64().unwrap() as u8
                ),
                "{row}"
            );
            assert_eq!(accepted, after["result"].as_u64().unwrap() == 0, "{row}");
            assert_eq!(
                after["out"].as_u64().unwrap(),
                if accepted { 0xfeedface } else { 0x820008 },
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires private original-Mac unity converter fixtures"]
    fn unity_source_span_matches_original_converter_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_UNITY_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 300);
        for row in rows {
            let n = |name: &str| row[name].as_u64().unwrap() as u32;
            let block = MacSoundSourceBlock {
                fields: [0, 0, 0, 0, n("remaining"), n("source"), 0],
            };
            let mut progress = MacSoundConverterProgress {
                position: n("position"),
                excess: 0,
                phase: 0,
            };
            let span = mac_sound_unity_source_span(
                &block,
                &mut progress,
                n("limit"),
                n("shift") as u16,
                n("flags"),
            )
            .unwrap();
            let after = &row["after"];
            assert_eq!(
                progress.position as u64,
                after["position"].as_u64().unwrap(),
                "{row}"
            );
            assert_eq!(
                span.pointer as u64,
                after["output_pointer"].as_u64().unwrap(),
                "{row}"
            );
            assert_eq!(
                span.frames as u64,
                after["output_frames"].as_u64().unwrap(),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires private live Mac unity converter trace"]
    fn unity_source_span_matches_live_converter_boundaries() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_UNITY_LIVE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(rows.len() > 100);
        for row in rows {
            let before = &row["before"];
            let n = |value: &serde_json::Value, name: &str| value[name].as_u64().unwrap() as u32;
            let mut block = MacSoundSourceBlock {
                fields: [0, 0, 0, 0, n(before, "remaining"), n(before, "source"), 0],
            };
            let mut progress = MacSoundConverterProgress {
                position: n(before, "position"),
                excess: n(before, "excess"),
                phase: 0,
            };
            let shift = n(before, "shift") as u16;
            let span = mac_sound_unity_source_span(
                &block,
                &mut progress,
                n(before, "limit"),
                shift,
                n(before, "flags"),
            )
            .unwrap();
            let expected = &row["span"];
            assert_eq!(
                (span.pointer, span.frames, progress.position),
                (
                    n(expected, "output_pointer"),
                    n(expected, "output_frames"),
                    n(expected, "position")
                ),
                "{row}"
            );
            mac_sound_finish_source_block(&mut block, &mut progress, shift);
            let expected = &row["after"];
            assert_eq!(
                (block.fields[4], progress.position, progress.excess),
                (
                    n(expected, "remaining"),
                    n(expected, "position"),
                    n(expected, "excess")
                ),
                "{row}"
            );
        }
    }

    #[test]
    fn source_requests_next_buffer_after_converter_finishes_the_current_one() {
        let mut buffers = MacSoundDoubleBuffer {
            state: 6,
            flags: [1, 1],
            frames: [370, 370],
        };
        let mut block = MacSoundSourceBlock { fields: [0; 7] };
        let mut progress = MacSoundConverterProgress {
            position: 0,
            excess: 0,
            phase: 0,
        };
        let incoming = [0, 0x7261_7720, 0x0002_0008, 0x56ee_8ba3, 1024, 0, 0];
        let (format, allocation) = mac_retain_sound_description(incoming).unwrap();
        assert_eq!(allocation, 2048);
        let mut converter = MacSoundConverterSetup {
            description: format,
            limit: 0,
            flags: 0,
            rate: 0,
            changed: 0,
        };
        assert!(mac_setup_sound_converter(&mut converter, format));
        let mut played = Vec::new();
        for expected in [0, 1, 0] {
            assert!(mac_sound_source_request(
                &mut block,
                Some(|| {
                    let mut selected = None;
                    let ready = mac_dispatch_sound_double_buffer(
                        &mut buffers,
                        |buffer| selected = buffer,
                        |_, flags| mac_refill_sound_buffer(flags, true, true, |_| {}),
                    );
                    selected.map(|index| {
                        assert!(ready);
                        played.push(index);
                        [0, 0, 0, 0, buffers.frames[index], index as u32, 0]
                    })
                })
            ));
            assert_eq!(block.fields[5], expected);
            // A nonzero placeholder pointer represents the selected source
            // buffer; the real address is supplied by the sound driver.
            block.fields[5] = 0x1000 + expected;
            let span = mac_sound_unity_source_span(
                &block,
                &mut progress,
                converter.limit,
                0,
                converter.flags,
            )
            .unwrap();
            assert_eq!(span.frames, 370);
            mac_sound_finish_source_block(&mut block, &mut progress, 0);
            assert_eq!((block.fields[4], progress.position), (0, 0));
        }
        assert_eq!(played, [0, 1, 0]);
    }

    #[test]
    #[ignore = "requires private original-Mac converter fixtures"]
    fn source_consumption_matches_original_converter_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_CONSUME_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 300);
        for row in rows {
            let n = |name: &str| row[name].as_u64().unwrap() as u32;
            let mut block = MacSoundSourceBlock {
                fields: [0, 0, 0, 0, n("remaining"), 0, 0],
            };
            let mut progress = MacSoundConverterProgress {
                position: n("position"),
                excess: n("old_excess"),
                phase: n("phase"),
            };
            let phase_copy =
                mac_sound_finish_source_block(&mut block, &mut progress, n("shift") as u16);
            let after = &row["after"];
            assert_eq!(
                block.fields[4] as u64,
                after["remaining"].as_u64().unwrap(),
                "{row}"
            );
            assert_eq!(
                progress.position as u64,
                after["position"].as_u64().unwrap(),
                "{row}"
            );
            assert_eq!(
                progress.excess as u64,
                after["excess"].as_u64().unwrap(),
                "{row}"
            );
            assert_eq!(
                phase_copy as u64,
                after["phase_copy"].as_u64().unwrap(),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires private original-Mac sound component fixtures"]
    fn source_block_request_matches_original_component_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_SOURCE_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 200);
        for row in rows {
            let array = |key: &str| -> [u32; 7] {
                std::array::from_fn(|i| row[key][i].as_u64().unwrap() as u32)
            };
            let mut block = MacSoundSourceBlock {
                fields: array("block"),
            };
            let next = array("next");
            let callback = (row["has_callback"].as_u64().unwrap() != 0)
                .then_some(|| (row["accepted"].as_u64().unwrap() != 0).then_some(next));
            let called = mac_sound_source_request(&mut block, callback);
            assert_eq!(
                called as u64,
                row["after"]["calls"].as_u64().unwrap(),
                "{row}"
            );
            let expected: [u32; 7] =
                std::array::from_fn(|i| row["after"]["block"][i].as_u64().unwrap() as u32);
            assert_eq!(block.fields, expected, "{row}");
            assert_eq!(row["after"]["out"].as_u64().unwrap(), 0x810000, "{row}");
        }
    }

    #[test]
    #[ignore = "requires private original-Mac Sound Manager fixtures"]
    fn double_buffer_dispatch_matches_original_manager_instructions() {
        use std::cell::RefCell;
        let path = std::env::var_os("CITYGAME_MAC_SOUND_MANAGER_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 300);
        for row in rows {
            let pair = |name: &str| {
                [
                    row[name][0].as_u64().unwrap() as u32,
                    row[name][1].as_u64().unwrap() as u32,
                ]
            };
            let mut buffers = MacSoundDoubleBuffer {
                state: row["state"].as_u64().unwrap() as u16,
                flags: pair("flags"),
                frames: pair("frames"),
            };
            let calls = RefCell::new(Vec::new());
            let result = mac_dispatch_sound_double_buffer(
                &mut buffers,
                |i| calls.borrow_mut().push(("play", i.map_or(0, |n| n + 1))),
                |i, flags| {
                    calls.borrow_mut().push(("callback", i + 1));
                    *flags |= row["callback_or"].as_u64().unwrap() as u32;
                },
            );
            let after = &row["after"];
            let expected: Vec<_> = after["calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| (v[0].as_str().unwrap(), v[1].as_u64().unwrap() as usize))
                .collect();
            assert_eq!(*calls.borrow(), expected, "{row}");
            assert_eq!(result as u64, after["result"].as_u64().unwrap(), "{row}");
            assert_eq!(
                buffers.state as u64,
                after["state"].as_u64().unwrap(),
                "{row}"
            );
            assert_eq!(
                buffers.flags,
                [
                    after["flags"][0].as_u64().unwrap() as u32,
                    after["flags"][1].as_u64().unwrap() as u32
                ],
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires private live Mac Sound Manager trace"]
    fn double_buffer_dispatch_matches_live_manager_sequence() {
        use std::cell::RefCell;
        let path = std::env::var_os("CITYGAME_MAC_SOUND_MANAGER_LIVE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(rows.len() > 100);
        let first = &rows[0];
        let pair = |row: &serde_json::Value, name: &str| {
            [
                row[name][0].as_u64().unwrap() as u32,
                row[name][1].as_u64().unwrap() as u32,
            ]
        };
        let mut buffers = MacSoundDoubleBuffer {
            state: first["state"].as_u64().unwrap() as u16,
            flags: pair(first, "flags"),
            frames: pair(first, "frames"),
        };
        for row in &rows {
            assert_eq!(
                buffers.state as u64,
                row["state"].as_u64().unwrap(),
                "{row}"
            );
            assert_eq!(buffers.flags, pair(row, "flags"), "{row}");
            assert_eq!(buffers.frames, pair(row, "frames"), "{row}");
            let calls = RefCell::new(Vec::new());
            assert!(mac_dispatch_sound_double_buffer(
                &mut buffers,
                |i| calls.borrow_mut().push(("play", i.map_or(0, |n| n + 1))),
                |i, flags| {
                    calls.borrow_mut().push(("callback", i + 1));
                    mac_refill_sound_buffer(flags, true, true, |_| {});
                }
            ));
            let after = &row["after"];
            let expected: Vec<_> = after["calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| (v[0].as_str().unwrap(), v[1].as_u64().unwrap() as usize))
                .collect();
            assert_eq!(*calls.borrow(), expected, "{row}");
            assert_eq!(
                buffers.state as u64,
                after["state"].as_u64().unwrap(),
                "{row}"
            );
            assert_eq!(buffers.flags, pair(after, "flags"), "{row}");
        }
    }

    #[test]
    #[ignore = "requires private original-Mac callback fixtures"]
    fn refill_matches_original_mdrv_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_CALLBACK_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 100);
        for row in rows {
            let mut flags = row["flags"].as_u64().unwrap() as u32;
            let mut calls = Vec::new();
            mac_refill_sound_buffer(
                &mut flags,
                row["channel"].as_u64().unwrap() != 0,
                row["half_rate"].as_u64().unwrap() != 0,
                |step| {
                    calls.push(match step {
                        MacSoundRefillStep::MixVoices => "mix",
                        MacSoundRefillStep::UpdateMusic => "music",
                        MacSoundRefillStep::Convert { half_rate: false } => "full",
                        MacSoundRefillStep::Convert { half_rate: true } => "half",
                    })
                },
            );
            assert_eq!(
                flags,
                row["after"]["flags"].as_u64().unwrap() as u32,
                "{row}"
            );
            let expected: Vec<_> = row["after"]["calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(calls, expected, "{row}");
        }
    }

    #[test]
    #[ignore = "requires private original-Mac mixer scan fixtures"]
    fn service_scan_matches_original_mdrv_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_SCAN_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 200);
        for row in rows {
            let mut voices: Vec<_> = row["states"]
                .as_array()
                .unwrap()
                .iter()
                .map(|state| MacOneShotVoice {
                    state: state.as_i64().unwrap() as i16,
                    position: 0,
                    end: 0,
                    pitch: 0,
                    phase: 0,
                    cached_span: 0,
                })
                .collect();
            let mut visited = Vec::new();
            assert_eq!(
                mac_scan_sound_service(
                    &mut voices,
                    row["start"].as_u64().unwrap() as u16,
                    row["count"].as_u64().unwrap() as u16,
                    |i, _| visited.push(i)
                ),
                Some(())
            );
            let expected: Vec<_> = row["visited"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_u64().unwrap() as usize)
                .collect();
            assert_eq!(visited, expected, "{row}");
        }
    }

    #[test]
    #[ignore = "requires private original-Mac mixer fixtures"]
    fn one_shot_service_matches_original_mdrv_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_SERVICE_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 200);
        for row in rows {
            let num = |key: &str| row[key].as_u64().unwrap() as u32;
            let mut voice = MacOneShotVoice {
                state: num("state") as i16,
                position: 0,
                end: num("length"),
                pitch: num("pitch"),
                phase: num("phase") as u16,
                cached_span: num("span"),
            };
            assert!(mac_service_one_shot_voice(
                &mut voice,
                370,
                370,
                num("half_rate") != 0
            ));
            let after = &row["after"];
            assert_eq!(
                voice.state as u16,
                after["state"].as_u64().unwrap() as u16,
                "{row}"
            );
            assert_eq!(
                voice.position,
                after["position"].as_u64().unwrap() as u32,
                "{row}"
            );
            assert_eq!(
                voice.phase,
                after["phase"].as_u64().unwrap() as u16,
                "{row}"
            );
            assert_eq!(
                voice.cached_span,
                after["span"].as_u64().unwrap() as u32,
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires private live Mac mixer trace"]
    fn one_shot_service_matches_live_sound_507_cycles() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_SERVICE_LIVE").unwrap();
        let fixture: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let runs = fixture["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 7);
        for run in runs {
            let steps = run.as_array().unwrap();
            assert_eq!(steps.len(), 247);
            let first = &steps[0]["before"];
            let n = |row: &serde_json::Value, key: &str| row[key].as_u64().unwrap() as u32;
            let mut voice = MacOneShotVoice {
                state: n(first, "state") as u16 as i16,
                position: n(first, "position"),
                end: n(first, "end"),
                pitch: n(first, "pitch"),
                phase: 0,
                cached_span: n(first, "span"),
            };
            for step in steps {
                let before = &step["before"];
                assert_eq!(
                    (
                        voice.state as u16,
                        voice.position,
                        voice.end,
                        voice.pitch,
                        voice.cached_span
                    ),
                    (
                        n(before, "state") as u16,
                        n(before, "position"),
                        n(before, "end"),
                        n(before, "pitch"),
                        n(before, "span")
                    )
                );
                assert_eq!((n(before, "transition"), n(before, "flag")), (0, 0));
                assert!(mac_service_one_shot_voice(
                    &mut voice,
                    fixture["span_frames"].as_u64().unwrap() as u16,
                    fixture["output_frames"].as_u64().unwrap() as u16,
                    fixture["half_rate"].as_bool().unwrap()
                ));
                let after = &step["after"];
                assert_eq!(
                    (voice.state as u16, voice.position, voice.cached_span),
                    (
                        n(after, "state") as u16,
                        n(after, "position"),
                        n(after, "span")
                    ),
                    "tick {}",
                    n(before, "tick")
                );
            }
            assert_eq!(voice.state, -1);
        }
    }

    #[test]
    #[ignore = "requires private original-Mac sound record fixtures"]
    fn record_allocation_matches_original_code9_and_mdrv_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_RECORD_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 160);
        for row in rows {
            let record = |r: &serde_json::Value| MacManagedSoundRecord {
                active: r["active"].as_u64().unwrap() != 0,
                descriptor: r["descriptor"].as_u64().unwrap() as u16,
                rank: r["rank"].as_u64().unwrap() as u32 as i32,
            };
            let voice = |v: &serde_json::Value| MacSoundVoice {
                descriptor: v["descriptor"].as_u64().unwrap() as u16,
                state: v["state"].as_u64().unwrap() as u16 as i16,
            };
            let mut records: Vec<_> = row["records"]
                .as_array()
                .unwrap()
                .iter()
                .map(record)
                .collect();
            let mut voices: Vec<_> = row["voices"]
                .as_array()
                .unwrap()
                .iter()
                .map(voice)
                .collect();
            let mut calls = Vec::new();
            let selected = mac_reserve_sound_record(
                &mut records,
                &mut voices,
                row["inhibit"].as_u64().unwrap() != 0,
                row["mode"].as_u64().unwrap() as u16,
                row["watermark"].as_u64().unwrap() as u32 as i32,
                |op, id| calls.push([op, id]),
            );
            assert_eq!(
                selected,
                Some(row["selected"].as_u64().unwrap() as usize),
                "{row}"
            );
            assert_eq!(
                records,
                row["after"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(record)
                    .collect::<Vec<_>>(),
                "{row}"
            );
            assert_eq!(
                voices,
                row["after_voices"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(voice)
                    .collect::<Vec<_>>(),
                "{row}"
            );
            assert_eq!(
                calls,
                row["calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| [v[0].as_u64().unwrap() as u16, v[1].as_u64().unwrap() as u16])
                    .collect::<Vec<_>>(),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires private original-Mac sound lifecycle fixtures"]
    fn voice_admission_and_stop_match_original_mdrv_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_LIFECYCLE_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 203);
        for row in rows {
            let to_voice = |value: &serde_json::Value| MacSoundVoice {
                descriptor: value["descriptor"].as_u64().unwrap() as u16,
                state: value["state"].as_u64().unwrap() as u16 as i16,
            };
            let mut voices: Vec<_> = row["voices"]
                .as_array()
                .unwrap()
                .iter()
                .map(to_voice)
                .collect();
            let descriptor = row["descriptor"].as_u64().unwrap() as u16;
            let selected = if row["operation"] == "start" {
                mac_start_sound_voice(&mut voices, descriptor, row["valid"].as_u64().unwrap() != 0)
            } else {
                mac_stop_sound_voices(&mut voices, descriptor);
                None
            };
            assert_eq!(
                selected,
                row["chosen"].as_u64().map(|v| v as usize),
                "{row}"
            );
            assert_eq!(
                voices,
                row["after"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(to_voice)
                    .collect::<Vec<_>>(),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires private original-Mac sound query fixtures"]
    fn descriptor_queries_match_original_code9_and_mdrv_instructions() {
        let path = std::env::var_os("CITYGAME_MAC_SOUND_QUERY_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 151);
        for row in rows {
            let count = row["count"].as_u64().unwrap() as usize;
            let records: Vec<_> = row["records"]
                .as_array()
                .unwrap()
                .iter()
                .take(count)
                .map(|r| MacSoundRecord {
                    active: r["active"].as_u64().unwrap() != 0,
                    descriptor: r["descriptor"].as_u64().unwrap() as u16,
                })
                .collect();
            let voices: Vec<_> = row["slots"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| MacSoundVoice {
                    descriptor: s["descriptor"].as_u64().unwrap() as u16,
                    state: s["state"].as_u64().unwrap() as u16 as i16,
                })
                .collect();
            let mut calls = Vec::new();
            let result = mac_sound_records_are_idle(
                row["unavailable"].as_u64().unwrap() != 0,
                row["inhibit"].as_u64().unwrap() != 0,
                row["mode"].as_u64().unwrap() as u16,
                &records,
                &voices,
                |id| calls.push(id),
            );
            assert_eq!(u64::from(result), row["result"].as_u64().unwrap(), "{row}");
            assert_eq!(
                calls,
                row["calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u16)
                    .collect::<Vec<_>>(),
                "{row}"
            );
        }
    }
}
