use super::*;
use crate::city::CityFileKind;
use crate::mac_random::{MacGameLcgRng, MacToolboxRng};

fn run_sweep(city: &mut City, quickdraw: &mut MacToolboxRng) -> Vec<PlacementSound> {
    let mut game_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
    let mut sound_deadline = 0;
    let mut now = 0;
    super::super::tick_mac_actors_with_rng_and_clock(
        city,
        quickdraw,
        &mut game_rng,
        &mut sound_deadline,
        0,
        &mut now,
    )
}

#[test]
fn kind16_state_two_uses_native_stride_twice_and_updates_altm_height() {
    let mut city =
        City::new_blank("/tmp/mac-kind16-state-two.sc2", "Kind16StateTwo").expect("blank city");
    let (x, y, slot) = (20, 20, XTHG_FIRST_USABLE_SLOT);
    city.set_altitude(x + 2, y, 5, 0);
    city.set_text_overlay(x, y, XTHG_PROXY_BASE + slot as u8);
    city.mac_object_slot_records_mut()[slot] = Win95OverlayRecord {
        kind: XTHG_MAXIS_MAN,
        lane: 2,
        state: 2,
        tile_x: x as u8,
        tile_y: y as u8,
        height: 1,
        sub_x: 8,
        sub_y: 8,
        saved_a: 0x44,
        saved_b: 0xff,
        ..Win95OverlayRecord::default()
    };

    let mut quickdraw = MacToolboxRng::new(0);
    let sounds = run_sweep(&mut city, &mut quickdraw);

    let actor = city.traffic_overlays().unwrap()[slot];
    assert_eq!(
        (actor.kind, actor.state, actor.lane),
        (XTHG_MAXIS_MAN, 2, 2)
    );
    assert_eq!(
        (actor.tile_x, actor.tile_y, actor.sub_x, actor.sub_y),
        (22, 20, 8, 8)
    );
    assert_eq!(actor.height, 7);
    assert_eq!(city.text_overlay_at(x, y), Some(0x44));
    assert_eq!(city.text_overlay_at(21, 20), Some(0));
    assert_eq!(
        city.text_overlay_at(22, 20),
        Some(XTHG_PROXY_BASE + slot as u8)
    );
    assert_eq!(city.mac_kind16_actor_counter, 1);
    assert!(sounds.is_empty());
}

#[test]
fn kind16_matching_proxy_hands_off_to_kind6_using_released_slot() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city =
            City::new_blank("/tmp/mac-kind16-handoff.sc2", "Kind16Handoff").expect("blank city");
        city.kind = kind;
        let (x, y, slot, target_slot) = (20, 20, 1, 2);
        city.set_text_overlay(x + 1, y, XTHG_PROXY_BASE + target_slot as u8);
        city.set_text_overlay(x + 2, y, 0xf1);
        city.mac_object_slot_records_mut()[slot] = Win95OverlayRecord {
            kind: XTHG_MAXIS_MAN,
            state: 0,
            tile_x: x as u8,
            tile_y: y as u8,
            height: 6,
            saved_b: target_slot as u8,
            ..Win95OverlayRecord::default()
        };
        city.mac_object_slot_records_mut()[target_slot] = Win95OverlayRecord {
            kind: 2,
            tile_x: (x + 1) as u8,
            tile_y: y as u8,
            height: 9,
            target_x: 0x5a,
            target_y: 0x6b,
            ..Win95OverlayRecord::default()
        };
        assert_eq!(city.text_overlay_at(x + 2, y), Some(0xf1));
        assert_eq!(
            crate::mac_traffic_geometry::mac_direction_to_tile(x, y, x + 2, y),
            2
        );

        let mut quickdraw = MacToolboxRng::new(0);
        let sounds = run_sweep(&mut city, &mut quickdraw);

        let records = city.traffic_overlays().unwrap();
        let maxis_man = records[slot];
        let fire = records[target_slot];
        assert_eq!(maxis_man.kind, XTHG_MAXIS_MAN, "{kind:?}");
        assert_eq!(maxis_man.state, 2, "{kind:?}");
        assert_eq!(maxis_man.height, 6, "reads referenced +5 after constructor",);
        assert_eq!(fire.kind, XTHG_EXPLOSION, "{kind:?}");
        // The freed slot is later than the sweep cursor, so the newly created
        // kind-6 record receives its first same-pass phase-zero visit.
        assert_eq!(
            (fire.state, fire.lane, fire.height, fire.saved_b),
            (0, 1, 6, 1)
        );
        assert_eq!((fire.target_x, fire.target_y), (0x5a, 0x6b));
        assert_eq!(
            city.text_overlay_at(x + 1, y),
            Some(XTHG_PROXY_BASE + target_slot as u8)
        );
        assert_eq!(city.mac_kind16_actor_counter, 1);
        assert_eq!(
            sounds,
            vec![PlacementSound::Explode, PlacementSound::Explode]
        );
    }
}
