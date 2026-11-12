//! Win95 `XTHG` overlay actor table (`004ca434` in `SIMCITY.EXE`).
//!
//! Separate from `XTRF` congestion density: the saved chunk is 40 slots × 12 bytes
//! (`0x1e0`), used by `FUN_004540c0` / `FUN_00455020` for moving traffic decoration.
//! SC2KFix wiki labels `0x4CA434` as a Win95 `dwMapXTHG` Things pointer; keep that
//! as Win95 comparison evidence, not default Mac traffic behavior.
//! CityGame loads and round-trips this chunk; optional simulation lives in
//! `win95_overlay_sim.rs`.
//!
//! See `outputs/disasm_win95_pe/sc2000_win95_chunk_save_notes.md` and
//! `outputs/disasm_win95_pe_overlay_probe/`.

pub const XTHG_RECORD_SIZE: usize = 12;
pub const XTHG_RECORD_COUNT: usize = 40;
pub const XTHG_CHUNK_SIZE: usize = XTHG_RECORD_SIZE * XTHG_RECORD_COUNT;
pub const XTHG_FIRST_USABLE_SLOT: usize = 1;
pub const XTHG_PROXY_BASE: u8 = 201;

/// Win95 `XTHG` type byte (`+0` / `map_XTHG_t.iType`).
pub const XTHG_AIRPLANE: u8 = 0x01;
pub const XTHG_HELICOPTER: u8 = 0x02;
/// Win95 monster start `0x45ee10` XTHG `+0`.
pub const XTHG_MONSTER: u8 = 5;
/// PE `XTHG_EXPLOSION`; Things JT targets `0x4546f0` (age `+1` then clear).
/// Kind `0x07`/`0x08` RET stubs are separate (`0x4554f0` / `0x4554e0`).
pub const XTHG_EXPLOSION: u8 = 0x06;
/// PE `FUN_00453280` sub-tile center written to XTHG `+6`/`+7`.
pub const XTHG_EXPLOSION_SUBTILE: u8 = 8;
/// PE `FUN_00453280` rejects tiles whose `0x4CA600` (XTXT) occupancy is `>= 0xC9`.
pub const XTHG_EXPLOSION_XTXT_GATE: u8 = XTHG_PROXY_BASE;
/// Win95 monster start `0x45ee10` XTHG `+5`.
pub const XTHG_MONSTER_HEIGHT: u8 = 0x0f;
/// Win95 monster start `0x45ee10` XTHG `+6`/`+7`.
pub const XTHG_MONSTER_SUBTILE: u8 = 8;
/// Win95 tornado start `0x45f090` XTHG `+0`.
pub const XTHG_TORNADO: u8 = 0x0f;
/// Win95 tornado start `0x45f090` XTHG `+6`/`+7`.
pub const XTHG_TORNADO_SUBTILE: u8 = 8;
/// Win95 air-crash start `0x45e7d0` XTHG `+2` (`state=7`, not Light/Heavy/SpawnPending).
pub const XTHG_AIRCRASH_STATE: u8 = 7;
/// Win95 air-crash start `0x45e7d0` XTHG `+5`.
pub const XTHG_AIRCRASH_HEIGHT: u8 = 0x10;
/// Win95 air-crash start `0x45e7d0` XTHG `+6`/`+7`.
pub const XTHG_AIRCRASH_SUBTILE: u8 = 8;
pub const XTHG_CARGO_SHIP: u8 = 0x03;
/// PE `XTHG_DEPLOY_FIRE`; raw kind `0x08` is not a sailboat.
pub const XTHG_DEPLOY_FIRE: u8 = 0x08;
pub const XTHG_SAILBOAT: u8 = 0x09;
pub const XTHG_TRAIN_ENGINE: u8 = 0x0a;
pub const XTHG_TRAIN_CAR: u8 = 0x0b;
pub const XTHG_SUBWAY_TRAIN_ENGINE: u8 = 0x0c;
pub const XTHG_SUBWAY_TRAIN_CAR: u8 = 0x0d;
/// sc2kfix `XTHG_MAXIS_MAN`; PE dispatcher entry 15 calls `FUN_00455020`.
pub const XTHG_MAXIS_MAN: u8 = 0x10;
/// PE kind `0x06` is also the Things explosion handler (`0x4546f0`); road
/// decoration reuses this numeric kind only on the synthetic `0xd0` path.
pub const OVERLAY_KIND_TRAFFIC: u8 = 0x06;
/// Runtime-only kind for port-generated animated road traffic. It is outside the PE kind table.
pub const RUST_TRAFFIC_ACTOR_KIND: u8 = 0xd0;
pub const OVERLAY_KIND_DISPATCH_POLICE: u8 = 0x07;
pub const OVERLAY_KIND_DISPATCH_FIRE: u8 = XTHG_DEPLOY_FIRE;
pub const OVERLAY_KIND_DISPATCH_MILITARY: u8 = 0x0e;
pub const RUST_DISPATCH_ACTOR_SENTINEL: u8 = 0xd1;

