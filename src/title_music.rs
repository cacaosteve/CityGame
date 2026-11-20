//! Menu music is rendered locally into a dedicated SDL stream. The authored cue
//! contains no imported notes; the optional legacy cue reads the user's own MIDI.
use crate::asset_profile::{AssetProfile, TitleMusicPreference};
use crate::mac_music_worker::{MAC_MUSIC_RATE, MacMusicWorker};
use sdl3::AudioSubsystem;
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};
use std::f32::consts::TAU;
use std::io::Write;
use std::path::PathBuf;

const RATE: usize = 22_050;
const CHUNK: usize = RATE / 2;
const CITYGAME_CITY_MIDI: [&[u8]; 5] = [
    include_bytes!("../assets/music/citygame-city.mid"),
    include_bytes!("../assets/music/citygame-city-02.mid"),
    include_bytes!("../assets/music/citygame-city-03.mid"),
    include_bytes!("../assets/music/citygame-city-04.mid"),
    include_bytes!("../assets/music/citygame-city-05.mid"),
];
// OpenSC2K's music director names these as its general city-play rotation.
// The Win95 PE MIDI loader accepts the same numbered cue family, but its
// caller-to-state mapping is still being traced independently.
const CITY_GENERAL_TRACKS: [u16; 5] = [10_001, 10_004, 10_008, 10_012, 10_018];

pub struct TitleMusic {
    stream: Option<AudioStreamOwner>,
    samples: Vec<f32>,
    cursor: usize,
    native_midi: bool,
    looping: bool,
    mac_worker: Option<MacMusicWorker>,
}

pub struct CityMusic {
    stream: AudioStreamOwner,
    tracks: Vec<(u16, PathBuf)>,
    next_track: usize,
    next_replacement: usize,
    samples: Vec<f32>,
    cursor: usize,
    suspended: bool,
    exhausted: bool,
    replacement: bool,
    selector_driven: bool,
    startup: Option<TitleMusic>,
    mac_stream: Option<AudioStreamOwner>,
    mac_worker: Option<MacMusicWorker>,
}

impl CityMusic {
    pub fn new(
        audio: Option<&AudioSubsystem>,
        profile: &AssetProfile,
        enabled: bool,
        prefer_citygame_music: bool,
        mac_fidelity: bool,
        startup: Option<TitleMusic>,
    ) -> Option<Self> {
        if !enabled {
            return None;
        }
        let audio = audio?;
        let tracks: Vec<_> = if prefer_citygame_music {
            Vec::new()
        } else {
            CITY_GENERAL_TRACKS
                .iter()
                .filter_map(|&id| profile.original_music_path(id).map(|path| (id, path)))
                .collect()
        };
        let replacement = tracks.is_empty();
        if prefer_citygame_music {
            log::info!("CityGame city music selected");
        } else if replacement {
            log::info!(
                "no imported general-city MIDI tracks are available; using embedded CityGame score"
            );
        }
        let spec = AudioSpec {
            freq: Some(RATE as i32),
            channels: Some(1),
            format: Some(AudioFormat::F32LE),
        };
        let stream = audio
            .default_playback_device()
            .open_device_stream(Some(&spec))
            .ok()?;
        stream.resume().ok()?;
        let mac_stream = if mac_fidelity {
            let spec = AudioSpec {
                freq: Some(MAC_MUSIC_RATE),
                channels: Some(1),
                format: Some(AudioFormat::F32LE),
            };
            audio
                .default_playback_device()
                .open_device_stream(Some(&spec))
                .ok()
                .and_then(|s| s.resume().ok().map(|_| s))
        } else {
            None
        };
        let mut player = Self {
            stream,
            tracks,
            next_track: 0,
            next_replacement: 0,
            samples: Vec::new(),
            cursor: 0,
            suspended: false,
            exhausted: false,
            replacement,
            selector_driven: mac_fidelity && !prefer_citygame_music && !replacement,
            startup,
            mac_stream,
            mac_worker: None,
        };
        if !player.selector_driven && !player.load_next_track() {
            return None;
        }
        Some(player)
    }

