//! Citywide service and civic report snapshots.

use super::*;

impl City {
    /// Win95 `wSewerBonus` @ `0x4cb3dc`.
    pub fn set_pe_w_sewer_bonus(&mut self, bonus: u16) {
        self.pe_w_sewer_bonus = bonus;
    }

    pub fn pe_w_sewer_bonus(&self) -> u16 {
        self.pe_w_sewer_bonus
    }

    /// PE 0x464868 reads a signed census WORD, divides by four with
    /// truncation toward zero, and compares the product as an unsigned DWORD.
    pub fn publish_pe_sewer_bonus(&mut self, served: u32, treatment_tiles: u16) {
        let threshold = (i32::from(treatment_tiles as i16) / 4 * 2_000) as u32;
        let bonus = u16::from(served <= threshold);
        self.pe_w_sewer_bonus = bonus;
        // MISC `+0x1050` is Mac CODE 4 state slot 1044, not the PE snapshot.
        if self.kind != CityFileKind::MacCity && self.misc_u32_at(MISC_SEWER_BONUS_OFFSET).is_some()
        {
            self.set_misc_u32_at(MISC_SEWER_BONUS_OFFSET, u32::from(bonus));
        }
    }
    pub fn reset_pe_transit_passenger_totals(&mut self) {
        self.pe_dw_bus_passengers = 0;
        self.pe_dw_rail_passengers = 0;
        self.pe_dw_subway_passengers = 0;
        // CODE 4 zero-fills these viewer-extension slots in Mac saves. Keep
        // resetting the PE runtime counters, but never publish the PE snapshot
        // into a Mac-layout city's serialized state.
        if self.kind != CityFileKind::MacCity {
            self.set_misc_u32_at(MISC_BUS_PASSENGERS_OFFSET, 0);
            self.set_misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET, 0);
            self.set_misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET, 0);
        }
    }

    /// Win95 `0x4CC4B8` coverage percent used by water/desal microsims.
    pub fn set_pe_water_coverage_percent(&mut self, percent: u32) {
        self.pe_water_coverage_percent = percent;
    }

    pub fn pe_water_coverage_percent(&self) -> u32 {
        self.pe_water_coverage_percent
    }

    /// Win95 `dwPowerUsedPercentage` @ `0x4caa50`.
    pub fn set_pe_power_used_percent(&mut self, percent: u32) {
        self.pe_power_used_percent = percent;
    }

    pub fn pe_power_used_percent(&self) -> u32 {
        self.pe_power_used_percent
    }

    /// Win95 `dwBusPassengers` @ `0x4c85a0`.
    pub fn set_pe_dw_bus_passengers(&mut self, passengers: u32) {
        self.pe_dw_bus_passengers = passengers;
    }

    pub fn pe_dw_bus_passengers(&self) -> u32 {
        self.pe_dw_bus_passengers
    }

    /// Win95 `dwRailPassengers` @ `0x4c85a4`.
    pub fn set_pe_dw_rail_passengers(&mut self, passengers: u32) {
        self.pe_dw_rail_passengers = passengers;
    }

    pub fn pe_dw_rail_passengers(&self) -> u32 {
        self.pe_dw_rail_passengers
    }

    /// Win95 `dwSubwayPassengers` @ `0x4c8600`.
    pub fn set_pe_dw_subway_passengers(&mut self, passengers: u32) {
        self.pe_dw_subway_passengers = passengers;
    }

    pub fn pe_dw_subway_passengers(&self) -> u32 {
        self.pe_dw_subway_passengers
    }

    pub fn pe_dw_city_population(&self) -> u32 {
        self.pe_dw_city_population
    }

    /// Keep Win95's runtime cache aligned with the shared saved population
    /// scalar when another selected engine publishes that field.
    pub fn set_pe_dw_city_population(&mut self, population: u32) {
        self.pe_dw_city_population = population;
    }

    /// Win95 `dwArcologyPopulation` @ `0x4c94c4`.
    pub fn set_pe_dw_arcology_population(&mut self, population: u32) {
        self.pe_dw_arcology_population = population;
    }

    pub fn pe_dw_arcology_population(&self) -> u32 {
        self.pe_dw_arcology_population
    }

    /// Win95 `wOldArrests` @ `0x4ca430`.
    pub fn set_pe_w_old_arrests(&mut self, arrests: u16) {
        self.pe_w_old_arrests = arrests;
    }

    pub fn pe_w_old_arrests(&self) -> u16 {
        self.pe_w_old_arrests
    }

    /// Win95 `wPrisonBonus` @ `0x4ca1dc` (0/1 after year-end finalize).
    pub fn set_pe_w_prison_bonus(&mut self, bonus: u16) {
        self.pe_w_prison_bonus = bonus;
    }

    pub fn pe_w_prison_bonus(&self) -> u16 {
        self.pe_w_prison_bonus
    }
}