/// Win95 `XTHG` direction byte (`+1` / `map_XTHG_t.iDirection`).
pub const XTHG_DIRECTION_NORTH: u8 = 0;
pub const XTHG_DIRECTION_EAST: u8 = 2;

/// Compass delta for Win95 `XTHG` directions (`lane` / `iDirection`).
pub fn xthg_direction_delta(direction: u8) -> (i16, i16) {
    match direction % 8 {
        0 => (0, -1),  // N
        1 => (1, -1),  // NE
        2 => (1, 0),   // E
        3 => (1, 1),   // SE
        4 => (0, 1),   // S
        5 => (-1, 1),  // SW
        6 => (-1, 0),  // W
        _ => (-1, -1), // NW
    }
}

/// OpenSC2K / large-bank base sprites for ambient actors (facing 0).
pub const XTHG_AIRPLANE_SPRITE_BASE: u32 = 1359;
pub const XTHG_HELICOPTER_SPRITE_BASE: u32 = 1364;
pub const XTHG_CARGO_SHIP_SPRITE_BASE: u32 = 1369;
pub const XTHG_TRAIN_SPRITE_BASE: u32 = 1374;
pub const XTHG_SAILBOAT_SPRITE_BASE: u32 = 1380;

// PE DrawTrain tables. The first maps normalized surface rail tile IDs 44..62
// and 69..72 to draw variants. Variant 0x32 means look up the train iDX code
// in the second table before choosing a sprite and horizontal flip.
const PE_TRAIN_TILE_VARIANTS: [u8; 23] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0x32, 0x32, 0x32, 0x32, 0x32, 10, 11, 12, 13, 1, 0, 0, 1,
];
const PE_TRAIN_IDX_VARIANTS: [u8; 8] = [0, 17, 1, 16, 0, 17, 1, 16];
const PE_TRAIN_SPRITE_OFFSETS: [u8; 18] = [0, 0, 3, 3, 4, 4, 2, 1, 2, 1, 3, 3, 4, 4, 2, 1, 2, 1];
const PE_TRAIN_SPRITE_FLIPS: [bool; 18] = [
    false, true, true, false, false, true, false, false, false, true, false, false, false, true,
    false, true, false, false,
];
// PE DrawTrain adds these to the `16 * (x - y)` and `8 * (x + y)` screen axes.
const PE_TRAIN_SCREEN_X_OFFSETS: [i16; 18] =
    [0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0];
const PE_TRAIN_SCREEN_Y_OFFSETS: [i16; 18] = [0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 4, 4, 3, 3, 0, 0, 0, 3];

/// Win95 `FUN_00450690` height table at `0x004E8100`, indexed by XTHG kind.
/// Ambient spawn routines use these fixed `+5` values; they are not the map altitude.
pub fn xthg_spawn_height(kind: u8) -> u8 {
    match kind {
        XTHG_AIRPLANE | XTHG_CARGO_SHIP => 6,
        XTHG_HELICOPTER => 4,
        XTHG_MONSTER => 8,
        XTHG_SAILBOAT => 1,
        _ => 0,
    }
}

const XTHG_ZOOM_X_DIVISOR: [f32; 4] = [4.0, 2.0, 1.0, 1.0];
const XTHG_ZOOM_Y_DIVISOR: [f32; 4] = [8.0, 4.0, 2.0, 2.0];

