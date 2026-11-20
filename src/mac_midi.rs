//! Timing and track state of the Mac MDRV MIDI sequencer.
//!
//! One `service` corresponds to one delivered driver refill, not one frame or
//! TickCount. This models non-looping tracks and emits note events without
//! synthesizing them; the runtime still needs sound-manager callback timing.

const DRIVER_PERIOD_US: u32 = 16_667;
const MAX_TRACKS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MacMidiEvent {
    NoteOn {
        track: usize,
        channel: u8,
        key: u8,
        velocity: u8,
    },
    NoteOff {
        track: usize,
        channel: u8,
        key: u8,
        velocity: u8,
    },
}

/// Sequencer events relevant to the driver's program and voice state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MacMidiDriverEvent {
    Note(MacMidiEvent),
    ProgramChange {
        track: usize,
        channel: u8,
        program: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MacSongMetadata {
    pub midi_id: u16,
    pub transpose: u16,
    pub instrument_map: Vec<(u16, u16)>,
}

impl MacSongMetadata {
    pub(crate) fn parse(data: &[u8]) -> Result<Self, &'static str> {
        let word = |at: usize| {
            data.get(at..at + 2)
                .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
                .ok_or("short SONG resource")
        };
        let midi_id = word(0)?;
        let transpose = word(6)?;
        let count = usize::from(word(16)?);
        let end = 18_usize.checked_add(count * 4).ok_or("long SONG map")?;
        if end > data.len() {
            return Err("short SONG map");
        }
        let mut instrument_map = Vec::with_capacity(count);
        for index in 0..count {
            instrument_map.push((word(18 + index * 4)?, word(20 + index * 4)?));
        }
        Ok(Self {
            midi_id,
            transpose,
            instrument_map,
        })
    }
}

/// Fields read from an INST resource before MDRV resolves its sound handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MacInstrumentMetadata {
    pub sound_id: u16,
    pub base_key: u16,
    pub flags: u16,
    pub region_count: u16,
}

impl MacInstrumentMetadata {
    pub(crate) fn parse(data: &[u8]) -> Result<Self, &'static str> {
        let word = |offset: usize| {
            data.get(offset..offset + 2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .ok_or("short INST resource")
        };
        let region_count = word(12)?;
        let regions_end = 14 + usize::from(region_count) * 8;
        if regions_end > data.len() {
            return Err("short INST region table");
        }
        Ok(Self {
            sound_id: word(0)?,
            base_key: word(2)?,
            flags: word(6)?,
            region_count,
        })
    }

    /// MDRV offsets the note relative to middle C when the base key is set.
    pub(crate) fn adjusted_key(&self, key: u16) -> u16 {
        if self.base_key == 0 {
            key
        } else {
            key.wrapping_sub(self.base_key).wrapping_add(60)
        }
    }
}

/// Calculate the address MDRV uses at 0x329c..0x32f2. This is not a general
/// `snd ` parser: for example, its format-1 calculation assumes one modifier.
/// Callers must independently verify that this points to valid sample data.
pub(crate) fn mac_midi_sample_data_offset(data: &[u8]) -> Result<usize, &'static str> {
    let word = |offset: usize| {
        data.get(offset..offset + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .ok_or("short snd resource")
    };
    let (count_offset, header_base) = match word(0)? {
        1 => (10, 12_usize),
        2 => (4, 6_usize),
        _ => return Err("unsupported snd resource format"),
    };
    // ASL.W followed by ADDA.W sign-extends the 16-bit result.
    let displacement = word(count_offset)?.wrapping_shl(3) as i16;
    let header = header_base
        .checked_add_signed(displacement.into())
        .ok_or("invalid snd command count")?;
    let samples = header.checked_add(22).ok_or("invalid snd header")?;
    if samples > data.len() {
        return Err("short snd header");
    }
    Ok(samples)
}

/// Offsets from the first sample byte held in the original voice state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MacMidiSampleBounds {
    pub last_sample: u32,
    pub loop_start: Option<u32>,
    pub loop_end: Option<u32>,
}

/// The MDRV loop initialization at 0x33f0..0x3434. This models the driver's
/// arithmetic; callers must validate offsets before indexing sample storage.
pub(crate) fn mac_midi_sample_bounds(
    sample_length: u32,
    loop_start: u32,
    loop_end: u32,
    instrument_flags_low: u8,
) -> MacMidiSampleBounds {
    let looping = loop_start != 0
        && instrument_flags_low & 0x20 == 0
        && loop_end != 0
        && (loop_end.wrapping_sub(loop_start) as u16) >= 100;
    MacMidiSampleBounds {
        last_sample: sample_length.wrapping_sub(1),
        loop_start: looping.then_some(loop_start),
        loop_end: looping.then_some(loop_end),
    }
}

/// Initial pitch values before the optional random pitch modulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MacMidiPitch {
    pub step: u32,
    pub scaled_step: u32,
}

