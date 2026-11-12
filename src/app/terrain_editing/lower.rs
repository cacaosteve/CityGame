use super::{
    PE_447490_UI_WAIT_4012C1_PER_CALL, PE_RAISE_SUBMODE_DEFAULT, PE_TERRAIN_SLOPE_BITS,
    PE_TERRAIN_SLOPE_DX, PE_TERRAIN_SLOPE_DY, offset_city_tile, pe_lower_unpack_tile_444bf0,
    pe_stretch_hill_crt_jitter, pe_terrain_neighbor_fun_00447020_447490_signed,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};
use crate::win95_random::Win95CrtRand;

/// PE `0x444440` mode≠0 cost table `0x4dc140` indexed by Raise submode `0x4cc950`.
pub(in crate::app) const PE_LOWER_444440_MODE_COST: [i32; 5] = [1, 0x19, 0x19, 0x19, 1];

/// Win95 LowerTerrain dig `0x402ce8`→`0x444440` (mode 0): refuse land 0, else ALTM land--,
/// relax higher neighbors through the native queue, union their bounds with Win32 SetRect /
/// UnionRect, then classify the one-tile-padded rectangle with `0x447490`.
/// PE @ `0x444480`: `test mode,mode` / `je` skips the funds debit path used by mode≠0
/// (Raise adjust-toward dig pushes mode 1 @ `0x44323e`; Lower CRT pushes mode 0 @ `0x4439bf`).
pub(in crate::app) fn pe_lower_terrain_land_dec_444440(
    city: &mut City,
    x: usize,
    y: usize,
) -> bool {
    pe_lower_terrain_land_dec_444440_mode(city, x, y, 0)
}

/// Win95 `0x444440` with explicit mode. Mode 0 matches Lower CRT dig (no funds).
/// Mode ≠0 bills `PE_LOWER_444440_MODE_COST[raise_submode]` like PE `0x444487`..`0x444510`.
pub(in crate::app) fn pe_lower_terrain_land_dec_444440_mode(
    city: &mut City,
    x: usize,
    y: usize,
    mode: u32,
) -> bool {
    pe_lower_terrain_land_dec_444440_mode_with_submode(city, x, y, mode, PE_RAISE_SUBMODE_DEFAULT)
}

pub(in crate::app) fn pe_lower_terrain_land_dec_444440_mode_with_submode(
    city: &mut City,
    x: usize,
    y: usize,
    mode: u32,
    raise_submode: u16,
) -> bool {
    pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt(
        city,
        x,
        y,
        mode,
        raise_submode,
        None,
    )
}

pub(in crate::app) fn pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt(
    city: &mut City,
    x: usize,
    y: usize,
    mode: u32,
    raise_submode: u16,
    crt: Option<&mut Win95CrtRand>,
) -> bool {
    pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt_effects(
        city,
        crt,
        x,
        y,
        mode,
        raise_submode,
    )
    .is_some()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) struct PeLowerTerrainEffects {
    pub(in crate::app) ui_waits_4012c1: u32,
    #[cfg(test)]
    pub(in crate::app) queue_dropped_oldest: u32,
}

/// PE `0x4CB468` coordinate ring: 512 slots, masked 9-bit cursors, one slot
/// kept empty, and drop-oldest behavior when the write cursor catches the read.
pub(in crate::app) struct PeTerrainCoordinateQueue {
    pub(in crate::app) entries: [(i16, i16); 512],
    pub(in crate::app) read: usize,
    pub(in crate::app) write: usize,
}

impl PeTerrainCoordinateQueue {
    pub(in crate::app) fn new() -> Self {
        Self {
            entries: [(0, 0); 512],
            read: 0,
            write: 0,
        }
    }

    pub(in crate::app) fn is_empty(&self) -> bool {
        self.read == self.write
    }

    pub(in crate::app) fn push(&mut self, coordinate: (i16, i16)) -> bool {
        const MASK: usize = 0x1ff;
        self.entries[self.write] = coordinate;
        self.write = (self.write + 1) & MASK;
        if self.write == self.read {
            self.read = (self.read + 1) & MASK;
            true
        } else {
            false
        }
    }

    pub(in crate::app) fn pop(&mut self) -> Option<(i16, i16)> {
        if self.is_empty() {
            return None;
        }
        const MASK: usize = 0x1ff;
        let coordinate = self.entries[self.read];
        self.read = (self.read + 1) & MASK;
        Some(coordinate)
    }
}

