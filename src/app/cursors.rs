//! Tool cursor mapping, extracted cursor resources, and system-cursor fallback.
//!
//! Resource IDs, hotspots, active-cursor selection, and SDL cursor ownership retain
//! their existing behavior; the runtime still owns and replaces the cursor set.

use crate::ui::EditTool;
use anyhow::{Context, Result};
use sdl3::mouse::{Cursor, SystemCursor};
use sdl3::pixels::PixelFormat;
use sdl3::surface::Surface;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum GameCursor {
    Arrow,
    Query,
    Bulldoze,
    Power,
    Road,
    Highway,
    Rail,
    ZoneResidential,
    ZoneCommercial,
    ZoneIndustrial,
    Education,
    WaterUtility,
    Services,
    Rewards,
    Ports,
    Parks,
    Signs,
    Dispatch,
    RaiseTerrain,
    LowerTerrain,
    LevelTerrain,
    Water,
    Trees,
    Pan,
}

impl GameCursor {
    const ALL: [Self; 24] = [
        Self::Arrow,
        Self::Query,
        Self::Bulldoze,
        Self::Power,
        Self::Road,
        Self::Highway,
        Self::Rail,
        Self::ZoneResidential,
        Self::ZoneCommercial,
        Self::ZoneIndustrial,
        Self::Education,
        Self::WaterUtility,
        Self::Services,
        Self::Rewards,
        Self::Ports,
        Self::Parks,
        Self::Signs,
        Self::Dispatch,
        Self::RaiseTerrain,
        Self::LowerTerrain,
        Self::LevelTerrain,
        Self::Water,
        Self::Trees,
        Self::Pan,
    ];

    pub(super) fn for_tool(tool: EditTool) -> Self {
        match tool {
            EditTool::Query => Self::Query,
            EditTool::Bulldoze => Self::Bulldoze,
            EditTool::RaiseTerrain => Self::RaiseTerrain,
            EditTool::LowerTerrain => Self::LowerTerrain,
            EditTool::StretchTerrain => Self::RaiseTerrain,
            EditTool::LevelTerrain => Self::LevelTerrain,
            EditTool::SpreadTerrain => Self::LevelTerrain,
            EditTool::Water | EditTool::Stream => Self::Water,
            EditTool::Trees | EditTool::Forest => Self::Trees,
            EditTool::Road | EditTool::Tunnel | EditTool::Onramp | EditTool::BusDepot => Self::Road,
            EditTool::Highway => Self::Highway,
            EditTool::Rail
            | EditTool::Subway
            | EditTool::RailDepot
            | EditTool::SubStation
            | EditTool::SubToRail => Self::Rail,
            EditTool::Power => Self::Power,
            EditTool::ZoneResidential | EditTool::ZoneResidentialDense => Self::ZoneResidential,
            EditTool::ZoneCommercial | EditTool::ZoneCommercialDense => Self::ZoneCommercial,
            EditTool::ZoneIndustrial | EditTool::ZoneIndustrialDense => Self::ZoneIndustrial,
            EditTool::DeZone => Self::Bulldoze,
            EditTool::Pipes
            | EditTool::WaterPump
            | EditTool::WaterTower
            | EditTool::WaterTreatment
            | EditTool::Desalinization => Self::WaterUtility,
            EditTool::Sign => Self::Signs,
            EditTool::Seaport | EditTool::Airport => Self::Ports,
            EditTool::DispatchPolice
            | EditTool::DispatchFirefighters
            | EditTool::DispatchMilitary => Self::Dispatch,
            EditTool::MayorHouse
            | EditTool::CityHall
            | EditTool::Statue
            | EditTool::BraunLlamaDome
            | EditTool::Arcologies
            | EditTool::ArcologyPlymouth
            | EditTool::ArcologyForest
            | EditTool::ArcologyDarco
            | EditTool::ArcologyLaunch => Self::Rewards,
            EditTool::SmallPark
            | EditTool::BigPark
            | EditTool::Zoo
            | EditTool::Stadium
            | EditTool::Marina => Self::Parks,
            EditTool::Police | EditTool::FireStation | EditTool::Hospital | EditTool::Prison => {
                Self::Services
            }
            EditTool::School | EditTool::College | EditTool::Library | EditTool::Museum => {
                Self::Education
            }
            EditTool::PowerPlant
            | EditTool::PowerPlantCoal
            | EditTool::PowerPlantHydro
            | EditTool::PowerPlantOil
            | EditTool::PowerPlantGas
            | EditTool::PowerPlantNuclear
            | EditTool::PowerPlantWind
            | EditTool::PowerPlantSolar
            | EditTool::PowerPlantMicrowave
            | EditTool::PowerPlantFusion => Self::Power,
        }
    }

    pub(super) fn resource_id(self) -> u16 {
        match self {
            Self::Arrow => 1000,
            Self::Query => 1023,
            Self::Bulldoze => 1002,
            Self::Power => 1008,
            Self::Road => 1017,
            Self::Highway => 1021,
            Self::Rail => 1022,
            Self::ZoneResidential => 1004,
            Self::ZoneCommercial => 1005,
            Self::ZoneIndustrial => 1006,
            Self::Education => 1007,
            Self::WaterUtility => 1019,
            Self::Services => 1020,
            Self::Rewards => 1024,
            Self::Ports => 1025,
            Self::Parks => 1026,
            Self::Signs => 1027,
            Self::Dispatch => 1028,
            Self::RaiseTerrain => 1012,
            Self::LowerTerrain => 1013,
            Self::LevelTerrain => 1015,
            Self::Water => 1016,
            Self::Trees => 1003,
            Self::Pan => 1010,
        }
    }