    pub fn update(&mut self) {
        if self.suspended {
            return;
        }
        if let Some(startup) = &mut self.startup {
            startup.update();
            if startup.song_active() {
                return;
            }
            self.startup = None;
        }
        if let (Some(worker), Some(stream)) = (&mut self.mac_worker, &self.mac_stream) {
            worker.pump(stream, MAC_MUSIC_RATE * 4 / 2);
            if !worker.active(stream) {
                self.mac_worker = None;
            }
            return;
        }
        let target_queued_bytes = (CHUNK * 4) as i32;
        while self.stream.queued_bytes().unwrap_or(i32::MAX) < target_queued_bytes {
            if self.cursor >= self.samples.len() {
                if self.selector_driven || !self.load_next_track() {
                    break;
                }
            }
            let end = (self.cursor + CHUNK).min(self.samples.len());
            if self
                .stream
                .put_data_f32(&self.samples[self.cursor..end])
                .is_err()
            {
                break;
            }
            self.cursor = end;
        }
    }

    pub fn song_active(&self) -> bool {
        self.startup.as_ref().is_some_and(TitleMusic::song_active)
            || self
                .mac_worker
                .as_ref()
                .zip(self.mac_stream.as_ref())
                .is_some_and(|(worker, stream)| worker.active(stream))
            || (!self.exhausted
                && (self.cursor < self.samples.len()
                    || self.stream.queued_bytes().is_ok_and(|bytes| bytes > 0)))
    }

    pub fn request_original_song(&mut self, profile: &AssetProfile, id: u16) {
        if profile.citygame_city_music_enabled() {
            return;
        }
        if self.selector_driven && self.mac_stream.is_some() {
            if let Some(worker) = MacMusicWorker::start(profile, id) {
                self.startup = None;
                let _ = self.stream.clear();
                if let Some(stream) = &self.mac_stream {
                    let _ = stream.clear();
                }
                self.mac_worker = Some(worker);
                self.samples.clear();
                self.cursor = 0;
                self.exhausted = false;
                log::info!("playing original Mac music cue {id}");
                return;
            }
        }
        self.mac_worker = None;
        if let Some(stream) = &self.mac_stream {
            let _ = stream.clear();
        }
        let Some(path) = profile.original_music_path(id) else {
            return;
        };
        if let Ok(bytes) = std::fs::read(path)
            && let Ok(notes) = parse_midi(&bytes)
        {
            self.startup = None;
            let _ = self.stream.clear();
            self.samples = synthesize(&notes);
            self.cursor = 0;
            self.exhausted = false;
            log::info!("playing original city MIDI cue {id}");
        }
    }

    pub fn set_suspended(&mut self, suspended: bool) {
        if self.suspended == suspended {
            return;
        }
        self.suspended = suspended;
        if suspended {
            let _ = self.stream.pause();
            if let Some(stream) = &self.mac_stream {
                let _ = stream.pause();
            }
        } else {
            let _ = self.stream.resume();
            if let Some(stream) = &self.mac_stream {
                let _ = stream.resume();
            }
        }
    }

    fn load_next_track(&mut self) -> bool {
        if self.exhausted {
            return false;
        }
        if self.replacement {
            return self.load_replacement_score();
        }
        for _ in 0..self.tracks.len() {
            let (id, path) = &self.tracks[self.next_track];
            self.next_track = (self.next_track + 1) % self.tracks.len();
            let parsed = std::fs::read(path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| parse_midi(&bytes).map_err(str::to_owned));
            match parsed {
                Ok(notes) => {
                    self.samples = synthesize(&notes);
                    self.cursor = 0;
                    log::info!("playing city MIDI cue {id} ({})", path.display());
                    return true;
                }
                Err(error) => log::warn!(
                    "city MIDI cue {id} unavailable: {error} ({})",
                    path.display()
                ),
            }
        }
        // Keep city music available when an import is incomplete or contains
        // MIDI files this lightweight cross-platform renderer cannot parse.
        self.replacement = true;
        log::warn!("imported city MIDI unavailable; falling back to embedded CityGame score");
        self.load_replacement_score()
    }

