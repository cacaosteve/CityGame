use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};

pub(super) const DISASTER_MARKER_MIN: u8 = 0xfb;
pub(super) const DISASTER_MARKER_MAX: u8 = 0xff;
pub(super) const DISASTER_DAMAGE_MAX_TILES_PER_MONTH: usize = 4;
pub(super) const DISASTER_SPREAD_MAX_TILES_PER_MONTH: usize = 2;
pub(super) const AMBIENT_DISASTER_MIN_POPULATION: u32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DisasterKind {
    Fire,
    Flood,
    AirCrash,
    /// Win95 consume type 0x05: `mov esi,1` then persist, no start thunk.
    NopAirCrash,
    /// Win95 consume type 0x11: same silent persist; SCEN label "Helicopter Crash".
    HelicopterCrash,
    Tornado,
    Earthquake,
    Monster,
    Hurricane,
    Rioters,
    Meltdown,
    Microwave,
    ToxicSpill,
    Volcano,
    Firestorm,
    PollutionAccident,
    BlizzardFlood,
    MassRiot,
}

impl DisasterKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Fire => "Fire",
            Self::Flood => "Flood",
            Self::AirCrash => "Air Crash",
            Self::NopAirCrash => "Air Crash",
            Self::HelicopterCrash => "Helicopter Crash",
            Self::Tornado => "Tornado",
            Self::Earthquake => "Earthquake",
            Self::Monster => "Monster",
            Self::Hurricane => "Hurricane",
            Self::Rioters => "Rioters",
            Self::Meltdown => "Meltdown",
            Self::Microwave => "Microwave",
            Self::ToxicSpill => "Toxic Spill",
            Self::Volcano => "Volcano",
            Self::Firestorm => "Firestorm",
            Self::PollutionAccident => "Pollution Accident",
            Self::BlizzardFlood => "Mass Floods",
            Self::MassRiot => "Mass Riots",
        }
    }

    fn disaster_marker(self) -> u8 {
        match self {
            Self::Fire => 0xff,
            Self::Flood => 0xfc,
            Self::AirCrash => 0xff,
            Self::NopAirCrash => 0xff,
            Self::HelicopterCrash => 0xff,
            Self::Tornado => 0xff,
            Self::Earthquake => 0xff,
            Self::Monster => 0xff,
            Self::Hurricane => 0xfc,
            Self::Rioters => 0xfd,
            Self::Meltdown => 0xfb,
            Self::Microwave => 0xff,
            Self::ToxicSpill => 0xfb,
            Self::Volcano => 0xff,
            Self::Firestorm => 0xff,
            Self::PollutionAccident => 0xfb,
            Self::BlizzardFlood => 0xfc,
            Self::MassRiot => 0xfd,
        }
    }
}

/// Win95 type-4 start @ `0x45e590`: one XTXT `0xfb` at the trigger tile.
const TOXIC_SPILL_DISASTER_FOOTPRINT: &[(isize, isize)] = &[(0, 0)];

pub(super) fn disaster_footprint_offsets(kind: DisasterKind) -> &'static [(isize, isize)] {
    match kind {
        DisasterKind::Fire
        | DisasterKind::Flood
        | DisasterKind::AirCrash
        | DisasterKind::NopAirCrash
        | DisasterKind::HelicopterCrash
        | DisasterKind::Tornado
        | DisasterKind::Earthquake
        | DisasterKind::Monster
        | DisasterKind::Hurricane
        | DisasterKind::Rioters
        | DisasterKind::Meltdown
        | DisasterKind::Microwave
        | DisasterKind::Volcano
        | DisasterKind::Firestorm
        | DisasterKind::PollutionAccident
        | DisasterKind::BlizzardFlood
        | DisasterKind::MassRiot => &[],
        DisasterKind::ToxicSpill => TOXIC_SPILL_DISASTER_FOOTPRINT,
    }
}

pub(super) fn disaster_footprint_for_city(
    _city: &City,
    kind: DisasterKind,
) -> &'static [(isize, isize)] {
    disaster_footprint_offsets(kind)
}

const DISASTER_SPREAD_OFFSETS: [(isize, isize); 8] = [
    (1, 0),
    (0, 1),
    (-1, 0),
    (0, -1),
    (1, 1),
    (-1, 1),
    (1, -1),
    (-1, -1),
];

pub(super) fn disaster_marker_for_step(kind: DisasterKind, step: usize) -> u8 {
    match kind {
        DisasterKind::Rioters | DisasterKind::MassRiot if step % 2 == 1 => 0xfe,
        DisasterKind::Volcano if step % 2 == 1 => 0xfb,
        _ => kind.disaster_marker(),
    }
}

pub(super) fn is_disaster_text_overlay(value: u8) -> bool {
    (DISASTER_MARKER_MIN..=DISASTER_MARKER_MAX).contains(&value)
}

pub(super) fn next_disaster_marker(value: u8) -> u8 {
    match value {
        0xfd => 0xfe,
        0xfe => 0xfd,
        _ => value,
    }
}

pub(super) fn active_disaster_tiles_for_city(city: &City) -> Vec<(usize, usize, u8)> {
    let mut tiles = Vec::new();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let value = city.text_overlay_at(x, y).unwrap_or(0);
            if is_disaster_text_overlay(value) {
                tiles.push((x, y, value));
            }
        }
    }
    tiles
}

pub(super) fn disaster_spread_offsets(marker: u8) -> [(isize, isize); 8] {
    let start = usize::from(marker - DISASTER_MARKER_MIN) % DISASTER_SPREAD_OFFSETS.len();
    std::array::from_fn(|index| {
        DISASTER_SPREAD_OFFSETS[(start + index) % DISASTER_SPREAD_OFFSETS.len()]
    })
}
