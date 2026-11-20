use std::cell::Cell;
use std::collections::HashMap;

use crate::asset_profile::SoundPaths;
use sdl3::AudioSubsystem;
use sdl3::audio::{AudioFormat, AudioSpec, AudioSpecWAV, AudioStreamOwner};

pub const SOUND_BUILD: u16 = 500;
pub const SOUND_WIND: u16 = 502;
pub const SOUND_PLOP: u16 = 503;
pub const SOUND_EXPLODE: u16 = 504;
pub const SOUND_POLICE: u16 = 506;
pub const SOUND_FIRE: u16 = 507;
pub const SOUND_FIRETRUCK: u16 = 509;
pub const SOUND_FLOOD: u16 = 511;
pub const SOUND_RIOT: u16 = 512;
pub const SOUND_CHEERS: u16 = 513;
pub const SOUND_ZAP: u16 = 514;
pub const SOUND_MAYDAY: u16 = 515;
/// Macintosh 68k kind-1 phase-zero takeoff request (`SND` resource 518).
pub const SOUND_MAC_TAKEOFF: u16 = 518;
/// Macintosh 68k kind-1 phase-one landing request (`SND` resource 519).
pub const SOUND_MAC_LAND: u16 = 519;
/// Macintosh 68k kind-2 traffic actor periodic sound selector `0x1fe`.
pub const SOUND_MAC_KIND2_TRAFFIC: u16 = 510;
pub const SOUND_CARGO: u16 = 0x205;
pub const SOUND_DISASTER: u16 = 520;
pub const SOUND_HORNS: u16 = 521;
pub const SOUND_PRISON: u16 = 522;
pub const SOUND_SCHOOL: u16 = 523;
pub const SOUND_TRAIN: u16 = 524;
pub const SOUND_MILITARY: u16 = 525;
pub const SOUND_MONSTER: u16 = 527;
/// Macintosh 68k kind-9 marina actor activation sound (`SND` resource Roar1).
/// The Win95 catalog uses the same numeric slot for its monster sound.
pub const SOUND_MAC_SAILBOAT_ROAR: u16 = SOUND_MONSTER;
/// Win95 Launch arco mass-wipe pre-walk @ `0x44f29c` (`0x4b174f(0x211)` / `529.WAV`).
pub const SOUND_LAUNCH_ARCO_WIPE_START: u16 = 0x211;
/// Win95 Launch arco mass-wipe post-funds @ `0x44f2f6` (`0x4b174f(0x212)` / `530.WAV`).
pub const SOUND_LAUNCH_ARCO_WIPE_END: u16 = 0x212;

const RATE: usize = 22_050;

/// Resolve each event by slot ID and convert mixed-edition WAV formats for playback.
pub struct SoundPlayer {
    stream: AudioStreamOwner,
    mac_second_stream: Option<AudioStreamOwner>,
    mac_voice_mode: Cell<bool>,
    mac_voice_order: Cell<[u64; 2]>,
    mac_next_order: Cell<u64>,
    effects: HashMap<u16, Vec<f32>>,
    suspended: bool,
}

