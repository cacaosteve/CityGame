use crate::engine_fidelity::EngineFidelity;
use crate::mac_random::{MacAppLfsrRng, MacToolboxRng};
#[cfg(test)]
use crate::win95_overlay::XTHG_RECORD_COUNT;
use crate::win95_overlay::{
    DispatchKind, OVERLAY_KIND_TRAFFIC, RUST_DISPATCH_ACTOR_SENTINEL, RUST_TRAFFIC_ACTOR_KIND,
    Win95OverlayRecord, XTHG_AIRCRASH_HEIGHT, XTHG_AIRCRASH_STATE, XTHG_AIRCRASH_SUBTILE,
    XTHG_AIRPLANE, XTHG_CARGO_SHIP, XTHG_CHUNK_SIZE, XTHG_DIRECTION_EAST, XTHG_EXPLOSION,
    XTHG_EXPLOSION_SUBTILE, XTHG_EXPLOSION_XTXT_GATE, XTHG_FIRST_USABLE_SLOT, XTHG_HELICOPTER,
    XTHG_MONSTER, XTHG_MONSTER_HEIGHT, XTHG_MONSTER_SUBTILE, XTHG_PROXY_BASE, XTHG_SAILBOAT,
    XTHG_SUBWAY_TRAIN_CAR, XTHG_SUBWAY_TRAIN_ENGINE, XTHG_TORNADO, XTHG_TORNADO_SUBTILE,
    XTHG_TRAIN_CAR, XTHG_TRAIN_ENGINE, decode_xthg_records, empty_overlay_table,
    encode_xthg_records, pe_overlay_spawn_slot, xthg_direction_delta, xthg_proxy_for_slot,
    xthg_slot_for_proxy, xthg_spawn_height,
};
use crate::win95_random::{Win95CrtRand, Win95GameRand, Win95Lfsr};
use anyhow::{Context, Result, anyhow, bail};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

mod construction;
#[cfg(test)]
use construction::city_name_from_chunks;
mod destruction;
pub(crate) use destruction::{
    pe_choose_and_place_corner_bits, pe_destroy_building_corner_origin,
    pe_destroy_building_mode1_render_scatter_draw_count,
};
mod ambient_actors;
mod disaster_actors;
mod disaster_state;
mod economy;
mod eq_life;
mod scenario_state;
pub(crate) use scenario_state::MacScenarioResult;
mod service_state;
#[cfg(test)]
use eq_life::PeEqLeMonthParams;
#[allow(unused_imports)]
pub use eq_life::{
    pe_eq_birth_life_points, pe_eq_college_aux_param, pe_eq_education_life_penalty,
    pe_eq_fertile_averages, pe_eq_free_clinics_hospital_bump, pe_eq_hospital_birth_param,
    pe_eq_immigrant_chunk, pe_eq_ordinance_life_expectancy, pe_eq_ordinance_pro_reading,
    pe_eq_school_aux_param, pe_eq_survival_score,
};
mod file_format;
mod layer_codec;
mod layers;
use layers::MAC_XBLD_CENSUS_MISC_OFFSET;
#[cfg(test)]
use layers::{MAC_XUND_CLASS_COUNT_MISC_OFFSET, MAC_ZONE7_CENSUS_MISC_OFFSET};
mod mac_cohort;
mod mac_monthly_microsims;
mod mac_monthly_ordered_rows;
#[cfg(test)]
use mac_monthly_ordered_rows::{
    mac_college_professors_raw, mac_hospital_funding_half, mac_hospital_physicians_raw,
    mac_prison_guard_candidate, mac_prison_overcrowding_field, mac_school_funding_quarters,
    mac_school_teachers_raw,
};
mod microsim_capacity;
mod microsim_lifecycle;
mod misc_state;
mod state_layout;
use state_layout::*;
mod newspaper;
use newspaper::PeNewspaperStorage;
pub(crate) use newspaper::PeNewspaperTables;
use newspaper::{
    PE_NEWSPAPER_STORY_FUND_EDUCATION, PE_NEWSPAPER_STORY_NATIONAL_TAX_DOWN,
    PE_NEWSPAPER_STORY_NATIONAL_TAX_UP, PE_NEWSPAPER_STORY_NATIONAL_TREND,
    PE_NEWSPAPER_STORY_POWER_PLANT_AGING, PE_NEWSPAPER_STORY_PRISON_OVERCROWDED,
};
#[allow(unused_imports)]
pub use newspaper::{
    pe_attitude_education_divisor, pe_attitude_scores_from_inputs,
    pe_newspaper_story_triggers_extra_popup, pe_opinion_poll_weights,
};
mod overlay_actors;
#[cfg(test)]
use overlay_actors::dispatch_capacity_from_station_tile_count;
mod pe_misc_state;
mod pe_progression;
#[cfg(test)]
use microsim_lifecycle::{
    MAC_MICROSIM_CLASS, PE_MICROSIM_CLASS, budget_row_misc_offset_for_city,
    initial_structure_microsim_record, mac_microsim_class, pe_microsim_class,
};
mod queries;
pub(crate) use layers::multi_tile_building_size;
use microsim_capacity::{
    count_building_tiles, pe_microsim_population_load, pe_microsim_scaled_capacity,
    pe_occupancy_div9_count, pe_occupancy_div16_count, pe_occupancy_divisor,
    pe_occupancy_tile_words,
};
pub(crate) use microsim_capacity::{pe_arco_tile_pop_bonus, pe_occupancy_arco_footprints};
use misc_state::{
    misc_word_from_saved_dword, misc_word_to_saved_dword, misc_year_founded_saved_value,
    misc_year_founded_word, weather_cache_byte_from_misc,
};
#[cfg(test)]
use queries::density_sum;
mod save;
mod yearly_microsim_schedule;
mod yearly_microsims;
use economy::*;
#[allow(unused_imports)]
pub use economy::{pe_national_prosperity, pe_national_trend_from_prosperity};
use file_format::{
    CITYGAME_EXTENSION_CHUNK, city_rle_decode, decode_citygame_extension,
    encode_citygame_extension, upsert_chunk, upsert_chunk_for_kind,
};
#[allow(unused_imports)]
pub use file_format::{SAVE_CHUNK_ORDER, chunk_tags_in_file, sc2_rle_encode};
pub use file_format::{
    detect_city_kind, encode_scdh_file, mac_rle_decode, mac_rle_encode, parse_scdh_chunks,
    sc2_rle_decode,
};
use layer_codec::{
    CITYGAME_FIRE_COVERAGE_CHUNK, CITYGAME_FIRE_COVERAGE_VERSION, CITYGAME_MAP_PLANES_CHUNK,
    CITYGAME_MAP_PLANES_VERSION, cnam_text, decode_altm, decode_city_date, decode_city_view,
    decode_cnam_chunk, decode_expected_chunk, decode_expected_chunk_for_kind, decode_labels,
    decode_microsim64_chunk, decode_misc_bytes, decode_misc_bytes_for_kind, decode_misc_u32,
    decode_misc_u32_for_kind, decode_optional_chunk_layer, decode_optional_microsim32_chunk,
    decode_rotation_for_kind, decode_tile_bits, decode_win95_misc_record, decode_xmic, decode_xthg,
    decode_xtrf, decode_zones, encode_altm, encode_microsim32_for_save, encode_microsim64_for_save,
    encode_xtrf_for_save,
};
#[cfg(test)]
use layer_codec::{MicrosimOrder, resize_map_64_to_128, resize_map_128_to_64};
use yearly_microsim_schedule::{
    YearlyMicrosimRngInputs, power_plant_rebuild_cost, yearly_microsim_rng_schedule,
};
#[cfg(test)]
use yearly_microsims::{
    pe_hospital_doctors_raw, pe_hospital_funding_half, pe_park_attendance,
    pe_school_funding_quarters, pe_school_teachers_raw, pe_transit_depot_count,
};

