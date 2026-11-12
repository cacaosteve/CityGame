//! City valuation and bond eligibility, rate and credit-rating calculations.
//!
//! Original PE, native Mac and SC2KFix-corrected PE paths remain distinct.
//! Arithmetic, census/cost sources and read-only versus recalculating probes
//! retain their existing behavior. Callers own path selection, budget/loan
//! accounting, UI routing and simulation scheduling.

use super::{
    BOND_RATE_SCALE, HIGHWAY_COST_PER_SEGMENT, NetworkKind, UndergroundKind,
    outstanding_bond_count_for_city, tool_build_cost,
};
use crate::city::City;
use crate::ui::EditTool;

/// PE RecalculateCityValue infrastructure arm @ `0x46a270`–`0x46a344` (before building
/// jump-table `0xc6..`). Seeds `ecx = sx(0x4ca41c) * [0x4dc2b8]` (`-1`), then for each
/// XBLD id `0x0e..0x6f` adds `sx(dwTileCount[id]) * band_factor`.
pub(super) fn pe_city_value_infra_subtotal_46a270(city: &City) -> i32 {
    // PE factors from `.data`: `0x4dc2b8=-1`, `0x4dc1d0=2`, `0x4dc260=10`, `0x4dc290=25`,
    // tunnel/crossover `*15`, bridge/onramp `*100`, `0x4dc264=100`, `0x4dc2a0=250`.
    let mut total = i32::from(city.pe_xund_occupancy as i16).wrapping_mul(-1);
    for tile in 0x0eu8..0x70 {
        let count = i32::from(city.pe_dw_tile_count[tile as usize] as i16);
        let factor = match tile {
            0x0e..0x1d => 2,
            0x1d..0x2c => 10,
            0x2c..0x3f => 25,
            0x3f..0x51 => 15,
            0x51..0x61 => 100,
            0x61..0x6c => 100,
            _ => 250, // 0x6c..0x70
        };
        total = total.wrapping_add(count.wrapping_mul(factor));
    }
    total
}

/// PE jump-index bytes @ `0x46a6dc` for XBLD `0xc6..0xfe` (`edx = tile - 0xc6`).
/// `0x20` is the no-op case (jmp `0x46a63f`).
const PE_CITY_VALUE_BUILDING_CASE: [u8; 0x39] = [
    0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x20, 0x02, 0x03, 0x04, 0x05, 0x06,
    0x07, 0x08, 0x09, 0x0a, 0x0b, 0x20, 0x0c, 0x0d, 0x0d, 0x0e, 0x0e, 0x0f, 0x20, 0x10, 0x11, 0x11,
    0x11, 0x20, 0x20, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x20, 0x18, 0x20, 0x19, 0x20, 0x1a, 0x1b,
    0x1c, 0x20, 0x1d, 0x20, 0x1e, 0x1f, 0x1f, 0x1f, 0x1f,
];

/// PE `.data` dword costs @ `0x4dbec0` for XBLD `0xc6..=0xfe` (case 0 / case-1 prior slot).
const PE_CITY_VALUE_COST_4DBEC0_C6: [i32; 0x39] = [
    4000, 400, 6600, 2000, 15000, 100, 1300, 28000, 40000, -1, 3, 100, 250, 500, 1000, -1, -1, -1,
    -1, -1, -1, -1, 0, 0, 0, 0, 0, 100000, 120000, 150000, 200000, -1, -1, -1, 10, 100, 150, 25,
    250, -1, -1, -1, -1, -1, -1, -1, 25, 100, 500, 250, 250, -1, -1, -1, -1, -1, -1,
];

/// PE arco costs @ `0x4dbe58 + tile*4` for case `0x1f` (XBLD `0xfb..=0xfe`).
const PE_CITY_VALUE_ARCO_COST: [i32; 4] = [100_000, 120_000, 150_000, 200_000];

fn pe_city_value_sar_div_16(n: i32) -> i32 {
    // PE `cdq` / `and edx,0xf` / `add` / `sar 4`.
    n.wrapping_add((n >> 31) & 0xf) >> 4
}

fn pe_city_value_sar_div_4(n: i32) -> i32 {
    // PE `cdq` / `and edx,3` / `add` / `sar 2`.
    n.wrapping_add((n >> 31) & 0x3) >> 2
}