/// Win95 `0x444440`: lower the requested cell, relax neighboring heights through
/// the native 512-slot queue, then classify the padded dirty rectangle with `0x447490`.
pub(in crate::app) fn pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt_effects(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    mode: u32,
    raise_submode: u16,
) -> Option<PeLowerTerrainEffects> {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return None;
    }
    let sample = city.altm_at(x, y);
    if sample.land == 0 {
        return None;
    }
    let cost = if pe_lower_terrain_444440_mode_skips_funds(mode) {
        0
    } else {
        let cost = pe_lower_444440_mode_cost(raise_submode);
        if cost < 0 || city.city_funds() < cost {
            return None;
        }
        cost
    };
    if !pe_lower_terrain_444440_mode_skips_funds(mode) {
        city.set_city_funds(city.city_funds() - cost);
    }
    city.set_altitude(x, y, sample.land - 1, sample.water);

    // 0x4014b0 resets the ring and seeds it; 0x40234c pops and 0x402ea5
    // pushes, advancing the read cursor if the 9-bit write cursor catches it.
    let mut queue = PeTerrainCoordinateQueue::new();
    let _ = queue.push((x as i16, y as i16));
    #[cfg(test)]
    let mut queue_dropped_oldest = 0u32;

    // The first Win32 RECT is the seed's half-open unit rectangle [x,y,x+1,y+1].
    // Later SetRect/UnionRect calls add each seed-to-dequeued-coordinate bound.
    let (mut min_x, mut max_x) = (x as i16, x as i16 + 1);
    let (mut min_y, mut max_y) = (y as i16, y as i16 + 1);
    let mut waits = 0u32;
    while let Some((cx, cy)) = queue.pop() {
        min_x = min_x.min(cx);
        max_x = max_x.max(cx);
        min_y = min_y.min(cy);
        max_y = max_y.max(cy);

        let center_land = city.altitude_at(cx as usize, cy as usize) & 0x1f;
        if let Some(zone) = city.zone_at(cx as usize, cy as usize) {
            city.set_zone(cx as usize, cy as usize, zone.raw_bits() & 0xf0);
        }
        let mut higher_neighbor_mask = 0u16;
        for i in 0..8 {
            let nx = cx + PE_TERRAIN_SLOPE_DX[i];
            let ny = cy + PE_TERRAIN_SLOPE_DY[i];
            if !(0..0x80).contains(&nx) || !(0..0x80).contains(&ny) {
                continue;
            }
            if (city.altitude_at(nx as usize, ny as usize) & 0x1f) > center_land {
                higher_neighbor_mask |= PE_TERRAIN_SLOPE_BITS[i];
            }
        }

        // `0x444674..0x444764`: lower neighbors over one level higher, or
        // exactly one level higher when the union of slope bits is `0x0f`.
        for i in 0..8 {
            let nx = cx + PE_TERRAIN_SLOPE_DX[i];
            let ny = cy + PE_TERRAIN_SLOPE_DY[i];
            if !(0..0x80).contains(&nx) || !(0..0x80).contains(&ny) {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            let neighbor = city.altm_at(nx, ny);
            let neighbor_land = neighbor.land & 0x1f;
            let lower_neighbor = neighbor_land > center_land.saturating_add(1)
                || (neighbor_land > center_land && higher_neighbor_mask == 0x0f);
            if !lower_neighbor {
                continue;
            }

            if !pe_lower_terrain_444440_mode_skips_funds(mode) {
                city.set_city_funds(city.city_funds().wrapping_sub(cost));
            }
            waits = waits.wrapping_add(1); // `0x4012c1` before each neighbor adjustment
            let _dropped_oldest = queue.push((nx as i16, ny as i16));
            #[cfg(test)]
            if _dropped_oldest {
                queue_dropped_oldest = queue_dropped_oldest.wrapping_add(1);
            }
            city.set_altitude(nx, ny, neighbor_land - 1, neighbor.water);
        }
    }

    if !pe_lower_terrain_444440_mode_skips_funds(mode) && city.city_funds() < 0 {
        city.set_city_funds(0);
    }

    // PE SetRect/UnionRect accumulates the seed and dequeued-tile bounds. The worker
    // expands that union by one tile and calls `0x40215d` for every coordinate. Each
    // call performs nine `FUN_00447020` classifications, with two waits per cell.
    for refresh_x in (min_x - 1)..=(max_x + 1) {
        for refresh_y in (min_y - 1)..=(max_y + 1) {
            pe_terrain_neighbor_fun_00447020_447490_signed(
                city,
                crt.as_deref_mut(),
                refresh_x,
                refresh_y,
            );
            waits = waits.wrapping_add(PE_447490_UI_WAIT_4012C1_PER_CALL);
        }
    }
    Some(PeLowerTerrainEffects {
        ui_waits_4012c1: waits,
        #[cfg(test)]
        queue_dropped_oldest,
    })
}

/// PE `0x4dc140` dig cost for Raise submode; unknown submodes refuse (`-1`).
pub(in crate::app) fn pe_lower_444440_mode_cost(raise_submode: u16) -> i32 {
    PE_LOWER_444440_MODE_COST
        .get(raise_submode as usize)
        .copied()
        .unwrap_or(-1)
}

/// PE `0x444440` @ `0x444480`: mode 0 skips funds; nonzero mode bills.
pub(in crate::app) fn pe_lower_terrain_444440_mode_skips_funds(mode: u32) -> bool {
    mode == 0
}

/// Win95 Lower samples four CRT draws and calls the free dig worker if the
/// jittered tile is in bounds. The boolean reports whether that worker dug.
pub(in crate::app) fn pe_lower_terrain_crt_step_443900(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
) -> Option<bool> {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return None;
    }
    let (dx, dy) = pe_stretch_hill_crt_jitter(crt);
    let (tx, ty) = offset_city_tile(x, y, dx, dy)?;
    Some(pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt(
        city,
        tx,
        ty,
        0,
        PE_RAISE_SUBMODE_DEFAULT,
        Some(crt),
    ))
}

/// PE Lower `0x443900` @ `0x443961`/`0x443964`: unpack `0x444bf0` (`& 0x7f`) before CRT jitter.
pub(in crate::app) fn pe_lower_terrain_from_packed_443900(
    city: &mut City,
    crt: &mut Win95CrtRand,
    packed: i16,
) -> Option<bool> {
    let (x, y) = pe_lower_unpack_tile_444bf0(packed)?;
    pe_lower_terrain_crt_step_443900(city, crt, x, y)
}