    fn load_replacement_score(&mut self) -> bool {
        for _ in 0..CITYGAME_CITY_MIDI.len() {
            let index = self.next_replacement;
            self.next_replacement = (self.next_replacement + 1) % CITYGAME_CITY_MIDI.len();
            match parse_midi(CITYGAME_CITY_MIDI[index]) {
                Ok(notes) => {
                    self.samples = synthesize(&notes);
                    self.cursor = 0;
                    self.exhausted = false;
                    log::info!("playing embedded CityGame city MIDI cue {}", index + 1);
                    return true;
                }
                Err(error) => log::error!(
                    "embedded CityGame city MIDI cue {} is invalid: {error}",
                    index + 1
                ),
            }
        }
        self.samples.clear();
        self.cursor = 0;
        self.exhausted = true;
        false
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn citygame_start_title_midi(path: *const std::ffi::c_char) -> bool;
    fn citygame_update_title_midi();
    fn citygame_stop_title_midi();
    fn citygame_title_midi_is_playing() -> bool;
    #[cfg(test)]
    fn citygame_midi_position_is_active(playing: bool, position: f64, duration: f64) -> bool;
}

#[derive(Clone, Copy)]
struct Note {
    start: f32,
    duration: f32,
    key: u8,
    velocity: u8,
    channel: u8,
}

#[derive(Clone, Copy)]
enum MidiEvent {
    Tempo(u32),
    On(u8, u8, u8),
    Off(u8, u8),
}

impl TitleMusic {
    pub fn new(
        audio: Option<&AudioSubsystem>,
        profile: &AssetProfile,
        startup_song: Option<u16>,
        looping: bool,
        mac_fidelity: bool,
    ) -> Option<Self> {
        if profile.title_music_preference() == TitleMusicPreference::Off {
            return None;
        }
        let original_path = || {
            title_midi_override()
                .or_else(|| startup_song.and_then(|song| profile.original_music_path(song)))
                .or_else(|| profile.original_music_path(10018))
        };
        if mac_fidelity
            && profile.title_music_preference() == TitleMusicPreference::Original
            && title_midi_override().is_none()
        {
            if let (Some(audio), Some(worker)) = (
                audio,
                MacMusicWorker::start(profile, startup_song.unwrap_or(10018)),
            ) {
                let spec = AudioSpec {
                    freq: Some(MAC_MUSIC_RATE),
                    channels: Some(1),
                    format: Some(AudioFormat::F32LE),
                };
                if let Ok(stream) = audio
                    .default_playback_device()
                    .open_device_stream(Some(&spec))
                {
                    if stream.resume().is_ok() {
                        return Some(Self {
                            stream: Some(stream),
                            samples: Vec::new(),
                            cursor: 0,
                            native_midi: false,
                            looping,
                            mac_worker: Some(worker),
                        });
                    }
                }
            }
        }
        #[cfg(target_os = "macos")]
        if profile.title_music_preference() == TitleMusicPreference::Original {
            if let Some(path) = original_path() {
                if let Ok(path) = std::ffi::CString::new(path.to_string_lossy().as_bytes()) {
                    if unsafe { citygame_start_title_midi(path.as_ptr()) } {
                        log::info!("playing imported original title MIDI through macOS");
                        return Some(Self {
                            stream: None,
                            samples: Vec::new(),
                            cursor: 0,
                            native_midi: true,
                            looping,
                            mac_worker: None,
                        });
                    }
                }
            }
        }
        let audio = audio?;
        let samples = match profile.title_music_preference() {
            TitleMusicPreference::Original => original_path()
                .and_then(|path| std::fs::read(&path).ok())
                .and_then(|bytes| parse_midi(&bytes).ok())
                .map(|notes| synthesize(&notes))
                .unwrap_or_else(|| {
                    log::warn!("original title MIDI unavailable; using the CityGame theme");
                    synthesize(&citygame_theme())
                }),
            TitleMusicPreference::Replacement => {
                log::info!("playing authored CityGame title theme");
                synthesize(&citygame_theme())
            }
            TitleMusicPreference::Off => unreachable!(),
        };
        let spec = AudioSpec {
            freq: Some(RATE as i32),
            channels: Some(1),
            format: Some(AudioFormat::F32LE),
        };
        let stream = audio
            .default_playback_device()
            .open_device_stream(Some(&spec))
            .ok()?;
        stream.resume().ok()?;
        Some(Self {
            stream: Some(stream),
            samples,
            cursor: 0,
            native_midi: false,
            looping,
            mac_worker: None,
        })
    }

