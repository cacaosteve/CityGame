use super::*;

impl City {
    /// PE `wScenarioTimeLimitMonths` (`0x4CC91C`), SCEN BE u16 @ `0x08`.
    pub fn scenario_time_limit_months(&self) -> Option<u16> {
        let raw = self.scen_chunk_bytes()?;
        if raw.len() < 0x0a {
            return None;
        }
        Some(u16::from_be_bytes([raw[0x08], raw[0x09]]))
    }

    pub fn set_scenario_time_limit_months(&mut self, months: u16) {
        let mut raw = self.scen_chunk_bytes().unwrap_or_default();
        if raw.len() < 0x0a {
            raw.resize(0x0a, 0);
        }
        raw[0x08..0x0a].copy_from_slice(&months.to_be_bytes());
        // Stock Win95 SCEN chunks are raw records, unlike compressed map
        // planes. Preserve that on disk even when reading an older CityGame
        // save that encoded the record using the generic RLE writer.
        if let Some(index) = self.chunks.iter().position(|chunk| chunk.name == "SCEN") {
            self.chunks[index].raw_len = raw.len();
            self.chunks[index].data = raw;
            self.chunk_index.insert("SCEN".to_owned(), index);
        } else {
            let index = self.chunks.len();
            self.chunks.push(CityChunk {
                name: "SCEN".to_owned(),
                offset: 0,
                raw_len: raw.len(),
                data: raw,
            });
            self.chunk_index.insert("SCEN".to_owned(), index);
        }
        self.modified = true;
    }

    pub fn pe_scenario_deadline_expired(&self) -> bool {
        self.pe_scenario_deadline_expired
    }

    pub fn set_pe_scenario_deadline_expired(&mut self, expired: bool) {
        self.pe_scenario_deadline_expired = expired;
    }

    /// Win95 runtime scenario-active global `0x4CAD44` (not persisted in MISC).
    pub fn pe_scenario_active(&self) -> bool {
        self.pe_scenario_active
    }

    pub fn set_pe_scenario_active(&mut self, active: bool) {
        self.pe_scenario_active = active;
    }

    fn scen_chunk_bytes(&self) -> Option<Vec<u8>> {
        let chunk = self.chunks.iter().find(|chunk| chunk.name == "SCEN")?;
        Some(sc2_rle_decode(&chunk.data).unwrap_or_else(|_| chunk.data.clone()))
    }

    /// Decoded SCEN payload (RLE-aware) for scenario goal parsing.
    pub fn scen_bytes(&self) -> Option<Vec<u8>> {
        self.scen_chunk_bytes()
    }
}