pub const CITY_WIDTH: usize = 128;
pub const CITY_HEIGHT: usize = 128;
pub const CITY_TILES: usize = CITY_WIDTH * CITY_HEIGHT;
/// Win95 `XMIC`: 150 microsim records × 8 bytes.
pub const MICROSIM_COUNT: usize = 150;
pub const MICROSIM_RECORD_LEN: usize = 8;
/// First structure microsim index (0 is unused; 1..=9 are city-wide system microsims).
pub const MICROSIM_STRUCTURE_FIRST: usize = 10;
/// Win95 user-sign XLAB slots (`MIN_USER_TEXT_ENTRIES`..=`MAX_USER_TEXT_ENTRIES`).
/// Overlay 0 is the mayor name, not a map sign.
pub const MIN_USER_TEXT_ENTRIES: u8 = 1;
pub const MAX_USER_TEXT_ENTRIES: u8 = 50;
/// Win95 `MIN_SIM_TEXT_ENTRIES` / `MAX_SIM_TEXT_ENTRIES` (`XTXT` ↔ `XMIC`).
pub const MIN_SIM_TEXT_ENTRIES: u8 = 51;
pub const MAX_SIM_TEXT_ENTRIES: u8 = 200;
/// Win95 `QuerySpecificItem` (`0x44d1fe`): `XTXT == 0x6f` runs
/// `RecalculateMayorsHouseStats` (`0x414b40`) before the XLAB title.
/// Overlay 111 is microsim index 60 (`51+60`), not the first mayor house
/// (typically index 10 / overlay 61).
pub const QUERY_SPECIFIC_MAYOR_STATS_OVERLAY: u8 = 0x6f;
/// Absolute XZON corner bit checked by Win95 `XZONCornerAbsoluteCheckMask(..., CORNER_TRIGHT)`.
pub const ZONE_CORNER_TRIGHT: u8 = 0x80;
/// Win95 ChooseAndPlace pop1 (`0x436e80`) `or byte [XZON], 0xf0` (`CORNER_ALL`).
pub const ZONE_CORNER_ALL: u8 = 0xf0;
/// Win95 ChooseAndPlace (`0x436e80`) `or ..., 0xe0` (`POWERABLE|POWERED|PIPED`).
/// Pop1 ORs the call tile; pop≥2 ORs every tile in the footprint.
pub const XBIT_CHOOSE_AND_PLACE: u8 = 0xe0;

/// City-wide system microsim slots (`XTXT` 0x34..=0x3C).
pub const MICROSIM_SYSTEM_BUS: usize = 1;
pub const MICROSIM_SYSTEM_RAIL: usize = 2;
pub const MICROSIM_SYSTEM_SUBWAY: usize = 3;
pub const MICROSIM_SYSTEM_WIND: usize = 4;
pub const MICROSIM_SYSTEM_HYDRO: usize = 5;
pub const MICROSIM_SYSTEM_PARK: usize = 6;
pub const MICROSIM_SYSTEM_MUSEUM: usize = 7;
pub const MICROSIM_SYSTEM_LIBRARY: usize = 8;
pub const MICROSIM_SYSTEM_MARINA: usize = 9;
const DEFAULT_CORNER_KEY_BITS: [u8; 4] = [0x40, 0x20, 0x10, 0x80];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ZoneFlags {
    bits: u8,
}

/// Per-tile `XBIT` flags (OpenSC2K / Mac save format).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TileBits {
    bits: u8,
}

pub const CITY_LABEL_COUNT: usize = 256;
const CITY_LABEL_RECORD_LEN: usize = 25;
const CITY_LABEL_TEXT_MAX: usize = 23;

impl TileBits {
    pub fn new(bits: u8) -> Self {
        Self { bits }
    }

    pub fn raw_bits(self) -> u8 {
        self.bits
    }

    /// Per-tile 90° orientation for bridges, on-ramps, pier, and runway (XBIT bit 1).
    pub fn rotate(self) -> bool {
        self.bits & 0b0000_0010 != 0
    }

    pub fn with_rotate(self, rotate: bool) -> Self {
        if rotate {
            Self {
                bits: self.bits | 0b0000_0010,
            }
        } else {
            Self {
                bits: self.bits & !0b0000_0010,
            }
        }
    }

    /// OpenSC2K `XBIT` bit 5 — tile can receive piped water.
    pub fn piped(self) -> bool {
        self.bits & 0b0010_0000 != 0
    }

    /// OpenSC2K `XBIT` bit 4 — tile currently has piped water.
    pub fn watered(self) -> bool {
        self.bits & 0b0001_0000 != 0
    }

    pub fn with_piped(self, piped: bool) -> Self {
        if piped {
            Self {
                bits: self.bits | 0b0010_0000,
            }
        } else {
            Self {
                bits: self.bits & !0b0010_0000,
            }
        }
    }

    pub fn with_watered(self, watered: bool) -> Self {
        if watered {
            Self {
                bits: self.bits | 0b0001_0000,
            }
        } else {
            Self {
                bits: self.bits & !0b0001_0000,
            }
        }
    }