    pub fn update(&mut self) {
        if let (Some(worker), Some(stream)) = (&mut self.mac_worker, &self.stream) {
            worker.pump(stream, MAC_MUSIC_RATE * 4 * 4);
            return;
        }
        #[cfg(target_os = "macos")]
        if self.native_midi {
            if self.looping {
                unsafe {
                    citygame_update_title_midi();
                }
            } else {
                unsafe {
                    citygame_title_midi_is_playing();
                }
            }
            return;
        }
        if self.samples.is_empty() {
            return;
        }
        let Some(stream) = &self.stream else {
            return;
        };
        if !self.looping {
            // City opening is synchronous, so enqueue the rest of the startup
            // song now and let the device continue playing while it loads.
            if self.cursor < self.samples.len()
                && stream.put_data_f32(&self.samples[self.cursor..]).is_ok()
            {
                self.cursor = self.samples.len();
            }
            return;
        }
        while stream.queued_bytes().unwrap_or(i32::MAX) < (CHUNK * 4) as i32 {
            if self.cursor >= self.samples.len() {
                self.cursor = 0;
            }
            let end = (self.cursor + CHUNK).min(self.samples.len());
            if stream
                .put_data_f32(&self.samples[self.cursor..end])
                .is_err()
            {
                break;
            }
            self.cursor = end;
        }
    }

    fn song_active(&self) -> bool {
        if let (Some(worker), Some(stream)) = (&self.mac_worker, &self.stream) {
            return worker.active(stream);
        }
        #[cfg(target_os = "macos")]
        if self.native_midi {
            return unsafe { citygame_title_midi_is_playing() };
        }
        self.cursor < self.samples.len()
            || self
                .stream
                .as_ref()
                .is_some_and(|stream| stream.queued_bytes().is_ok_and(|bytes| bytes > 0))
    }
}

impl Drop for TitleMusic {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if self.native_midi {
            unsafe {
                citygame_stop_title_midi();
            }
        }
    }
}