pub(super) fn pe_city_value_building_contrib_46a34a(city: &City, tile: u8) -> i32 {
    let idx = usize::from(tile.wrapping_sub(0xc6));
    if idx > 0x38 {
        return 0;
    }
    let case = PE_CITY_VALUE_BUILDING_CASE[idx];
    if case == 0x20 {
        return 0;
    }
    let count = i32::from(city.pe_dw_tile_count[tile as usize] as i16);
    let cost_c6 = |t: u8| PE_CITY_VALUE_COST_4DBEC0_C6[usize::from(t.wrapping_sub(0xc6))];
    match case {
        0x00 => count.wrapping_mul(cost_c6(tile)),
        0x01 => pe_city_value_sar_div_16(count).wrapping_mul(cost_c6(tile.wrapping_sub(1))),
        0x02 => (count / 9).wrapping_mul(500),
        0x03 => (count / 9).wrapping_mul(500),
        0x04 => (count / 9).wrapping_mul(500),
        0x05 => (count / 9).wrapping_mul(1000),
        0x06 => (count / 9).wrapping_mul(150),
        0x07 => (count / 9).wrapping_mul(250),
        0x08 => pe_city_value_sar_div_16(count).wrapping_mul(3000),
        0x09 => pe_city_value_sar_div_16(count).wrapping_mul(3000),
        0x0a => pe_city_value_sar_div_16(count).wrapping_mul(1000),
        0x0b => pe_city_value_sar_div_16(count).wrapping_mul(5000),
        0x0c => count.wrapping_mul(100),
        0x0d | 0x0f | 0x11 | 0x13 | 0x17 | 0x1c => count.wrapping_mul(250),
        0x0e | 0x10 | 0x18 | 0x19 => count.wrapping_mul(150),
        0x12 => count.wrapping_mul(250),
        0x14 | 0x15 => pe_city_value_sar_div_4(count).wrapping_mul(250),
        0x16 => pe_city_value_sar_div_4(count).wrapping_mul(500),
        0x1a => (count / 9).wrapping_mul(500),
        0x1b => pe_city_value_sar_div_4(count).wrapping_mul(500),
        0x1d => (count / 9).wrapping_mul(1000),
        0x1e => (count / 9).wrapping_mul(1000),
        0x1f => {
            let arco = PE_CITY_VALUE_ARCO_COST[usize::from(tile.wrapping_sub(0xfb))];
            pe_city_value_sar_div_16(count).wrapping_mul(arco)
        }
        _ => 0,
    }
}

/// PE RecalculateCityValue @ `0x46a270`–`0x46a656` (thunk `0x401f50`): infra + buildings,
/// then store dword `[0x4ca4d0]`.
pub(super) fn pe_recalculate_city_value_46a270(city: &mut City) {
    city.pe_city_value = pe_city_value_total_46a270(city);
}

fn pe_city_value_total_46a270(city: &City) -> i32 {
    let mut total = pe_city_value_infra_subtotal_46a270(city);
    for tile in 0xc6u8..=0xfe {
        total = total.wrapping_add(pe_city_value_building_contrib_46a34a(city, tile));
    }
    total
}