/// The verified MDRV table imported alongside a user's selected Mac pack.
pub(crate) fn mac_midi_pitch_table(data: &[u8]) -> Result<Vec<u32>, &'static str> {
    if data.len() != 132 * 4 {
        return Err("invalid MDRV pitch table length");
    }
    Ok(data
        .chunks_exact(4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .collect())
}

/// MDRV 0x3342..0x3376 and the corresponding unmodulated 0x33a4 shift.
/// The table comes from the user's original driver; `table_origin` identifies
/// its address within the supplied slice, including any preceding words.
pub(crate) fn mac_midi_initial_pitch(
    adjusted_key: u16,
    sample_pitch_word: u16,
    instrument_flags: u16,
    table: &[u32],
    table_origin: usize,
) -> Option<MacMidiPitch> {
    let mut step = if instrument_flags & 0x4000 != 0 {
        0x0002_0000
    } else {
        let byte_offset = adjusted_key
            .wrapping_add(60)
            .wrapping_sub(sample_pitch_word)
            .wrapping_mul(4) as i16;
        let index = table_origin.checked_add_signed(isize::from(byte_offset) / 4)?;
        *table.get(index)?
    };
    if (step as u16) < 4 {
        step &= 0xffff_0000;
    }
    Some(MacMidiPitch {
        step,
        scaled_step: step >> 10,
    })
}

/// Inputs used by the MDRV note wrappers before they call the voice helpers.
/// `track_base` is the low word of the native track-table base address.
pub(crate) struct MacMidiVoiceRouting<'a> {
    pub use_track_program: bool,
    pub track_programs: &'a [u16],
    pub channel_programs: &'a [u8; 16],
    pub song_map: &'a [(u16, u16)],
    pub transpose: u16,
    pub track_base: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MacMidiVoiceRequest {
    pub on: bool,
    pub instrument: u16,
    pub key: u16,
    pub velocity: u8,
    pub channel: u8,
    pub track_tag: u16,
    /// MDRV supplies priority 10000 for note-on; note-off does not set it.
    pub priority: Option<u16>,
}

#[derive(Debug, Clone)]
pub(crate) struct MacMidiPrograms {
    pub track: [u16; MAX_TRACKS],
    pub channel: [u8; 16],
}

impl Default for MacMidiPrograms {
    fn default() -> Self {
        Self {
            track: [0; MAX_TRACKS],
            channel: std::array::from_fn(|index| index as u8),
        }
    }
}

impl MacMidiPrograms {
    /// Apply driver-gated MIDI program changes or route a note to its voice.
    pub(crate) fn handle_event(
        &mut self,
        event: MacMidiDriverEvent,
        driver_flags: u16,
        song: &MacSongMetadata,
        track_base: u16,
    ) -> Option<MacMidiVoiceRequest> {
        match event {
            MacMidiDriverEvent::ProgramChange {
                track,
                channel,
                program,
            } => {
                if driver_flags & 0x0400 != 0 && track < MAX_TRACKS && channel < 16 {
                    self.track[track] = u16::from(program);
                    self.channel[channel as usize] = program;
                }
                None
            }
            MacMidiDriverEvent::Note(note) => mac_midi_route_voice(
                note,
                &MacMidiVoiceRouting {
                    use_track_program: driver_flags & 0x0800 != 0,
                    track_programs: &self.track,
                    channel_programs: &self.channel,
                    song_map: &song.instrument_map,
                    transpose: song.transpose,
                    track_base,
                },
            ),
        }
    }
}

pub(crate) fn mac_midi_route_voice(
    event: MacMidiEvent,
    routing: &MacMidiVoiceRouting<'_>,
) -> Option<MacMidiVoiceRequest> {
    let (on, track, channel, key, velocity) = match event {
        MacMidiEvent::NoteOn {
            track,
            channel,
            key,
            velocity,
        } => (true, track, channel, key, velocity),
        MacMidiEvent::NoteOff {
            track,
            channel,
            key,
            velocity,
        } => (false, track, channel, key, velocity),
    };
    if track >= MAX_TRACKS || channel >= 16 {
        return None;
    }
    let program = if routing.use_track_program {
        *routing.track_programs.get(track)?
    } else {
        u16::from(routing.channel_programs[channel as usize])
    };
    let instrument = routing
        .song_map
        .iter()
        .find_map(|&(from, to)| (from == program).then_some(to))
        .unwrap_or(program);
    let transposed = u16::from(key).wrapping_add(routing.transpose);
    let key = if transposed as u8 & 0x80 != 0 {
        u16::from(key)
    } else {
        transposed
    };
    Some(MacMidiVoiceRequest {
        on,
        instrument,
        key,
        velocity,
        channel,
        track_tag: routing
            .track_base
            .wrapping_add((track as u16).wrapping_mul(4)),
        priority: on.then_some(10_000),
    })
}

/// The voice fields consulted by MDRV's note-on selection and note-off scan.
/// Negative status is free, zero is releasing, and positive is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MacMidiVoice {
    pub status: i16,
    pub track_tag: u16,
    pub channel: u16,
    pub key: u16,
    pub instrument: u16,
    pub release_flags: u16,
}