fn title_midi_override() -> Option<PathBuf> {
    std::env::var_os("CITYGAME_TITLE_MIDI")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

fn original_theme_path(profile: &AssetProfile) -> Option<PathBuf> {
    title_midi_override().or_else(|| profile.original_music_path(10018))
}

pub fn original_theme_available(profile: &AssetProfile) -> bool {
    original_theme_path(profile).is_some()
}

pub fn export_citygame_theme(path: &std::path::Path) -> std::io::Result<()> {
    let samples = synthesize(&citygame_theme());
    let data_size = (samples.len() * 2) as u32;
    let mut file = std::fs::File::create(path)?;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + data_size).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16_u32.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?; // PCM
    file.write_all(&1_u16.to_le_bytes())?; // mono
    file.write_all(&(RATE as u32).to_le_bytes())?;
    file.write_all(&((RATE * 2) as u32).to_le_bytes())?;
    file.write_all(&2_u16.to_le_bytes())?;
    file.write_all(&16_u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_size.to_le_bytes())?;
    for sample in samples {
        file.write_all(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
    }
    Ok(())
}

fn citygame_theme() -> Vec<Note> {
    // Independently composed 16-bar cue: a slow ascending city-light motif over
    // alternating minor/add-nine harmony. All timings and pitches are authored here.
    let beat = 60.0 / 94.0;
    let mut notes = Vec::new();
    let bass = [
        38, 34, 41, 36, 38, 34, 43, 36, 38, 34, 41, 36, 43, 41, 36, 38,
    ];
    let chords: [[u8; 3]; 4] = [[50, 53, 57], [46, 50, 55], [53, 57, 62], [48, 53, 57]];
    let melody: [[u8; 8]; 4] = [
        [62, 65, 69, 0, 72, 69, 65, 67],
        [58, 62, 65, 67, 0, 70, 67, 65],
        [69, 72, 74, 0, 77, 74, 72, 69],
        [67, 69, 72, 0, 74, 72, 69, 65],
    ];
    for bar in 0..16 {
        let start = bar as f32 * 4.0 * beat;
        notes.push(Note {
            start,
            duration: 3.65 * beat,
            key: bass[bar],
            velocity: 74,
            channel: 1,
        });
        for &key in &chords[bar % 4] {
            notes.push(Note {
                start: start + 0.1 * beat,
                duration: 3.55 * beat,
                key,
                velocity: 46,
                channel: 2,
            });
        }
        for (step, &key) in melody[bar % 4].iter().enumerate() {
            if key == 0 {
                continue;
            }
            notes.push(Note {
                start: start + step as f32 * 0.5 * beat,
                duration: 0.42 * beat,
                key,
                velocity: if step == 0 { 86 } else { 69 },
                channel: 0,
            });
        }
    }
    notes
}

fn synthesize(notes: &[Note]) -> Vec<f32> {
    let length = notes
        .iter()
        .map(|note| note.start + note.duration)
        .fold(1.0_f32, f32::max)
        + 0.6;
    let mut samples = vec![0.0_f32; (length * RATE as f32).ceil() as usize];
    for note in notes {
        let frequency = 440.0 * 2.0_f32.powf((note.key as f32 - 69.0) / 12.0);
        let first = (note.start * RATE as f32) as usize;
        let count = ((note.duration + 0.15) * RATE as f32) as usize;
        let gain = (note.velocity as f32 / 127.0) * 0.12;
        for offset in 0..count.min(samples.len().saturating_sub(first)) {
            let t = offset as f32 / RATE as f32;
            let attack = (t / 0.025).min(1.0);
            let release = ((note.duration + 0.15 - t) / 0.2).clamp(0.0, 1.0);
            let decay = (-t * if note.channel == 0 { 1.1 } else { 0.35 }).exp();
            let phase = TAU * frequency * t;
            let tone = match note.channel {
                0 => phase.sin() * 0.75 + (phase * 2.0).sin() * 0.18 + (phase * 3.0).sin() * 0.07,
                1 => phase.sin() * 0.86 + (phase * 2.0).sin() * 0.14,
                _ => phase.sin() * 0.7 + (phase * 2.0).sin() * 0.2 + (phase * 4.0).sin() * 0.1,
            };
            samples[first + offset] += tone * gain * attack * release * decay;
        }
    }
    for sample in &mut samples {
        *sample = *sample / (1.0 + sample.abs());
    }
    samples
}

fn parse_midi(bytes: &[u8]) -> Result<Vec<Note>, &'static str> {
    if bytes.get(0..4) != Some(b"MThd") {
        return Err("missing MIDI header");
    }
    let header_len = be_u32(bytes, 4)? as usize;
    if header_len < 6 {
        return Err("short MIDI header");
    }
    let tracks = be_u16(bytes, 10)? as usize;
    let division = be_u16(bytes, 12)?;
    if division == 0 || division & 0x8000 != 0 {
        return Err("unsupported MIDI timebase");
    }
    let mut cursor = 8 + header_len;
    let mut events = Vec::new();
    for track in 0..tracks {
        if bytes.get(cursor..cursor + 4) != Some(b"MTrk") {
            return Err("missing MIDI track");
        }
        let len = be_u32(bytes, cursor + 4)? as usize;
        cursor += 8;
        let end = cursor
            .checked_add(len)
            .filter(|&end| end <= bytes.len())
            .ok_or("short MIDI track")?;
        let mut tick = 0_u64;
        let mut running = 0_u8;
        while cursor < end {
            tick += vlq(bytes, &mut cursor, end)? as u64;
            let first = *bytes.get(cursor).ok_or("short MIDI event")?;
            let status = if first & 0x80 != 0 {
                cursor += 1;
                first
            } else {
                running
            };
            if status == 0 {
                return Err("missing MIDI running status");
            }
            if status == 0xff {
                let kind = *bytes.get(cursor).ok_or("short MIDI meta event")?;
                cursor += 1;
                let size = vlq(bytes, &mut cursor, end)? as usize;
                let payload = bytes
                    .get(cursor..cursor + size)
                    .ok_or("short MIDI meta data")?;
                if kind == 0x51 && size == 3 {
                    let tempo = u32::from_be_bytes([0, payload[0], payload[1], payload[2]]);
                    events.push((tick, track, MidiEvent::Tempo(tempo.max(1))));
                }
                cursor += size;
            } else if status == 0xf0 || status == 0xf7 {
                let size = vlq(bytes, &mut cursor, end)? as usize;
                cursor = cursor
                    .checked_add(size)
                    .filter(|&at| at <= end)
                    .ok_or("short MIDI sysex")?;
            } else if status & 0x80 != 0 {
                running = status;
                let kind = status & 0xf0;
                let channel = status & 0x0f;
                let key = *bytes.get(cursor).ok_or("short MIDI channel data")?;
                cursor += 1;
                if kind != 0xc0 && kind != 0xd0 {
                    let value = *bytes.get(cursor).ok_or("short MIDI channel data")?;
                    cursor += 1;
                    match kind {
                        0x90 if value != 0 => {
                            events.push((tick, track, MidiEvent::On(channel, key, value)))
                        }
                        0x80 | 0x90 => events.push((tick, track, MidiEvent::Off(channel, key))),
                        _ => {}
                    }
                }
            } else {
                return Err("invalid MIDI status");
            }
        }
        cursor = end;
    }
    events.sort_by_key(|(tick, track, _)| (*tick, *track));
    let mut tempo = 500_000_u32;
    let mut last_tick = 0_u64;
    let mut seconds = 0.0_f64;
    let mut active = [[None::<(f32, u8)>; 128]; 16];
    let mut notes = Vec::new();
    for (tick, _, event) in events {
        seconds += (tick - last_tick) as f64 * tempo as f64 / (division as f64 * 1_000_000.0);
        last_tick = tick;
        match event {
            MidiEvent::Tempo(value) => tempo = value,
            MidiEvent::On(channel, key, velocity) if key < 128 => {
                active[channel as usize][key as usize] = Some((seconds as f32, velocity));
            }
            MidiEvent::Off(channel, key) if key < 128 => {
                if let Some((start, velocity)) = active[channel as usize][key as usize].take() {
                    notes.push(Note {
                        start,
                        duration: (seconds as f32 - start).max(0.06),
                        key,
                        velocity,
                        channel: if channel == 9 { 2 } else { channel % 3 },
                    });
                }
            }
            _ => {}
        }
    }
    if notes.is_empty() {
        return Err("MIDI contains no notes");
    }
    Ok(notes)
}