/// SC2KFix's PE `Hook_RecalculateCityValue` replacement. Retail PE valuation
/// uses fixed tile-ID coefficients above; SC2KFix values placed infrastructure
/// from its construction-cost table and normalizes multi-tile footprints.
/// The matching Rust placement prices are the `tool_build_cost`/network costs.
pub(super) fn sc2kfix_pe_city_value_total(city: &City) -> i32 {
    let cost = |tool: EditTool| -> i32 {
        tool_build_cost(tool).expect("SC2KFix city-value tool has a placement cost") as i32
    };
    let mut total = i32::from(city.pe_xund_occupancy)
        .wrapping_mul(UndergroundKind::Subway.cost_per_tile() as i32);

    for tile in 0x0eu8..=0x6f {
        let count = i32::from(city.pe_dw_tile_count[tile as usize]);
        let per_tile_cost = match tile {
            0x0e..=0x1c => NetworkKind::Power.cost_per_tile().unwrap_or(0) as i32,
            0x1d..=0x2b => NetworkKind::Road.cost_per_tile().unwrap_or(0) as i32,
            0x2c..=0x3e => NetworkKind::Rail.cost_per_tile().unwrap_or(0) as i32,
            0x3f..=0x50 => 15,  // tunnel and crossover structures
            0x51..=0x60 => 100, // bridge decks and onramps
            0x61..=0x6b => HIGHWAY_COST_PER_SEGMENT as i32,
            _ => cost(EditTool::SubToRail), // 0x6c..=0x6f
        };
        total = total.wrapping_add(count.wrapping_mul(per_tile_cost));
    }

    for tile in 0xc6u8..=0xfe {
        let count = i32::from(city.pe_dw_tile_count[tile as usize]);
        let contribution = match tile {
            0xc6 | 0xc7 => count.wrapping_mul(cost(EditTool::PowerPlantHydro)),
            0xc8 => count.wrapping_mul(cost(EditTool::PowerPlantWind)),
            0xc9..=0xcf => {
                let tool = match tile {
                    0xc9 => EditTool::PowerPlantGas,
                    0xca => EditTool::PowerPlantOil,
                    0xcb => EditTool::PowerPlantNuclear,
                    0xcc => EditTool::PowerPlantSolar,
                    0xcd => EditTool::PowerPlantMicrowave,
                    0xce => EditTool::PowerPlantFusion,
                    _ => EditTool::PowerPlantCoal,
                };
                (count / 16).wrapping_mul(cost(tool))
            }
            0xd1 => (count / 9).wrapping_mul(cost(EditTool::Hospital)),
            0xd2 => (count / 9).wrapping_mul(cost(EditTool::Police)),
            0xd3 => (count / 9).wrapping_mul(cost(EditTool::FireStation)),
            0xd4 => (count / 9).wrapping_mul(cost(EditTool::Museum)),
            0xd5 => (count / 9).wrapping_mul(cost(EditTool::BigPark)),
            0xd6 => (count / 9).wrapping_mul(cost(EditTool::School)),
            0xd7 => (count / 16).wrapping_mul(cost(EditTool::Stadium)),
            0xd8 => (count / 16).wrapping_mul(cost(EditTool::Prison)),
            0xd9 => (count / 16).wrapping_mul(cost(EditTool::College)),
            0xda => (count / 16).wrapping_mul(cost(EditTool::Zoo)),
            0xdc => count.wrapping_mul(cost(EditTool::WaterPump)),
            0xdd | 0xde | 0xe1 | 0xe4..=0xe6 | 0xea | 0xee | 0xf6 => {
                count.wrapping_mul(cost(EditTool::Airport))
            }
            0xdf | 0xe0 | 0xe3 | 0xf0 | 0xf2 => count.wrapping_mul(cost(EditTool::Seaport)),
            0xe9 => count.wrapping_mul(cost(EditTool::SubStation)),
            0xeb => (count / 4).wrapping_mul(cost(EditTool::WaterTower)),
            0xec => (count / 4).wrapping_mul(cost(EditTool::BusDepot)),
            0xed => (count / 4).wrapping_mul(cost(EditTool::RailDepot)),
            0xf4 => (count / 4).wrapping_mul(cost(EditTool::WaterTreatment)),
            0xf5 => (count / 4).wrapping_mul(cost(EditTool::Library)),
            0xf8 => (count / 9).wrapping_mul(cost(EditTool::Marina)),
            0xfa => (count / 9).wrapping_mul(cost(EditTool::Desalinization)),
            0xfb..=0xfe => {
                let tool = match tile {
                    0xfb => EditTool::ArcologyPlymouth,
                    0xfc => EditTool::ArcologyForest,
                    0xfd => EditTool::ArcologyDarco,
                    _ => EditTool::ArcologyLaunch,
                };
                (count / 16).wrapping_mul(cost(tool))
            }
            // City Hall, statue, and non-valuation infrastructure tiles have
            // no case in SC2KFix's hook.
            _ => 0,
        };
        total = total.wrapping_add(contribution);
    }
    total
}

pub(super) fn sc2kfix_pe_recalculate_city_value(city: &mut City) {
    city.pe_city_value = sc2kfix_pe_city_value_total(city);
}