pub(crate) fn xthg_zoom_projection_for_scale(zoom: f32) -> (usize, f32, f32) {
    let level = zoom.max(1.0).log2().round().clamp(0.0, 3.0) as usize;
    (
        level,
        XTHG_ZOOM_X_DIVISOR[level],
        XTHG_ZOOM_Y_DIVISOR[level],
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchKind {
    Police,
    Firefighters,
    Military,
}

impl DispatchKind {
    pub const fn index(self) -> usize {
        match self {
            Self::Police => 0,
            Self::Firefighters => 1,
            Self::Military => 2,
        }
    }

    pub const fn capacity_limit(self) -> usize {
        match self {
            // Police/fire use one-based 32-byte native coordinate arrays. PE does not clamp the
            // derived count before indexing them, so keep the Rust ring inside the safe 1..=31
            // range rather than reproducing an out-of-bounds write for oversized cities.
            Self::Police | Self::Firefighters => 31,
            Self::Military => 15,
        }
    }

    pub const fn overlay_kind(self) -> u8 {
        match self {
            Self::Police => OVERLAY_KIND_DISPATCH_POLICE,
            Self::Firefighters => OVERLAY_KIND_DISPATCH_FIRE,
            Self::Military => OVERLAY_KIND_DISPATCH_MILITARY,
        }
    }

    /// Large (1000+) zoom-bank sprite used by the continuous-scale renderer.
    pub const fn large_sprite_id(self) -> u32 {
        match self {
            Self::Police => 1382,
            Self::Firefighters => 1383,
            Self::Military => 1497,
        }
    }

    pub const fn from_overlay_kind(kind: u8) -> Option<Self> {
        match kind {
            OVERLAY_KIND_DISPATCH_POLICE => Some(Self::Police),
            OVERLAY_KIND_DISPATCH_FIRE => Some(Self::Firefighters),
            OVERLAY_KIND_DISPATCH_MILITARY => Some(Self::Military),
            _ => None,
        }
    }
}

pub fn xthg_proxy_for_slot(slot: usize) -> Option<u8> {
    (XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT)
        .contains(&slot)
        .then(|| XTHG_PROXY_BASE + slot as u8)
}

pub fn xthg_slot_for_proxy(proxy: u8) -> Option<usize> {
    let slot = proxy.checked_sub(XTHG_PROXY_BASE)? as usize;
    (XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT)
        .contains(&slot)
        .then_some(slot)
}

/// State values consumed by PE `FUN_00455020` for `XTHG_MAXIS_MAN` records.
///
/// Keep this separate from `OverlayState`: the port uses a temporary
/// `SpawnPending` value between ticks, while PE state 3 belongs to a different
/// handler and is not one of these PE states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PeMaxisManState {
    Inactive = 0,
    LightActive = 1,
    HeavyActive = 2,
}

/// Port overlay state byte (`+2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum OverlayState {
    #[default]
    Inactive = 0,
    LightActive = 1,
    HeavyActive = 2,
    SpawnPending = 3,
}

/// One 12-byte Win95 overlay record (`004ca434` layout).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Win95OverlayRecord {
    pub kind: u8,
    pub lane: u8,
    pub state: u8,
    pub tile_x: u8,
    pub tile_y: u8,
    pub height: u8,
    pub sub_x: u8,
    pub sub_y: u8,
    pub target_x: u8,
    pub target_y: u8,
    pub saved_a: u8,
    pub saved_b: u8,
}

/// Map and event values consumed by the PE Maxis Man constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeMaxisManSpawnInputs {
    pub tile_x: u8,
    pub tile_y: u8,
    pub direction: u8,
    pub altm: u8,
    pub target_x: u8,
    pub target_y: u8,
    /// Prior XTXT at the spawn tile (`0x4CA600`), saved to XTHG `+A` before proxy paint.
    pub saved_xtxt: u8,
    pub saved_payload: u8,
}

/// Select the byte written to XTHG `+B` by the PE Maxis Man constructor.
///
/// `random_low_bit` is the low bit of the constructor's `Random()` result.
/// For IDs `5|8..=12`, `global_payload` is the low byte of `wMonsterXTHGIndex`
/// (`0x4CA81C`): the active monster slot, or `0xFF` when the word is cleared to
/// `0xFFFF`. The `None` cases correspond to switch values for which the PE
/// constructor performs no explicit payload write.
pub fn pe_maxis_man_saved_payload(
    disaster_id: u8,
    random_low_bit: u8,
    global_payload: u8,
) -> Option<u8> {
    match disaster_id {
        1 | 6 | 7 => Some(0xff),
        2 | 14 => Some(0xfc),
        3 | 13 => Some(0xfd + (random_low_bit & 1)),
        4 | 15 => Some(0xfb),
        5 | 8..=12 => Some(global_payload),
        _ => None,
    }
}