impl SoundPlayer {
    pub fn new(audio: &AudioSubsystem, paths: &SoundPaths) -> Option<Self> {
        let wav_paths = if paths.use_citygame_effects {
            &paths.citygame_effects
        } else {
            &paths.effects
        };
        let mut effects = HashMap::new();
        if paths.use_citygame_effects {
            for id in 500..=531 {
                effects.insert(id, citygame_effect(id));
            }
            log::info!("using authored CityGame sound replacements");
        }
        for (&id, path) in wav_paths {
            match AudioSpecWAV::load_wav(path) {
                Ok(wav) => match decode_wav(&wav) {
                    Some(samples) => {
                        effects.insert(id, samples);
                    }
                    None => log::warn!("unsupported sound format for {}", path.display()),
                },
                Err(error) => log::warn!("sound {id} disabled: {error} ({})", path.display()),
            }
        }
        if !paths.use_citygame_effects {
            for id in 500..=531 {
                effects.entry(id).or_insert_with(|| citygame_effect(id));
            }
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
        // The captured Mac driver configures two effect voices. SDL mixes
        // logical playback streams, allowing both to remain audible together.
        let mac_second_stream = audio.default_playback_device()
            .open_device_stream(Some(&spec)).ok()
            .and_then(|stream| stream.resume().ok().map(|_| stream));
        Some(Self {
            stream,
            mac_second_stream,
            mac_voice_mode: Cell::new(false),
            mac_voice_order: Cell::new([0; 2]),
            mac_next_order: Cell::new(1),
            effects,
            suspended: false,
        })
    }

    /// Pause playback when the app backgrounds (Android / iPadOS).
    pub fn set_suspended(&mut self, suspended: bool) {
        self.suspended = suspended;
        if suspended {
            let _ = self.stream.clear();
            let _ = self.stream.pause();
            if let Some(stream) = &self.mac_second_stream {
                let _ = stream.clear();
                let _ = stream.pause();
            }
        } else {
            let _ = self.stream.resume();
            if let Some(stream) = &self.mac_second_stream { let _ = stream.resume(); }
        }
    }

    /// Select the edition's effect playback policy when the active game changes.
    pub(crate) fn set_mac_voice_mode(&self, mac: bool) {
        if self.mac_voice_mode.replace(mac) != mac {
            self.stop_effects();
        }
    }

    /// Clear queued effects at an original-game stop-all boundary.
    pub(crate) fn stop_effects(&self) {
        let _ = self.stream.clear();
        if let Some(stream) = &self.mac_second_stream { let _ = stream.clear(); }
        self.mac_voice_order.set([0; 2]);
        self.mac_next_order.set(1);
    }

    pub fn play_click(&self) {
        self.play_sc2k_effect(505, "click");
    }
    pub fn play_power_line(&self) {
        self.play_sc2k_effect(SOUND_ZAP, "power line");
    }
    pub fn play_build(&self) {
        self.play_sc2k_effect(SOUND_BUILD, "build");
    }
    pub fn play_pipe(&self) {
        self.play_sc2k_effect(SOUND_BUILD, "pipe");
    }

    pub fn play_sc2k_effect(&self, sound_id: u16, label: &str) {
        if self.suspended {
            return;
        }
        let Some(samples) = self.effects.get(&sound_id) else {
            log::debug!("{label} sound disabled: missing sound slot {sound_id}");
            return;
        };
        let stream = if self.mac_voice_mode.get() {
            if let Some(second) = &self.mac_second_stream {
                let slot = choose_mac_effect_slot(
                    [self.stream.queued_bytes().ok(), second.queued_bytes().ok()],
                    self.mac_voice_order.get(),
                );
                let mut order = self.mac_voice_order.get();
                order[slot] = self.mac_next_order.get();
                self.mac_voice_order.set(order);
                self.mac_next_order.set(self.mac_next_order.get().wrapping_add(1));
                if slot == 0 { &self.stream } else { second }
            } else { &self.stream }
        } else { &self.stream };
        let _ = stream.clear();
        if let Err(error) = stream.put_data_f32(samples) {
            log::debug!("{label} sound enqueue failed: {error}");
        }
    }
}

// The native driver selects the first free slot, then the lowest voice state
// with ties resolved to the later slot. SDL queue state and start order are an
// approximation; they do not reproduce the original service-counter ties.
fn choose_mac_effect_slot(queued: [Option<i32>; 2], order: [u64; 2]) -> usize {
    if queued[0].is_some_and(|bytes| bytes <= 0) { return 0; }
    if queued[1].is_some_and(|bytes| bytes <= 0) { return 1; }
    if order[0] < order[1] { 0 } else { 1 }
}

fn decode_wav(wav: &AudioSpecWAV) -> Option<Vec<f32>> {
    let bytes = wav.buffer();
    let width = match wav.format {
        AudioFormat::U8 | AudioFormat::S8 => 1,
        AudioFormat::S16LE | AudioFormat::S16BE => 2,
        AudioFormat::S32LE | AudioFormat::S32BE | AudioFormat::F32LE | AudioFormat::F32BE => 4,
        AudioFormat::UNKNOWN => return None,
    };
    let channels = usize::from(wav.channels);
    if channels == 0 || wav.freq <= 0 {
        return None;
    }
    let frames = bytes.len() / (width * channels);
    if frames == 0 {
        return Some(Vec::new());
    }
    let mut mono = Vec::with_capacity(frames);
    for frame in bytes.chunks_exact(width * channels) {
        let mut sum = 0.0;
        for sample in frame.chunks_exact(width) {
            sum += match wav.format {
                AudioFormat::U8 => (f32::from(sample[0]) - 128.0) / 128.0,
                AudioFormat::S8 => f32::from(sample[0] as i8) / 128.0,
                AudioFormat::S16LE => {
                    f32::from(i16::from_le_bytes([sample[0], sample[1]])) / 32768.0
                }
                AudioFormat::S16BE => {
                    f32::from(i16::from_be_bytes([sample[0], sample[1]])) / 32768.0
                }
                AudioFormat::S32LE => {
                    i32::from_le_bytes(sample.try_into().ok()?) as f32 / 2147483648.0
                }
                AudioFormat::S32BE => {
                    i32::from_be_bytes(sample.try_into().ok()?) as f32 / 2147483648.0
                }
                AudioFormat::F32LE => f32::from_le_bytes(sample.try_into().ok()?),
                AudioFormat::F32BE => f32::from_be_bytes(sample.try_into().ok()?),
                AudioFormat::UNKNOWN => unreachable!(),
            };
        }
        mono.push(sum / channels as f32);
    }
    let output_len = (frames as f64 * RATE as f64 / wav.freq as f64).round() as usize;
    let mut output = Vec::with_capacity(output_len);
    for index in 0..output_len {
        let position = index as f64 * wav.freq as f64 / RATE as f64;
        let first = (position as usize).min(frames - 1);
        let second = (first + 1).min(frames - 1);
        let fraction = (position - first as f64) as f32;
        output.push(mono[first] * (1.0 - fraction) + mono[second] * fraction);
    }
    Some(output)
}

/// Build an original fallback cue for a missing catalog slot. Imported WAVs
/// remain preferred; this keeps a clean install audible without bundling game
/// recordings. These are deliberately CityGame sounds, not reconstructions.
fn citygame_effect(id: u16) -> Vec<f32> {
    let duration = match id {
        502 | 507 | 511 | 512 | 513 | 520 | 527 => 1.15,
        506 | 509 | 515 | 521 | 524 => 0.85,
        504 | 529 | 530 => 0.72,
        514 | 517 | 525 => 0.58,
        _ => 0.18,
    };
    let frames = (duration * RATE as f32) as usize;
    let mut samples = Vec::with_capacity(frames);
    let mut noise_state = 0x9e37_79b9_u32 ^ u32::from(id).wrapping_mul(0x85eb_ca6b);
    let mut filtered_noise = 0.0_f32;
    let mut phase = 0.0_f32;
    for index in 0..frames {
        let t = index as f32 / RATE as f32;
        let progress = t / duration;
        let attack = (t / 0.012).clamp(0.0, 1.0);
        let release = (1.0 - progress).clamp(0.0, 1.0);
        let env = attack
            * release.powf(if matches!(id, 502 | 507 | 511 | 512 | 513) {
                0.65
            } else {
                1.7
            });
        noise_state ^= noise_state << 13;
        noise_state ^= noise_state >> 17;
        noise_state ^= noise_state << 5;
        let white = (noise_state as i32 as f32) / i32::MAX as f32;
        filtered_noise += (white - filtered_noise)
            * match id {
                502 | 520 | 527 => 0.08,
                507 | 511 | 512 | 513 => 0.24,
                _ => 0.8,
            };
        let pulse = ((t * 3.7).sin() * 0.5 + 0.5).powi(2);
        let frequency = match id {
            // Siren family: alternating rising and falling emergency tones.
            506 | 509 | 515 => 530.0 + 360.0 * (t * 1.5).sin(),
            // Power zap / electrical transient.
            514 => 1450.0 * (1.0 - progress).powf(2.3) + 95.0,
            // Plop, bell and short confirmation tones.
            503 => 620.0 * (1.0 - progress * 0.58),
            522 | 523 => {
                if (t * 5.0) as i32 % 2 == 0 {
                    880.0
                } else {
                    660.0
                }
            }
            505 => 1050.0,
            // Train and cargo rumble pulses.
            517 | 524 => 92.0 + 22.0 * (t * 14.0).sin(),
            // Launch wipe cues, military acknowledgment and horns.
            521 => 230.0 + (id as f32 - 520.0) * 18.0,
            525 => 110.0 + 30.0 * (t * 8.0).sin(),
            529 => 100.0 + 190.0 * progress,
            530 => 520.0 - 290.0 * progress,
            // Construction / click fallback.
            500 | 501 | 508 | 510 | 516 | 518 | 519 | 526 | 528 | 531 => 240.0,
            _ => 72.0 + 42.0 * (1.0 - progress),
        };
        phase += std::f32::consts::TAU * frequency.max(30.0) / RATE as f32;
        let tonal = phase.sin();
        let sample = match id {
            502 | 507 | 511 => filtered_noise * 0.58 + tonal * 0.14,
            504 | 520 | 529 | 530 => filtered_noise * (1.0 - progress) * 0.82 + tonal * 0.18,
            512 | 513 => filtered_noise * (0.35 + 0.25 * pulse) + tonal * 0.12,
            514 => tonal * 0.48 + white * (1.0 - progress) * 0.32,
            517 | 524 | 525 => tonal * (0.32 + 0.22 * pulse) + filtered_noise * 0.2,
            506 | 509 | 515 | 521 | 522 | 523 | 505 => tonal * 0.58,
            527 => (tonal * 0.55 + filtered_noise * 0.48) * (0.65 + 0.35 * (t * 9.0).sin()),
            _ => tonal * 0.6 + filtered_noise * 0.12,
        };
        samples.push((sample * env * 0.38).clamp(-0.8, 0.8));
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_effect_slots_prefer_free_then_oldest() {
        assert_eq!(choose_mac_effect_slot([Some(0), Some(0)], [3, 4]), 0);
        assert_eq!(choose_mac_effect_slot([Some(20), Some(0)], [3, 4]), 1);
        assert_eq!(choose_mac_effect_slot([Some(20), Some(30)], [3, 4]), 0);
        assert_eq!(choose_mac_effect_slot([Some(20), Some(30)], [4, 4]), 1);
    }

    #[test]
    #[ignore = "requires private live Mac admission trace"]
    fn mac_effect_slot_policy_matches_observed_live_admissions() {
        use crate::mac_sound_state::{mac_start_sound_voice, MacSoundVoice};
        let path = std::env::var_os("CITYGAME_MAC_SOUND_ADMISSIONS_LIVE").unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(rows.len() > 10);
        let mut order = [0; 2];
        for (i, row) in rows.iter().enumerate() {
            let mut voices: [MacSoundVoice; 2] = std::array::from_fn(|slot| {
                let voice = &row["voices"][slot];
                MacSoundVoice { descriptor: voice["descriptor"].as_u64().unwrap() as u16,
                    state: voice["state"].as_u64().unwrap() as u16 as i16 }
            });
            let queued = voices.map(|voice| Some(if voice.state < 0 { 0 } else { 1 }));
            let expected = row["selected"].as_u64().unwrap() as usize;
            assert_eq!(choose_mac_effect_slot(queued, order), expected, "{row}");
            assert_eq!(mac_start_sound_voice(&mut voices, row["descriptor"].as_u64().unwrap() as u16, true),
                Some(expected), "{row}");
            order[expected] = i as u64 + 1;
        }
    }

    #[test]
    #[ignore = "requires SDL's dummy audio driver"]
    fn mac_effect_streams_overlap_and_win95_replaces() {
        let sdl = sdl3::init().unwrap();
        let audio = sdl.audio().unwrap();
        let paths = SoundPaths { effects: HashMap::new(), citygame_effects: HashMap::new(),
            use_citygame_effects: true };
        let player = SoundPlayer::new(&audio, &paths).unwrap();
        let second = player.mac_second_stream.as_ref().expect("second SDL stream");
        player.stream.pause().unwrap();
        second.pause().unwrap();
        player.set_mac_voice_mode(true);
        player.play_sc2k_effect(500, "first");
        player.play_sc2k_effect(507, "second");
        let (first_bytes, second_bytes) = (player.stream.queued_bytes().unwrap(), second.queued_bytes().unwrap());
        assert!(first_bytes > 0 && second_bytes > 0);
        player.play_sc2k_effect(504, "third");
        assert_ne!(player.stream.queued_bytes().unwrap(), first_bytes);
        assert_eq!(second.queued_bytes().unwrap(), second_bytes);
        player.stop_effects();
        assert_eq!((player.stream.queued_bytes().unwrap(), second.queued_bytes().unwrap()), (0, 0));
        player.play_sc2k_effect(520, "disaster alert");
        assert!(player.stream.queued_bytes().unwrap() > 0);
        assert_eq!(second.queued_bytes().unwrap(), 0);
        player.set_mac_voice_mode(false);
        assert_eq!((player.stream.queued_bytes().unwrap(), second.queued_bytes().unwrap()), (0, 0));
        player.play_sc2k_effect(500, "win first");
        player.play_sc2k_effect(507, "win second");
        assert_eq!(player.stream.queued_bytes().unwrap(), second_bytes);
        assert_eq!(second.queued_bytes().unwrap(), 0);
    }
}