fn be_u16(bytes: &[u8], at: usize) -> Result<u16, &'static str> {
    let b = bytes.get(at..at + 2).ok_or("short MIDI integer")?;
    Ok(u16::from_be_bytes([b[0], b[1]]))
}
fn be_u32(bytes: &[u8], at: usize) -> Result<u32, &'static str> {
    let b = bytes.get(at..at + 4).ok_or("short MIDI integer")?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}
fn vlq(bytes: &[u8], cursor: &mut usize, end: usize) -> Result<u32, &'static str> {
    let mut value = 0_u32;
    for _ in 0..4 {
        if *cursor >= end {
            return Err("short MIDI variable integer");
        }
        let byte = bytes[*cursor];
        *cursor += 1;
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("long MIDI variable integer")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original Mac resources and an SDL audio device"]
    fn mac_title_and_city_play_imported_mdrv_audio() {
        let source = PathBuf::from(std::env::var_os("CITYGAME_MAC_RESOURCE_ROOT").unwrap());
        let root =
            std::env::temp_dir().join(format!("citygame-mac-worker-ui-{}", std::process::id()));
        let pack = root.join("pack");
        crate::audio_import::import_mac(&source, &pack).unwrap();
        let settings = root.join("settings.json");
        std::fs::write(&settings,serde_json::to_vec(&serde_json::json!({"schema_version":1,"original_music":pack,"title_music_preference":"original"})).unwrap()).unwrap();
        let profile = AssetProfile::load_from(settings).unwrap();
        let sdl = sdl3::init().unwrap();
        let audio = sdl.audio().unwrap();
        let mut title = TitleMusic::new(Some(&audio), &profile, Some(10018), false, true).unwrap();
        assert!(title.mac_worker.is_some());
        for _ in 0..200 {
            title.update();
            if title.stream.as_ref().unwrap().queued_bytes().unwrap() > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(title.stream.as_ref().unwrap().queued_bytes().unwrap() > 0);
        assert!(title.song_active());
        drop(title);
        let mut city = CityMusic::new(Some(&audio), &profile, true, false, true, None).unwrap();
        city.request_original_song(&profile, 10018);
        assert!(city.mac_worker.is_some());
        for _ in 0..200 {
            city.update();
            if city.mac_stream.as_ref().unwrap().queued_bytes().unwrap() > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(city.mac_stream.as_ref().unwrap().queued_bytes().unwrap() > 0);
        assert!(city.song_active());
        city.set_suspended(true);
        city.set_suspended(false);
        drop(city);
        std::fs::remove_file(pack.join("catalog/mac-audio/MDRV/11-pitch.bin")).unwrap();
        assert!(MacMusicWorker::start(&profile, 10018).is_none());
        let mut fallback = CityMusic::new(Some(&audio), &profile, true, false, true, None).unwrap();
        fallback.request_original_song(&profile, 10018);
        assert!(fallback.mac_worker.is_none());
        assert!(!fallback.samples.is_empty());
        drop(fallback);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_midi_completion_uses_position_when_playing_stays_true() {
        unsafe {
            assert!(citygame_midi_position_is_active(true, 57.0, 57.115));
            assert!(!citygame_midi_position_is_active(true, 57.115, 57.115));
            assert!(!citygame_midi_position_is_active(true, 65.0, 57.115));
            assert!(!citygame_midi_position_is_active(false, 0.0, 57.115));
            assert!(citygame_midi_position_is_active(true, 0.0, 0.0));
        }
    }

    #[test]
    #[ignore = "requires an SDL audio device"]
    fn mac_selector_waits_for_startup_and_does_not_autoplay_next_track() {
        let sdl = sdl3::init().unwrap();
        let audio = sdl.audio().unwrap();
        let spec = AudioSpec {
            freq: Some(RATE as i32),
            channels: Some(1),
            format: Some(AudioFormat::F32LE),
        };
        let open = || {
            let stream = audio
                .default_playback_device()
                .open_device_stream(Some(&spec))
                .unwrap();
            stream.resume().unwrap();
            stream
        };
        let startup = TitleMusic {
            stream: Some(open()),
            samples: vec![0.0; RATE],
            cursor: 0,
            native_midi: false,
            looping: false,
            mac_worker: None,
        };
        let mut city = CityMusic {
            stream: open(),
            tracks: Vec::new(),
            next_track: 0,
            next_replacement: 0,
            samples: Vec::new(),
            cursor: 0,
            suspended: false,
            exhausted: false,
            replacement: false,
            selector_driven: true,
            startup: Some(startup),
            mac_stream: None,
            mac_worker: None,
        };
        assert!(city.song_active());
        city.update();
        assert!(city.song_active());
        let startup = city.startup.as_mut().unwrap();
        startup.cursor = startup.samples.len();
        startup.stream.as_ref().unwrap().clear().unwrap();
        city.update();
        assert!(city.startup.is_none());
        assert!(!city.song_active());

        city.samples = vec![0.0; 16];
        city.update();
        assert_eq!(city.cursor, 16);
        city.stream.clear().unwrap();
        city.update();
        assert_eq!(city.cursor, 16);
        assert!(!city.song_active());
    }

    #[test]
    fn parses_imported_theme_without_bundling_original_audio() {
        let Some(path) = crate::paths::test_title_music() else {
            return;
        };
        let notes = parse_midi(&std::fs::read(path).unwrap()).unwrap();
        assert!(notes.len() > 100);
        assert!(notes.iter().any(|note| note.start > 30.0));
    }

    #[test]
    #[ignore = "requires the user's Win95 original MIDI files"]
    fn win95_newspaper_cues_parse_and_synthesize() {
        let directory = std::path::PathBuf::from(
            std::env::var_os("CITYGAME_WIN95_SOUND_DIR").expect("set CITYGAME_WIN95_SOUND_DIR"),
        );
        for id in [10_009, 10_015, 10_014, 10_006, 10_002] {
            let bytes = std::fs::read(directory.join(format!("{id}.mid"))).unwrap();
            let samples = synthesize(&parse_midi(&bytes).unwrap());
            assert!(
                samples.iter().any(|sample| sample.abs() > 0.001),
                "cue {id}"
            );
        }
    }

    #[test]
    fn authored_theme_has_audio_and_a_bounded_peak() {
        let samples = synthesize(&citygame_theme());
        assert!(samples.len() > RATE * 35);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        assert!(samples.iter().all(|sample| sample.abs() < 1.0));
    }
}
