//! Test-only loader/writer for the Mac scan-state snapshots used by the
//! Unicorn differential probes. CGMS is an internal diagnostic format, not a
//! user-facing city format.

use super::*;

const CGMS_HEADER_LEN: usize = 20;
const CGMS_V4_LEN: usize = 201_908;
const CGMS_MISC_DWORDS: usize = 1_200;
const CGMS_XTHG_LEN: usize = XTHG_RECORD_COUNT * 12;

pub(crate) fn city_from_cgms_v4_for_test(bytes: &[u8]) -> Result<City> {
    anyhow::ensure!(bytes.len() == CGMS_V4_LEN, "expected a CGMS v4 snapshot");
    anyhow::ensure!(&bytes[..4] == b"CGMS", "bad CGMS magic");
    anyhow::ensure!(
        u16::from_be_bytes([bytes[4], bytes[5]]) == 4
            && u16::from_be_bytes([bytes[6], bytes[7]]) == CITY_WIDTH as u16
            && u16::from_be_bytes([bytes[8], bytes[9]]) == CITY_HEIGHT as u16
            && u16::from_be_bytes([bytes[10], bytes[11]]) == 10
            && u32::from_be_bytes(bytes[12..16].try_into()?) as usize == CGMS_MISC_DWORDS,
        "unsupported CGMS v4 layout"
    );

    let tiles = CITY_TILES;
    let mut cursor = CGMS_HEADER_LEN;
    let mut plane = || {
        let result = bytes[cursor..cursor + tiles].to_vec();
        cursor += tiles;
        result
    };
    let terrain = plane();
    let buildings = plane();
    let zones = plane();
    let tile_bits = plane();
    let underground = plane();
    let text_overlays = plane();
    let land_value = plane();
    let crime = plane();
    let pollution = plane();

    let altm = bytes[cursor..cursor + tiles * 2]
        .chunks_exact(2)
        .map(|raw| {
            let word = u16::from_be_bytes([raw[0], raw[1]]);
            AltmSample {
                land: (word & 0x1f) as u8,
                water: ((word >> 5) & 0x1f) as u8,
                tunnel_levels: ((word >> 10) & 0x3f) as u8,
            }
        })
        .collect::<Vec<_>>();
    cursor += tiles * 2;
    let traffic = bytes[cursor..cursor + tiles].to_vec();
    cursor += tiles;
    let misc = bytes[cursor..cursor + CGMS_MISC_DWORDS * 4].to_vec();
    cursor += CGMS_MISC_DWORDS * 4;
    let xthg = &bytes[cursor..cursor + CGMS_XTHG_LEN];

    let mut city = City::new_blank("/tmp/cgms-differential-probe.sc2", "Flint")?;
    city.kind = CityFileKind::MacCity;
    city.terrain = Some(terrain);
    city.buildings = Some(buildings);
    city.zones = Some(zones.into_iter().map(ZoneFlags::new).collect());
    city.tile_bits = Some(tile_bits.into_iter().map(TileBits::new).collect());
    city.underground = Some(underground);
    city.text_overlays = Some(text_overlays);
    city.land_value_density = Some(land_value);
    city.crime_density = Some(crime);
    city.pollution_density = Some(pollution);
    city.altm = Some(altm);
    city.traffic_density = Some(traffic);
    city.xtrf_storage = XtrfStorageFormat::PerTile128;
    city.mac_budget_book = Some(
        misc[MAC_MISC_BUDGET_BOOK_OFFSET
            ..MAC_MISC_BUDGET_BOOK_OFFSET
                + MISC_BUDGET_ROW_DWORDS * MISC_BUDGET_ROW_COUNT * MISC_FIELD_LEN]
            .to_vec(),
    );
    city.chunks = vec![CityChunk {
        name: "MISC".to_owned(),
        offset: 0,
        raw_len: misc.len(),
        data: misc,
    }];
    city.chunk_index.insert("MISC".to_owned(), 0);
    city.traffic_overlays = Some(decode_xthg_records(xthg)?);
    city.traffic_overlays_from_file = true;
    city.saved_rotation = city.mac_state_misc_u32_at(8).unwrap_or(0) as u8;
    // The native CODE 2 calendar replay restores its date from the Mac
    // MISC year-founded word and phase counter. Keep that context when a
    // diagnostic CGMS is loaded; otherwise calendar probes silently fall
    // back to January 1900 and cannot exercise year-roll callbacks.
    if let Some(founded) = city
        .mac_state_misc_u32_at(MISC_YEAR_FOUNDED_OFFSET)
        .map(|raw| raw as u16)
        .filter(|year| *year > 0)
    {
        if let Some(counter) = city.mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET) {
            let year = u32::from(founded).saturating_add(counter / MAC_CALENDAR_PHASES_PER_YEAR);
            if let Ok(year) = u16::try_from(year) {
                let within_year = counter % MAC_CALENDAR_PHASES_PER_YEAR;
                city.set_saved_date_and_day(
                    CityDate {
                        year,
                        month: (within_year / MAC_CALENDAR_PHASES_PER_MONTH + 1) as u8,
                    },
                    (within_year % MAC_CALENDAR_PHASES_PER_MONTH) as u8,
                );
            }
        }
    }
    city.rebuild_corner_metadata();
    city.pe_rebuild_dw_tile_count_434f80();
    city.modified = false;
    Ok(city)
}