/// Mac CODE 8 `export_239 @ 0x3a714`: value weights recovered from the A5 DATA image.
/// Its saved selector-counter multiplier is zero; ordinary bands and building
/// costs otherwise match PE. Counts come from the Mac saved XBLD census on Mac files.
pub(super) fn mac_city_value_total_3a714(city: &City) -> u32 {
    let mut total = 0u32; // Mac `ffffda78` is initialized to zero.
    for tile in 0x0eu8..0x70 {
        let count = u32::from(city.mac_xbld_tile_count(tile));
        let contribution = match tile {
            0x0e..=0x1c => count.wrapping_mul(2),
            0x1d..=0x2b => count.wrapping_mul(10),
            0x2c..=0x3e => count.wrapping_mul(25),
            0x3f..=0x50 => u32::from((count.wrapping_mul(15)) as u16),
            0x51..=0x60 => u32::from((count.wrapping_mul(100)) as u16),
            0x61..=0x6b => count.wrapping_mul(100),
            _ => count.wrapping_mul(250),
        };
        total = total.wrapping_add(contribution);
    }

    const MAC_CITY_VALUE_COST_C6_CF: [u32; 10] = [
        4_000, 400, 6_600, 6_600, 2_000, 15_000, 100, 1_300, 28_000, 40_000,
    ];
    const MAC_CITY_VALUE_ARCO_COST: [u32; 4] = [100_000, 120_000, 150_000, 200_000];
    for tile in 0xc6u8..=0xfe {
        let count = u32::from(city.mac_xbld_tile_count(tile));
        let contribution = match tile {
            0xc6..=0xc8 => count.wrapping_mul(MAC_CITY_VALUE_COST_C6_CF[usize::from(tile - 0xc6)]),
            0xc9..=0xcf => {
                (count >> 4).wrapping_mul(MAC_CITY_VALUE_COST_C6_CF[usize::from(tile - 0xc6)])
            }
            0xd1 => (count / 9).wrapping_mul(500),
            0xd2 => (count / 9).wrapping_mul(500),
            0xd3 => (count / 9).wrapping_mul(500),
            0xd4 => (count / 9).wrapping_mul(1_000),
            0xd5 => (count / 9).wrapping_mul(150),
            0xd6 => (count / 9).wrapping_mul(250),
            0xd7 => (count >> 4).wrapping_mul(3_000),
            0xd8 => (count >> 4).wrapping_mul(3_000),
            0xd9 => (count >> 4).wrapping_mul(1_000),
            0xda => (count >> 4).wrapping_mul(5_000),
            0xdc => count.wrapping_mul(100),
            0xdd | 0xde | 0xe1 | 0xe4 | 0xe5 | 0xe6 | 0xea | 0xee | 0xf6 => count.wrapping_mul(250),
            0xdf | 0xe0 | 0xe3 | 0xf0 | 0xf2 => count.wrapping_mul(150),
            0xe9 => count.wrapping_mul(250),
            0xeb => (count >> 2).wrapping_mul(250),
            0xec => (count >> 2).wrapping_mul(250),
            0xed => (count >> 2).wrapping_mul(500),
            0xf4 => (count / 9).wrapping_mul(500),
            0xf5 => (count >> 2).wrapping_mul(500),
            0xf8 => (count / 9).wrapping_mul(1_000),
            0xfa => (count / 9).wrapping_mul(1_000),
            0xfb..=0xfe => {
                (count >> 4).wrapping_mul(MAC_CITY_VALUE_ARCO_COST[usize::from(tile - 0xfb)])
            }
            _ => 0, // Includes the deliberately unhandled `0xd0`.
        };
        total = total.wrapping_add(contribution);
    }
    total
}

fn mac_recalculate_city_value_3a714(city: &mut City) -> u32 {
    let total = mac_city_value_total_3a714(city);
    // CODE 8 writes the result to `ffffeff6` (shared MISC `+0x24` role).
    city.pe_city_value = total as i32;
    total
}

/// Mac CODE 4 bond issue/credit paths use `bonds * 25000 / (city_value + 1)`;
/// the rooted Win95 issue path uses 2500 and compares the full quotient.
pub(super) fn mac_bond_city_value_ratio_3a714(city_value: u32, bonds: u32) -> Option<u32> {
    let denom = city_value.wrapping_add(1);
    if denom == 0 {
        return None;
    }
    Some(bonds.wrapping_mul(25_000) / denom)
}

pub(super) fn mac_bond_issue_city_value_allows_3a714(city: &mut City) -> bool {
    let value = mac_recalculate_city_value_3a714(city);
    mac_bond_city_value_ratio_3a714(value, outstanding_bond_count_for_city(city))
        .is_some_and(mac_bond_city_value_ratio_allows_issue_3a714)
}

pub(super) fn mac_bond_issue_city_value_would_allow_3a714(city: &City) -> bool {
    mac_bond_city_value_ratio_3a714(
        mac_city_value_total_3a714(city),
        outstanding_bond_count_for_city(city),
    )
    .is_some_and(mac_bond_city_value_ratio_allows_issue_3a714)
}