impl PeMaxisManSpawnInputs {
    /// PE stores `(ALTM & 0x1F) + 2` in XTHG `+5`.
    pub const fn pe_height(self) -> u8 {
        (self.altm & 0x1f).wrapping_add(2)
    }

    pub const fn into_record(self) -> Win95OverlayRecord {
        Win95OverlayRecord::pe_maxis_man(
            self.tile_x,
            self.tile_y,
            self.direction,
            self.pe_height(),
            self.target_x,
            self.target_y,
            self.saved_xtxt,
            self.saved_payload,
        )
    }
}

impl Win95OverlayRecord {
    /// Construct the PE Maxis Man record written by FUN_00452FA0.
    pub const fn pe_maxis_man(
        tile_x: u8,
        tile_y: u8,
        direction: u8,
        height: u8,
        target_x: u8,
        target_y: u8,
        saved_a: u8,
        saved_b: u8,
    ) -> Self {
        Self {
            kind: XTHG_MAXIS_MAN,
            lane: direction,
            state: 0,
            tile_x,
            tile_y,
            height,
            sub_x: 8,
            sub_y: 8,
            target_x,
            target_y,
            saved_a,
            saved_b,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            kind: bytes.first().copied().unwrap_or(0),
            lane: bytes.get(1).copied().unwrap_or(0),
            state: bytes.get(2).copied().unwrap_or(0),
            tile_x: bytes.get(3).copied().unwrap_or(0),
            tile_y: bytes.get(4).copied().unwrap_or(0),
            height: bytes.get(5).copied().unwrap_or(0),
            sub_x: bytes.get(6).copied().unwrap_or(0),
            sub_y: bytes.get(7).copied().unwrap_or(0),
            target_x: bytes.get(8).copied().unwrap_or(0),
            target_y: bytes.get(9).copied().unwrap_or(0),
            saved_a: bytes.get(10).copied().unwrap_or(0),
            saved_b: bytes.get(11).copied().unwrap_or(0),
        }
    }

    pub fn to_bytes(self) -> [u8; XTHG_RECORD_SIZE] {
        [
            self.kind,
            self.lane,
            self.state,
            self.tile_x,
            self.tile_y,
            self.height,
            self.sub_x,
            self.sub_y,
            self.target_x,
            self.target_y,
            self.saved_a,
            self.saved_b,
        ]
    }

    pub fn overlay_state(self) -> OverlayState {
        match self.state {
            1 => OverlayState::LightActive,
            2 => OverlayState::HeavyActive,
            3 => OverlayState::SpawnPending,
            _ => OverlayState::Inactive,
        }
    }

    pub fn is_traffic_slot(self) -> bool {
        matches!(self.kind, OVERLAY_KIND_TRAFFIC | RUST_TRAFFIC_ACTOR_KIND)
    }

    pub fn dispatch_kind(self) -> Option<DispatchKind> {
        DispatchKind::from_overlay_kind(self.kind)
    }

    pub fn is_dispatch_actor(self) -> bool {
        self.dispatch_kind().is_some()
    }

    pub fn is_rust_dispatch_actor(self) -> bool {
        self.is_dispatch_actor() && self.saved_b == RUST_DISPATCH_ACTOR_SENTINEL
    }

    pub fn is_ambient_port_actor(self) -> bool {
        matches!(
            self.kind,
            XTHG_AIRPLANE
                | XTHG_HELICOPTER
                | XTHG_CARGO_SHIP
                | XTHG_SAILBOAT
                | XTHG_TRAIN_ENGINE
                | XTHG_TRAIN_CAR
                | XTHG_SUBWAY_TRAIN_ENGINE
                | XTHG_SUBWAY_TRAIN_CAR
        )
    }

    /// Win95 air-crash airplane from consume `0x45e7d0` (`state=7`, height `0x10`).
    pub fn is_pe_aircrash_actor(self) -> bool {
        self.kind == XTHG_AIRPLANE && self.state == XTHG_AIRCRASH_STATE
    }

    /// Win95 monster from consume `0x45ee10` (`iType=5`).
    pub fn is_pe_monster_actor(self) -> bool {
        self.kind == XTHG_MONSTER
    }

    /// Win95 tornado from consume `0x45f090` (`iType=0x0f`).
    pub fn is_pe_tornado_actor(self) -> bool {
        self.kind == XTHG_TORNADO
    }

    /// Win95 explosion / transition record from `FUN_00453280` (`iType=0x06`).
    pub fn is_pe_explosion_actor(self) -> bool {
        self.kind == XTHG_EXPLOSION
    }