/// Selects the voice at the original driver's 0x3188..0x3242 decision point.
pub(crate) fn mac_midi_select_voice(
    voices: &[MacMidiVoice],
    request: &MacMidiVoiceRequest,
    instrument_flags: u16,
    driver_flags: u16,
    active_limit: u16,
) -> Option<usize> {
    let mut active = 0_u16;
    for (index, voice) in voices.iter().enumerate() {
        if voice.status >= 0 {
            active = active.wrapping_add(1);
        }
        if voice.key == request.key
            && voice.channel == u16::from(request.channel)
            && voice.track_tag == request.track_tag
            && voice.instrument == request.instrument
            && (instrument_flags & 0x0400 != 0 || voice.status == 0)
        {
            return Some(index);
        }
    }
    if driver_flags & 0x4000 != 0 && active >= active_limit {
        if let Some(index) = voices.iter().position(|voice| voice.status == 0) {
            return Some(index);
        }
    } else if let Some(index) = voices.iter().position(|voice| voice.status < 0) {
        return Some(index);
    }
    voices.iter().position(|voice| voice.status <= 0)
}

/// Releases every matching voice, including voices already in release.
pub(crate) fn mac_midi_release_voices(
    voices: &mut [MacMidiVoice],
    request: &MacMidiVoiceRequest,
    driver_flags: u16,
) {
    for voice in voices {
        if voice.status >= 0
            && voice.key == request.key
            && voice.track_tag == request.track_tag
            && voice.channel == u16::from(request.channel)
        {
            voice.status = 0;
            voice.release_flags = driver_flags & 0x3f;
        }
    }
}

#[derive(Debug, Clone)]
struct Track {
    data: Vec<u8>,
    cursor: usize,
    remaining: u32,
    first: bool,
    active: bool,
    running: u8,
}

impl Track {
    fn byte(&mut self) -> Result<u8, &'static str> {
        let byte = *self.data.get(self.cursor).ok_or("short MIDI track")?;
        self.cursor += 1;
        Ok(byte)
    }

    fn variable(&mut self) -> Result<u32, &'static str> {
        let mut value = 0_u32;
        for _ in 0..4 {
            let byte = self.byte()?;
            value = (value << 7) | u32::from(byte & 0x7f);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err("long MIDI variable integer")
    }

    fn next_delta(&mut self) -> Result<bool, &'static str> {
        let delta = self.variable()?;
        if delta == 0 {
            return Ok(true);
        }
        self.remaining = self.remaining.wrapping_add(delta.wrapping_shl(6));
        Ok((self.remaining as i32) < 0)
    }

    fn service(
        &mut self,
        track: usize,
        division: u16,
        step: &mut u32,
        emit: &mut impl FnMut(MacMidiDriverEvent),
    ) -> Result<(), &'static str> {
        let mut due = if self.first {
            self.first = false;
            self.next_delta()?
        } else {
            let (value, borrow) = self.remaining.overflowing_sub(*step);
            self.remaining = value;
            borrow
        };
        while due {
            let mut status = self.byte()?;
            if status < 0x80 {
                self.cursor -= 1;
                status = self.running;
            }
            if status == 0xff {
                let kind = self.byte()?;
                if kind == 0x2f {
                    self.active = false;
                    return Ok(());
                }
                let length = self.variable()? as usize;
                let end = self
                    .cursor
                    .checked_add(length)
                    .filter(|&end| end <= self.data.len())
                    .ok_or("short MIDI metadata")?;
                if kind == 0x51 {
                    if length != 3 {
                        return Err("invalid MIDI tempo");
                    }
                    let tempo = u32::from_be_bytes([
                        0,
                        self.data[self.cursor],
                        self.data[self.cursor + 1],
                        self.data[self.cursor + 2],
                    ]);
                    *step = tempo_step(division, tempo)?;
                }
                self.cursor = end;
            } else if (0x80..=0xef).contains(&status) {
                self.running = status;
                let key = self.byte()?;
                if !matches!(status & 0xf0, 0xc0 | 0xd0) {
                    let velocity = self.byte()?;
                    let channel = status & 0x0f;
                    match status & 0xf0 {
                        0x90 if velocity != 0 => {
                            emit(MacMidiDriverEvent::Note(MacMidiEvent::NoteOn {
                                track,
                                channel,
                                key,
                                velocity,
                            }))
                        }
                        0x80 | 0x90 => emit(MacMidiDriverEvent::Note(MacMidiEvent::NoteOff {
                            track,
                            channel,
                            key,
                            velocity,
                        })),
                        _ => {}
                    }
                } else if status & 0xf0 == 0xc0 {
                    emit(MacMidiDriverEvent::ProgramChange {
                        track,
                        channel: status & 0x0f,
                        program: key,
                    });
                }
            } else {
                return Err("unsupported MIDI event");
            }
            due = self.next_delta()?;
        }
        Ok(())
    }
}