/// The dialog compares D5w with `CMPI.W #6` / signed `BGE`, so the test is on
/// the quotient's low signed WORD, not its full unsigned DIVUL result.
pub(super) fn mac_bond_city_value_ratio_allows_issue_3a714(ratio: u32) -> bool {
    (ratio as u16 as i16) < 6
}

/// Mac CODE 4 `FUN_0002b668 @ 0x2b668` selects the rate immediately before
/// calling `export_226 @ 0x2cb9e`: low-WORD national tax + the 25,000 city-value
/// quotient + one percentage point. The caller has already recalculated the
/// Mac city value and passed the signed low-WORD issue gate.
pub(super) fn mac_bond_issue_rate_scaled_3a714(city: &City) -> Option<u32> {
    let ratio = mac_bond_city_value_ratio_3a714(
        city.pe_city_value as u32,
        outstanding_bond_count_for_city(city),
    )?;
    let rate_word = city
        .pe_w_national_tax
        .wrapping_add(ratio as u16)
        .wrapping_add(1);
    Some(u32::from(rate_word) * BOND_RATE_SCALE)
}

/// Win95 bond gate enters at `0x41b270` and recalculates at `0x41b274`. The
/// gameplay adapter uses the SC2KFix-corrected valuation before its ratio check.
pub(super) fn pe_bond_issue_city_value_allows_41b270(city: &mut City) -> bool {
    sc2kfix_pe_recalculate_city_value(city);
    pe_bond_issue_city_value_ratio_ok_41b270(city)
}

fn pe_bond_issue_city_value_ratio_ok_41b270(city: &City) -> bool {
    pe_bond_issue_gate_from(city.pe_city_value, outstanding_bond_count_for_city(city))
        .unwrap_or(false)
}

/// PE `0x41b287..0x41b29c`: DIV uses a DWORD quotient, but `CMP AX,6`
/// followed by signed JGE reads only its low signed WORD. A zero divisor is
/// malformed state and remains fail-closed instead of reproducing a CPU fault.
pub(super) fn pe_bond_issue_gate_from(city_value: i32, bonds: u32) -> Option<bool> {
    let denom = (city_value as u32).wrapping_add(1);
    let quotient = bonds.wrapping_mul(2500).checked_div(denom)?;
    Some((quotient as u16 as i16) < 6)
}

/// Read-only probe for the Win95 bond gate using the SC2KFix-corrected total
/// without writing `[0x4ca4d0]` (UI enable paths).
pub(super) fn pe_bond_issue_city_value_would_allow_41b270(city: &City) -> bool {
    pe_bond_issue_gate_from(
        sc2kfix_pe_city_value_total(city),
        outstanding_bond_count_for_city(city),
    )
    .unwrap_or(false)
}

/// Win95 bond credit ratio path enters at `0x41e580` and recalculates at
/// `0x41e58e`; CityGame uses the SC2KFix-corrected construction-cost valuation.
pub(super) fn pe_bond_city_value_ratio_score_41e580(city: &mut City) -> u16 {
    sc2kfix_pe_recalculate_city_value(city);
    pe_bond_city_value_ratio_score_from(city.pe_city_value, outstanding_bond_count_for_city(city))
}

pub(super) fn pe_bond_city_value_ratio_score_from(city_value: i32, bonds: u32) -> u16 {
    let denom = (city_value as u32).wrapping_add(1);
    if denom == 0 {
        return 6;
    }
    (bonds.wrapping_mul(25_000) / denom).min(6) as u16
}

/// Read-only probe for the Win95 credit score without writing `[0x4ca4d0]`.
pub(super) fn pe_bond_city_value_ratio_score_would_41e580(city: &City) -> u16 {
    pe_bond_city_value_ratio_score_from(
        sc2kfix_pe_city_value_total(city),
        outstanding_bond_count_for_city(city),
    )
}

/// PE caption pointers @ `0x4e6a78` for bond credit scores 0..=6:
/// `"AAA"`, `"AA"`, `"A"`, `"B"`, `"C"`, `"D"`, `"F"`.
const PE_BOND_CREDIT_RATING_CAPTIONS_4E6A78: [&str; 7] = ["AAA", "AA", "A", "B", "C", "D", "F"];

pub(super) fn pe_bond_credit_rating_caption_4e6a78(score: u16) -> &'static str {
    PE_BOND_CREDIT_RATING_CAPTIONS_4E6A78[usize::from(score.min(6))]
}