    /// Disaster XTHG driven by PE Things walker (`0x4509ba`), not ambient port motion.
    pub fn is_pe_disaster_xthg_actor(self) -> bool {
        self.is_pe_aircrash_actor()
            || self.is_pe_monster_actor()
            || self.is_pe_tornado_actor()
            || self.is_pe_explosion_actor()
    }

    pub fn is_ambient_train_engine(self) -> bool {
        matches!(self.kind, XTHG_TRAIN_ENGINE | XTHG_SUBWAY_TRAIN_ENGINE)
    }

    pub fn is_ambient_train_car(self) -> bool {
        matches!(self.kind, XTHG_TRAIN_CAR | XTHG_SUBWAY_TRAIN_CAR)
    }

    pub fn is_ambient_train_family(self) -> bool {
        self.is_ambient_train_engine() || self.is_ambient_train_car()
    }

    /// Active traffic decoration in the Win95 overlay update loop (`FUN_00455020`).
    pub fn is_drawable_traffic(self) -> bool {
        self.is_traffic_slot() && matches!(self.state, 1..=2)
    }

    pub fn ambient_port_uses_water_elevation(self) -> bool {
        matches!(self.kind, XTHG_CARGO_SHIP | XTHG_SAILBOAT)
    }

    /// PE DrawTrain's 90/91 rail-bridge branch derives its elevation from
    /// ALTM bits 5..9 (water) rather than the ordinary land-height field.
    pub fn ambient_train_uses_water_elevation(self, building: u8) -> bool {
        matches!(self.kind, XTHG_TRAIN_ENGINE | XTHG_TRAIN_CAR) && matches!(building, 90 | 91)
    }

    /// Large-bank sprite for airplane / helicopter / cargo ship / train / sailboat facings.
    /// Subway train kinds are intentionally undrawn on the surface map (Win95 `DrawThings`).
    pub fn ambient_port_sprite_id(self) -> Option<u32> {
        if self.kind == XTHG_SAILBOAT {
            // DrawThings uses a two-entry sailboat art family. The next IDs
            // belong to dispatch markers, not additional boat directions.
            return Some(XTHG_SAILBOAT_SPRITE_BASE + u32::from([0u8, 1, 1, 0][(self.lane & 3) as usize]));
        }
        let base = match self.kind {
            XTHG_AIRPLANE => XTHG_AIRPLANE_SPRITE_BASE,
            XTHG_HELICOPTER => XTHG_HELICOPTER_SPRITE_BASE,
            XTHG_CARGO_SHIP => XTHG_CARGO_SHIP_SPRITE_BASE,
            XTHG_TRAIN_ENGINE | XTHG_TRAIN_CAR => XTHG_TRAIN_SPRITE_BASE,
            _ => return None,
        };
        // OpenSC2K packs five facings; fold the eight compass dirs into 0..4.
        // PE DrawTrain uses iDX (+8), which stores a curve-aware draw code;
        // all other ambient actors use iDirection (+1).
        let direction = if self.is_ambient_train_family() {
            self.target_x
        } else {
            self.lane
        };
        let facing = match direction % 8 {
            0 => 0,
            1 | 2 => 1,
            3 | 4 => 2,
            5 | 6 => 3,
            _ => 4,
        };
        Some(base + facing)
    }

    /// Surface actor sprite selected by PE `DrawTrain` where its rail-tile
    /// tables are recovered. The portal/bridge branches remain on the prior
    /// generic fallback until their separate PE draw paths are recovered.
    pub fn ambient_port_sprite(self, building: u8, xbit: Option<u8>) -> Option<(u32, bool)> {
        if self.kind == XTHG_SAILBOAT {
            return self.ambient_port_sprite_id().map(|id| (id, (self.lane & 3) >= 2));
        }
        if matches!(self.kind, XTHG_TRAIN_ENGINE | XTHG_TRAIN_CAR) {
            if let Some((sprite_id, flip_x)) =
                pe_train_sprite_for_tile(building, self.target_x, xbit)
            {
                return Some((sprite_id, flip_x));
            }
            // PE DrawTrain 0x451264 normalizes only rail IDs 44..62 and
            // 69..72; 77/78 are valid movement tiles but return before blit.
            if matches!(building, 77 | 78) {
                return None;
            }
        }
        self.ambient_port_sprite_id()
            .map(|sprite_id| (sprite_id, false))
    }

