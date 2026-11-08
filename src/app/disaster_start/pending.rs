use super::*;

impl ViewerState {
    pub(super) fn queue_pe_disaster_start_effects(
        &mut self,
        kind: DisasterKind,
        origin: (usize, usize),
    ) {
        self.queue_pe_disaster_auto_goto(kind, origin);
        if self.sound_effects_enabled {
            let sound = disaster_start_sound(kind);
            self.pending_disaster_start_sound = Some(sound);
            // Win95 consume common-path `0x208` / OpenSC2K appends SOUND_SIREN after the
            // type-specific start sound. Types 0x05 / 0x11 already use Disaster alone.
            self.pending_disaster_start_siren = sound != PlacementSound::Disaster;
        }
    }

    fn queue_pe_disaster_auto_goto(&mut self, kind: DisasterKind, origin: (usize, usize)) {
        if !self.auto_goto_enabled {
            return;
        }
        // Types 5 / 0x11 have no PE start tile; do not invent a hover fallback.
        // Microwave / air-crash / flood goto are written after their process RNG probes.
        if matches!(
            kind,
            DisasterKind::NopAirCrash
                | DisasterKind::HelicopterCrash
                | DisasterKind::Microwave
                | DisasterKind::AirCrash
                | DisasterKind::Flood
                | DisasterKind::Fire
                | DisasterKind::Firestorm
        ) {
            return;
        }
        if kind == DisasterKind::BlizzardFlood {
            let mut peek_crt = self.pe_process_crt;
            let mut peek_lfsr = self.pe_process_lfsr;
            self.pending_disaster_auto_goto = pe_blizzard_flood_attempt_ok_with_rng(
                &self.city,
                origin.0,
                origin.1,
                &mut peek_crt,
                &mut peek_lfsr,
            )
            .then_some(origin)
            .or(Some(origin));
            return;
        }
        self.pending_disaster_auto_goto =
            pe_disaster_auto_goto_tile(&self.city, kind, origin).or(Some(origin));
    }

    pub(in crate::app) fn take_pending_disaster_auto_goto(&mut self) -> Option<(usize, usize)> {
        self.pending_disaster_auto_goto.take()
    }

    pub(in crate::app) fn take_pending_disaster_start_sound(&mut self) -> Option<PlacementSound> {
        self.pending_disaster_start_sound.take()
    }

    pub(in crate::app) fn take_pending_disaster_start_siren(&mut self) -> bool {
        let pending = self.pending_disaster_start_siren;
        self.pending_disaster_start_siren = false;
        pending
    }

    pub(in crate::app) fn queue_disaster_extra_sounds(
        &mut self,
        sound: PlacementSound,
        count: usize,
    ) {
        if !self.sound_effects_enabled || count == 0 {
            return;
        }
        self.pending_disaster_extra_sounds
            .extend(std::iter::repeat(sound).take(count));
    }

    pub(in crate::app) fn take_pending_disaster_extra_sounds(&mut self) -> Vec<PlacementSound> {
        std::mem::take(&mut self.pending_disaster_extra_sounds)
    }
}
