fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        println!("cargo:rerun-if-changed=native/ios_documents.m");
        cc::Build::new()
            .file("native/ios_documents.m")
            .flag("-fobjc-arc")
            .compile("citygame_ios_documents");
        for framework in ["UIKit", "Foundation", "UniformTypeIdentifiers"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=native/mac_menu.m");
        cc::Build::new()
            .file("native/mac_menu.m")
            .file("native/title_midi.m")
            .flag("-fobjc-arc")
            .compile("sc2000_mac_menu");

        println!("cargo:rerun-if-changed=native/folder_picker.m");
        println!("cargo:rerun-if-changed=native/folder_picker.plist");
        let helper = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
            .join("CityGameFolderPicker");
        let status = cc::Build::new()
            .flag("-fobjc-arc")
            .get_compiler()
            .to_command()
            .arg("native/folder_picker.m")
            .args(["-framework", "Cocoa", "-o"])
            .arg(&helper)
            .status()
            .expect("compile the native macOS folder picker");
        assert!(
            status.success(),
            "native macOS folder picker compilation failed"
        );

        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rerun-if-changed=native/title_midi.m");
    }
}