    /// PE DrawTrain's per-variant offset from the tile anchor, in unscaled
    /// screen pixels. Subway train kinds do not reach the surface draw path.
    pub fn ambient_train_draw_offset(self, building: u8, xbit: Option<u8>) -> Option<(i16, i16)> {
        if !matches!(self.kind, XTHG_TRAIN_ENGINE | XTHG_TRAIN_CAR) {
            return None;
        }
        let variant = pe_train_draw_variant(building, self.target_x, xbit)?;
        Some((
            PE_TRAIN_SCREEN_X_OFFSETS[variant],
            PE_TRAIN_SCREEN_Y_OFFSETS[variant],
        ))
    }
}

fn pe_train_sprite_for_tile(building: u8, draw_code: u8, xbit: Option<u8>) -> Option<(u32, bool)> {
    let variant = pe_train_draw_variant(building, draw_code, xbit)?;
    Some((
        XTHG_TRAIN_SPRITE_BASE + u32::from(PE_TRAIN_SPRITE_OFFSETS[variant]),
        PE_TRAIN_SPRITE_FLIPS[variant],
    ))
}

fn pe_train_draw_variant(building: u8, draw_code: u8, xbit: Option<u8>) -> Option<usize> {
    if matches!(building, 90 | 91) {
        // PE DrawTrain 0x4512DD: rail-bridge tiles use XBIT bit 1 to choose
        // variant 0 or 1, bypassing the ordinary iDX route-table path.
        return Some(usize::from(xbit.is_some_and(|bits| bits & 0x02 != 0)));
    }
    let tile_index = match building {
        44..=62 => usize::from(building - 44),
        69..=72 => usize::from(building - 50),
        _ => return None,
    };
    let mut variant = *PE_TRAIN_TILE_VARIANTS.get(tile_index)?;
    if variant == 0x32 {
        variant = PE_TRAIN_IDX_VARIANTS[(draw_code % 8) as usize];
    }
    Some(usize::from(variant))
}

pub fn empty_overlay_table() -> Vec<Win95OverlayRecord> {
    vec![Win95OverlayRecord::default(); XTHG_RECORD_COUNT]
}

pub fn decode_xthg_records(raw: &[u8]) -> anyhow::Result<Vec<Win95OverlayRecord>> {
    if raw.len() != XTHG_CHUNK_SIZE {
        anyhow::bail!(
            "XTHG decoded to {} bytes, expected {}",
            raw.len(),
            XTHG_CHUNK_SIZE
        );
    }
    Ok(raw
        .chunks_exact(XTHG_RECORD_SIZE)
        .map(Win95OverlayRecord::from_bytes)
        .collect())
}

pub fn encode_xthg_records(records: &[Win95OverlayRecord]) -> anyhow::Result<Vec<u8>> {
    if records.len() != XTHG_RECORD_COUNT {
        anyhow::bail!(
            "XTHG encode needs {} records, got {}",
            XTHG_RECORD_COUNT,
            records.len()
        );
    }
    let mut raw = Vec::with_capacity(XTHG_CHUNK_SIZE);
    for record in records {
        raw.extend_from_slice(&record.to_bytes());
    }
    Ok(raw)
}

/// Return the PE Maxis Man spawn slot, if the one-actor gate permits it.
///
/// PE scans XTHG slots 1 through 39 and uses the first empty slot. Slot zero
/// is reserved and an existing Maxis Man blocks the spawn entirely.
pub fn pe_overlay_spawn_slot(records: &[Win95OverlayRecord]) -> Option<usize> {
    records
        .get(XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT)?
        .iter()
        .position(|record| record.kind == 0)
        .map(|offset| offset + XTHG_FIRST_USABLE_SLOT)
}

pub fn pe_maxis_man_spawn_slot(records: &[Win95OverlayRecord]) -> Option<usize> {
    let slots = records.get(XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT)?;
    if slots.iter().any(|record| record.kind == XTHG_MAXIS_MAN) {
        return None;
    }
    slots
        .iter()
        .position(|record| record.kind == 0)
        .map(|offset| offset + XTHG_FIRST_USABLE_SLOT)
}