    /// OpenSC2K `XBIT` bit 0 — salt water, not a traffic lane flag.
    pub fn salt_water(self) -> bool {
        self.bits & 0b0000_0001 != 0
    }

    /// SC2KRender / ALTM placement — bit 2 marks tiles whose structures sit on the water surface.
    pub fn water_tile(self) -> bool {
        self.bits & 0b0000_0100 != 0
    }

    /// XBIT bit 2 — surface-water mark (Win95 `MapToolPlaceWater` `0x446a58`;
    /// Mac terrain/new-map and cursor-dispatch handlers use the same bit).
    pub fn with_water_tile(self, water: bool) -> Self {
        if water {
            Self {
                bits: self.bits | 0b0000_0100,
            }
        } else {
            Self {
                bits: self.bits & !0b0000_0100,
            }
        }
    }

    /// Win95 `XBIT` bit 3 — terrain flatten mark (`0x40237e` / volcano dummy-funds).
    pub fn flatten_mark(self) -> bool {
        self.bits & 0b0000_1000 != 0
    }

    /// Win95 `XBIT` bit 6 — tile on an energized power network (`XBIT_POWERED` / `0x40`).
    pub fn powered(self) -> bool {
        self.bits & 0b0100_0000 != 0
    }

    /// Win95 `XBIT` bit 7 — power-source eligibility (`XBIT_POWERABLE` / `0x80`).
    pub fn powerable(self) -> bool {
        self.bits & 0b1000_0000 != 0
    }

    pub fn with_powered(self, powered: bool) -> Self {
        if powered {
            Self {
                bits: self.bits | 0b0100_0000,
            }
        } else {
            Self {
                bits: self.bits & !0b0100_0000,
            }
        }
    }

    pub fn with_powerable(self, powerable: bool) -> Self {
        if powerable {
            Self {
                bits: self.bits | 0b1000_0000,
            }
        } else {
            Self {
                bits: self.bits & !0b1000_0000,
            }
        }
    }

    pub fn with_flatten_mark(self, mark: bool) -> Self {
        if mark {
            Self {
                bits: self.bits | 0b0000_1000,
            }
        } else {
            Self {
                bits: self.bits & !0b0000_1000,
            }
        }
    }
}

/// One tile of the `ALTM` chunk: land height in bits 0–4, water level in bits 5–9,
/// and tunnel depth in the upper bits. The original game uses five tunnel bits but the sixth
/// upper bit is retained so loading and saving does not discard unknown source data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AltmSample {
    pub land: u8,
    pub water: u8,
    pub tunnel_levels: u8,
}

impl ZoneFlags {
    fn new(bits: u8) -> Self {
        Self { bits }
    }

    pub fn top(self) -> bool {
        self.bits & 0b0001_0000 != 0
    }

    pub fn right(self) -> bool {
        self.bits & 0b0010_0000 != 0
    }

    pub fn bottom(self) -> bool {
        self.bits & 0b0100_0000 != 0
    }

    pub fn left(self) -> bool {
        self.bits & 0b1000_0000 != 0
    }

    pub fn any_corner(self) -> bool {
        self.bits & 0b1111_0000 != 0
    }

    pub fn zone_sprite_id(self) -> Option<u32> {
        match self.bits & 0b0000_1111 {
            1..=9 => Some(1290 + (self.bits & 0b0000_1111) as u32),
            _ => None,
        }
    }

    /// XZON tile-class nibble. PE `FUN_004358B0` reads `XZON[x][y] & 0x0f`
    /// as the growth category used to index `0x4CA428`.
    pub fn zone_id(self) -> u8 {
        self.bits & 0b0000_1111
    }

    pub fn raw_bits(self) -> u8 {
        self.bits
    }