pub(crate) fn cgms_v4_from_city_for_test(city: &City) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(CGMS_V4_LEN);
    out.extend_from_slice(b"CGMS");
    out.extend_from_slice(&4u16.to_be_bytes());
    out.extend_from_slice(&(CITY_WIDTH as u16).to_be_bytes());
    out.extend_from_slice(&(CITY_HEIGHT as u16).to_be_bytes());
    out.extend_from_slice(&10u16.to_be_bytes());
    out.extend_from_slice(&(CGMS_MISC_DWORDS as u32).to_be_bytes());
    out.extend_from_slice(&0x0fffu32.to_be_bytes());

    for getter in [
        City::terrain_at as fn(&City, usize, usize) -> u8,
        City::building_at,
        |city: &City, x, y| city.zone_at(x, y).map_or(0, ZoneFlags::raw_bits),
        |city: &City, x, y| city.tile_bits_at(x, y).map_or(0, TileBits::raw_bits),
        City::underground_at,
        |city: &City, x, y| city.text_overlay_at(x, y).unwrap_or(0),
        |city: &City, x, y| city.land_value_at(x, y).unwrap_or(0),
        |city: &City, x, y| city.crime_density_at(x, y).unwrap_or(0),
        |city: &City, x, y| city.pollution_density_at(x, y).unwrap_or(0),
    ] {
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                out.push(getter(city, x, y));
            }
        }
    }

    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let sample = city.altm_at(x, y);
            let word = u16::from(sample.land & 0x1f)
                | (u16::from(sample.water & 0x1f) << 5)
                | (u16::from(sample.tunnel_levels & 0x3f) << 10);
            out.extend_from_slice(&word.to_be_bytes());
        }
    }
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            out.push(city.traffic_density_at(x, y).unwrap_or(0));
        }
    }
    for index in 0..CGMS_MISC_DWORDS {
        out.extend_from_slice(
            &city
                .mac_state_misc_u32_at(index * MISC_FIELD_LEN)
                .unwrap_or(0)
                .to_be_bytes(),
        );
    }
    let records = city
        .traffic_overlays()
        .map(<[Win95OverlayRecord]>::to_vec)
        .unwrap_or_else(empty_overlay_table);
    out.extend_from_slice(&encode_xthg_records(&records)?);
    anyhow::ensure!(out.len() == CGMS_V4_LEN, "internal CGMS writer size error");
    Ok(out)
}