/// Spawn one PE Maxis Man directly into an XTHG table.
pub fn spawn_pe_maxis_man(
    records: &mut [Win95OverlayRecord],
    inputs: PeMaxisManSpawnInputs,
) -> Option<usize> {
    let slot = pe_maxis_man_spawn_slot(records)?;
    records[slot] = inputs.into_record();
    Some(slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xthg_record_round_trip_bytes() {
        let record = Win95OverlayRecord {
            kind: OVERLAY_KIND_TRAFFIC,
            lane: 3,
            state: OverlayState::LightActive as u8,
            tile_x: 10,
            tile_y: 20,
            height: 5,
            sub_x: 8,
            sub_y: 8,
            target_x: 11,
            target_y: 21,
            saved_a: 0x51,
            saved_b: 0x33,
        };
        assert_eq!(Win95OverlayRecord::from_bytes(&record.to_bytes()), record);
    }

    #[test]
    fn optional_traffic_kind_keeps_overlay_slot() {
        assert_eq!(XTHG_EXPLOSION, 0x06);
        assert_eq!(OVERLAY_KIND_TRAFFIC, 0x06);
        assert_eq!(XTHG_MAXIS_MAN, 0x10);
        assert_eq!(XTHG_EXPLOSION_SUBTILE, 8);
        assert_eq!(XTHG_EXPLOSION_XTXT_GATE, XTHG_PROXY_BASE);
        assert_eq!(XTHG_EXPLOSION_XTXT_GATE, 0xc9);
    }

    #[test]
    fn pe_overlay_spawn_slot_matches_fixed_xthg_range() {
        let mut records = empty_overlay_table();
        records[0].kind = 0xff;
        assert_eq!(
            pe_overlay_spawn_slot(&records),
            Some(XTHG_FIRST_USABLE_SLOT)
        );

        records[1].kind = 0x01;
        assert_eq!(pe_overlay_spawn_slot(&records), Some(2));

        for record in &mut records[XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT] {
            record.kind = 0x01;
        }
        assert_eq!(pe_overlay_spawn_slot(&records), None);

        records.push(Win95OverlayRecord::default());
        assert_eq!(pe_overlay_spawn_slot(&records), None);
    }

    #[test]
    fn xthg_table_encode_decode() {
        let mut table = empty_overlay_table();
        table[0].kind = OVERLAY_KIND_TRAFFIC;
        table[0].state = OverlayState::SpawnPending as u8;
        let raw = encode_xthg_records(&table).expect("encode");
        assert_eq!(raw.len(), XTHG_CHUNK_SIZE);
        let back = decode_xthg_records(&raw).expect("decode");
        assert_eq!(back, table);
    }

    #[test]
    fn sailboat_directions_never_select_dispatch_sprites() {
        let mut record = Win95OverlayRecord { kind: XTHG_SAILBOAT, ..Default::default() };
        for (lane, expected) in [(0, (1380, false)), (1, (1381, false)), (2, (1381, true)), (3, (1380, true))] {
            record.lane = lane;
            assert_eq!(record.ambient_port_sprite(0, None), Some(expected));
        }
        for lane in 0..=u8::MAX {
            record.lane = lane;
            assert!(matches!(record.ambient_port_sprite_id(), Some(1380 | 1381)));
        }
    }

    #[test]
    fn dispatch_kinds_use_recovered_large_sprite_bank() {
        assert_eq!(DispatchKind::Police.overlay_kind(), 7);
        assert_eq!(DispatchKind::Police.large_sprite_id(), 1382);
        assert_eq!(DispatchKind::Firefighters.large_sprite_id(), 1383);
        assert_eq!(DispatchKind::Military.large_sprite_id(), 1497);
        let record = Win95OverlayRecord {
            kind: DispatchKind::Police.overlay_kind(),
            saved_b: RUST_DISPATCH_ACTOR_SENTINEL,
            ..Default::default()
        };
        assert!(record.is_rust_dispatch_actor());
    }

    #[test]
    fn dispatch_proxy_maps_slots_one_through_thirty_nine() {
        assert_eq!(xthg_proxy_for_slot(0), None);
        assert_eq!(xthg_proxy_for_slot(1), Some(202));
        assert_eq!(
            xthg_proxy_for_slot(1),
            Some(1u8.wrapping_sub(0x37)),
            "PE air-crash XTXT is slot-0x37"
        );
        assert_eq!(xthg_proxy_for_slot(39), Some(240));
        assert_eq!(xthg_slot_for_proxy(202), Some(1));
        assert_eq!(xthg_slot_for_proxy(240), Some(39));
        assert_eq!(xthg_slot_for_proxy(201), None);
    }
}
