use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    use crate::paths::test_asset_root;
    use std::time::{SystemTime, UNIX_EPOCH};

    macro_rules! assert_sprite_bottom_centered {
        ($frame:ident, $center:ident) => {{
            assert!(
                // A half logical pixel of authored-art asymmetry is within the
                // export's 8x sampling precision; the manifest anchor remains exact.
                $center.is_some_and(|center| ($frame.w as f32 * 0.5 - center).abs() <= 4.0),
                "replacement sprite {} artwork should remain centered on its footprint",
                $frame.id
            );
        }};
    }

    macro_rules! assert_eight_x_dimensions {
        ($frame:ident) => {{
            let logical = $frame.logical_size.expect("replacement logical size");
            assert_eq!(
                ($frame.w, $frame.h),
                (logical[0] * 8, logical[1] * 8),
                "replacement sprite {} uses 8x source dimensions",
                $frame.id
            );
        }};
    }

    fn scratch(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("sc2k-assets-{name}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn newspaper_pictures_load_both_editions_without_using_traffic_ids() {
        let root = scratch("newspaper");
        let mac = root.join("mac");
        let windows = root.join("windows");
        fs::create_dir_all(mac.join("pict")).unwrap();
        fs::create_dir_all(windows.join("bitmaps_png")).unwrap();
        let mac_ink = Rgba([35, 35, 35, 255]);
        let windows_ink = Rgba([150, 150, 150, 255]);
        RgbaImage::from_pixel(155, 100, mac_ink)
            .save(mac.join("pict/PICT_00400.png"))
            .unwrap();
        for id in [400, 411] {
            RgbaImage::from_pixel(154, 100, windows_ink)
                .save(windows.join(format!("bitmaps_png/{id}.png")))
                .unwrap();
        }
        for (roots, expected) in [
            (vec![mac.clone(), windows.clone()], mac_ink),
            (vec![windows.clone(), mac.clone()], windows_ink),
        ] {
            let source = load_newspaper_pictures(&roots).unwrap().unwrap();
            let image = source.load_rgba().unwrap();
            assert_eq!(source.manifest.frames.len(), 2);
            assert!(source.frame_by_id(400).is_none());
            let first = source.frame_by_id(NEWSPAPER_PICTURE_BASE + 400).unwrap();
            assert_eq!(*image.get_pixel(first.x, first.y), expected);
            let last = source.frame_by_id(NEWSPAPER_PICTURE_BASE + 411).unwrap();
            assert_eq!(*image.get_pixel(last.x, last.y), windows_ink);
            for frame in &source.manifest.frames {
                assert!(frame.x + frame.w <= image.width());
                assert!(frame.y + frame.h <= image.height());
            }
        }
        assert!(load_newspaper_pictures(&[root.clone()]).unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parses_sprite_atlas_manifest() {
        let Some(root) = test_asset_root() else {
            return;
        };
        let atlas = AtlasSource::load(root.join("sprites").join("SPRT_00129_ShapeData"))
            .expect("load atlas");

        assert_eq!(atlas.manifest.frames.len(), 501);
        assert!(atlas.frame_by_id(1256).is_some());
        assert!(atlas.image_path.ends_with("atlas.png"));
    }

    #[test]
    fn atlas_rects_are_nonzero() {
        let Some(root) = test_asset_root() else {
            return;
        };
        let atlas = AtlasSource::load(root.join("sprites").join("SPRT_00130_ShapeData"))
            .expect("load atlas");

        for frame in &atlas.manifest.frames {
            assert!(frame.w > 0);
            assert!(frame.h > 0);
        }
    }

    #[test]
    fn pc_manifest_without_indices_uses_manifest_order() {
        let root = scratch("pc-atlas-order");
        RgbaImage::from_pixel(2, 1, Rgba([0, 0, 0, 0]))
            .save(root.join("atlas.png"))
            .unwrap();
        fs::write(
            root.join("atlas.json"),
            r#"{
                "image": "atlas.png",
                "frames": [
                    {"file":"a.png","id":1,"source_offset":0,"x":0,"y":0,"w":1,"h":1},
                    {"file":"b.png","id":2,"source_offset":1,"x":1,"y":0,"w":1,"h":1}
                ]
            }"#,
        )
        .unwrap();

        let source = AtlasSource::load(&root).unwrap();
        assert_eq!(source.frame_by_id(1).unwrap().index, 0);
        assert_eq!(source.frame_by_id(2).unwrap().index, 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reviewed_replacement_sprites_build_a_runtime_atlas() {
        let root = scratch("replacement");
        let sprite_path = root.join("water.png");
        RgbaImage::from_pixel(3, 2, Rgba([12, 34, 56, 255]))
            .save(&sprite_path)
            .unwrap();
        fs::write(
            root.join("asset-pack.json"),
            r#"{
                "schema_version": 1,
                "id": "ours",
                "name": "Our graphics",
                "kind": "replacement",
                "author": "Test",
                "license": "CC0",
                "assets": [{
                    "key": "citygame.sprite.01270",
                    "file": "water.png",
                    "status": "reviewed",
                    "logical_size": [3, 2],
                    "anchor": [1.5, 2.0],
                    "animation": "Static test replacement."
                }]
            }"#,
        )
        .unwrap();

        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed sprite atlas");
        let frame = source.frame_by_id(1270).expect("replacement frame");
        assert_eq!((frame.w, frame.h), (3, 2));
        assert_eq!(frame.logical_size, Some([3, 2]));
        assert_eq!(frame.anchor, Some([1.5, 2.0]));
        assert_eq!(
            source.load_rgba().unwrap().get_pixel(0, 0).0,
            [12, 34, 56, 255]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires CITYGAME_TEST_GEOMETRY_DIR pointing to locally extracted geometry references"]
    fn authored_roads_match_locally_supplied_geometry() {
        let reference_root = std::env::var_os("CITYGAME_TEST_GEOMETRY_DIR")
            .map(std::path::PathBuf::from)
            .expect("set CITYGAME_TEST_GEOMETRY_DIR to locally extracted geometry references");
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let atlas_image = source.load_rgba().unwrap();
        // Compare the complete silhouette and lane/material placement against
        // measured imported geometry. A full diamond can join itself while still
        // being incompatible with bends and slopes, so canvas-only tests are insufficient.
        let mut geometry: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(reference_root.join("highway-reference-geometry.json"))
                .expect("read locally supplied geometry reference"),
        )
        .unwrap();
        assert_eq!(geometry.as_array().unwrap().len(), 23);
        let road_slopes: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(reference_root.join("road-slope-reference-geometry.json"))
                .expect("read locally supplied geometry reference"),
        )
        .unwrap();
        assert_eq!(road_slopes.as_array().unwrap().len(), 6);
        geometry
            .as_array_mut()
            .unwrap()
            .extend(road_slopes.as_array().unwrap().iter().cloned());
        for reference in geometry.as_array().unwrap() {
            let id = reference["id"].as_u64().unwrap() as u32;
            let frame = source
                .frame_by_id(id)
                .expect("calibrated highway replacement");
            let width = reference["logical_size"][0].as_u64().unwrap() as u32;
            let height = reference["logical_size"][1].as_u64().unwrap() as u32;
            assert_eq!(frame.logical_size, Some([width, height]));
            assert_eq!((frame.w, frame.h), (width * 8, height * 8));
            assert_eq!(frame.anchor, Some([width as f32 / 2.0, height as f32]));
            for y in 0..height {
                let row = |key: &str| {
                    u64::from_str_radix(reference[key][y as usize].as_str().unwrap(), 16).unwrap()
                };
                let opaque = row("opaque_rows");
                let asphalt = row("asphalt_rows");
                let lanes = row("lane_rows");
                for x in 0..width {
                    let pixel = atlas_image
                        .get_pixel(frame.x + x * 8 + 4, frame.y + y * 8 + 4)
                        .0;
                    assert_eq!(
                        pixel[3] > 128,
                        opaque & (1 << x) != 0,
                        "highway {id} silhouette moved at ({x}, {y})"
                    );
                    if lanes & (1 << x) != 0 {
                        assert!(
                            pixel[0] > 200 && pixel[1] > 200 && pixel[2] > 200,
                            "highway {id} lane marking moved at ({x}, {y})"
                        );
                    }
                    if asphalt & (1 << x) != 0 {
                        assert!(
                            pixel[0] < 150 && pixel[1] < 160 && pixel[2] < 170,
                            "highway {id} deck moved at ({x}, {y})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn bundled_replacement_sprites_are_runtime_ready() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        for id in 1400..=1449 {
            assert!(
                source.frame_by_id(id).is_none(),
                "high-resolution car replacement {id} should stay removed"
            );
        }
        // Large-bank XBLD 0x0e..=0x1c are power-line variants. Former tree
        // art used these IDs and obscured the actual connected-wire sprites.
        for id in 1014..=1028 {
            assert!(
                source.frame_by_id(id).is_none(),
                "power-line sprite {id} should use the imported artwork"
            );
        }
        let atlas_image = source.load_rgba().unwrap();
        assert!(atlas_image.width() <= 6656);
        assert!(atlas_image.height() <= 16_384);
        for (id, expected_size) in [
            (1073, [32, 31]),
            (1074, [32, 31]),
            (1075, [32, 31]),
            (1076, [32, 31]),
            (1077, [32, 31]),
            (1078, [32, 31]),
            (1079, [32, 31]),
            (1080, [32, 31]),
            (1097, [64, 59]),
            (1098, [64, 59]),
            (1099, [64, 47]),
            (1100, [64, 47]),
            (1101, [64, 46]),
            (1102, [64, 33]),
            (1103, [64, 46]),
            (1104, [64, 47]),
            (1105, [64, 49]),
            (1081, [32, 48]),
            (1082, [32, 67]),
            (1083, [32, 67]),
            (1084, [32, 52]),
            (1085, [32, 37]),
            (1086, [32, 39]),
            (1087, [32, 31]),
            (1088, [32, 31]),
            (1089, [32, 43]),
            (1090, [32, 27]),
            (1091, [32, 27]),
            (1092, [32, 35]),
            (1093, [32, 32]),
            (1094, [32, 29]),
            (1095, [32, 21]),
            (1096, [32, 21]),
            (1106, [64, 63]),
            (1107, [64, 63]),
            (1256, [32, 16]),
            (1267, [32, 17]),
            (1270, [32, 16]),
            (1271, [32, 16]),
            (1272, [32, 16]),
            (1273, [32, 16]),
            (1274, [32, 16]),
            (1275, [32, 16]),
            (1276, [32, 16]),
            (1277, [32, 16]),
            (1278, [32, 16]),
            (1279, [32, 16]),
            (1280, [32, 16]),
            (1281, [32, 16]),
            (1282, [32, 16]),
            (1283, [32, 16]),
            (1285, [32, 16]),
            (1286, [32, 16]),
            (1287, [32, 16]),
            (1288, [32, 16]),
            (1289, [32, 16]),
            (1290, [32, 16]),
            (1291, [32, 16]),
            (1292, [32, 16]),
            (1293, [32, 16]),
            (1294, [32, 16]),
            (1295, [32, 16]),
            (1296, [32, 16]),
            (1297, [32, 16]),
            (1298, [32, 16]),
            (1299, [32, 16]),
            (1300, [32, 16]),
            (1301, [32, 16]),
            (1302, [32, 16]),
            (1303, [32, 16]),
            (1304, [32, 16]),
            (1305, [32, 18]),
            (1306, [32, 30]),
            (1307, [32, 30]),
            (1308, [32, 21]),
            (1309, [32, 21]),
            (1310, [32, 30]),
            (1311, [32, 29]),
            (1312, [32, 21]),
            (1313, [32, 29]),
            (1314, [32, 30]),
            (1315, [32, 22]),
            (1316, [32, 40]),
            (1317, [32, 22]),
            (1318, [32, 29]),
            (1319, [32, 34]),
            (1320, [32, 34]),
            (1321, [32, 46]),
            (1322, [32, 46]),
            (1323, [32, 30]),
            (1324, [32, 30]),
            (1325, [32, 34]),
            (1326, [32, 34]),
            (1327, [32, 34]),
            (1328, [32, 26]),
            (1329, [32, 34]),
            (1330, [32, 34]),
            (1331, [32, 34]),
            (1332, [32, 34]),
            (1333, [32, 34]),
            (1334, [32, 18]),
            (1335, [32, 18]),
            (1336, [32, 30]),
            (1337, [32, 30]),
            (1338, [32, 21]),
            (1339, [32, 21]),
            (1340, [32, 18]),
            (1341, [32, 18]),
            (1342, [32, 18]),
            (1343, [32, 18]),
            (1344, [32, 18]),
            (1345, [32, 18]),
            (1346, [32, 18]),
            (1347, [32, 18]),
            (1348, [32, 18]),
            (1349, [32, 34]),
            (1350, [32, 34]),
            (1351, [32, 19]),
            (1352, [32, 28]),
            (1353, [32, 39]),
            (1354, [32, 17]),
            (1355, [32, 17]),
            (1356, [32, 17]),
            (1357, [32, 17]),
            (1358, [32, 17]),
            (1001, [32, 17]),
            (1002, [32, 17]),
            (1003, [32, 17]),
            (1004, [32, 17]),
            (1006, [32, 19]),
            (1007, [32, 21]),
            (1008, [32, 24]),
            (1009, [32, 25]),
            (1010, [32, 23]),
            (1011, [32, 25]),
            (1012, [32, 24]),
            (1013, [32, 23]),
            (1029, [32, 16]),
            (1030, [32, 16]),
            (1031, [32, 29]),
            (1032, [32, 29]),
            (1033, [32, 21]),
            (1034, [32, 21]),
            (1108, [32, 20]),
            (1109, [32, 20]),
            (1110, [32, 22]),
            (1111, [32, 22]),
            (1112, [32, 18]),
            (1113, [32, 18]),
            (1114, [32, 18]),
            (1115, [32, 17]),
            (1116, [32, 17]),
            (1117, [32, 17]),
            (1118, [32, 17]),
            (1119, [32, 17]),
        ] {
            let frame = source.frame_by_id(id).expect("terrain replacement sprite");
            let expected_pixels = (expected_size[0] as u32 * 8, expected_size[1] as u32 * 8);
            assert_eq!((frame.w, frame.h), expected_pixels);
            assert_eq!(frame.logical_size, Some(expected_size));
            assert_eq!(
                frame.anchor,
                Some([expected_size[0] as f32 / 2.0, expected_size[1] as f32])
            );
        }
        for (id, logical_size) in [
            (1359, [64, 27]),
            (1360, [64, 19]),
            (1361, [64, 19]),
            (1362, [64, 23]),
            (1363, [64, 31]),
        ] {
            let frame = source
                .frame_by_id(id)
                .expect("ambient aircraft replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(frame.anchor, Some([32.0, logical_size[1] as f32]));
        }
        for (id, logical_size) in [
            (1364, [32, 20]),
            (1365, [32, 13]),
            (1366, [32, 12]),
            (1367, [32, 17]),
            (1368, [32, 21]),
        ] {
            let frame = source
                .frame_by_id(id)
                .expect("ambient helicopter replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(frame.anchor, Some([16.0, logical_size[1] as f32]));
        }
        for (id, logical_size) in [
            (1369, [64, 49]),
            (1370, [64, 41]),
            (1371, [64, 35]),
            (1372, [64, 42]),
            (1373, [64, 54]),
        ] {
            let frame = source
                .frame_by_id(id)
                .expect("ambient ship replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(frame.anchor, Some([32.0, logical_size[1] as f32]));
        }
        for (id, logical_size) in [
            (1374, [32, 20]),
            (1375, [32, 21]),
            (1376, [32, 19]),
            (1377, [32, 33]),
            (1378, [32, 28]),
        ] {
            let frame = source.frame_by_id(id).expect("train replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(frame.anchor, Some([16.0, logical_size[1] as f32]));
        }
        for (id, logical_size) in [
            (1379, [32, 16]),
            (1380, [32, 22]),
            (1381, [32, 22]),
            (1382, [32, 70]),
            (1383, [32, 78]),
            (1384, [32, 69]),
        ] {
            let frame = source
                .frame_by_id(id)
                .expect("special ambient or dispatch replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(frame.anchor, Some([16.0, logical_size[1] as f32]));
        }
        for (id, logical_size) in [
            (1385, [32, 91]),
            (1386, [32, 16]),
            (1387, [64, 35]),
            (1388, [64, 41]),
            (1389, [64, 40]),
        ] {
            let frame = source.frame_by_id(id).expect("effect replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([(logical_size[0] / 2) as f32, logical_size[1] as f32])
            );
        }
        for (id, logical_size) in [
            (1390, [32, 21]),
            (1391, [32, 21]),
            (1392, [32, 64]),
            (1393, [32, 49]),
            (1394, [32, 39]),
            (1395, [32, 32]),
        ] {
            let frame = source
                .frame_by_id(id)
                .expect("bulldozer or dust replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(frame.anchor, Some([16.0, logical_size[1] as f32]));
        }
        for (id, logical_size) in [
            (1396, [32, 24]),
            (1397, [32, 20]),
            (1398, [32, 23]),
            (1399, [32, 22]),
        ] {
            let frame = source.frame_by_id(id).expect("fire animation sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(frame.anchor, Some([16.0, logical_size[1] as f32]));
        }
        for (id, logical_size) in [
            (1450, [32, 18]),
            (1451, [32, 18]),
            (1452, [32, 30]),
            (1453, [32, 30]),
            (1454, [32, 21]),
        ] {
            let frame = source
                .frame_by_id(id)
                .expect("watered-pipe replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([logical_size[0] as f32 / 2.0, logical_size[1] as f32])
            );
        }
        for (id, logical_size) in [
            (1455, [32, 21]),
            (1456, [32, 18]),
            (1457, [32, 18]),
            (1458, [32, 18]),
            (1459, [32, 18]),
        ] {
            let frame = source
                .frame_by_id(id)
                .expect("watered-pipe slope or turn replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([logical_size[0] as f32 / 2.0, logical_size[1] as f32])
            );
        }
        for id in 1460..=1464 {
            let frame = source
                .frame_by_id(id)
                .expect("watered-pipe junction replacement sprite");
            assert_eight_x_dimensions!(frame);
            assert_eq!(frame.logical_size, Some([32, 18]));
            assert_eq!(frame.anchor, Some([16.0, 18.0]));
        }
        for (id, logical_size) in [(1465, [32, 34]), (1466, [32, 34]), (1467, [32, 19])] {
            let frame = source
                .frame_by_id(id)
                .expect("waterworks replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([logical_size[0] as f32 / 2.0, logical_size[1] as f32])
            );
        }
        for id in 1468..=1472 {
            let frame = source
                .frame_by_id(id)
                .expect("water-status pulse replacement sprite");
            assert_eight_x_dimensions!(frame);
            assert_eq!(frame.logical_size, Some([32, 17]));
            assert_eq!(frame.anchor, Some([16.0, 17.0]));
        }
        for id in 1473..=1477 {
            let frame = source
                .frame_by_id(id)
                .expect("water-status overlay replacement sprite");
            assert_eight_x_dimensions!(frame);
            assert_eq!(frame.logical_size, Some([32, 17]));
            assert_eq!(frame.anchor, Some([16.0, 17.0]));
        }
        let radioactive_waste = source
            .frame_by_id(1005)
            .expect("reviewed radioactive-waste replacement sprite");
        assert_eight_x_dimensions!(radioactive_waste);
        assert_eq!(radioactive_waste.logical_size, Some([32, 17]));
        assert_eq!(radioactive_waste.anchor, Some([16.0, 17.0]));
        for (id, height) in [
            (1478, 39),
            (1479, 86),
            (1480, 67),
            (1481, 58),
            (1482, 25),
            (1483, 50),
            (1484, 39),
            (1485, 65),
            (1486, 75),
            (1487, 70),
            (1488, 44),
            (1489, 48),
            (1490, 90),
            (1491, 90),
        ] {
            let frame = source.frame_by_id(id).expect("monster-part replacement");
            assert_eq!((frame.w, frame.h), (512, height * 8));
            assert_eq!(frame.logical_size, Some([64, height]));
            assert_eq!(frame.anchor, Some([32.0, height as f32]));
        }
        for (id, logical_size) in [(1497, [64, 125]), (1498, [64, 125]), (1499, [64, 126])] {
            let frame = source
                .frame_by_id(id)
                .expect("tornado animation replacement frame");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([logical_size[0] as f32 / 2.0, logical_size[1] as f32])
            );
        }
        for (id, logical_size) in [(1252, [128, 182]), (1253, [128, 210]), (1254, [128, 235])] {
            let frame = source
                .frame_by_id(id)
                .expect("special landmark replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([logical_size[0] as f32 / 2.0, logical_size[1] as f32])
            );
        }
        for (id, logical_size) in [
            (1156, [64, 72]),
            (1158, [64, 72]),
            (1162, [64, 53]),
            (1164, [64, 61]),
            (1167, [64, 44]),
            (1168, [64, 44]),
            (1170, [64, 46]),
            (1171, [64, 53]),
            (1172, [64, 61]),
            (1173, [64, 73]),
            (1174, [96, 77]),
            (1175, [96, 88]),
            (1176, [96, 58]),
            (1177, [96, 96]),
            (1178, [96, 61]),
            (1179, [96, 91]),
            (1180, [96, 65]),
            (1181, [96, 90]),
            (1182, [96, 58]),
            (1183, [96, 117]),
            (1187, [96, 137]),
            (1188, [96, 50]),
            (1190, [96, 55]),
            (1191, [96, 63]),
            (1192, [96, 56]),
            (1193, [96, 49]),
            (1194, [96, 85]),
            (1195, [96, 65]),
            (1196, [96, 74]),
            (1197, [96, 90]),
            (1140, [64, 34]),
            (1141, [64, 39]),
            (1142, [64, 36]),
            (1143, [64, 44]),
            (1144, [64, 53]),
            (1145, [64, 58]),
            (1146, [64, 78]),
            (1147, [64, 77]),
            (1148, [64, 33]),
            (1149, [64, 37]),
            (1150, [64, 42]),
            (1151, [64, 46]),
            (1152, [64, 46]),
            (1153, [64, 51]),
            (1154, [64, 70]),
            (1155, [64, 66]),
            (1157, [64, 84]),
            (1159, [64, 38]),
            (1160, [64, 45]),
            (1161, [64, 35]),
            (1163, [64, 56]),
            (1165, [64, 66]),
            (1166, [64, 33]),
            (1169, [64, 65]),
            (1184, [96, 123]),
            (1185, [96, 78]),
            (1186, [96, 108]),
            (1189, [96, 86]),
            (1198, [32, 32]),
            (1199, [32, 32]),
            (1120, [32, 19]),
            (1121, [32, 19]),
            (1122, [32, 20]),
            (1123, [32, 21]),
            (1124, [32, 23]),
            (1125, [32, 27]),
            (1126, [32, 20]),
            (1127, [32, 18]),
            (1128, [32, 32]),
            (1129, [32, 43]),
            (1130, [32, 25]),
            (1131, [32, 34]),
            (1132, [32, 21]),
            (1133, [32, 28]),
            (1134, [32, 20]),
            (1135, [32, 26]),
            (1136, [32, 18]),
            (1137, [32, 18]),
            (1138, [32, 17]),
            (1139, [32, 17]),
            (1200, [32, 48]),
            (1202, [128, 115]),
            (1205, [128, 95]),
        ] {
            let frame = source
                .frame_by_id(id)
                .expect("utility-building replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8),
                "replacement sprite {id} dimensions"
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([logical_size[0] as f32 / 2.0, logical_size[1] as f32])
            );
        }
        for (id, logical_size) in [(1224, [32, 40]), (1226, [32, 69])] {
            let frame = source
                .frame_by_id(id)
                .expect("infrastructure replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([logical_size[0] as f32 / 2.0, logical_size[1] as f32])
            );
        }
        for (id, logical_size) in [(1221, [32, 17]), (1222, [32, 17]), (1223, [32, 21])] {
            let frame = source.frame_by_id(id).expect("road replacement tile");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([logical_size[0] as f32 / 2.0, logical_size[1] as f32])
            );
        }
        for (id, logical_size) in [(1219, [32, 42]), (1239, [64, 37])] {
            let frame = source
                .frame_by_id(id)
                .expect("civic infrastructure replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([logical_size[0] as f32 / 2.0, logical_size[1] as f32])
            );
        }
        for (id, logical_size) in [(1229, [32, 22]), (1230, [32, 17]), (1231, [32, 18])] {
            let frame = source
                .frame_by_id(id)
                .expect("service and aircraft replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([logical_size[0] as f32 / 2.0, logical_size[1] as f32])
            );
        }
        for (id, logical_size) in [
            (1492, [32, 19]),
            (1493, [32, 24]),
            (1494, [32, 25]),
            (1496, [64, 51]),
        ] {
            let frame = source
                .frame_by_id(id)
                .expect("disaster-effect replacement sprite");
            assert_eq!(
                (frame.w, frame.h),
                (logical_size[0] * 8, logical_size[1] * 8)
            );
            assert_eq!(frame.logical_size, Some(logical_size));
            assert_eq!(
                frame.anchor,
                Some([logical_size[0] as f32 / 2.0, logical_size[1] as f32])
            );
        }
        let flash = source
            .frame_by_id(1495)
            .expect("special-effect replacement sprite");
        assert_eight_x_dimensions!(flash);
        assert_eq!(flash.logical_size, Some([32, 15]));
        assert_eq!(flash.anchor, Some([16.0, 15.0]));
        for id in [1257, 1258, 1261, 1262, 1264, 1265, 1269] {
            let frame = source
                .frame_by_id(id)
                .expect("raised terrain replacement sprite");
            assert_eight_x_dimensions!(frame);
            assert_eq!(frame.logical_size, Some([32, 29]));
            assert_eq!(frame.anchor, Some([16.0, 29.0]));
        }
        let waterfall = source
            .frame_by_id(1284)
            .expect("waterfall replacement sprite");
        assert_eight_x_dimensions!(waterfall);
        assert_eq!(waterfall.logical_size, Some([32, 29]));
        assert_eq!(waterfall.anchor, Some([16.0, 29.0]));
        for id in [1259, 1260, 1263, 1266, 1268] {
            let frame = source
                .frame_by_id(id)
                .expect("diagonal cliff replacement sprite");
            assert_eight_x_dimensions!(frame);
            assert_eq!(frame.logical_size, Some([32, 21]));
            assert_eq!(frame.anchor, Some([16.0, 21.0]));
        }
        let water_markers = image::open(root.join("terrain/water-open@8x.png"))
            .unwrap()
            .to_rgba8();
        for color in [
            [0, 76, 25, 255],
            [0, 99, 25, 255],
            [0, 127, 25, 255],
            [0, 151, 23, 255],
            [0, 179, 25, 255],
            [0, 207, 23, 255],
            [0, 227, 48, 255],
            [0, 255, 25, 255],
        ] {
            assert!(water_markers.pixels().any(|pixel| pixel.0 == color));
        }
        let nuclear = source.frame_by_id(1203).expect("nuclear power sprite");
        assert_eight_x_dimensions!(nuclear);
        assert_eq!(nuclear.logical_size, Some([128, 81]));
        assert_eq!(nuclear.anchor, Some([64.0, 81.0]));
        let coal = source.frame_by_id(1207).expect("coal power sprite");
        assert_eight_x_dimensions!(coal);
        assert_eq!(coal.logical_size, Some([128, 106]));
        assert_eq!(coal.anchor, Some([64.0, 106.0]));
        let gas_power = source.frame_by_id(1201).expect("gas power sprite");
        assert_eight_x_dimensions!(gas_power);
        assert_eq!(gas_power.logical_size, Some([128, 86]));
        assert_eq!(gas_power.anchor, Some([64.0, 86.0]));
        let solar_power = source.frame_by_id(1204).expect("solar power sprite");
        assert_eight_x_dimensions!(solar_power);
        assert_eq!(solar_power.logical_size, Some([128, 86]));
        assert_eq!(solar_power.anchor, Some([64.0, 86.0]));
        let fusion_power = source.frame_by_id(1206).expect("fusion power sprite");
        assert_eight_x_dimensions!(fusion_power);
        assert_eq!(fusion_power.logical_size, Some([128, 96]));
        assert_eq!(fusion_power.anchor, Some([64.0, 96.0]));
        let stadium = source.frame_by_id(1215).expect("stadium sprite");
        assert_eight_x_dimensions!(stadium);
        assert_eq!(stadium.logical_size, Some([128, 83]));
        assert_eq!(stadium.anchor, Some([64.0, 83.0]));
        let prison = source.frame_by_id(1216).expect("prison sprite");
        assert_eight_x_dimensions!(prison);
        assert_eq!(prison.logical_size, Some([128, 80]));
        assert_eq!(prison.anchor, Some([64.0, 80.0]));
        let zoo = source.frame_by_id(1218).expect("zoo sprite");
        assert_eight_x_dimensions!(zoo);
        assert_eq!(zoo.logical_size, Some([128, 80]));
        assert_eq!(zoo.anchor, Some([64.0, 80.0]));
        let city_hall = source.frame_by_id(1208).expect("City Hall sprite");
        assert_eight_x_dimensions!(city_hall);
        assert_eq!(city_hall.logical_size, Some([96, 80]));
        assert_eq!(city_hall.anchor, Some([48.0, 80.0]));
        let museum = source.frame_by_id(1212).expect("museum sprite");
        assert_eight_x_dimensions!(museum);
        assert_eq!(museum.logical_size, Some([96, 80]));
        assert_eq!(museum.anchor, Some([48.0, 80.0]));
        let college = source.frame_by_id(1217).expect("college sprite");
        assert_eight_x_dimensions!(college);
        assert_eq!(college.logical_size, Some([128, 85]));
        assert_eq!(college.anchor, Some([64.0, 85.0]));
        let library = source.frame_by_id(1245).expect("library sprite");
        assert_eight_x_dimensions!(library);
        assert_eq!(library.logical_size, Some([64, 56]));
        assert_eq!(library.anchor, Some([32.0, 56.0]));
        let church = source.frame_by_id(1247).expect("church sprite");
        assert_eight_x_dimensions!(church);
        assert_eq!(church.logical_size, Some([64, 72]));
        assert_eq!(church.anchor, Some([32.0, 72.0]));
        let water_treatment = source.frame_by_id(1244).expect("water treatment sprite");
        assert_eight_x_dimensions!(water_treatment);
        assert_eq!(water_treatment.logical_size, Some([64, 56]));
        assert_eq!(water_treatment.anchor, Some([32.0, 56.0]));
        let mayor_house = source.frame_by_id(1243).expect("Mayor's House sprite");
        assert_eight_x_dimensions!(mayor_house);
        assert_eq!(mayor_house.logical_size, Some([64, 72]));
        assert_eq!(mayor_house.anchor, Some([32.0, 72.0]));
        let cargo_yard = source.frame_by_id(1242).expect("Cargo Yard sprite");
        assert_eight_x_dimensions!(cargo_yard);
        assert_eq!(cargo_yard.logical_size, Some([64, 56]));
        assert_eq!(cargo_yard.anchor, Some([32.0, 56.0]));
        let water_tower = source.frame_by_id(1235).expect("Water Tower sprite");
        assert_eight_x_dimensions!(water_tower);
        assert_eq!(water_tower.logical_size, Some([64, 96]));
        assert_eq!(water_tower.anchor, Some([32.0, 96.0]));
        let bus_depot = source.frame_by_id(1236).expect("Bus Depot sprite");
        assert_eight_x_dimensions!(bus_depot);
        assert_eq!(bus_depot.logical_size, Some([64, 56]));
        assert_eq!(bus_depot.anchor, Some([32.0, 56.0]));
        let rail_depot = source.frame_by_id(1237).expect("Rail Depot sprite");
        assert_eight_x_dimensions!(rail_depot);
        assert_eq!(rail_depot.logical_size, Some([64, 64]));
        assert_eq!(rail_depot.anchor, Some([32.0, 64.0]));
        let loading_bay = source.frame_by_id(1240).expect("Loading Bay sprite");
        assert_eight_x_dimensions!(loading_bay);
        assert_eq!(loading_bay.logical_size, Some([64, 56]));
        assert_eq!(loading_bay.anchor, Some([32.0, 56.0]));
        let large_hangar = source.frame_by_id(1246).expect("Large Hangar sprite");
        assert_eight_x_dimensions!(large_hangar);
        assert_eq!(large_hangar.logical_size, Some([64, 56]));
        assert_eq!(large_hangar.anchor, Some([32.0, 56.0]));
        let arcology = source.frame_by_id(1251).expect("Arcology sprite");
        assert_eight_x_dimensions!(arcology);
        assert_eq!(arcology.logical_size, Some([128, 144]));
        assert_eq!(arcology.anchor, Some([64.0, 144.0]));
        let dome = source.frame_by_id(1254).expect("Biodome sprite");
        let eiffel_tower = source.frame_by_id(1255).expect("Eiffel Tower sprite");
        assert_eight_x_dimensions!(eiffel_tower);
        assert_eq!(eiffel_tower.logical_size, Some([128, 154]));
        assert_eq!(eiffel_tower.anchor, Some([64.0, 154.0]));
        let marina = source.frame_by_id(1248).expect("Marina sprite");
        assert_eight_x_dimensions!(marina);
        assert_eq!(marina.logical_size, Some([96, 72]));
        assert_eq!(marina.anchor, Some([48.0, 72.0]));
        let missile_silo = source.frame_by_id(1249).expect("Missile Silo sprite");
        assert_eight_x_dimensions!(missile_silo);
        assert_eq!(missile_silo.logical_size, Some([96, 80]));
        assert_eq!(missile_silo.anchor, Some([48.0, 80.0]));
        let desalination = source.frame_by_id(1250).expect("Desalination Plant sprite");
        assert_eight_x_dimensions!(desalination);
        assert_eq!(desalination.logical_size, Some([96, 80]));
        assert_eq!(desalination.anchor, Some([48.0, 80.0]));
        let control_tower = source.frame_by_id(1225).expect("Control Tower sprite");
        assert_eight_x_dimensions!(control_tower);
        assert_eq!(control_tower.logical_size, Some([48, 96]));
        assert_eq!(control_tower.anchor, Some([24.0, 96.0]));
        let subway_station = source.frame_by_id(1233).expect("Subway Station sprite");
        assert_eight_x_dimensions!(subway_station);
        assert_eq!(subway_station.logical_size, Some([32, 40]));
        assert_eq!(subway_station.anchor, Some([16.0, 40.0]));
        let warehouse = source.frame_by_id(1227).expect("Warehouse sprite");
        assert_eight_x_dimensions!(warehouse);
        assert_eq!(warehouse.logical_size, Some([32, 32]));
        assert_eq!(warehouse.anchor, Some([16.0, 32.0]));
        let military_facility = source.frame_by_id(1241).expect("Military Facility sprite");
        assert_eight_x_dimensions!(military_facility);
        assert_eq!(military_facility.logical_size, Some([64, 56]));
        assert_eq!(military_facility.anchor, Some([32.0, 56.0]));
        let water_pump = source.frame_by_id(1220).expect("Water Pump sprite");
        assert_eight_x_dimensions!(water_pump);
        assert_eq!(water_pump.logical_size, Some([32, 40]));
        assert_eq!(water_pump.anchor, Some([16.0, 40.0]));
        let airport_building = source.frame_by_id(1228).expect("Airport Building sprite");
        assert_eight_x_dimensions!(airport_building);
        assert_eq!(airport_building.logical_size, Some([32, 32]));
        assert_eq!(airport_building.anchor, Some([16.0, 32.0]));
        let radar_station = source.frame_by_id(1234).expect("Radar Station sprite");
        assert_eight_x_dimensions!(radar_station);
        assert_eq!(radar_station.logical_size, Some([32, 40]));
        assert_eq!(radar_station.anchor, Some([16.0, 40.0]));
        let botanical_conservatory = source
            .frame_by_id(1213)
            .expect("Botanical Conservatory sprite");
        assert_eight_x_dimensions!(botanical_conservatory);
        assert_eq!(botanical_conservatory.logical_size, Some([96, 80]));
        assert_eq!(botanical_conservatory.anchor, Some([48.0, 80.0]));
        let parking_garage = source.frame_by_id(1238).expect("Parking Garage sprite");
        assert_eight_x_dimensions!(parking_garage);
        assert_eq!(parking_garage.logical_size, Some([64, 56]));
        assert_eq!(parking_garage.anchor, Some([32.0, 56.0]));
        let military_hangar = source.frame_by_id(1232).expect("Military Hangar sprite");
        assert_eight_x_dimensions!(military_hangar);
        assert_eq!(military_hangar.logical_size, Some([32, 32]));
        assert_eq!(military_hangar.anchor, Some([16.0, 32.0]));
        let atlas = source.load_rgba().unwrap();
        for frame in [
            nuclear,
            coal,
            gas_power,
            solar_power,
            fusion_power,
            city_hall,
            museum,
            stadium,
            prison,
            college,
            library,
            church,
            water_treatment,
            mayor_house,
            cargo_yard,
            water_tower,
            bus_depot,
            rail_depot,
            loading_bay,
            large_hangar,
            arcology,
            dome,
            marina,
            missile_silo,
            desalination,
            control_tower,
            subway_station,
            warehouse,
            military_facility,
            water_pump,
            airport_building,
            radar_station,
            botanical_conservatory,
            parking_garage,
            military_hangar,
        ] {
            assert!(frame.x + frame.w <= atlas.width());
            assert!(frame.y + frame.h <= atlas.height());
            assert!(
                (frame.y..frame.y + frame.h).any(|y| {
                    (frame.x..frame.x + frame.w).any(|x| atlas.get_pixel(x, y).0[3] > 0)
                })
            );
            let (logical_w, logical_h) = frame.display_size();
            assert_eq!(
                frame.anchor,
                Some([logical_w as f32 * 0.5, logical_h as f32]),
                "multi-tile building sprites use the engine's bottom-center footprint anchor"
            );
        }

        let coal_bottom_center = (coal.y..coal.y + coal.h).rev().find_map(|y| {
            let visible: Vec<_> = (coal.x..coal.x + coal.w)
                .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                .collect();
            visible
                .first()
                .zip(visible.last())
                .map(|(&first, &last)| (first + last) as f32 * 0.5 - coal.x as f32)
        });
        assert!(
            coal_bottom_center.is_some_and(|center| (center - coal.w as f32 * 0.5).abs() <= 2.0),
            "coal sprite artwork should remain centered on its footprint"
        );

        let gas_power_bottom_center =
            (gas_power.y..gas_power.y + gas_power.h)
                .rev()
                .find_map(|y| {
                    let visible: Vec<_> = (gas_power.x..gas_power.x + gas_power.w)
                        .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                        .collect();
                    visible
                        .first()
                        .zip(visible.last())
                        .map(|(&first, &last)| (first + last) as f32 * 0.5 - gas_power.x as f32)
                });
        assert!(
            gas_power_bottom_center
                .is_some_and(|center| (center - gas_power.w as f32 * 0.5).abs() <= 2.0),
            "gas_power sprite artwork should remain centered on its footprint"
        );

        let solar_power_bottom_center = (solar_power.y..solar_power.y + solar_power.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (solar_power.x..solar_power.x + solar_power.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - solar_power.x as f32)
            });
        assert!(
            solar_power_bottom_center
                .is_some_and(|center| (center - solar_power.w as f32 * 0.5).abs() <= 2.0),
            "solar_power sprite artwork should remain centered on its footprint"
        );

        let fusion_power_bottom_center = (fusion_power.y..fusion_power.y + fusion_power.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (fusion_power.x..fusion_power.x + fusion_power.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - fusion_power.x as f32)
            });
        assert!(
            fusion_power_bottom_center
                .is_some_and(|center| (center - fusion_power.w as f32 * 0.5).abs() <= 2.0),
            "fusion_power sprite artwork should remain centered on its footprint"
        );

        let conservatory_bottom_center = (botanical_conservatory.y
            ..botanical_conservatory.y + botanical_conservatory.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (botanical_conservatory.x
                    ..botanical_conservatory.x + botanical_conservatory.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible.first().zip(visible.last()).map(|(&first, &last)| {
                    (first + last) as f32 * 0.5 - botanical_conservatory.x as f32
                })
            });
        assert_sprite_bottom_centered!(botanical_conservatory, conservatory_bottom_center);

        let parking_garage_bottom_center = (parking_garage.y..parking_garage.y + parking_garage.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (parking_garage.x..parking_garage.x + parking_garage.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - parking_garage.x as f32)
            });
        assert_sprite_bottom_centered!(parking_garage, parking_garage_bottom_center);

        let stadium_bottom_center = (stadium.y..stadium.y + stadium.h).rev().find_map(|y| {
            let visible: Vec<_> = (stadium.x..stadium.x + stadium.w)
                .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                .collect();
            visible
                .first()
                .zip(visible.last())
                .map(|(&first, &last)| (first + last) as f32 * 0.5 - stadium.x as f32)
        });
        assert_sprite_bottom_centered!(stadium, stadium_bottom_center);

        let prison_bottom_center = (prison.y..prison.y + prison.h).rev().find_map(|y| {
            let visible: Vec<_> = (prison.x..prison.x + prison.w)
                .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                .collect();
            visible
                .first()
                .zip(visible.last())
                .map(|(&first, &last)| (first + last) as f32 * 0.5 - prison.x as f32)
        });
        assert_sprite_bottom_centered!(prison, prison_bottom_center);

        let military_hangar_bottom_center = (military_hangar.y
            ..military_hangar.y + military_hangar.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (military_hangar.x..military_hangar.x + military_hangar.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - military_hangar.x as f32)
            });
        assert_sprite_bottom_centered!(military_hangar, military_hangar_bottom_center);

        let city_hall_bottom_center =
            (city_hall.y..city_hall.y + city_hall.h)
                .rev()
                .find_map(|y| {
                    let visible: Vec<_> = (city_hall.x..city_hall.x + city_hall.w)
                        .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                        .collect();
                    visible
                        .first()
                        .zip(visible.last())
                        .map(|(&first, &last)| (first + last) as f32 * 0.5 - city_hall.x as f32)
                });
        assert_sprite_bottom_centered!(city_hall, city_hall_bottom_center);

        let museum_bottom_center = (museum.y..museum.y + museum.h).rev().find_map(|y| {
            let visible: Vec<_> = (museum.x..museum.x + museum.w)
                .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                .collect();
            visible
                .first()
                .zip(visible.last())
                .map(|(&first, &last)| (first + last) as f32 * 0.5 - museum.x as f32)
        });
        assert_sprite_bottom_centered!(museum, museum_bottom_center);

        let college_bottom_center = (college.y..college.y + college.h).rev().find_map(|y| {
            let visible: Vec<_> = (college.x..college.x + college.w)
                .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                .collect();
            visible
                .first()
                .zip(visible.last())
                .map(|(&first, &last)| (first + last) as f32 * 0.5 - college.x as f32)
        });
        assert_sprite_bottom_centered!(college, college_bottom_center);

        let library_bottom_center = (library.y..library.y + library.h).rev().find_map(|y| {
            let visible: Vec<_> = (library.x..library.x + library.w)
                .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                .collect();
            visible
                .first()
                .zip(visible.last())
                .map(|(&first, &last)| (first + last) as f32 * 0.5 - library.x as f32)
        });
        assert_sprite_bottom_centered!(library, library_bottom_center);

        let church_bottom_center = (church.y..church.y + church.h).rev().find_map(|y| {
            let visible: Vec<_> = (church.x..church.x + church.w)
                .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                .collect();
            visible
                .first()
                .zip(visible.last())
                .map(|(&first, &last)| (first + last) as f32 * 0.5 - church.x as f32)
        });
        assert_sprite_bottom_centered!(church, church_bottom_center);

        let water_treatment_bottom_center = (water_treatment.y
            ..water_treatment.y + water_treatment.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (water_treatment.x..water_treatment.x + water_treatment.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - water_treatment.x as f32)
            });
        assert_sprite_bottom_centered!(water_treatment, water_treatment_bottom_center);

        let mayor_house_bottom_center = (mayor_house.y..mayor_house.y + mayor_house.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (mayor_house.x..mayor_house.x + mayor_house.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - mayor_house.x as f32)
            });
        assert_sprite_bottom_centered!(mayor_house, mayor_house_bottom_center);

        let cargo_yard_bottom_center =
            (cargo_yard.y..cargo_yard.y + cargo_yard.h)
                .rev()
                .find_map(|y| {
                    let visible: Vec<_> = (cargo_yard.x..cargo_yard.x + cargo_yard.w)
                        .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                        .collect();
                    visible
                        .first()
                        .zip(visible.last())
                        .map(|(&first, &last)| (first + last) as f32 * 0.5 - cargo_yard.x as f32)
                });
        assert_sprite_bottom_centered!(cargo_yard, cargo_yard_bottom_center);

        let water_tower_bottom_center = (water_tower.y..water_tower.y + water_tower.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (water_tower.x..water_tower.x + water_tower.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - water_tower.x as f32)
            });
        assert_sprite_bottom_centered!(water_tower, water_tower_bottom_center);

        let bus_depot_bottom_center =
            (bus_depot.y..bus_depot.y + bus_depot.h)
                .rev()
                .find_map(|y| {
                    let visible: Vec<_> = (bus_depot.x..bus_depot.x + bus_depot.w)
                        .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                        .collect();
                    visible
                        .first()
                        .zip(visible.last())
                        .map(|(&first, &last)| (first + last) as f32 * 0.5 - bus_depot.x as f32)
                });
        assert_sprite_bottom_centered!(bus_depot, bus_depot_bottom_center);

        // The rail tracks intentionally project beyond the footprint; the explicit
        // bottom-center anchor above is the placement contract, not the art bounds.

        let large_hangar_bottom_center = (large_hangar.y..large_hangar.y + large_hangar.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (large_hangar.x..large_hangar.x + large_hangar.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - large_hangar.x as f32)
            });
        assert_sprite_bottom_centered!(large_hangar, large_hangar_bottom_center);

        let arcology_bottom_center = (arcology.y..arcology.y + arcology.h).rev().find_map(|y| {
            let visible: Vec<_> = (arcology.x..arcology.x + arcology.w)
                .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                .collect();
            visible
                .first()
                .zip(visible.last())
                .map(|(&first, &last)| (first + last) as f32 * 0.5 - arcology.x as f32)
        });
        assert_sprite_bottom_centered!(arcology, arcology_bottom_center);

        let eiffel_tower_bottom_center = (eiffel_tower.y..eiffel_tower.y + eiffel_tower.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (eiffel_tower.x..eiffel_tower.x + eiffel_tower.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - eiffel_tower.x as f32)
            });
        assert_sprite_bottom_centered!(eiffel_tower, eiffel_tower_bottom_center);

        let marina_bottom_center = (marina.y..marina.y + marina.h).rev().find_map(|y| {
            let visible: Vec<_> = (marina.x..marina.x + marina.w)
                .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                .collect();
            visible
                .first()
                .zip(visible.last())
                .map(|(&first, &last)| (first + last) as f32 * 0.5 - marina.x as f32)
        });
        assert_sprite_bottom_centered!(marina, marina_bottom_center);

        let missile_silo_bottom_center = (missile_silo.y..missile_silo.y + missile_silo.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (missile_silo.x..missile_silo.x + missile_silo.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - missile_silo.x as f32)
            });
        assert_sprite_bottom_centered!(missile_silo, missile_silo_bottom_center);

        let desalination_bottom_center = (desalination.y..desalination.y + desalination.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (desalination.x..desalination.x + desalination.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - desalination.x as f32)
            });
        assert_sprite_bottom_centered!(desalination, desalination_bottom_center);

        let loading_bay_bottom_center = (loading_bay.y..loading_bay.y + loading_bay.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (loading_bay.x..loading_bay.x + loading_bay.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - loading_bay.x as f32)
            });
        assert_sprite_bottom_centered!(loading_bay, loading_bay_bottom_center);

        let control_tower_bottom_center = (control_tower.y..control_tower.y + control_tower.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (control_tower.x..control_tower.x + control_tower.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - control_tower.x as f32)
            });
        assert_sprite_bottom_centered!(control_tower, control_tower_bottom_center);

        let subway_station_bottom_center = (subway_station.y..subway_station.y + subway_station.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (subway_station.x..subway_station.x + subway_station.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - subway_station.x as f32)
            });
        assert_sprite_bottom_centered!(subway_station, subway_station_bottom_center);

        let warehouse_bottom_center =
            (warehouse.y..warehouse.y + warehouse.h)
                .rev()
                .find_map(|y| {
                    let visible: Vec<_> = (warehouse.x..warehouse.x + warehouse.w)
                        .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                        .collect();
                    visible
                        .first()
                        .zip(visible.last())
                        .map(|(&first, &last)| (first + last) as f32 * 0.5 - warehouse.x as f32)
                });
        assert_sprite_bottom_centered!(warehouse, warehouse_bottom_center);

        let military_facility_bottom_center =
            (military_facility.y..military_facility.y + military_facility.h)
                .rev()
                .find_map(|y| {
                    let visible: Vec<_> = (military_facility.x
                        ..military_facility.x + military_facility.w)
                        .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                        .collect();
                    visible.first().zip(visible.last()).map(|(&first, &last)| {
                        (first + last) as f32 * 0.5 - military_facility.x as f32
                    })
                });
        assert_sprite_bottom_centered!(military_facility, military_facility_bottom_center);

        let water_pump_bottom_center =
            (water_pump.y..water_pump.y + water_pump.h)
                .rev()
                .find_map(|y| {
                    let visible: Vec<_> = (water_pump.x..water_pump.x + water_pump.w)
                        .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                        .collect();
                    visible
                        .first()
                        .zip(visible.last())
                        .map(|(&first, &last)| (first + last) as f32 * 0.5 - water_pump.x as f32)
                });
        assert_sprite_bottom_centered!(water_pump, water_pump_bottom_center);

        let airport_building_bottom_center = (airport_building.y
            ..airport_building.y + airport_building.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (airport_building.x..airport_building.x + airport_building.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - airport_building.x as f32)
            });
        assert_sprite_bottom_centered!(airport_building, airport_building_bottom_center);

        let radar_station_bottom_center = (radar_station.y..radar_station.y + radar_station.h)
            .rev()
            .find_map(|y| {
                let visible: Vec<_> = (radar_station.x..radar_station.x + radar_station.w)
                    .filter(|&x| atlas.get_pixel(x, y).0[3] >= 128)
                    .collect();
                visible
                    .first()
                    .zip(visible.last())
                    .map(|(&first, &last)| (first + last) as f32 * 0.5 - radar_station.x as f32)
            });
        assert_sprite_bottom_centered!(radar_station, radar_station_bottom_center);
    }

    #[test]
    fn repaired_building_foundations_follow_the_map_grid() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        for file in [
            "buildings/lavender-glass-highrise-1157@8x.png",
            "buildings/mayor-house/large@8x.png",
            "buildings/desalination-plant/large@8x.png",
            "buildings/landmark-1252@8x.png",
            "buildings/landmark-1253@8x.png",
            "buildings/landmark-1254@8x.png",
        ] {
            let image = image::open(root.join(file)).unwrap().to_rgba8();
            let (width, height) = image.dimensions();
            // The exposed front edges must meet adjacent 16x8 ground tiles.
            // Check the image itself: a correct manifest anchor alone cannot
            // catch a steep or flattened foundation inside the canvas.
            let foundation_ranges = if file == "buildings/landmark-1254@8x.png" {
                // Its detached observation pylons and planted plinth corners
                // interrupt the outer alpha silhouette; check the continuous
                // central front edge against the 2:1 tile plane instead.
                [(42, 50), (50, 58)]
            } else {
                [(12, 42), (58, 88)]
            };
            for (start, end) in foundation_ranges {
                for x in width * start / 100..width * end / 100 {
                    let bottom = (0..height)
                        .rev()
                        .find(|&y| image.get_pixel(x, y).0[3] > 128)
                        .expect("continuous foundation edge");
                    let expected =
                        height as f32 - 1.0 - (x as f32 - (width - 1) as f32 * 0.5).abs() * 0.5;
                    // The desalination plant has a two-game-pixel stone fascia
                    // below its exposed ground edge. The bottommost-alpha probe
                    // includes that thickness.
                    let tolerance = match file {
                        "buildings/desalination-plant/large@8x.png" => 16.0,
                        // The pod bases and landscaping alter 1254's outer
                        // alpha silhouette; its central checked edge is within
                        // two logical pixels of the 2:1 lot plane.
                        "buildings/landmark-1252@8x.png"
                        | "buildings/landmark-1253@8x.png"
                        | "buildings/landmark-1254@8x.png" => 16.0,
                        _ => 10.0,
                    };
                    assert!(
                        (bottom as f32 - expected).abs() <= tolerance,
                        "{file}: foundation at x={x}, y={bottom} misses ground y={expected}"
                    );
                }
            }
        }
        let tower = image::open(root.join("buildings/lavender-glass-highrise-1157@8x.png"))
            .unwrap()
            .to_rgba8();
        // Both white corner piers must extend into the former 96-pixel gap.
        // Previously these locations showed the green lot behind the tower.
        for x in [64, 448] {
            let [r, _, b, a] = tower.get_pixel(x, 528).0;
            assert!(
                r >= 150 && b >= 145 && a > 128,
                "tower wall stops above its lot"
            );
        }
    }

    #[test]
    fn radioactive_waste_and_monster_part_replacements_are_loaded() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let frame = source
            .frame_by_id(1005)
            .expect("radioactive-waste replacement");
        assert_eq!((frame.w, frame.h), (256, 136));
        assert_eq!(frame.logical_size, Some([32, 17]));
        assert_eq!(frame.anchor, Some([16.0, 17.0]));
        for (id, height) in [
            (1478, 39),
            (1479, 86),
            (1480, 67),
            (1481, 58),
            (1482, 25),
            (1483, 50),
            (1484, 39),
            (1485, 65),
            (1486, 75),
            (1487, 70),
            (1488, 44),
            (1489, 48),
            (1490, 90),
            (1491, 90),
        ] {
            let frame = source.frame_by_id(id).expect("monster-part replacement");
            assert_eq!((frame.w, frame.h), (512, height * 8));
            assert_eq!(frame.logical_size, Some([64, height]));
            assert_eq!(frame.anchor, Some([32.0, height as f32]));
        }
    }

    #[test]
    fn special_landmark_751_keeps_its_source_footprint_and_anchor() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let frame = source
            .frame_by_id(751)
            .expect("special landmark replacement");
        assert_eq!((frame.w, frame.h), (512, 784));
        assert_eq!(frame.logical_size, Some([64, 98]));
        assert_eq!(frame.anchor, Some([32.0, 98.0]));

        let atlas = source.load_rgba().unwrap();
        let base_row = frame.y + frame.h - 1;
        let base_pixel = atlas.get_pixel(frame.x + 255, base_row);
        assert!(
            base_pixel.0[3] > 0,
            "base must reach the source ground anchor"
        );
    }

    #[test]
    fn special_landmark_752_keeps_its_source_footprint_and_anchor() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let frame = source
            .frame_by_id(752)
            .expect("special landmark replacement");
        assert_eq!((frame.w, frame.h), (512, 736));
        assert_eq!(frame.logical_size, Some([64, 92]));
        assert_eq!(frame.anchor, Some([32.0, 92.0]));

        let atlas = source.load_rgba().unwrap();
        let base_row = frame.y + frame.h - 1;
        let base_pixel = atlas.get_pixel(frame.x + 256, base_row);
        assert!(
            base_pixel.0[3] > 0,
            "base must reach the source ground anchor"
        );
    }

    #[test]
    fn next_special_sprite_batch_keeps_source_canvases_and_anchors() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (573, 16, 16),
            (574, 16, 16),
            (575, 16, 16),
            (576, 16, 16),
            (577, 16, 16),
            (578, 16, 16),
            (579, 16, 16),
            (580, 16, 16),
            (581, 16, 24),
            (582, 16, 34),
            (701, 64, 42),
            (702, 64, 58),
            (703, 64, 41),
            (704, 64, 40),
            (705, 64, 48),
            (706, 64, 36),
            (707, 64, 55),
            (753, 64, 106),
            (754, 64, 119),
            (755, 64, 78),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new special replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn special_effect_and_guideway_batch_keeps_source_canvases_and_anchors() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (583, 16, 33),
            (584, 16, 26),
            (585, 16, 19),
            (586, 16, 20),
            (587, 16, 16),
            (588, 16, 16),
            (589, 16, 22),
            (590, 16, 14),
            (591, 16, 14),
            (592, 16, 18),
            (593, 16, 16),
            (594, 16, 15),
            (595, 16, 11),
            (596, 16, 11),
            (597, 32, 30),
            (598, 32, 30),
            (599, 32, 24),
            (600, 32, 24),
            (601, 32, 23),
            (602, 32, 16),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new special replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_and_special_batch_keeps_source_canvases_and_anchors() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (201, 32, 23),
            (202, 32, 29),
            (203, 32, 21),
            (204, 32, 20),
            (205, 32, 24),
            (206, 32, 18),
            (207, 32, 28),
            (208, 24, 18),
            (209, 24, 21),
            (210, 24, 19),
            (211, 24, 17),
            (212, 24, 15),
            (603, 32, 23),
            (604, 32, 24),
            (605, 32, 26),
            (606, 32, 33),
            (607, 32, 33),
            (698, 16, 17),
            (699, 16, 17),
            (700, 16, 24),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new replacement sprite");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_640_through_659_keep_source_canvases_and_anchors() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (640, 32, 17),
            (641, 32, 20),
            (642, 32, 19),
            (643, 32, 22),
            (644, 32, 28),
            (645, 32, 28),
            (646, 32, 40),
            (647, 32, 39),
            (648, 32, 17),
            (649, 32, 20),
            (650, 32, 21),
            (651, 32, 24),
            (652, 32, 23),
            (653, 32, 26),
            (654, 32, 36),
            (655, 32, 34),
            (656, 32, 37),
            (657, 32, 43),
            (658, 32, 19),
            (659, 32, 20),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_660_through_679_keep_source_canvases_and_anchors() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (660, 32, 23),
            (661, 32, 18),
            (662, 32, 19),
            (663, 32, 27),
            (664, 32, 29),
            (665, 32, 31),
            (666, 32, 18),
            (667, 32, 22),
            (668, 32, 22),
            (669, 32, 33),
            (670, 32, 24),
            (671, 32, 28),
            (672, 32, 33),
            (673, 32, 36),
            (674, 48, 37),
            (675, 48, 45),
            (676, 48, 29),
            (677, 48, 48),
            (678, 48, 30),
            (679, 48, 43),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_680_through_697_and_708_through_709_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (680, 48, 33),
            (681, 48, 47),
            (682, 48, 30),
            (683, 48, 59),
            (684, 48, 71),
            (685, 48, 40),
            (686, 48, 51),
            (687, 48, 72),
            (688, 48, 26),
            (689, 48, 44),
            (690, 48, 28),
            (691, 48, 27),
            (692, 48, 28),
            (693, 48, 25),
            (694, 48, 42),
            (695, 48, 33),
            (696, 48, 36),
            (697, 48, 46),
            (708, 48, 39),
            (709, 48, 41),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_710_through_718_and_735_through_745_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (710, 48, 37),
            (711, 48, 33),
            (712, 48, 28),
            (713, 48, 27),
            (714, 48, 29),
            (715, 64, 42),
            (716, 64, 37),
            (717, 64, 43),
            (718, 64, 34),
            (735, 32, 32),
            (736, 32, 19),
            (737, 32, 21),
            (738, 32, 22),
            (739, 32, 19),
            (740, 32, 19),
            (741, 32, 30),
            (742, 32, 17),
            (743, 32, 18),
            (744, 32, 19),
            (745, 32, 27),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_746_through_750_and_863_through_923_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (746, 32, 21),
            (747, 32, 24),
            (748, 48, 28),
            (749, 48, 26),
            (750, 48, 31),
            (863, 32, 16),
            (869, 48, 25),
            (870, 48, 23),
            (871, 48, 20),
            (872, 48, 23),
            (873, 48, 27),
            (887, 32, 17),
            (888, 32, 22),
            (889, 32, 24),
            (918, 32, 28),
            (919, 32, 28),
            (920, 32, 22),
            (921, 32, 22),
            (922, 32, 23),
            (923, 32, 15),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_174_through_194_and_196_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (174, 24, 18),
            (175, 24, 23),
            (176, 24, 15),
            (177, 24, 24),
            (179, 24, 22),
            (180, 24, 17),
            (181, 24, 24),
            (182, 24, 16),
            (183, 24, 30),
            (184, 24, 35),
            (185, 24, 20),
            (186, 24, 28),
            (187, 24, 38),
            (189, 24, 24),
            (190, 24, 15),
            (191, 24, 15),
            (192, 24, 15),
            (193, 24, 13),
            (194, 24, 21),
            (196, 24, 18),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn twenty_smallmed_replacements_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (146, 16, 20),
            (147, 16, 20),
            (154, 16, 19),
            (156, 16, 19),
            (157, 16, 22),
            (178, 24, 16),
            (188, 24, 13),
            (195, 24, 17),
            (197, 24, 24),
            (214, 24, 15),
            (215, 32, 22),
            (216, 32, 19),
            (217, 32, 23),
            (218, 32, 17),
            (250, 24, 16),
            (251, 32, 50),
            (252, 32, 47),
            (253, 32, 53),
            (254, 32, 59),
            (255, 32, 40),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn twenty_smallmed_facilities_and_towers_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (144, 16, 16),
            (145, 16, 14),
            (151, 16, 12),
            (152, 16, 12),
            (153, 16, 14),
            (155, 16, 17),
            (163, 16, 14),
            (164, 16, 15),
            (165, 16, 16),
            (167, 16, 12),
            (168, 16, 12),
            (169, 16, 17),
            (170, 16, 14),
            (171, 16, 15),
            (172, 16, 18),
            (173, 16, 19),
            (235, 16, 17),
            (241, 16, 14),
            (245, 16, 14),
            (247, 16, 13),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn first_twenty_smallmed_micro_sprites_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (1, 8, 5),
            (2, 8, 5),
            (3, 8, 5),
            (4, 8, 5),
            (5, 8, 5),
            (6, 8, 6),
            (7, 8, 6),
            (8, 8, 7),
            (9, 8, 7),
            (10, 8, 7),
            (11, 8, 8),
            (12, 8, 7),
            (13, 8, 6),
            (14, 8, 5),
            (15, 8, 5),
            (16, 8, 8),
            (17, 8, 8),
            (18, 8, 6),
            (19, 8, 6),
            (20, 8, 5),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_micro_terrain_21_through_40_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (21, 8, 5),
            (22, 8, 5),
            (23, 8, 5),
            (24, 8, 5),
            (25, 8, 5),
            (26, 8, 5),
            (27, 8, 5),
            (28, 8, 5),
            (29, 8, 5),
            (30, 8, 5),
            (31, 8, 8),
            (32, 8, 8),
            (33, 8, 6),
            (34, 8, 6),
            (35, 8, 5),
            (36, 8, 5),
            (37, 8, 5),
            (38, 8, 5),
            (39, 8, 5),
            (40, 8, 5),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_micro_details_41_through_60_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (41, 8, 5),
            (42, 8, 5),
            (43, 8, 5),
            (44, 8, 5),
            (45, 8, 5),
            (46, 8, 8),
            (47, 8, 8),
            (48, 8, 6),
            (49, 8, 6),
            (50, 8, 5),
            (51, 8, 5),
            (52, 8, 5),
            (53, 8, 5),
            (54, 8, 5),
            (55, 8, 5),
            (56, 8, 5),
            (57, 8, 5),
            (58, 8, 5),
            (59, 8, 5),
            (60, 8, 5),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_micro_details_61_through_80_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (61, 8, 5),
            (62, 8, 5),
            (63, 8, 8),
            (64, 8, 8),
            (65, 8, 7),
            (66, 8, 7),
            (67, 8, 5),
            (68, 8, 5),
            (69, 8, 5),
            (70, 8, 5),
            (71, 8, 5),
            (72, 8, 5),
            (73, 8, 8),
            (74, 8, 8),
            (75, 8, 8),
            (76, 8, 8),
            (77, 8, 8),
            (78, 8, 8),
            (79, 8, 8),
            (80, 8, 8),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_micro_details_81_through_100_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (81, 8, 12),
            (82, 8, 16),
            (83, 8, 17),
            (84, 8, 14),
            (85, 8, 10),
            (86, 8, 10),
            (87, 8, 8),
            (88, 8, 8),
            (89, 8, 11),
            (90, 8, 7),
            (91, 8, 7),
            (92, 8, 9),
            (93, 8, 8),
            (94, 8, 8),
            (95, 8, 6),
            (96, 8, 6),
            (97, 16, 17),
            (98, 16, 17),
            (99, 16, 12),
            (100, 16, 12),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_micro_details_101_through_120_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (101, 16, 12),
            (102, 16, 9),
            (103, 16, 12),
            (104, 16, 13),
            (105, 16, 14),
            (106, 16, 17),
            (107, 16, 17),
            (108, 8, 6),
            (109, 8, 6),
            (110, 8, 6),
            (111, 8, 6),
            (112, 8, 5),
            (113, 8, 5),
            (114, 8, 5),
            (115, 8, 5),
            (116, 8, 5),
            (117, 8, 5),
            (118, 8, 5),
            (119, 8, 5),
            (120, 8, 5),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_micro_details_121_through_140_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (121, 8, 5),
            (122, 8, 5),
            (123, 8, 6),
            (124, 8, 8),
            (125, 8, 7),
            (126, 8, 6),
            (127, 8, 6),
            (128, 8, 8),
            (129, 8, 12),
            (130, 8, 7),
            (131, 8, 9),
            (132, 8, 5),
            (133, 8, 8),
            (134, 8, 6),
            (135, 8, 7),
            (136, 8, 5),
            (137, 8, 5),
            (138, 8, 5),
            (139, 8, 5),
            (140, 16, 10),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn new_smallmed_sprites_141_143_148_150_158_160_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (141, 16, 11),
            (142, 16, 11),
            (143, 16, 11),
            (148, 16, 9),
            (149, 16, 11),
            (150, 16, 11),
            (158, 16, 11),
            (159, 16, 11),
            (160, 16, 11),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn next_missing_smallmed_sprites_161_through_231_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (161, 16, 10),
            (162, 16, 10),
            (166, 16, 10),
            (198, 8, 9),
            (199, 8, 9),
            (200, 8, 11),
            (213, 24, 14),
            (219, 8, 13),
            (220, 8, 5),
            (221, 8, 5),
            (222, 8, 5),
            (223, 8, 6),
            (224, 8, 11),
            (225, 8, 16),
            (226, 8, 18),
            (227, 8, 5),
            (228, 8, 6),
            (229, 8, 6),
            (230, 8, 5),
            (231, 8, 5),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn missing_smallmed_sprites_232_through_261_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (232, 8, 6),
            (233, 8, 6),
            (234, 8, 5),
            (236, 16, 10),
            (237, 16, 11),
            (238, 16, 10),
            (239, 16, 10),
            (240, 16, 10),
            (242, 16, 9),
            (243, 16, 10),
            (244, 16, 10),
            (246, 16, 10),
            (248, 24, 14),
            (249, 24, 13),
            (256, 8, 5),
            (257, 8, 8),
            (258, 8, 8),
            (259, 8, 6),
            (260, 8, 6),
            (261, 8, 8),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_terrain_sprites_262_through_281_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (262, 8, 8),
            (263, 8, 6),
            (264, 8, 8),
            (265, 8, 8),
            (266, 8, 6),
            (267, 8, 5),
            (268, 8, 6),
            (269, 8, 8),
            (270, 8, 5),
            (271, 8, 5),
            (272, 8, 5),
            (273, 8, 5),
            (274, 8, 5),
            (275, 8, 5),
            (276, 8, 5),
            (277, 8, 5),
            (278, 8, 5),
            (279, 8, 5),
            (280, 8, 5),
            (281, 8, 5),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_water_and_ground_markers_282_through_301_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (282, 8, 5),
            (283, 8, 5),
            (284, 8, 8),
            (285, 8, 4),
            (286, 8, 4),
            (287, 8, 4),
            (288, 8, 4),
            (289, 8, 4),
            (290, 8, 4),
            (291, 8, 5),
            (292, 8, 5),
            (293, 8, 5),
            (294, 8, 5),
            (295, 8, 5),
            (296, 8, 5),
            (297, 8, 5),
            (298, 8, 5),
            (299, 8, 5),
            (300, 8, 5),
            (301, 8, 5),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_path_and_green_ground_details_305_through_324_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (305, 8, 5),
            (306, 8, 8),
            (307, 8, 8),
            (308, 8, 6),
            (309, 8, 6),
            (310, 8, 8),
            (311, 8, 8),
            (312, 8, 6),
            (313, 8, 8),
            (314, 8, 8),
            (315, 8, 6),
            (316, 8, 5),
            (317, 8, 6),
            (318, 8, 8),
            (319, 8, 9),
            (320, 8, 9),
            (321, 8, 12),
            (322, 8, 12),
            (323, 8, 8),
            (324, 8, 8),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_ground_markers_and_water_edges_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (302, 8, 5),
            (303, 8, 5),
            (304, 8, 5),
            (325, 8, 9),
            (326, 8, 9),
            (327, 8, 9),
            (328, 8, 7),
            (329, 8, 9),
            (330, 8, 9),
            (331, 8, 9),
            (332, 8, 9),
            (333, 8, 9),
            (334, 8, 5),
            (335, 8, 5),
            (336, 8, 8),
            (337, 8, 8),
            (338, 8, 6),
            (339, 8, 6),
            (340, 8, 5),
            (341, 8, 5),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_blue_markers_stones_and_utility_details_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (342, 8, 5),
            (343, 8, 5),
            (344, 8, 5),
            (345, 8, 5),
            (346, 8, 5),
            (347, 8, 5),
            (348, 8, 5),
            (349, 8, 9),
            (350, 8, 9),
            (351, 8, 5),
            (352, 8, 8),
            (353, 8, 11),
            (354, 8, 5),
            (355, 8, 5),
            (356, 8, 5),
            (357, 8, 5),
            (358, 8, 5),
            (359, 16, 8),
            (360, 16, 6),
            (361, 16, 5),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_signals_impacts_and_lattice_details_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (362, 16, 6),
            (363, 16, 8),
            (369, 24, 13),
            (370, 24, 11),
            (371, 24, 10),
            (372, 24, 12),
            (373, 24, 14),
            (382, 8, 22),
            (383, 8, 24),
            (384, 8, 19),
            (385, 8, 21),
            (386, 8, 5),
            (387, 16, 9),
            (388, 16, 11),
            (389, 16, 12),
            (390, 8, 7),
            (391, 8, 7),
            (392, 8, 17),
            (393, 8, 13),
            (394, 8, 11),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_fire_and_smoke_effects_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (395, 8, 8),
            (396, 8, 8),
            (397, 8, 9),
            (398, 8, 8),
            (399, 8, 9),
            (400, 8, 4),
            (401, 8, 4),
            (402, 8, 7),
            (403, 8, 7),
            (404, 8, 5),
            (405, 8, 5),
            (406, 8, 4),
            (407, 8, 3),
            (408, 8, 4),
            (409, 8, 5),
            (410, 8, 7),
            (411, 8, 7),
            (412, 8, 7),
            (413, 8, 7),
            (414, 8, 8),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_road_fragments_and_water_markers_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (415, 8, 7),
            (416, 8, 5),
            (417, 8, 5),
            (418, 16, 16),
            (419, 16, 16),
            (420, 16, 11),
            (421, 16, 11),
            (422, 16, 11),
            (423, 16, 8),
            (424, 16, 11),
            (425, 16, 12),
            (426, 16, 13),
            (450, 8, 5),
            (451, 8, 5),
            (452, 8, 8),
            (453, 8, 8),
            (454, 8, 6),
            (455, 8, 6),
            (456, 8, 5),
            (457, 8, 5),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_water_and_color_markers_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (458, 8, 5),
            (459, 8, 5),
            (460, 8, 5),
            (461, 8, 5),
            (462, 8, 5),
            (463, 8, 5),
            (464, 8, 5),
            (465, 8, 9),
            (466, 8, 9),
            (467, 8, 5),
            (468, 8, 5),
            (469, 8, 5),
            (470, 8, 5),
            (471, 8, 5),
            (472, 8, 5),
            (473, 8, 5),
            (474, 8, 5),
            (475, 8, 5),
            (476, 8, 5),
            (477, 8, 5),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_gray_fittings_and_lattice_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (478, 16, 10),
            (479, 16, 21),
            (480, 16, 16),
            (481, 16, 15),
            (482, 16, 7),
            (483, 16, 12),
            (484, 16, 10),
            (485, 16, 16),
            (486, 16, 18),
            (487, 16, 17),
            (488, 16, 10),
            (489, 16, 11),
            (490, 16, 22),
            (491, 16, 23),
            (492, 8, 6),
            (493, 8, 10),
            (494, 8, 10),
            (495, 8, 4),
            (496, 16, 13),
            (497, 16, 31),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn smallmed_lattice_rubble_trees_and_branches_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (498, 16, 31),
            (499, 16, 31),
            (501, 16, 9),
            (502, 16, 9),
            (503, 16, 9),
            (504, 16, 9),
            (505, 16, 8),
            (506, 16, 10),
            (507, 16, 11),
            (508, 16, 13),
            (509, 16, 13),
            (510, 16, 12),
            (511, 16, 13),
            (512, 16, 12),
            (513, 16, 12),
            (514, 16, 13),
            (515, 16, 13),
            (516, 16, 19),
            (517, 16, 19),
            (518, 16, 15),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }

    #[test]
    fn toolbar_palettes_are_bundled_at_eight_x_source_density() {
        let root = scratch("fallback-ui");
        let sources = load_ui_palette_sources(&root).unwrap();
        assert_eq!(sources.len(), 6);
        assert_eq!(sources[0].name, "citygame_city_toolbar");
        assert_eq!(sources[1].name, "citygame_edit_toolbar");
        assert_eq!(sources[2].name, "classic_city_toolbar");
        assert_eq!(sources[3].name, "classic_edit_toolbar");
        assert_eq!(
            sources[0].load_rgba().unwrap().dimensions(),
            (71 * 8, 312 * 8)
        );
        assert_eq!(
            sources[1].load_rgba().unwrap().dimensions(),
            (50 * 8, 306 * 8)
        );
        assert_eq!(
            sources[2].load_rgba().unwrap().dimensions(),
            (71 * 8, 312 * 8)
        );
        assert_eq!(
            sources[3].load_rgba().unwrap().dimensions(),
            (50 * 8, 306 * 8)
        );
        let city = sources[0].load_rgba().unwrap();
        let classic_city = sources[2].load_rgba().unwrap();
        let sample = (56 * 8, 235 * 8);
        let original_pixel = city.get_pixel(sample.0, sample.1).0;
        let classic_pixel = classic_city.get_pixel(sample.0, sample.1).0;
        assert_ne!(original_pixel[0], original_pixel[2]);
        assert_eq!(classic_pixel[0], classic_pixel[1]);
        assert_eq!(classic_pixel[1], classic_pixel[2]);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod smallmed_batch22_regression {
    use super::*;

    #[test]
    fn smallmed_branch_and_smoke_effects_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (519, 16, 15),
            (520, 16, 13),
            (521, 16, 12),
            (522, 16, 13),
            (523, 16, 13),
            (524, 16, 13),
            (525, 16, 13),
            (526, 16, 13),
            (527, 16, 13),
            (528, 16, 13),
            (529, 16, 9),
            (530, 16, 9),
            (531, 16, 15),
            (532, 16, 15),
            (533, 16, 11),
            (534, 16, 11),
            (535, 16, 9),
            (536, 16, 9),
            (537, 16, 9),
            (538, 16, 9),
        ];

        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }

        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch23_regression {
    use super::*;

    #[test]
    fn smallmed_smoke_and_red_debris_effects_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (539, 16, 9),
            (540, 16, 9),
            (541, 16, 9),
            (542, 16, 9),
            (543, 16, 9),
            (544, 16, 9),
            (545, 16, 9),
            (546, 16, 15),
            (547, 16, 15),
            (548, 16, 11),
            (549, 16, 11),
            (550, 16, 9),
            (551, 16, 9),
            (552, 16, 9),
            (553, 16, 9),
            (554, 16, 9),
            (555, 16, 9),
            (556, 16, 9),
            (557, 16, 9),
            (558, 16, 9),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch24_regression {
    use super::*;

    #[test]
    fn smallmed_smoke_and_colored_impact_sprites_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (559, 16, 10),
            (560, 16, 10),
            (561, 16, 9),
            (562, 16, 9),
            (563, 16, 15),
            (564, 16, 15),
            (565, 16, 14),
            (566, 16, 14),
            (567, 16, 13),
            (568, 16, 13),
            (569, 16, 9),
            (570, 16, 9),
            (571, 16, 13),
            (572, 16, 13),
            (608, 16, 11),
            (609, 16, 11),
            (610, 16, 12),
            (611, 16, 12),
            (612, 16, 9),
            (613, 16, 10),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch25_regression {
    use super::*;

    #[test]
    fn smallmed_impact_and_small_facility_sprites_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (614, 16, 10),
            (615, 16, 9),
            (616, 16, 9),
            (617, 16, 9),
            (618, 16, 9),
            (619, 16, 9),
            (620, 16, 10),
            (621, 16, 9),
            (622, 16, 10),
            (623, 16, 11),
            (624, 16, 11),
            (625, 16, 13),
            (626, 16, 10),
            (627, 16, 11),
            (628, 16, 16),
            (629, 16, 21),
            (630, 16, 13),
            (631, 16, 18),
            (632, 16, 11),
            (633, 16, 15),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch26_regression {
    use super::*;

    #[test]
    fn smallmed_smoke_markers_and_utility_props_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (634, 16, 10),
            (635, 16, 13),
            (636, 16, 9),
            (637, 16, 9),
            (638, 16, 9),
            (639, 16, 9),
            (719, 16, 24),
            (720, 16, 10),
            (721, 16, 9),
            (722, 16, 9),
            (723, 16, 11),
            (724, 16, 20),
            (725, 16, 32),
            (726, 16, 35),
            (727, 16, 10),
            (728, 16, 12),
            (729, 16, 12),
            (730, 16, 9),
            (731, 16, 9),
            (732, 16, 11),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch27_regression {
    use super::*;

    #[test]
    fn smallmed_dirt_and_water_edge_sprites_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (733, 16, 11),
            (734, 16, 10),
            (756, 16, 9),
            (757, 16, 15),
            (758, 16, 15),
            (759, 16, 11),
            (760, 16, 11),
            (761, 16, 15),
            (762, 16, 15),
            (763, 16, 11),
            (764, 16, 15),
            (765, 16, 15),
            (766, 16, 11),
            (767, 16, 9),
            (768, 16, 11),
            (769, 16, 15),
            (770, 16, 9),
            (771, 16, 9),
            (772, 16, 9),
            (773, 16, 9),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch28_regression {
    use super::*;

    #[test]
    fn smallmed_water_and_marker_tiles_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (774, 16, 9),
            (775, 16, 9),
            (776, 16, 9),
            (777, 16, 9),
            (778, 16, 9),
            (779, 16, 9),
            (780, 16, 9),
            (781, 16, 9),
            (782, 16, 9),
            (783, 16, 9),
            (784, 16, 15),
            (785, 16, 8),
            (786, 16, 8),
            (787, 16, 8),
            (788, 16, 8),
            (789, 16, 8),
            (790, 16, 8),
            (791, 16, 10),
            (792, 16, 10),
            (793, 16, 10),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch29_regression {
    use super::*;

    #[test]
    fn smallmed_ground_markers_and_gold_plot_outlines_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (794, 16, 10),
            (795, 16, 10),
            (796, 16, 10),
            (797, 16, 10),
            (798, 16, 10),
            (799, 16, 10),
            (800, 16, 9),
            (801, 16, 9),
            (802, 16, 9),
            (803, 16, 9),
            (804, 16, 9),
            (805, 16, 9),
            (806, 16, 15),
            (807, 16, 15),
            (808, 16, 11),
            (809, 16, 11),
            (810, 16, 15),
            (811, 16, 15),
            (812, 16, 11),
            (813, 16, 15),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch30_regression {
    use super::*;

    #[test]
    fn smallmed_gold_outlines_and_plot_tiles_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (814, 16, 15),
            (815, 16, 11),
            (816, 16, 9),
            (817, 16, 11),
            (818, 16, 15),
            (819, 16, 17),
            (820, 16, 17),
            (821, 16, 23),
            (822, 16, 23),
            (823, 16, 15),
            (824, 16, 15),
            (825, 16, 17),
            (826, 16, 17),
            (827, 16, 17),
            (828, 16, 13),
            (829, 16, 17),
            (830, 16, 17),
            (831, 16, 17),
            (832, 16, 17),
            (833, 16, 17),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch31_regression {
    use super::*;

    #[test]
    fn smallmed_colored_markers_and_smoke_sprites_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (834, 16, 9),
            (835, 16, 9),
            (836, 16, 15),
            (837, 16, 15),
            (838, 16, 11),
            (839, 16, 11),
            (840, 16, 9),
            (841, 16, 9),
            (842, 16, 9),
            (843, 16, 9),
            (844, 16, 9),
            (845, 16, 9),
            (846, 16, 9),
            (847, 16, 9),
            (848, 16, 9),
            (849, 16, 17),
            (850, 16, 17),
            (851, 16, 9),
            (852, 16, 14),
            (853, 16, 18),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch32_regression {
    use super::*;

    #[test]
    fn smallmed_markers_aircraft_beacons_and_sparks_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (854, 16, 9),
            (855, 16, 9),
            (856, 16, 9),
            (857, 16, 9),
            (858, 16, 9),
            (859, 32, 14),
            (860, 32, 10),
            (861, 32, 10),
            (862, 32, 12),
            (882, 16, 38),
            (883, 16, 40),
            (884, 16, 38),
            (885, 16, 42),
            (886, 16, 9),
            (890, 16, 11),
            (891, 16, 11),
            (892, 16, 31),
            (893, 16, 27),
            (894, 16, 21),
            (895, 16, 17),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch33_regression {
    use super::*;

    #[test]
    fn smallmed_gray_and_blue_line_fragments_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (916, 16, 10),
            (917, 16, 9),
            (924, 32, 23),
            (925, 32, 23),
            (926, 32, 21),
            (927, 16, 9),
            (928, 16, 9),
            (929, 16, 15),
            (930, 16, 14),
            (931, 16, 11),
            (932, 16, 7),
            (933, 16, 9),
            (934, 16, 7),
            (935, 16, 9),
            (936, 16, 9),
            (937, 16, 14),
            (938, 16, 14),
            (939, 16, 14),
            (940, 16, 14),
            (941, 32, 28),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch34_regression {
    use super::*;

    #[test]
    fn smallmed_blue_markers_and_mechanical_fittings_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (962, 16, 9),
            (963, 16, 9),
            (964, 16, 9),
            (965, 16, 17),
            (966, 16, 17),
            (967, 16, 9),
            (968, 16, 9),
            (969, 16, 9),
            (970, 16, 9),
            (971, 16, 9),
            (972, 16, 9),
            (973, 16, 9),
            (974, 16, 9),
            (975, 16, 9),
            (976, 16, 9),
            (977, 16, 9),
            (978, 32, 20),
            (979, 32, 43),
            (980, 32, 33),
            (981, 32, 29),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch35_regression {
    use super::*;

    #[test]
    fn smallmed_fire_and_smoke_fragments_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (896, 16, 14),
            (897, 16, 15),
            (898, 16, 15),
            (899, 16, 15),
            (900, 16, 9),
            (901, 16, 9),
            (902, 16, 14),
            (903, 16, 15),
            (904, 16, 11),
            (905, 16, 7),
            (906, 16, 9),
            (907, 16, 7),
            (908, 16, 9),
            (909, 16, 9),
            (910, 16, 14),
            (911, 16, 14),
            (912, 16, 14),
            (913, 16, 14),
            (914, 16, 15),
            (915, 16, 14),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch36_regression {
    use super::*;

    #[test]
    fn smallmed_bluegray_stripes_and_marker_variants_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (942, 32, 28),
            (943, 32, 22),
            (944, 32, 22),
            (945, 32, 23),
            (946, 32, 15),
            (947, 32, 23),
            (948, 32, 23),
            (949, 32, 21),
            (950, 16, 9),
            (951, 16, 9),
            (952, 16, 15),
            (953, 16, 15),
            (954, 16, 11),
            (955, 16, 11),
            (956, 16, 9),
            (957, 16, 9),
            (958, 16, 9),
            (959, 16, 9),
            (960, 16, 9),
            (961, 16, 9),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}

#[cfg(test)]
mod smallmed_batch37_regression {
    use super::*;

    #[test]
    fn smallmed_mechanical_and_weather_effect_sprites_keep_source_canvases() {
        let Some(root) = crate::paths::test_replacement_root() else {
            return;
        };
        let source = load_replacement_sprite_source(&root)
            .unwrap()
            .expect("reviewed replacement atlas");
        let expected = [
            (982, 32, 13),
            (983, 32, 25),
            (984, 32, 19),
            (985, 32, 32),
            (986, 32, 37),
            (987, 32, 35),
            (988, 32, 21),
            (989, 32, 23),
            (990, 32, 45),
            (991, 32, 45),
            (992, 16, 9),
            (993, 16, 13),
            (994, 16, 14),
            (995, 16, 7),
            (996, 32, 26),
            (997, 32, 63),
            (998, 32, 63),
            (999, 32, 63),
        ];
        for (id, width, height) in expected {
            let frame = source.frame_by_id(id).expect("new SMALLMED replacement");
            assert_eq!((frame.w, frame.h), (width * 8, height * 8), "sprite {id}");
            assert_eq!(frame.logical_size, Some([width, height]), "sprite {id}");
            assert_eq!(
                frame.anchor,
                Some([width as f32 / 2.0, height as f32]),
                "sprite {id}"
            );
        }
        let atlas = source.load_rgba().unwrap();
        assert!(atlas.width() <= 6656);
        assert!(atlas.height() <= 16_384);
    }
}
