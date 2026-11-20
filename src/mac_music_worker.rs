//! Off-thread rendering for the captured Quadra MDRV configuration.
use crate::asset_profile::AssetProfile;
use crate::mac_midi_bank::MacMidiBank;
use crate::mac_midi_engine::MacMidiEngine;
use crate::mac_midi_mix::MacOutputMode;
use sdl3::audio::AudioStreamOwner;
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// SDL accepts integer sample rates; the captured 16.16 rate is 22254.545 Hz.
pub(crate) const MAC_MUSIC_RATE: i32 = 22_255;

enum Chunk {
    Samples(Vec<f32>),
    Error(String),
}
pub(crate) struct MacMusicWorker {
    receiver: Receiver<Chunk>,
    finished: bool,
}

impl MacMusicWorker {
    pub(crate) fn start(profile: &AssetProfile, song_id: u16) -> Option<Self> {
        let root = profile.mac_audio_resource_directory_for_song(song_id)?;
        if !root.join("INST").is_dir() || !root.join("snd").is_dir() {
            return None;
        }
        let midi = std::fs::read(profile.original_music_path(song_id)?).ok()?;
        let bank = MacMidiBank::load(&root, song_id).ok()?;
        if bank.song.midi_id != song_id {
            return None;
        }
        let engine = MacMidiEngine::new_configured(
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
        .ok()?;
        Some(Self::spawn(engine))
    }

    fn spawn(mut engine: MacMidiEngine) -> Self {
        let (sender, receiver) = mpsc::sync_channel(256);
        std::thread::spawn(move || {
            while engine.has_audio_work() {
                match engine.refill() {
                    Ok(bytes) => {
                        let samples = bytes
                            .into_iter()
                            .map(|b| (f32::from(b) - 128.0) / 128.0)
                            .collect();
                        if sender.send(Chunk::Samples(samples)).is_err() {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Chunk::Error(format!("{error:#}")));
                        return;
                    }
                }
            }
        });
        Self {
            receiver,
            finished: false,
        }
    }

    pub(crate) fn pump(&mut self, stream: &AudioStreamOwner, target_bytes: i32) {
        while !self.finished && stream.queued_bytes().unwrap_or(i32::MAX) < target_bytes {
            match self.receiver.try_recv() {
                Ok(Chunk::Samples(samples)) => {
                    if let Err(error) = stream.put_data_f32(&samples) {
                        log::warn!("Mac music queue failed: {error}");
                        self.finished = true;
                    }
                }
                Ok(Chunk::Error(error)) => {
                    log::warn!("Mac music rendering stopped: {error}");
                    self.finished = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => self.finished = true,
            }
        }
    }

    pub(crate) fn active(&self, stream: &AudioStreamOwner) -> bool {
        !self.finished || stream.queued_bytes().is_ok_and(|bytes| bytes > 0)
    }
}
