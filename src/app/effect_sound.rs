//! Effect-sound mappings and optional/enabled-player playback gates.
//!
//! Effect IDs, labels, unknown-ID handling and playback operations retain their
//! existing behavior. Native effect queues, trigger decisions and audio resource
//! ownership stay with their existing callers.

use super::{DisasterKind, ViewerState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PlacementSound {
    Build,
    Plop,
    PowerLine,
    Pipe,
    Flood,
    Police,
    Riot,
    Fire,
    FireTruck,
    Cheers,
    Mayday,
    /// Win95 `0x208` / `520.WAV` from consume common path (`0x45d05c`).
    Disaster,
    Wind,
    Explode,
    /// Win95 cargo tick and Mac kind-3 actor request selector `0x205`.
    Cargo,
    /// Mac kind-9 marina actor activation requests SND 527 (`Roar1`).
    MacSailboatRoar,
    /// Mac kind-1 phase zero requests SND 518 (`Takeoff`).
    MacTakeoff,
    /// Mac kind-1 phase one requests SND 519 (`Land`).
    MacLand,
    /// Mac kind-2 state 2 posts sound selector `0x1fe` (resource 510).
    MacKind2Traffic,
    Horns,
    Prison,
    School,
    Train,
    Military,
    Monster,
    /// Win95 Launch arco wipe pre-walk `0x211` / `529.WAV` (`0x44f29c`).
    LaunchArcoWipeStart,
    /// Win95 Launch arco wipe post-funds `0x212` / `530.WAV` (`0x44f2f6`).
    LaunchArcoWipeEnd,
}

impl PlacementSound {
    pub(super) fn sound_id(self) -> u16 {
        match self {
            Self::Build | Self::Pipe => crate::audio::SOUND_BUILD,
            Self::Plop => crate::audio::SOUND_PLOP,
            Self::PowerLine => crate::audio::SOUND_ZAP,
            Self::Flood => crate::audio::SOUND_FLOOD,
            Self::Police => crate::audio::SOUND_POLICE,
            Self::Riot => crate::audio::SOUND_RIOT,
            Self::Fire => crate::audio::SOUND_FIRE,
            Self::FireTruck => crate::audio::SOUND_FIRETRUCK,
            Self::Cheers => crate::audio::SOUND_CHEERS,
            Self::Mayday => crate::audio::SOUND_MAYDAY,
            Self::Disaster => crate::audio::SOUND_DISASTER,
            Self::Wind => crate::audio::SOUND_WIND,
            Self::Explode => crate::audio::SOUND_EXPLODE,
            Self::Cargo => crate::audio::SOUND_CARGO,
            Self::MacSailboatRoar => crate::audio::SOUND_MAC_SAILBOAT_ROAR,
            Self::MacTakeoff => crate::audio::SOUND_MAC_TAKEOFF,
            Self::MacLand => crate::audio::SOUND_MAC_LAND,
            Self::MacKind2Traffic => crate::audio::SOUND_MAC_KIND2_TRAFFIC,
            Self::Horns => crate::audio::SOUND_HORNS,
            Self::Prison => crate::audio::SOUND_PRISON,
            Self::School => crate::audio::SOUND_SCHOOL,
            Self::Train => crate::audio::SOUND_TRAIN,
            Self::Military => crate::audio::SOUND_MILITARY,
            Self::Monster => crate::audio::SOUND_MONSTER,
            Self::LaunchArcoWipeStart => crate::audio::SOUND_LAUNCH_ARCO_WIPE_START,
            Self::LaunchArcoWipeEnd => crate::audio::SOUND_LAUNCH_ARCO_WIPE_END,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Plop => "plop",
            Self::PowerLine => "power line",
            Self::Pipe => "pipe",
            Self::Flood => "flood",
            Self::Police => "police",
            Self::Riot => "riot",
            Self::Fire => "fire",
            Self::FireTruck => "fire truck",
            Self::Cheers => "cheers",
            Self::Mayday => "mayday",
            Self::Disaster => "disaster",
            Self::Wind => "wind",
            Self::Explode => "explode",
            Self::Cargo => "cargo",
            Self::MacSailboatRoar => "Mac sailboat Roar1",
            Self::MacTakeoff => "Mac kind-1 Takeoff",
            Self::MacLand => "Mac kind-1 Land",
            Self::MacKind2Traffic => "Mac kind-2 traffic",
            Self::Horns => "horns",
            Self::Prison => "prison",
            Self::School => "school",
            Self::Train => "train",
            Self::Military => "military",
            Self::Monster => "monster",
            Self::LaunchArcoWipeStart => "launch arco wipe start",
            Self::LaunchArcoWipeEnd => "launch arco wipe end",
        }
    }

    pub(super) fn from_pe_effect_id(id: u16) -> Option<Self> {
        match id {
            crate::audio::SOUND_EXPLODE => Some(Self::Explode),
            crate::audio::SOUND_LAUNCH_ARCO_WIPE_START => Some(Self::LaunchArcoWipeStart),
            crate::audio::SOUND_LAUNCH_ARCO_WIPE_END => Some(Self::LaunchArcoWipeEnd),
            _ => None,
        }
    }

    fn play(self, sound: &crate::audio::SoundPlayer) {
        sound.play_sc2k_effect(self.sound_id(), self.label());
    }
}

impl ViewerState {
    pub(super) fn sound_effects_player<'a>(
        &self,
        sound: Option<&'a crate::audio::SoundPlayer>,
    ) -> Option<&'a crate::audio::SoundPlayer> {
        sound.filter(|_| self.sound_effects_enabled)
    }

    pub(super) fn play_click_sound(&self, sound: Option<&crate::audio::SoundPlayer>) {
        if let Some(sound) = self.sound_effects_player(sound) {
            sound.play_click();
        }
    }
}

pub(super) fn disaster_start_sound(kind: DisasterKind) -> PlacementSound {
    match kind {
        DisasterKind::Fire | DisasterKind::Firestorm => PlacementSound::Fire,
        DisasterKind::Flood | DisasterKind::BlizzardFlood => PlacementSound::Flood,
        DisasterKind::Hurricane => PlacementSound::Wind,
        DisasterKind::AirCrash => PlacementSound::Mayday,
        // Win95 types 0x05 / 0x11: only the consume common-path `0x208` alert.
        DisasterKind::NopAirCrash | DisasterKind::HelicopterCrash => PlacementSound::Disaster,
        DisasterKind::Tornado => PlacementSound::Wind,
        DisasterKind::Earthquake => PlacementSound::Explode,
        DisasterKind::Monster => PlacementSound::Monster,
        DisasterKind::Rioters | DisasterKind::MassRiot => PlacementSound::Riot,
        DisasterKind::Meltdown
        | DisasterKind::ToxicSpill
        | DisasterKind::Volcano
        | DisasterKind::PollutionAccident => PlacementSound::Explode,
        DisasterKind::Microwave => PlacementSound::PowerLine,
    }
}

pub(super) fn play_placement_sound(
    sound: Option<&crate::audio::SoundPlayer>,
    effect: PlacementSound,
) {
    if let Some(sound) = sound {
        effect.play(sound);
    }
}
