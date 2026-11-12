use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "sc2k-catalog-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn atlas(&self, pack: &str, directory: &str, ids: &[u32]) -> PathBuf {
        let root = self.0.join(pack);
        let dir = root.join(directory);
        fs::create_dir_all(&dir).unwrap();
        image::RgbaImage::new(64, 64)
            .save(dir.join("atlas.png"))
            .unwrap();
        let frames: Vec<_> = ids
            .iter()
            .map(|id| json!({"id":id,"x":0,"y":0,"w":32,"h":32,"source_offset":1234}))
            .collect();
        fs::write(
            dir.join("atlas.json"),
            serde_json::to_vec(&json!({"image":"atlas.png","frames":frames})).unwrap(),
        )
        .unwrap();
        root
    }
    fn replacement(&self, entries: serde_json::Value) -> PathBuf {
        let root = self.0.join("ours");
        fs::create_dir_all(&root).unwrap();
        image::RgbaImage::new(48, 64)
            .save(root.join("station.png"))
            .unwrap();
        fs::write(
            root.join("asset-pack.json"),
            serde_json::to_vec(&json!({
                "schema_version":1,"id":"our-city","name":"Our city","kind":"replacement",
                "author":"Test author","license":"CC0-1.0","assets":entries
            }))
            .unwrap(),
        )
        .unwrap();
        root
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn catalog_unifies_editions_without_losing_source_offsets_or_zoom_families() {
    let fixture = Fixture::new();
    let mac = fixture.atlas("mac", "sprites/SPRT_00129_ShapeData", &[211, 711, 1211]);
    let win = fixture.atlas("win", "win95_tiles/LARGE", &[1211]);
    let dos = fixture.atlas("dos", "tiles/SC2000_DAT/large", &[1211]);
    let catalog = Catalog::load(&[mac.clone(), win, dos, mac]).unwrap();
    assert_eq!(catalog.packs.len(), 3);
    assert_eq!(catalog.assets.len(), 3);
    let asset = catalog
        .assets
        .iter()
        .find(|a| a.sprite_id == Some(1211))
        .unwrap();
    assert_eq!(asset.variants.len(), 3);
    assert_eq!(asset.footprint, Some([3, 3]));
    assert_eq!(asset.zoom.as_deref(), Some("Large"));
    assert!(
        catalog
            .assets
            .iter()
            .all(|a| a.family == "citygame.shape.211")
    );
    assert!(asset.variants.iter().all(|v| v.source_offset == Some(1234)));
}

#[test]
fn catalog_brief_excludes_original_media_and_provenance() {
    let fixture = Fixture::new();
    let mac = fixture.atlas("mac", "sprites/shapes", &[1211]);
    let catalog = Catalog::load(&[mac]).unwrap();
    let brief = catalog.artist_brief().to_string();
    for forbidden in [
        "/media/",
        "atlas.png",
        "source_offset",
        "1234",
        fixture.0.to_str().unwrap(),
    ] {
        assert!(!brief.contains(forbidden), "brief leaked {forbidden}");
    }
    assert!(brief.contains("citygame.sprite.01211"));
    assert!(brief.contains("open_questions"));
}

#[test]
fn catalog_replacements_preserve_originals_and_resolve_regardless_of_input_order() {
    let fixture = Fixture::new();
    let mac = fixture.atlas("mac", "sprites/shapes", &[1211]);
    let ours=fixture.replacement(json!([{"key":"citygame.sprite.01211","file":"station.png","status":"reviewed","logical_size":[24,32],"anchor":[12,32],"animation":"static"}]));
    let catalog = Catalog::load(&[ours, mac]).unwrap();
    let variants = &catalog.assets[0].variants;
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].status, "imported");
    assert_eq!(variants[1].status, "reviewed");
    assert_eq!(variants[1].size, Some([48, 64]));
    assert_eq!(variants[1].logical_size, Some([24, 32]));
}

#[test]
fn catalog_rejects_unmapped_replacements_and_reviewed_images_without_contract() {
    let fixture = Fixture::new();
    let mac = fixture.atlas("mac", "sprites/shapes", &[1211]);
    let ours = fixture.replacement(
        json!([{"key":"citygame.sprite.09999","file":"station.png","status":"draft"}]),
    );
    assert!(
        Catalog::load(&[mac.clone(), ours])
            .unwrap_err()
            .to_string()
            .contains("unknown replacement key")
    );
    let ours=fixture.replacement(json!([{"key":"citygame.sprite.01211","file":"station.png","status":"reviewed","animation":"static"}]));
    assert!(
        Catalog::load(&[mac, ours])
            .unwrap_err()
            .to_string()
            .contains("anchor")
    );
}

#[test]
fn catalog_retains_duplicate_source_records_and_rejects_outside_files() {
    let fixture = Fixture::new();
    let mac = fixture.atlas("mac", "sprites/shapes", &[1211, 1211]);
    let catalog = Catalog::load(&[mac]).unwrap();
    assert_eq!(catalog.assets.len(), 1);
    assert_eq!(catalog.assets[0].variants.len(), 2);
    assert_eq!(catalog.assets[0].variants[1].source_record, Some(1));
    assert!(
        catalog.assets[0]
            .contract_notes
            .iter()
            .any(|note| note.contains("Multiple source records"))
    );
    let mac = fixture.atlas("mac", "sprites/shapes", &[1211]);
    let ours = fixture.replacement(
        json!([{"key":"citygame.sprite.01211","file":"../outside.png","status":"draft"}]),
    );
    assert!(
        Catalog::load(&[mac, ours])
            .unwrap_err()
            .to_string()
            .contains("inside its pack")
    );
}

#[test]
fn catalog_rejects_atlas_image_escape() {
    let fixture = Fixture::new();
    let mac = fixture.atlas("mac", "sprites/shapes", &[1211]);
    image::RgbaImage::new(64, 64)
        .save(fixture.0.join("outside.png"))
        .unwrap();
    fs::write(
        mac.join("sprites/shapes/atlas.json"),
        serde_json::to_vec(&json!({"image":"../../../outside.png","frames":[]})).unwrap(),
    )
    .unwrap();
    assert!(
        Catalog::load(&[mac])
            .unwrap_err()
            .to_string()
            .contains("escapes its pack")
    );
}

#[test]
fn catalog_bad_atlas_rectangle_is_reported_not_counted() {
    let fixture = Fixture::new();
    let mac = fixture.atlas("mac", "sprites/shapes", &[1211]);
    fs::write(
        mac.join("sprites/shapes/atlas.json"),
        serde_json::to_vec(&json!({"image":"atlas.png","frames":[
            {"id":1211,"x":0,"y":0,"w":32,"h":32},
            {"id":1212,"x":u32::MAX,"y":0,"w":32,"h":32}
        ]}))
        .unwrap(),
    )
    .unwrap();
    let catalog = Catalog::load(&[mac]).unwrap();
    assert_eq!(catalog.assets.len(), 1);
    assert_eq!(catalog.warnings.len(), 1);
}

#[test]
fn catalog_unknown_raw_media_is_not_claimed_as_supported() {
    let fixture = Fixture::new();
    assert!(
        Catalog::load(&[fixture.0.clone()])
            .unwrap_err()
            .to_string()
            .contains("Raw CDs and installers need extraction first")
    );
}
