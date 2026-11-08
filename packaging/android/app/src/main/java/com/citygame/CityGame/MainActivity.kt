package com.citygame.CityGame

import android.os.Bundle
import org.libsdl.app.SDLActivity
import java.io.File
import java.util.zip.ZipInputStream

/**
 * SDL3 Activity host. Loads libSDL3 + libcitygame (exports SDL_main).
 * Seeds graphics from assets/SeedAssets.zip into the SDL pref path when empty.
 */
class MainActivity : SDLActivity() {
    override fun getLibraries(): Array<String> {
        return arrayOf("SDL3", "citygame")
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        seedAssetsIfNeeded()
        super.onCreate(savedInstanceState)
    }

    private fun seedAssetsIfNeeded() {
        // Matches sdl3::filesystem::get_pref_path("CityGame", "CityGame")
        val dest = File(filesDir, "CityGame/CityGame")
        val settings = File(dest, "asset-settings.json")
        if (settings.isFile) {
            return
        }
        try {
            assets.open("SeedAssets.zip").use { input ->
                dest.mkdirs()
                ZipInputStream(input).use { zis ->
                    var entry = zis.nextEntry
                    while (entry != null) {
                        val out = File(dest, entry.name)
                        if (entry.isDirectory) {
                            out.mkdirs()
                        } else {
                            out.parentFile?.mkdirs()
                            out.outputStream().use { zis.copyTo(it) }
                        }
                        zis.closeEntry()
                        entry = zis.nextEntry
                    }
                }
            }
        } catch (_: Exception) {
            // First-run folder picker remains as fallback inside the Rust sim.
        }
    }
}