fn tempo_step(division: u16, tempo_us: u32) -> Result<u32, &'static str> {
    let denominator = tempo_us / DRIVER_PERIOD_US;
    if denominator == 0 || denominator > u16::MAX as u32 {
        return Err("unsupported MIDI tempo");
    }
    let step = (u32::from(division) << 6) / denominator;
    if step > u16::MAX as u32 {
        return Err("unsupported MIDI timebase");
    }
    Ok(step)
}

#[derive(Debug, Clone)]
pub(crate) struct MacMidiSequencer {
    tracks: Vec<Track>,
    division: u16,
    step: u32,
}

impl MacMidiSequencer {
    pub(crate) fn new(data: &[u8]) -> Result<Self, &'static str> {
        if data.get(..4) != Some(b"MThd") {
            return Err("missing MIDI header");
        }
        let read16 = |at| {
            data.get(at..at + 2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .ok_or("short MIDI header")
        };
        let read32 = |at| {
            data.get(at..at + 4)
                .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or("short MIDI integer")
        };
        let header_len = read32(4)? as usize;
        if header_len < 6 {
            return Err("short MIDI header");
        }
        let count = read16(10)? as usize;
        let division = read16(12)?;
        if count == 0 || count > MAX_TRACKS || division == 0 || division & 0x8000 != 0 {
            return Err("unsupported MIDI track count or timebase");
        }
        let mut cursor = 8_usize.checked_add(header_len).ok_or("long MIDI header")?;
        let mut tracks = Vec::with_capacity(count);
        for _ in 0..count {
            if data.get(cursor..cursor + 4) != Some(b"MTrk") {
                return Err("missing MIDI track");
            }
            let length = read32(cursor + 4)? as usize;
            cursor = cursor.checked_add(8).ok_or("long MIDI track")?;
            let end = cursor
                .checked_add(length)
                .filter(|&end| end <= data.len())
                .ok_or("short MIDI track")?;
            tracks.push(Track {
                data: data[cursor..end].to_vec(),
                cursor: 0,
                remaining: 0,
                first: true,
                active: true,
                running: 0,
            });
            cursor = end;
        }
        Ok(Self {
            tracks,
            division,
            step: tempo_step(division, 500_000)?,
        })
    }

    pub(crate) fn active(&self) -> bool {
        self.tracks.iter().any(|track| track.active)
    }