    fn fallback_system(self) -> SystemCursor {
        match self {
            Self::Arrow => SystemCursor::Arrow,
            Self::Query => SystemCursor::Hand,
            Self::Pan => SystemCursor::SizeAll,
            _ => SystemCursor::Crosshair,
        }
    }
}

#[derive(Default)]
pub(super) struct GameCursors {
    cursors: Vec<(GameCursor, Cursor)>,
    active: Option<GameCursor>,
}

impl GameCursors {
    /// Cursors are optional decoration, never a prerequisite for playing.
    pub(super) fn for_assets(asset_root: &Path) -> Self {
        // Touch backends can reject both color and system cursor creation.
        // Let iPadOS/Android own any attached pointer's appearance.
        if cfg!(any(target_os = "ios", target_os = "android")) {
            return Self::default();
        }
        Self::load(asset_root).unwrap_or_else(|error| {
            log::warn!("could not load imported cursors: {error:#}; trying system cursors");
            Self::fallback()
        })
    }

    pub(super) fn load(asset_root: &Path) -> Result<Self> {
        let hotspots = load_cursor_hotspots(asset_root)?;
        let mut cursors = Vec::with_capacity(GameCursor::ALL.len());
        for kind in GameCursor::ALL {
            let resource_id = kind.resource_id();
            let hotspot = cursor_hotspot(&hotspots, resource_id);
            let cursor = load_resource_cursor(asset_root, resource_id, hotspot)
                .with_context(|| format!("load CURS_{resource_id:05}"))?;
            cursors.push((kind, cursor));
        }
        Ok(Self {
            cursors,
            active: None,
        })
    }

    fn fallback() -> Self {
        Self::fallback_with(|kind| Cursor::from_system(kind.fallback_system()))
    }

    fn fallback_with<E: std::fmt::Display>(
        mut create: impl FnMut(GameCursor) -> std::result::Result<Cursor, E>,
    ) -> Self {
        let mut cursors = Vec::with_capacity(GameCursor::ALL.len());
        for kind in GameCursor::ALL {
            match create(kind) {
                Ok(cursor) => cursors.push((kind, cursor)),
                Err(error) => log::debug!("system cursor {kind:?} unavailable: {error}"),
            }
        }
        if cursors.is_empty() {
            log::info!("native mouse cursors unavailable; continuing without custom cursors");
        }
        Self {
            cursors,
            active: None,
        }
    }

    pub(super) fn set(&mut self, kind: GameCursor) {
        if self.active == Some(kind) {
            return;
        }
        if let Some((_, cursor)) = self
            .cursors
            .iter()
            .find(|(cursor_kind, _)| *cursor_kind == kind)
            .or_else(|| {
                self.cursors
                    .iter()
                    .find(|(cursor_kind, _)| *cursor_kind == GameCursor::Arrow)
            })
        {
            cursor.set();
        }
        self.active = Some(kind);
    }
}

fn load_resource_cursor(
    asset_root: &Path,
    resource_id: u16,
    hotspot: (i32, i32),
) -> Result<Cursor> {
    let path = asset_root
        .join("cursors")
        .join(format!("CURS_{resource_id:05}.png"));
    let image = image::open(&path)
        .with_context(|| format!("open cursor image {}", path.display()))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let mut pixels = image.into_raw();
    let surface = Surface::from_data(
        pixels.as_mut_slice(),
        width,
        height,
        width.saturating_mul(4),
        PixelFormat::RGBA32,
    )
    .map_err(|err| anyhow::anyhow!("create cursor surface {}: {err}", path.display()))?;
    Cursor::from_surface(&surface, hotspot.0, hotspot.1)
        .map_err(|err| anyhow::anyhow!("create cursor from {}: {err}", path.display()))
}

fn load_cursor_hotspots(asset_root: &Path) -> Result<Vec<(u16, i32, i32)>> {
    let path = asset_root.join("cursors").join("cursor_hotspots.csv");
    let text = fs::read_to_string(&path)
        .with_context(|| format!("read cursor hotspots {}", path.display()))?;
    let mut hotspots = Vec::new();
    for line in text.lines().skip(1) {
        let mut parts = line.split(',');
        let Some(file) = parts.next() else {
            continue;
        };
        let Some(resource_id) = cursor_resource_id_from_filename(file) else {
            continue;
        };
        let hot_x = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
        let hot_y = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
        hotspots.push((resource_id, hot_x, hot_y));
    }
    Ok(hotspots)
}

fn cursor_resource_id_from_filename(file: &str) -> Option<u16> {
    file.strip_prefix("CURS_")?
        .strip_suffix(".png")?
        .parse()
        .ok()
}

fn cursor_hotspot(hotspots: &[(u16, i32, i32)], resource_id: u16) -> (i32, i32) {
    hotspots
        .iter()
        .find_map(|&(id, x, y)| (id == resource_id).then_some((x, y)))
        .unwrap_or((0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_system_cursors_do_not_abort_game_startup() {
        let mut requested = Vec::new();
        let mut cursors = GameCursors::fallback_with(|kind| {
            requested.push(kind);
            Err::<Cursor, _>("CreateSystemCursor is not currently supported")
        });
        assert_eq!(requested, GameCursor::ALL);
        assert!(cursors.cursors.is_empty());
        for tool in [EditTool::Power, EditTool::Road, EditTool::Query] {
            let kind = GameCursor::for_tool(tool);
            cursors.set(kind);
            assert_eq!(cursors.active, Some(kind));
        }
    }
}