    /// Which corner tile of a multi-tile footprint anchors the building sprite at a given view
    /// rotation. Each footprint stores all four XZON corner bits; the renderer draws only the
    /// front-most projected corner so the single sprite covers the full footprint.
    pub fn key_for_rotation(self, rotation: u8) -> bool {
        self.raw_bits() & DEFAULT_CORNER_KEY_BITS[(rotation & 0x03) as usize] != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CityFileKind {
    MacCity,
    WinSc2,
    WinScenario,
}

impl CityFileKind {
    pub fn is_win95(self) -> bool {
        matches!(self, Self::WinSc2 | Self::WinScenario)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XtrfStorageFormat {
    #[default]
    PerTile128,
    Microsim64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CityChunk {
    pub name: String,
    pub offset: usize,
    pub raw_len: usize,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MicrosimRecord {
    pub tile_id: u8,
    pub stat0: u8,
    pub stat1: u16,
    pub stat2: u16,
    pub stat3: u16,
}

impl MicrosimRecord {
    fn from_bytes(raw: &[u8]) -> Self {
        Self {
            tile_id: raw[0],
            stat0: raw[1],
            stat1: u16::from_be_bytes([raw[2], raw[3]]),
            stat2: u16::from_be_bytes([raw[4], raw[5]]),
            stat3: u16::from_be_bytes([raw[6], raw[7]]),
        }
    }

    fn to_bytes(self) -> [u8; MICROSIM_RECORD_LEN] {
        let mut out = [0u8; MICROSIM_RECORD_LEN];
        out[0] = self.tile_id;
        out[1] = self.stat0;
        out[2..4].copy_from_slice(&self.stat1.to_be_bytes());
        out[4..6].copy_from_slice(&self.stat2.to_be_bytes());
        out[6..8].copy_from_slice(&self.stat3.to_be_bytes());
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CityDate {
    pub year: u16,
    pub month: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Technology {
    GasPower,
    NuclearPower,
    SolarPower,
    WindPower,
    MicrowavePower,
    FusionPower,
    Airport,
    Highways,
    Buses,
    Subways,
    WaterTreatment,
    Desalinization,
    PlymouthArcology,
    ForestArcology,
    DarcoArcology,
    LaunchArcology,
    Highways2,
}

impl Technology {
    pub const ALL: [Self; 17] = [
        Self::GasPower,
        Self::NuclearPower,
        Self::SolarPower,
        Self::WindPower,
        Self::MicrowavePower,
        Self::FusionPower,
        Self::Airport,
        Self::Highways,
        Self::Buses,
        Self::Subways,
        Self::WaterTreatment,
        Self::Desalinization,
        Self::PlymouthArcology,
        Self::ForestArcology,
        Self::DarcoArcology,
        Self::LaunchArcology,
        Self::Highways2,
    ];

    pub(crate) const fn misc_offset(self) -> usize {
        match self {
            Self::GasPower => MISC_NEIGHBOR_FAME_RANKS_OFFSET,
            Self::NuclearPower => 0x073c,
            Self::SolarPower => 0x0740,
            Self::WindPower => 0x0744,
            Self::MicrowavePower => 0x0748,
            Self::FusionPower => 0x074c,
            Self::Airport => 0x0750,
            Self::Highways => 0x0754,
            Self::Buses => 0x0758,
            Self::Subways => 0x075c,
            Self::WaterTreatment => 0x0760,
            Self::Desalinization => 0x0764,
            Self::PlymouthArcology => 0x0768,
            Self::ForestArcology => 0x076c,
            Self::DarcoArcology => 0x0770,
            Self::LaunchArcology => 0x0774,
            Self::Highways2 => 0x0778,
        }
    }
}

pub const DEFAULT_NEW_CITY_TECHNOLOGY_DISCOVERY_YEARS: [(Technology, u32); 17] = [
    (Technology::GasPower, 1946),
    (Technology::NuclearPower, 1963),
    (Technology::SolarPower, 1985),
    (Technology::WindPower, 1986),
    (Technology::MicrowavePower, 2034),
    (Technology::FusionPower, 2052),
    (Technology::Airport, 1938),
    (Technology::Highways, 1932),
    (Technology::Buses, 1929),
    (Technology::Subways, 1916),
    (Technology::WaterTreatment, 1940),
    (Technology::Desalinization, 1988),
    (Technology::PlymouthArcology, 1999),
    (Technology::ForestArcology, 2052),
    (Technology::DarcoArcology, 2094),
    (Technology::LaunchArcology, 2149),
    (Technology::Highways2, 2192),
];

/// Shared ordered base-year table used by the Mac QuickDraw scheduler and
/// Win95 CRT scheduler; the engines differ in random stream and lifecycle.
pub(super) const TECHNOLOGY_BASE_DISCOVERY_YEARS: [u16; PE_NEIGHBOR_FAME_RANK_COUNT] = [
    1940, 1950, 1980, 1970, 2020, 2050, 1920, 1920, 1910, 1900, 1925, 1980, 1990, 2040, 2090, 2140,
    2190,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CityView {
    /// Win95 MISC 0x1014. The saved view scale selector (0, 1, or 2).
    pub mode: u32,
    /// Win95 MISC 0x1010 packed word, low seven bits. The loader uses `(64, 128)` for `0xffff`.
    pub x: u16,
    /// Win95 MISC 0x1010 packed word, signed high byte.
    pub y: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    CityView,
    EditNewMap,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct City {
    pub path: PathBuf,
    pub kind: CityFileKind,
    pub name: String,
    pub chunks: Vec<CityChunk>,
    chunk_index: HashMap<String, usize>,
    /// Mac's saved budget book occupies bytes `0x77c..0xe3c` in MISC. Those
    /// bytes overlap several PE-only runtime snapshots, so keep the original
    /// Mac rows separate while the shared simulation updates its PE layout.
    mac_budget_book: Option<Vec<u8>>,
    terrain: Option<Vec<u8>>,
    buildings: Option<Vec<u8>>,
    underground: Option<Vec<u8>>,
    altm: Option<Vec<AltmSample>>,
    zones: Option<Vec<ZoneFlags>>,
    text_overlays: Option<Vec<u8>>,
    labels: Option<Vec<String>>,
    microsims: Option<Vec<MicrosimRecord>>,
    tile_bits: Option<Vec<TileBits>>,
    traffic_density: Option<Vec<u8>>,
    /// Win95 `XVAL` is 64×64 on disk and expanded to 128×128 in memory.
    land_value_density: Option<Vec<u8>>,
    /// Win95 `XCRM` is 64×64 on disk and expanded to 128×128 in memory.
    crime_density: Option<Vec<u8>>,
    pollution_density: Option<Vec<u8>>,
    /// Win95 `XPLC` / Mac `ffffeb40` police coverage (32×32 on disk; expanded to 128×128 in memory).
    police_coverage: Option<Vec<u8>>,
    /// Win95 `XFIR` / Mac `ffffebc0` fire coverage (32×32 on disk; expanded to 128×128 in memory).
    fire_coverage: Option<Vec<u8>>,
    /// Win95 `XPOP` / Mac `ffffec40` population density (32×32 on disk; expanded to 128×128 in memory).
    population_density: Option<Vec<u8>>,
    /// Win95 `XROG` / Mac `ffffecc0` rate of growth (32×32 on disk; expanded to 128×128 in memory).
    rate_of_growth: Option<Vec<u8>>,
    /// On-disk `XTRF` layout: Win95 microsim is 64×64 (`4096` bytes); Mac expanded cities use 128×128.
    xtrf_storage: XtrfStorageFormat,
    /// Win95 `XTHG` overlay actor table (`004ca434`). Loaded when present; simulated only by
    /// the optional Win95 debug path, not by the default shared XTRF road-overlay renderer.
    traffic_overlays: Option<Vec<Win95OverlayRecord>>,
    /// True when `traffic_overlays` came from serialized XTHG. Raw PE kind 0x06 is an explosion,
    /// so only the runtime port kind is eligible for road-traffic simulation after load.
    traffic_overlays_from_file: bool,
    /// PE `wActiveTrains` can temporarily exceed the count of usable XTHG head
    /// records when the native constructor writes through allocator slot zero.
    /// The next PE actor sweep rebuilds it from usable slots.
    pub(crate) pe_active_train_count_override: Option<u16>,
    /// Win95 BSS `0x4e99dc`: starts and visits since the last Things reset.
    pub(crate) pe_tornado_actor_counter: u16,
    /// Mac CODE 6 `ffffa09a`: transient visit count shared by marina kind-9 actors.
    /// It is separate from the Win95 sailboat budget and is reset by the Mac actor sweep.
    pub(crate) mac_marina_actor_counter: u16,
    /// Mac CODE 6 `ffffa096`: transient kind-3 visit/creator gate shared with export 116.
    pub(crate) mac_kind3_actor_counter: u16,
    /// Mac CODE 6 `ffffa092`: transient kind-1 visit/creator gate shared with export 114.
    pub(crate) mac_kind1_actor_counter: u16,
    /// Mac CODE 6 `ffffa094`: transient kind-2 visit/creator gate shared with export 115.
    pub(crate) mac_kind2_actor_counter: u16,
    /// Mac CODE 6 `ffffa09c`: kind-5 creator/handler count rebuilt by the
    /// actor sweep and incremented once at each kind-5 handler entry.
    pub(crate) mac_kind5_actor_counter: u16,
    /// Mac CODE 6 `ffffa098`: kind-16 visit/creator count rebuilt by the actor sweep.
    pub(crate) mac_kind16_actor_counter: u16,
    /// Mac CODE 6 `ffffa0a0`: transient kind-15 tornado starts/updates within
    /// the current actor-sweep interval.
    pub(crate) mac_tornado_actor_counter: u16,
    /// Mac CODE 6 `ffffa09e`: transient kind-10/12 rail-head count rebuilt by
    /// export 109 and incremented by the rail-group constructor.
    pub(crate) mac_rail_actor_counter: u16,
    /// Mac phase-2 `ffffbb50`, kept independently of the PE MISC layout.
    pub(crate) mac_developed_cell_snapshot: Option<u16>,
    /// Mac transit route accumulators when the loaded file has Win95 layout.
    pub(crate) mac_transit_accumulators_runtime: [u32; 3],
    /// Mac CODE 2 pending action selector when Win95 owns MISC `+0x70`.
    /// Mac-layout cities store the selector in their native MISC field instead.
    pub(crate) mac_pending_action_runtime: u32,
    /// Mac CODE 14 scenario-active byte when Win95 owns the colliding MISC slot.
    /// `None` means the first-open value still comes from the raw saved MISC.
    pub(crate) mac_scenario_active_runtime: Option<bool>,
    mac_scenario_runtime: Option<scenario_state::MacScenarioContinuation>,
    /// Mac monthly RCI scan counters for a city with Win95 storage layout.
    pub(crate) mac_rci_counters_runtime: [u32; 8],
    /// Mac's phase-21 residential pair snapshot when Win95 owns MISC `+0x74`.
    mac_residential_total_runtime: Option<u32>,
    /// Phase-21 change balances at Mac `fffff01e` and `fffff022`.
    pub(crate) mac_population_change_balances: [u32; 2],
    /// Win95 fidelity has explicitly prepared a Mac-layout city.
    pub(crate) pe_cross_layout_active: bool,
    /// PE neighbor/technology WORD[17] when its MISC bytes belong to Mac state.
    pub(crate) pe_cross_layout_saved_neighbor_fame_ranks:
        Option<[u16; PE_NEIGHBOR_FAME_RANK_COUNT]>,
    /// PE active-disaster values saved in CGEX when Mac owns the MISC slots.
    /// Kept separate until Win95 fidelity is selected for this city.
    pub(crate) pe_cross_layout_saved_current_disaster_type: Option<u16>,
    pub(crate) pe_cross_layout_saved_disaster_active: Option<u32>,
    /// PE annual police inputs saved in CGEX when Mac owns MISC `+0x1038/+0x103c`.
    pub(crate) pe_cross_layout_saved_old_arrests: Option<u16>,
    pub(crate) pe_cross_layout_saved_prison_bonus: Option<u16>,
    /// PE sewer pollution-blur bonus saved in CGEX when Mac owns MISC `+0x1050`.
    pub(crate) pe_cross_layout_saved_sewer_bonus: Option<u16>,
    /// PE hurricane/flood geography saved in CGEX when Mac owns MISC `+0xe44/+0xe48`.
    pub(crate) pe_cross_layout_saved_city_has_ocean: Option<u32>,
    pub(crate) pe_cross_layout_saved_city_has_river: Option<u32>,
    /// PE XUND occupancy when Mac owns the shared MISC `+0xfe8` slot.
    pub(crate) pe_cross_layout_saved_xund_occupancy: Option<u16>,
    /// PE city-progression level when Mac owns the shared MISC `+0x20` slot.
    pub(crate) pe_cross_layout_saved_city_progression: Option<u16>,
    /// PE rewards mask when Mac owns MISC `+0x78` as its command-strip latch.
    pub(crate) pe_cross_layout_saved_rewards_available: Option<u32>,
    /// PE industry-mix values when Mac owns MISC `+0x1030/+0x1034`.
    pub(crate) pe_cross_layout_saved_industrial_mix_bonus: Option<u16>,
    pub(crate) pe_cross_layout_saved_industrial_mix_pollution_bonus: Option<i16>,
    /// PE `dwCityTrafficCount` when Mac owns MISC `+0x30` as its XTRF total.
    pub(crate) pe_cross_layout_saved_traffic_count: Option<u32>,
    /// PE `wMonsterXTHGIndex` when Mac owns MISC `+0x1040` as native state.
    pub(crate) pe_cross_layout_saved_monster_xthg_index: Option<u16>,
    /// PE weather when Mac owns MISC `+0x60..+0x6c`.
    /// Mac fidelity has explicitly prepared a Windows-layout city.
    pub(crate) mac_cross_layout_active: bool,
    /// Mac CODE 13's paid road-link tally when PE owns MISC +0x1050.
    pub(crate) mac_road_neighbor_link_count_runtime: Option<u16>,
    /// Mac CODE 4 `ffffdc74` command-strip latch when PE owns MISC `+0x78`.
    pub(crate) mac_cross_layout_saved_civic_latch: Option<u32>,
    /// Mac saved XBLD census when running a Windows-layout city.
    pub(crate) mac_xbld_census_runtime: [u16; 256],
    /// True when this Win-layout city restored the Mac-owned state snapshot
    /// from its CityGame extension; otherwise Mac preparation reads the
    /// native low-WORD census values from MISC +0x1f0.
    pub(crate) mac_cross_layout_state_restored: bool,
    /// Mac CODE 6 `ffffa0a6/a8`: last successfully created kind-3 boundary
    /// coordinate in raw record row/column order. Runtime-only; the native
    /// process-global value's behavior across city replacement is unmodeled.
    pub(crate) mac_kind3_boundary_target: (u8, u8),
    /// Mac CODE 6 `ffffe4b4/e4b6`: developed-map hotspot. Win-layout files
    /// keep the Mac engine's value here instead of overwriting PE MISC slots.
    pub(crate) mac_kind2_hotspot_runtime: Option<(u16, u16)>,
    /// Win95 `0x4e99d0`: sailboat spawn/handler budget (reset each overlay tick).
    pub(crate) pe_sailboat_budget: u16,
    /// Win95 `0x4ca41c` PlaceXUND subway/crossover occupancy (MISC `0x0fe8`;
    /// hydrate `0x431a40`, persist `0x4340b8`).
    pub(crate) pe_xund_occupancy: u16,
    /// Win95 `0x4ca4c8` → `WORD[256]`: PlaceTile non-military XBLD census (BSS).
    pub(crate) pe_dw_tile_count: [u16; 256],
    /// Win95 `DAT_004CA3F0`: non-network neighbor links accepted through the
    /// paid confirmation flow. Runtime WORD cleared on city open; separate from
    /// edge geometry used by off-map trips.
    pub(crate) pe_nonnetwork_neighbor_links: u16,
    /// Difference between the live PE road-link WORD and its tile-class census.
    /// The original counts purchases by tool and reclassifies XTXT on load.
    pub(crate) pe_road_link_census_delta: u16,
    /// Session-only provenance for XTXT markers synthesized for off-map trips.
    /// Loaded markers have no provenance and remain native-visible.
    pe_synthetic_trip_markers: Vec<bool>,
    /// Win95 `0x4ca934` → `WORD[16]`: PlaceTile military XZON type-7 census (BSS).
    pub(crate) pe_military_tile_count: [u16; MILITARY_TILE_COUNT_VALUES],
    /// Win95 `0x4ca4d0`: RecalculateCityValue running total (BSS).
    pub(crate) pe_city_value: i32,
    /// Win95 `0x4e80cc` / `0x4e80d0`: SpawnShip border tile; state 3 copies into XTHG `+8/+9`.
    pub(crate) pe_cargo_spawn_tile: (u8, u8),
    /// Runtime cursors used by the original emergency dispatch palette's per-service rings.
    dispatch_cursors: [usize; 3],
    mac_dispatch: overlay_actors::MacDispatch,
    /// Whether the saved Win95 RCI demand triple has been loaded or written. A fresh
    /// MISC record is zero-filled, so byte availability alone cannot distinguish it
    /// from an explicit saved zero-demand triple.
    pe_rci_demand_initialized: bool,
    /// Win95 `0x4CC4B8`: watered consumers × 100 / demand (100 when demand is 0). Not in MISC.
    pe_water_coverage_percent: u32,
    /// Win95 `dwPowerUsedPercentage` @ `0x4caa50` (viewer MISC snapshot `+0x112c`).
    pe_power_used_percent: u32,
    /// Win95 `dwBusPassengers` @ `0x4c85a0` (viewer MISC snapshot `+0x1120`).
    pe_dw_bus_passengers: u32,
    /// Win95 `dwRailPassengers` @ `0x4c85a4` (viewer MISC snapshot `+0x1124`).
    pe_dw_rail_passengers: u32,
    /// Win95 `dwSubwayPassengers` @ `0x4c8600` (viewer MISC snapshot `+0x1128`).
    pe_dw_subway_passengers: u32,
    /// Mac CODE 3 `export_162` phase-local `fffffe04` police arrest subtotal.
    /// Rebuilt by the monthly police XMIC rows and consumed by prison rows;
    /// not serialized as independent city state.
    mac_monthly_arrest_total: u16,
    /// Mac process approval word. The session carries it across city changes.
    mac_mayor_approval: u16,
    /// Mac process background flag at ffff8c7e. Excluded from saves.
    mac_newspaper_suppressed: bool,
    /// Mac police gate for Win95-layout cities, whose MISC slot belongs to PE.
    mac_police_gate_runtime: i16,
    /// Mac phase-0 arcology total for a city with Win95 MISC ownership.
    mac_arcology_total_runtime: Option<u32>,
    /// Mac service summaries when Win95 owns the corresponding MISC slots.
    mac_power_summary_runtime: u32,
    mac_water_summary_runtime: u32,
    /// Mac water-treatment gate when Win95 owns the corresponding MISC slot.
    mac_treatment_gate_runtime: Option<i16>,
    /// Win95 EQ_LE age histogram at `0x4c94b4` (20 dwords). School uses [1]+[2], college [3].
    /// January occupancy reads restored MISC `0x7c` tables; empty cities stay empty.
    pe_age_cohorts: [u32; 20],
    /// Win95 `0x4caa70` parallel life-point totals per EQ_LE age bucket.
    pe_eq_life_points: [u32; 20],
    /// Win95 `0x4c94bc` parallel aux totals per EQ_LE age bucket.
    pe_eq_aux_points: [u32; 20],
    /// Win95 `0x4ca4cc` death accumulator from the EQ_LE mortality loop.
    pe_eq_deaths: u32,
    /// Win95 `0x4cb1a8` birth accumulator from the EQ_LE fertile-birth loop.
    pe_eq_births: u32,
    /// Win95 `0x4caa74` `dwCityPopulation`: day-22 `pZonePops[0]*10` snapshot.
    /// Day 21 EQ_LE tests this word, not the live census. Hydrated from MISC `0x102c`.
    pe_dw_city_population: u32,
    /// Win95 `0x4c94c4` `dwArcologyPopulation` (MISC `0x1020`); hydrate `0x431b8e`, persist `0x43421f`.
    pe_dw_arcology_population: u32,
    /// Win95 `0x4cc4e0` `wSportsTeams` (MISC `0x1028`); hydrate `0x431ba9`, persist `0x434251`.
    pe_w_sports_teams: u16,
    /// Win95 `0x4ca1e8` `wIndustrialMixBonus` (MISC `0x1030`); hydrate `0x431bd7`, persist `0x43427a`.
    pe_w_industrial_mix_bonus: u16,
    /// Win95 `0x4c9428` `wIndustrialMixPollutionBonus` (MISC `0x1034`); hydrate `0x431be6`, persist `0x434294`.
    pe_w_industrial_mix_pollution_bonus: i16,
    /// Win95 `0x4ca430` `wOldArrests` (MISC `0x1038`); hydrate `0x431bf5`, persist `0x43429f`.
    pe_w_old_arrests: u16,
    /// Win95 `0x4ca1dc` `wPrisonBonus` (MISC `0x103C`); hydrate `0x431c04`, persist `0x4342ba`.
    pe_w_prison_bonus: u16,
    /// Win95 `0x4cb3dc` `wSewerBonus` (MISC `0x1050`); hydrate `0x431c45`, persist `0x434310`.
    pe_w_sewer_bonus: u16,
    /// Win95 `0x4ca81c` `wMonsterXTHGIndex` (MISC `0x1040`); hydrate `0x431c0e`, persist `0x4342cf`.
    /// Cleared value is `0xffff` (low byte `0xff` for Maxis Man payload).
    pe_w_monster_xthg_index: u16,
    /// Win95 `0x4ca93c` `wCurrentDisasterType` (MISC `0x1044`); hydrate `0x431c2c`, persist `0x4342e4`.
    pe_w_current_disaster_type: u16,
    /// OpenSC2K / PE damage-class for disaster-end newspaper param (`0x4e8848` weights).
    /// `-1` means unset (OpenSC2K `disaster_damage_class`).
    pe_disaster_damage_class: i16,
    /// Win95 `0x4c9ee8` `dwDisasterActive` (MISC `0x1048`); hydrate `0x431c3b`, persist `0x4342f2`.
    pe_dw_disaster_active: u32,
    /// Win95 `0x4c94c0` `dwCityHasOcean` (MISC `0x0e44`); hydrate `0x43189a`, persist `0x433ed8`.
    pe_dw_city_has_ocean: u32,
    /// Win95 `0x4cb3f8` `dwCityHasRiver` (MISC `0x0e48`); hydrate `0x4318ab`, persist `0x433eec`.
    pe_dw_city_has_river: u32,
    /// Win95 national population dword `0x4ca928` (MISC `+0x50`).
    pub(crate) pe_dw_national_population: u32,
    /// Win95 national value dword `0x4cb014` (MISC `+0x54`).
    pub(crate) pe_dw_national_value: u32,
    /// Win95 national tax word `0x4ca938` (MISC `+0x58`).
    pub(crate) pe_w_national_tax: u16,
    /// Win95 national economy trend word `0x4ca1bc` (MISC `+0x5c`).
    pub(crate) pe_w_national_trend: u16,
    /// Growth-aggregate slot 7 snapped at day-21 publish for EQ_LE `esp+0x10`.
    pe_eq_growth_slot7: i32,
    /// Win95 `pZonePops[0..2]` from folded growth slots `(1+2)/(3+4)/(5+6)`.
    pe_zone_pops: [i32; 3],
    /// True after day-21 growth-aggregate publish while zone growth is enabled.
    pe_zone_pops_ready: bool,
    /// Day-22 leftover parked in `0x4ca4cc` until the next day-21 EQ_LE.
    pe_eq_leftover_deaths: u32,
    /// Day-22 leftover parked in `0x4cb1a8` until the next day-21 EQ_LE.
    pe_eq_leftover_births: u32,
    /// Win95 `0x4cb410` fertile-cohort (buckets 4..=10) average life points.
    pe_eq_life_average: u32,
    /// Win95 `0x4cc4b4` fertile-cohort average aux (education quotient).
    pe_eq_education_average: u32,
    /// Win95 `0x4cb460`: fertile pop × 100 / (`dwCityPopulation` + 1), with PE dword width.
    pe_eq_workforce_percent: u32,
    /// True after a monthly fertile-average write (`0x469be8`); seed leaves this false.
    pe_eq_averages_ready: bool,
    /// Coverage life expectancy (hospital overlay); EQ_LE seed/births use ordinance LE.
    pe_life_expectancy: u8,
    /// Win95 `0x4E9904` approval score latched by the opinion poll.
    pe_mayor_approval: u16,
    /// Win95 `0x4CA5F8` unemployment percent; app refreshes before January microsims.
    pe_unemployment_percent: u32,
    /// Win95 `0x4CC4B4` city EQ average / opinion-poll input, stored as a dword.
    pub(crate) pe_city_crime: u32,
    /// Win95 `0x4CB410` city LE average / opinion-poll input, stored as a dword.
    pub(crate) pe_city_pollution: u32,
    /// Win95 `0x4CC6F4` traffic total fed into attitude score 0 (`0x413f89`).
    pe_traffic_count: u32,
    /// R/C/I tax bases used as the `+0x4c0/+0x530/+0x5a0` attitude denominator.
    pe_attitude_tax_bases: [u32; 3],
    /// Win95 `wCityDevelopedTiles` `0x4c93b4` (XBLD `>= 0x1D` or XZON nibble).
    pub(crate) pe_w_city_developed_tiles: u32,
    /// Win95 `dwCityPollution` `0x4cc910` (XPLT 64×64 sum / MISC `+0x34`) → attitude score 1.
    pub(crate) pe_dw_city_pollution: u32,
    /// Win95 `dwCityLandValue` `0x4ca440` (XVAL sum / MISC `+0x28`) → attitude score 2.
    pub(crate) pe_dw_city_land_value: u32,
    /// Win95 city funds dword `0x4ca444` (MISC `+0x14`; signed treasury).
    pub(crate) pe_i32_city_funds: i32,
    /// Win95 city age (days) dword `0x4cae04` (MISC `+0x10`).
    pub(crate) pe_dw_city_age: u32,
    /// Win95 `wCityDifficulty` word `0x4cb404` (MISC `+0x1c`).
    pub(crate) pe_w_city_difficulty: u16,
    /// Win95 year founded word `0x4ca5f4` (MISC `+0x0c`).
    pub(crate) pe_w_year_founded: u16,
    /// Win95 outstanding bond count dword `0x4cc4e8` (MISC `+0x18`).
    pub(crate) pe_dw_city_bonds: u32,
    /// Win95 `wCityMode` word `0x4ca42c` (MISC `+0x04`).
    pub(crate) pe_w_city_mode: u16,
    /// Win95 city rotation word `0x4c942c` (MISC `+0x08`; low two bits are compass).
    pub(crate) pe_w_city_rotation: u16,
    /// Win95 city progression / newspaper choice word `0x4cb010` (MISC `+0x20`).
    pub(crate) pe_w_city_progression: u16,
    /// Win95 military-base selector (`0x4cc4e4`, MISC `+0x0e4c`). This is a
    /// runtime cache because Mac CODE 4 owns the same serialized dword (slot 915).
    pub(crate) pe_w_military_base_type: u32,
    /// Win95 `wCityGarbage` `0x4ca5f0` (MISC `+0x40`); day-21 fold accumulates `slot0*10`.
    pub(crate) pe_w_city_garbage: u32,
    /// Win95 `dwCityFame` `0x4cad28` (MISC `+0x38`).
    pub(crate) pe_dw_city_fame: u32,
    /// Win95 `dwCityAdvertising` `0x4cb018` (MISC `+0x3c`); load/save only in vanilla.
    pub(crate) pe_dw_city_advertising: u32,
    /// Win95 `bWeatherTrend` `0x4cb40c` (MISC `+0x6c`); disaster/newspaper weather gates.
    pub(crate) pe_b_weather_trend: u8,
    /// Win95 `bWeatherHeat` `0x4cade0` (MISC `+0x60`); fire ambient ready gate.
    pub(crate) pe_b_weather_heat: u8,
    /// Win95 `bWeatherWind` `0x4cae0c` (MISC `+0x64`).
    pub(crate) pe_b_weather_wind: u8,
    /// Win95 `bWeatherHumid` `0x4cb3d4` (MISC `+0x68`); weather + water rainfall.
    pub(crate) pe_b_weather_humid: u8,
    /// Win95 `wSetTriggerDisasterType` `0x4ca420` (MISC `+0x70`).
    pub(crate) pe_w_trigger_disaster: u16,
    /// Last PE cheat jump-table arm-0 sound id (`0x40cb0a` pushes `0x19` into `0x473a40`).
    pub(crate) pe_w_last_cheat_sound_id: u16,
    /// Ordered PE effect sound ids queued by sim (`0x4b174f`); app drains after the tick.
    pe_pending_effect_sound_ids: Vec<u16>,
    /// Plant-expiry (and similar) map centers for Options Auto-Goto; app drains after the tick.
    pe_pending_auto_goto: Vec<(usize, usize)>,
    /// Win95 `dwResidentialPopulation` / prior-month `pZonePops[0]` @ `0x4ca194` (MISC `+0x74`).
    pub(crate) pe_dw_residential_population: u32,
    /// Win95 rewards available dword `0x4e9a24` (MISC `+0x78`; low five bits are grant flags).
    pub(crate) pe_dw_rewards_available: u32,
    /// Win95 `dwCityCrime` `0x4cb454` (XCRM 64×64 sum / MISC `+0x2c`) → attitude score 3.
    pub(crate) pe_dw_city_crime: u32,
    /// True after [`Self::set_pe_attitude_demand_inputs`]; false keeps funding stand-ins.
    pe_attitude_totals_ready: bool,
    /// Win95 attitude pointer scores `*[0x4cc480/484/488/48c]`; PE stores full dwords.
    pe_attitude_scores: [u32; 4],
    /// Win95 local fame-base word @ `0x4cb020` (graph/microsim; + year-founded for newspaper).
    pe_w_local_fame_base: u16,
    /// Win95 calendar month-mod word @ `0x4cb01c` (`(age/0x19)%0xc` from UpdateGraphData).
    pe_w_calendar_month_mod: u16,
    /// Win95 season word @ `0x4cb3e8` (`((month_mod+1)%0xc)/3` from UpdateGraphData).
    pe_w_season: u16,
    /// Win95 tech-unlock mask word @ `0x4e9e88` (built from zero fame ranks @ `0x46a0be`).
    pe_w_tech_unlock_mask: u16,
    /// Win95 high-zero fame-rank counter @ `0x4cdb2c` (indices `12..=15` in `0x46a0be`).
    pe_w_tech_unlock_high_zero_count: u16,
    /// Win95 tech-unlock UI dwords @ `0x4e9a10` (18 slots; index 5 is rewards `0x4e9a24`).
    pe_dw_tech_ui_4e9a10: [u32; 18],
    /// Win95 neighbor-fame rank WORD[17] @ `0x4cb430` (`FUN_00468150` newspaper @ `0x4689c8`).
    pe_w_neighbor_fame_ranks: [u16; PE_NEIGHBOR_FAME_RANK_COUNT],
    /// Set by story insert when PE would auto-open Extra (`0x47b681`: ids 3..=5 or `0x24`).
    pe_newspaper_extra_popup_pending: bool,
    pe_newspaper_storage: PeNewspaperStorage,
    /// Set by disaster-end UI arm (`0x45cf10` / OpenSC2K DisasterEnd): open paper 0.
    pe_disaster_end_newspaper_pending: bool,
    /// MISC offset 0x2c. sc2kfix loads this as the city crime scenario counter.
    misc_crime_count: Option<u32>,
    /// MISC offset 0x30. SC2K-docs identifies this as total Traffic Count, but not an XTRF sum.
    misc_traffic_count: Option<u32>,
    /// MISC offset 0x34. sc2kfix loads this as the city pollution scenario counter.
    misc_pollution_count: Option<u32>,
    /// Set when day-22 PE `--wScenarioTimeLimitMonths` hits 0 (scenario failure).
    pe_scenario_deadline_expired: bool,
    /// Win95 `0x4CAD44`: runtime scenario-active flag set by scenario open and
    /// cleared by terminal outcomes. This PE global is not a serialized field.
    pe_scenario_active: bool,
    /// MISC offset 0x68. SC2K-docs names this `humid`; water production uses it as rainfall.
    misc_humidity: Option<u32>,
    /// MISC offset 0x0e40. SC2K-docs names this the water table level.
    misc_water_table_level: Option<u32>,
    /// Date reconstructed from `MISC` year-founded and city-age fields.
    saved_date: Option<CityDate>,
    /// Saved map view metadata from `MISC` offsets 0x1014/0x1018/0x101c.
    saved_view: Option<CityView>,
    /// XZON high-bit to projected draw-corner mapping for rotations 0..3.
    corner_key_bits: [u8; 4],
    /// Effective save/view compass used for keyed building corners and rotated infrastructure
    /// sprite ids. The renderer keeps a separate camera-grid rotation for screen placement.
    rotation: u8,
    /// Orientation selector recorded in MISC+8. Mac simulation reads this as
    /// `ffffe0c6`; the effective display compass may differ while playing.
    pub(crate) saved_rotation: u8,
    dirty_tiles: HashSet<(usize, usize)>,
    modified: bool,
}

/// Flat grass plateau height for **File → New City** / Edit Map (room to raise and lower).
pub const BLANK_MAP_BASE_HEIGHT: u8 = 8;

fn city_index(x: usize, y: usize) -> usize {
    x * CITY_HEIGHT + y
}

#[cfg(test)]
#[path = "city/tests/pe_capacity.rs"]
mod pe_capacity_tests;

#[cfg(test)]
#[path = "city/tests/model.rs"]
mod tests;

#[cfg(test)]
#[path = "city/cgms_test_support.rs"]
mod cgms_test_support;

#[cfg(test)]
pub(crate) use cgms_test_support::{cgms_v4_from_city_for_test, city_from_cgms_v4_for_test};