    pub(crate) fn service(&mut self) -> Result<(), &'static str> {
        self.service_with_events(|_| {})
    }

    pub(crate) fn service_with_events(
        &mut self,
        mut emit: impl FnMut(MacMidiEvent),
    ) -> Result<(), &'static str> {
        self.service_with_driver_events(|event| {
            if let MacMidiDriverEvent::Note(note) = event {
                emit(note);
            }
        })
    }

    pub(crate) fn service_with_driver_events(
        &mut self,
        mut emit: impl FnMut(MacMidiDriverEvent),
    ) -> Result<(), &'static str> {
        for (index, track) in self.tracks.iter_mut().enumerate() {
            if track.active {
                track.service(index, self.division, &mut self.step, &mut emit)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mac_sound_state::{MacSoundRefillStep, mac_refill_sound_buffer};

    #[test]
    fn song_metadata_reads_transpose_and_ordered_instrument_map() {
        let mut bytes = vec![0_u8; 18];
        bytes[0..2].copy_from_slice(&10018_u16.to_be_bytes());
        bytes[6..8].copy_from_slice(&0xffff_u16.to_be_bytes());
        bytes[16..18].copy_from_slice(&2_u16.to_be_bytes());
        for word in [3_u16, 9, 3, 12] {
            bytes.extend_from_slice(&word.to_be_bytes());
        }
        let song = MacSongMetadata::parse(&bytes).unwrap();
        assert_eq!((song.midi_id, song.transpose), (10018, 0xffff));
        assert_eq!(song.instrument_map, [(3, 9), (3, 12)]);
        bytes.pop();
        assert_eq!(MacSongMetadata::parse(&bytes), Err("short SONG map"));
    }

    #[test]
    fn instrument_and_sound_headers_are_checked() {
        let mut inst = [0_u8; 22];
        inst[0..2].copy_from_slice(&10015_u16.to_be_bytes());
        inst[2..4].copy_from_slice(&84_u16.to_be_bytes());
        let metadata = MacInstrumentMetadata::parse(&inst).unwrap();
        assert_eq!((metadata.sound_id, metadata.adjusted_key(60)), (10015, 36));
        inst[12..14].copy_from_slice(&2_u16.to_be_bytes());
        assert_eq!(
            MacInstrumentMetadata::parse(&inst),
            Err("short INST region table")
        );
        let mut snd = [0_u8; 42];
        snd[1] = 1;
        snd[11] = 1;
        assert_eq!(mac_midi_sample_data_offset(&snd), Ok(42));
        snd[1] = 2;
        snd[11] = 0;
        snd[5] = 1;
        assert_eq!(mac_midi_sample_data_offset(&snd), Ok(36));
        assert_eq!(
            mac_midi_sample_data_offset(&snd[..35]),
            Err("short snd header")
        );
    }

    #[test]
    fn sample_loop_requires_at_least_one_hundred_bytes() {
        let short = mac_midi_sample_bounds(1000, 1, 100, 0);
        assert_eq!(
            short,
            MacMidiSampleBounds {
                last_sample: 999,
                loop_start: None,
                loop_end: None
            }
        );
        let long = mac_midi_sample_bounds(1000, 1, 101, 0);
        assert_eq!((long.loop_start, long.loop_end), (Some(1), Some(101)));
        assert_eq!(mac_midi_sample_bounds(1000, 1, 101, 0x20).loop_start, None);
    }

    #[test]
    fn pitch_rounding_and_forced_step_match_driver() {
        let table = [0x1234_0003, 0x1234_0004];
        assert_eq!(
            mac_midi_initial_pitch(0, 60, 0, &table, 0),
            Some(MacMidiPitch {
                step: 0x1234_0000,
                scaled_step: 0x1234_0000 >> 10
            })
        );
        assert_eq!(
            mac_midi_initial_pitch(1, 60, 0, &table, 0).unwrap().step,
            0x1234_0004
        );
        assert_eq!(
            mac_midi_initial_pitch(0, 60, 0x4000, &[], 0).unwrap().step,
            0x20000
        );
        assert_eq!(mac_midi_initial_pitch(2, 60, 0, &table, 0), None);
    }

    #[test]
    #[ignore = "requires original-instruction pitch fixture"]
    fn pitch_lookup_matches_original_instructions() {
        let data = std::fs::read(std::env::var_os("CITYGAME_MAC_PITCH_FIXTURE").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&data).unwrap();
        assert_eq!(rows.len(), 560);
        let mut table = vec![0_u32; 16384];
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as u32;
            let offset = (n("key") as u16)
                .wrapping_add(60)
                .wrapping_sub(n("pitch") as u16)
                .wrapping_mul(4) as i16;
            let index = (8192_isize + isize::from(offset) / 4) as usize;
            table[index] = n("value");
            let actual = mac_midi_initial_pitch(
                n("key") as u16,
                n("pitch") as u16,
                n("flags") as u16,
                &table,
                8192,
            )
            .unwrap();
            assert_eq!(
                actual,
                MacMidiPitch {
                    step: n("step"),
                    scaled_step: n("scaled")
                },
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires original-instruction sample bounds fixture"]
    fn sample_bounds_match_original_instructions() {
        let data = std::fs::read(std::env::var_os("CITYGAME_MAC_SAMPLE_BOUNDS").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&data).unwrap();
        assert_eq!(rows.len(), 474);
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as u32;
            let expected = MacMidiSampleBounds {
                last_sample: n("last"),
                loop_start: row["loop_start"].as_u64().map(|v| v as u32),
                loop_end: row["loop_end"].as_u64().map(|v| v as u32),
            };
            assert_eq!(
                mac_midi_sample_bounds(n("length"), n("start"), n("end"), n("flag") as u8),
                expected,
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires user-owned Mac resources and original-instruction fixture"]
    fn original_instruments_and_sound_headers_match_driver() {
        let root =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_RESOURCE_ROOT").unwrap());
        let fixture =
            std::fs::read(std::env::var_os("CITYGAME_MAC_SAMPLE_HEADER").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&fixture).unwrap();
        assert_eq!(rows.len(), 45);
        for row in rows {
            let data = std::fs::read(root.join("snd").join(row["file"].as_str().unwrap())).unwrap();
            assert_eq!(
                mac_midi_sample_data_offset(&data).unwrap(),
                row["offset"].as_u64().unwrap() as usize,
                "{row}"
            );
        }
        let instruments: Vec<_> = std::fs::read_dir(root.join("INST"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "bin"))
            .collect();
        assert_eq!(instruments.len(), 17);
        for path in instruments {
            let data = std::fs::read(&path).unwrap();
            let metadata = MacInstrumentMetadata::parse(&data).unwrap();
            assert_eq!(metadata.region_count, 0, "{}", path.display());
            let matching = std::fs::read_dir(root.join("snd")).unwrap().any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&format!("snd_{:05}_", metadata.sound_id))
            });
            assert!(
                matching,
                "{} references missing snd {}",
                path.display(),
                metadata.sound_id
            );
        }
    }

    #[test]
    #[ignore = "requires the user's original Mac SONG resource"]
    fn song_metadata_matches_original_10018() {
        let data = std::fs::read(std::env::var_os("CITYGAME_MAC_SONG_RESOURCE").unwrap()).unwrap();
        let song = MacSongMetadata::parse(&data).unwrap();
        assert_eq!((song.midi_id, song.transpose), (10018, 0));
        assert_eq!(song.instrument_map.len(), 17);
        assert_eq!(song.instrument_map[0], (0, 1));
        assert_eq!(song.instrument_map[14], (14, 25));
        assert_eq!(song.instrument_map[16], (16, 16));
    }

    #[test]
    fn song_mapping_uses_first_match_and_rejects_negative_transposed_note() {
        let mut channels = [0_u8; 16];
        channels[3] = 3;
        let mapping = [(3, 9), (3, 12)];
        let routing = MacMidiVoiceRouting {
            use_track_program: false,
            track_programs: &[],
            channel_programs: &channels,
            song_map: &mapping,
            transpose: 1,
            track_base: 0x4100,
        };
        let request = mac_midi_route_voice(
            MacMidiEvent::NoteOn {
                track: 2,
                channel: 3,
                key: 127,
                velocity: 77,
            },
            &routing,
        )
        .unwrap();
        assert_eq!(
            (request.instrument, request.key, request.track_tag),
            (9, 127, 0x4108)
        );
        assert_eq!(request.priority, Some(10_000));
    }

    #[test]
    fn program_changes_obey_gate_and_program_source() {
        let song = MacSongMetadata {
            midi_id: 1,
            transpose: 0,
            instrument_map: vec![(5, 9), (4, 10)],
        };
        let mut programs = MacMidiPrograms::default();
        let change = MacMidiDriverEvent::ProgramChange {
            track: 2,
            channel: 3,
            program: 5,
        };
        programs.handle_event(change, 0, &song, 0x4100);
        assert_eq!(programs.channel[3], 3);
        programs.handle_event(change, 0x0400, &song, 0x4100);
        programs.channel[3] = 4;
        let note = MacMidiDriverEvent::Note(MacMidiEvent::NoteOn {
            track: 2,
            channel: 3,
            key: 60,
            velocity: 80,
        });
        assert_eq!(
            programs
                .handle_event(note, 0, &song, 0x4100)
                .unwrap()
                .instrument,
            10
        );
        assert_eq!(
            programs
                .handle_event(note, 0x0800, &song, 0x4100)
                .unwrap()
                .instrument,
            9
        );
    }

    #[test]
    #[ignore = "requires user Mac MIDI and original-instruction program fixture"]
    fn program_and_note_events_match_original_driver() {
        let midi = std::fs::read(std::env::var_os("CITYGAME_MAC_MIDI").unwrap()).unwrap();
        let data = std::fs::read(std::env::var_os("CITYGAME_MAC_PROGRAM_EVENTS").unwrap()).unwrap();
        let fixture: serde_json::Value = serde_json::from_slice(&data).unwrap();
        let mut sequencer = MacMidiSequencer::new(&midi).unwrap();
        let mut events = Vec::new();
        let mut call = 0;
        while sequencer.active() && call < 6200 {
            call += 1;
            sequencer
                .service_with_driver_events(|event| {
                    let row = match event {
                        MacMidiDriverEvent::ProgramChange {
                            track,
                            channel,
                            program,
                        } => serde_json::json!([call, track, "program", channel, program]),
                        MacMidiDriverEvent::Note(MacMidiEvent::NoteOn {
                            track,
                            channel,
                            key,
                            velocity,
                        }) => serde_json::json!([call, track, "on", channel, key, velocity]),
                        MacMidiDriverEvent::Note(MacMidiEvent::NoteOff {
                            track,
                            channel,
                            key,
                            velocity,
                        }) => serde_json::json!([call, track, "off", channel, key, velocity]),
                    };
                    events.push(row);
                })
                .unwrap();
        }
        assert_eq!(call, fixture["calls"].as_u64().unwrap() as usize);
        assert_eq!(events, *fixture["events"].as_array().unwrap());
    }

    #[test]
    fn voice_selection_and_release_keep_track_identity() {
        let request = MacMidiVoiceRequest {
            on: true,
            instrument: 9,
            key: 60,
            velocity: 77,
            channel: 3,
            track_tag: 0x4100,
            priority: Some(10_000),
        };
        let mut voices = [
            MacMidiVoice {
                status: 1,
                track_tag: 0x4100,
                channel: 3,
                key: 60,
                instrument: 9,
                release_flags: 0,
            },
            MacMidiVoice {
                status: 0,
                track_tag: 0x4104,
                channel: 3,
                key: 60,
                instrument: 9,
                release_flags: 0,
            },
            MacMidiVoice {
                status: -1,
                track_tag: 0x4100,
                channel: 3,
                key: 60,
                instrument: 9,
                release_flags: 0,
            },
        ];
        assert_eq!(
            mac_midi_select_voice(&voices, &request, 0x0400, 0, 0),
            Some(0)
        );
        assert_eq!(mac_midi_select_voice(&voices, &request, 0, 0, 0), Some(2));
        assert_eq!(
            mac_midi_select_voice(&voices, &request, 0, 0x4000, 1),
            Some(1)
        );
        mac_midi_release_voices(&mut voices, &request, 0x4035);
        assert_eq!((voices[0].status, voices[0].release_flags), (0, 0x35));
        assert_eq!((voices[1].status, voices[1].release_flags), (0, 0));
        assert_eq!((voices[2].status, voices[2].release_flags), (-1, 0));
    }

    #[test]
    #[ignore = "requires private original-instruction voice selection fixture"]
    fn voice_selection_and_release_match_original_instructions() {
        let data =
            std::fs::read(std::env::var_os("CITYGAME_MAC_VOICE_SELECTION").unwrap()).unwrap();
        let fixture: Vec<serde_json::Value> = serde_json::from_slice(&data).unwrap();
        assert_eq!(fixture.len(), 1728);
        let request = MacMidiVoiceRequest {
            on: true,
            instrument: 9,
            key: 60,
            velocity: 77,
            channel: 3,
            track_tag: 0x4100,
            priority: Some(10_000),
        };
        for row in fixture {
            let num = |key: &str| row[key].as_u64().unwrap() as u16;
            let statuses = row["statuses"].as_array().unwrap();
            let mut voices: Vec<_> = statuses
                .iter()
                .enumerate()
                .map(|(index, status)| MacMidiVoice {
                    status: status.as_i64().unwrap() as i16,
                    track_tag: if index == num("match") as usize {
                        0x4100
                    } else {
                        0x4104
                    },
                    channel: 3,
                    key: 60,
                    instrument: 9,
                    release_flags: 0x7777,
                })
                .collect();
            let expected = row["selected"].as_u64().map(|n| n as usize);
            assert_eq!(
                mac_midi_select_voice(
                    &voices,
                    &request,
                    num("iflags"),
                    num("dflags"),
                    num("limit")
                ),
                expected,
                "{row}"
            );
            mac_midi_release_voices(&mut voices, &request, num("dflags"));
            for (voice, expected) in voices.iter().zip(row["released"].as_array().unwrap()) {
                assert_eq!(
                    (voice.status as u16, voice.release_flags),
                    (
                        expected[0].as_u64().unwrap() as u16,
                        expected[1].as_u64().unwrap() as u16
                    ),
                    "{row}"
                );
            }
        }
    }

    #[test]
    #[ignore = "requires private original-instruction mapping fixture"]
    fn song_voice_routing_matches_original_instructions() {
        let data = std::fs::read(std::env::var_os("CITYGAME_MAC_MIDI_MAPPING").unwrap()).unwrap();
        let fixture: Vec<serde_json::Value> = serde_json::from_slice(&data).unwrap();
        assert_eq!(fixture.len(), 864);
        for row in fixture {
            let num = |key: &str| row[key].as_u64().unwrap() as u16;
            let channel = num("channel") as u8;
            let program = num("program");
            let mut channels = [0_u8; 16];
            channels[channel as usize] = program as u8;
            let track_programs = [program];
            let mapping: Vec<_> = row["map"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    (
                        pair[0].as_u64().unwrap() as u16,
                        pair[1].as_u64().unwrap() as u16,
                    )
                })
                .collect();
            let on = row["on"].as_bool().unwrap();
            let key = num("key") as u8;
            let event = if on {
                MacMidiEvent::NoteOn {
                    track: 0,
                    channel,
                    key,
                    velocity: 77,
                }
            } else {
                MacMidiEvent::NoteOff {
                    track: 0,
                    channel,
                    key,
                    velocity: 77,
                }
            };
            let routing = MacMidiVoiceRouting {
                use_track_program: num("mode") != 0,
                track_programs: &track_programs,
                channel_programs: &channels,
                song_map: &mapping,
                transpose: num("transpose"),
                track_base: 0x4100,
            };
            let request = mac_midi_route_voice(event, &routing).unwrap();
            let expected = row["result"].as_array().unwrap();
            let e = |index: usize| expected[index].as_u64().unwrap() as u16;
            assert_eq!(
                (
                    request.instrument,
                    request.key,
                    u16::from(request.velocity),
                    u16::from(request.channel),
                    request.track_tag
                ),
                (e(0), e(1), e(3), e(4), e(5)),
                "{row}"
            );
            assert_eq!(request.priority, on.then_some(e(2)));
        }
    }

    #[test]
    fn refill_advances_the_music_tracks_until_their_end_events() {
        // Two simultaneous tracks share the driver's integer tempo step.
        let t0: &[u8] = &[0, 0xff, 0x51, 3, 0x07, 0xa1, 0x20, 120, 0xff, 0x2f, 0];
        let t1: &[u8] = &[0, 0x90, 60, 80, 60, 0x80, 60, 0, 60, 0xff, 0x2f, 0];
        let mut midi = b"MThd\0\0\0\x06\0\x01\0\x02\0\x78".to_vec();
        for track in [t0, t1] {
            midi.extend_from_slice(b"MTrk");
            midi.extend_from_slice(&(track.len() as u32).to_be_bytes());
            midi.extend_from_slice(track);
        }
        let mut sequencer = MacMidiSequencer::new(&midi).unwrap();
        let mut flags = 0;
        let mut callbacks = 0;
        while sequencer.active() && callbacks < 100 {
            mac_refill_sound_buffer(&mut flags, true, true, |kind| {
                if kind == MacSoundRefillStep::UpdateMusic {
                    sequencer.service().unwrap();
                }
            });
            callbacks += 1;
        }
        assert!(!sequencer.active());
        assert_eq!(callbacks, 31);
        assert_eq!(flags, 1);
    }

    #[test]
    fn note_events_match_original_driver_order_and_running_status() {
        let track: &[u8] = &[
            0, 0x91, 60, 90, 0, 64, 0, 0, 0x81, 60, 30, 0, 0xb1, 7, 100, 0, 0xc1, 4, 4, 0x92, 67,
            100, 0, 0x82, 67, 20, 10, 0xff, 0x2f, 0,
        ];
        let mut midi = b"MThd\0\0\0\x06\0\0\0\x01\0\x78MTrk".to_vec();
        midi.extend_from_slice(&(track.len() as u32).to_be_bytes());
        midi.extend_from_slice(track);
        let mut sequencer = MacMidiSequencer::new(&midi).unwrap();
        let mut events = Vec::new();
        for call in 1..=5 {
            sequencer
                .service_with_events(|event| events.push((call, event)))
                .unwrap();
        }
        assert_eq!(
            events,
            [
                (
                    1,
                    MacMidiEvent::NoteOn {
                        track: 0,
                        channel: 1,
                        key: 60,
                        velocity: 90
                    }
                ),
                (
                    1,
                    MacMidiEvent::NoteOff {
                        track: 0,
                        channel: 1,
                        key: 64,
                        velocity: 0
                    }
                ),
                (
                    1,
                    MacMidiEvent::NoteOff {
                        track: 0,
                        channel: 1,
                        key: 60,
                        velocity: 30
                    }
                ),
                (
                    2,
                    MacMidiEvent::NoteOn {
                        track: 0,
                        channel: 2,
                        key: 67,
                        velocity: 100
                    }
                ),
                (
                    2,
                    MacMidiEvent::NoteOff {
                        track: 0,
                        channel: 2,
                        key: 67,
                        velocity: 20
                    }
                ),
            ]
        );
        assert!(!sequencer.active());
    }

    #[test]
    #[ignore = "requires private Mac MIDI and original-instruction event fixture"]
    fn note_events_match_original_multitrack_driver() {
        let midi = std::fs::read(std::env::var_os("CITYGAME_MAC_MIDI").unwrap()).unwrap();
        let fixture = std::fs::read(std::env::var_os("CITYGAME_MAC_MIDI_EVENTS").unwrap()).unwrap();
        let fixture: serde_json::Value = serde_json::from_slice(&fixture).unwrap();
        let mut sequencer = MacMidiSequencer::new(&midi).unwrap();
        let mut events = Vec::new();
        let mut call = 0;
        while sequencer.active() && call < 6200 {
            call += 1;
            sequencer
                .service_with_events(|event| {
                    let (kind, track, channel, key, velocity) = match event {
                        MacMidiEvent::NoteOn {
                            track,
                            channel,
                            key,
                            velocity,
                        } => ("on", track, channel, key, velocity),
                        MacMidiEvent::NoteOff {
                            track,
                            channel,
                            key,
                            velocity,
                        } => ("off", track, channel, key, velocity),
                    };
                    events.push(serde_json::json!([
                        call, track, kind, channel, key, velocity
                    ]));
                })
                .unwrap();
        }
        assert_eq!(call, fixture["calls"].as_u64().unwrap() as usize);
        assert_eq!(events, *fixture["events"].as_array().unwrap());
        assert!(!sequencer.active());
    }

    #[test]
    #[ignore = "requires private Mac MIDI and a same-session driver trace"]
    fn multitrack_timing_matches_original_live_driver() {
        let midi = std::fs::read(std::env::var_os("CITYGAME_MAC_MIDI").unwrap()).unwrap();
        let trace =
            std::fs::read_to_string(std::env::var_os("CITYGAME_MAC_MIDI_TRACE").unwrap()).unwrap();
        let song = std::env::var("CITYGAME_MAC_MIDI_SONG_HEX").unwrap_or_else(|_| "2722".into());
        let expected_count: usize = std::env::var("CITYGAME_MAC_MIDI_SERVICE_COUNT")
            .unwrap_or_else(|_| "6111".into())
            .parse()
            .unwrap();
        let mut sequencer = MacMidiSequencer::new(&midi).unwrap();
        let mut origin = None;
        let mut matched = 0;
        for line in trace.lines().skip(1) {
            let c: Vec<_> = line.split(',').collect();
            if c.len() < 15
                || c[0] != "service"
                || c[4] != song
                || c[5] != "ff00"
                || c[7] == "000000"
            {
                continue;
            }
            let hex = |s: &str| u32::from_str_radix(s, 16).unwrap();
            let mask: u32 = sequencer
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, track)| track.active)
                .map(|(i, _)| 1 << i)
                .sum();
            let ptr = hex(c[14]);
            let start = *origin.get_or_insert(ptr);
            assert_eq!(
                (
                    mask,
                    sequencer.tracks[0].cursor as u32,
                    sequencer.tracks[0].remaining,
                    sequencer.step
                ),
                (hex(c[7]), ptr - start, hex(c[13]), hex(c[12])),
                "row {matched}"
            );
            sequencer.service().unwrap();
            matched += 1;
        }
        assert_eq!(matched, expected_count);
        assert!(!sequencer.active());
    }
}
