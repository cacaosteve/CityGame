//! Newspaper story-id presentation and CityGame-authored story copy.
//! Story thresholds and event selection remain in `newspaper_events.rs`.

use super::*;

/// Win95 power-plant aging bulletin id (`FUN_0044e541` → story `0x24`).
pub(super) const PE_NEWSPAPER_STORY_POWER_PLANT_AGING: u16 = 0x24;
/// OpenSC2K annual prison overcrowding / DATA `fundprison`.
pub(super) const PE_NEWSPAPER_STORY_PRISON_OVERCROWDED: u16 = 0x25;
/// OpenSC2K annual school underfunding / DATA `fundeducation`.
pub(super) const PE_NEWSPAPER_STORY_FUND_EDUCATION: u16 = 0x26;
/// Win95 `DoBudgetBridgeCheck` queues story 39 after a bridge collapse.
pub(super) const PE_NEWSPAPER_STORY_BRIDGE_COLLAPSE: u16 = 39;

pub(super) fn pe_top_newspaper_story_for_city(city: &City) -> Option<(u16, u8)> {
    let story_id = city.pe_newspaper_story_id_at(0)? as u16;
    let priority = city.pe_newspaper_story_priority_at(0).unwrap_or(0);
    // Default filler rows are ids 0x0B..=0x13 with priority 0.
    if priority == 0 && (0x0B..=0x13).contains(&story_id) {
        return None;
    }
    let param = city.pe_newspaper_story_param_at(0).unwrap_or(0) as u8;
    Some((story_id, param))
}

fn pe_power_plant_type_name(param: u8) -> &'static str {
    match param {
        0 => "gas",
        1 => "oil",
        2 => "nuclear",
        3 => "solar",
        4 => "microwave",
        5 => "fusion",
        6 => "coal",
        _ => "power",
    }
}

/// PE `0x45d179` / OpenSC2K DisasterEnd summary story labels (short packaging headlines).
pub(super) fn pe_disaster_end_summary_label(story_id: u16) -> Option<&'static str> {
    Some(match story_id {
        0x16 => "Fire",
        0x17 => "Flood",
        0x18 => "Plane Crash",
        0x19 => "Helicopter Crash",
        0x1a => "Tornado",
        0x1b => "Earthquake",
        0x1c => "Monster",
        0x1d => "Meltdown",
        0x1e => "Microwave",
        0x1f => "Volcano",
        0x20 => "Toxic Spill",
        0x21 => "Pollution",
        0x22 => "Hurricane",
        0x23 => "Riot",
        _ => return None,
    })
}

pub(super) fn pe_disaster_end_summary_headline(edition: NewspaperEdition, story_id: u16) -> String {
    let label = pe_disaster_end_summary_label(story_id).unwrap_or("Disaster");
    match edition {
        NewspaperEdition::Extra => format!("Extra: {label} Contained"),
        NewspaperEdition::Courier | NewspaperEdition::Subscription | NewspaperEdition::Founding => {
            format!("{label} Contained")
        }
    }
}

pub(super) fn pe_disaster_end_summary_story(city_name: &str, story_id: u16) -> String {
    let disaster = pe_disaster_end_summary_label(story_id).unwrap_or("Disaster");
    format!(
        "Officials in {city_name} report the {} has been contained. The immediate threat is over, and residents can begin recovery.",
        disaster.to_ascii_lowercase()
    )
}

pub(super) fn pe_power_plant_aging_headline(edition: NewspaperEdition, param: u8) -> String {
    let kind = pe_power_plant_type_name(param);
    match edition {
        NewspaperEdition::Extra => format!("Extra: Aging {kind} plant"),
        NewspaperEdition::Courier | NewspaperEdition::Subscription | NewspaperEdition::Founding => {
            format!("Aging {kind} power plant")
        }
    }
}

pub(super) fn pe_power_plant_aging_story(city_name: &str, param: u8) -> String {
    let kind = pe_power_plant_type_name(param);
    format!(
        "{city_name} engineers warn that an aging {kind} power plant is nearing the end of its design life."
    )
}

pub(super) fn pe_fund_education_headline(edition: NewspaperEdition) -> String {
    match edition {
        NewspaperEdition::Extra => "Extra: Schools underfunded".to_string(),
        NewspaperEdition::Courier | NewspaperEdition::Subscription | NewspaperEdition::Founding => {
            "Schools need funding".to_string()
        }
    }
}

pub(super) fn pe_fund_education_story(city_name: &str) -> String {
    format!(
        "Parents in {city_name} complain that school funding is too low to keep classrooms running."
    )
}

pub(super) fn pe_prison_overcrowded_headline(edition: NewspaperEdition) -> String {
    match edition {
        NewspaperEdition::Extra => "Extra: Prisons overcrowded".to_string(),
        NewspaperEdition::Courier | NewspaperEdition::Subscription | NewspaperEdition::Founding => {
            "Prisons overcrowded".to_string()
        }
    }
}

pub(super) fn pe_prison_overcrowded_story(city_name: &str) -> String {
    format!("Wardens in {city_name} report prison populations have pushed past safe capacity.")
}

pub(super) fn pe_bridge_collapse_headline(edition: NewspaperEdition) -> String {
    match edition {
        NewspaperEdition::Extra => "Extra: Bridge collapse disrupts traffic".to_string(),
        NewspaperEdition::Courier | NewspaperEdition::Subscription | NewspaperEdition::Founding => {
            "Bridge collapse disrupts traffic".to_string()
        }
    }
}

pub(super) fn pe_bridge_collapse_story(city_name: &str) -> String {
    format!(
        "A weakened bridge in {city_name} has collapsed, interrupting travel across the route. Crews must clear the damaged structure before normal traffic can return."
    )
}

/// OpenSC2K `CityStatusBar.NEWS_NAMES` / `CityStatusMessages.NEEDS` for LE/EQ/
/// employment papers and `FUN_00471BC0` condition papers (`0x2E..=0x3C`).
pub(super) fn pe_health_amenities_newspaper_label(story_id: u16) -> Option<&'static str> {
    Some(match story_id {
        0x10 => "High crime",
        0x11 => "High traffic",
        0x12 => "High pollution",
        0x13 => "Poor education",
        0x14 => "Poor health",
        0x15 => "Poor employment",
        0x3d => "Low crime",
        0x3e => "Low traffic",
        0x3f => "Low pollution",
        0x40 => "Good education",
        0x41 => "Good health",
        0x42 => "Good employment",
        0x2e => "Power Plant Needed",
        0x2f => "Citizens Demand Road & Rail",
        0x30 => "Citizens Demand Police",
        0x31 => "Fire Protection Demanded",
        0x32 => "Water Shortage Reported",
        0x33 => "Hospitals Demanded",
        0x34 => "Citizens Demand Schools",
        0x35 => "Industry Demands Seaport",
        0x36 => "Commerce Demands Airport",
        0x37 => "Residents Demand Zoo",
        0x38 => "Residents Demand Stadium",
        0x39 => "Residents Demand Marina",
        0x3a => "Residents Demand Park",
        0x3b => "Industry Needs Connections",
        0x3c => "Commerce Needs Connections",
        _ => return None,
    })
}

pub(super) fn pe_health_amenities_newspaper_headline(
    edition: NewspaperEdition,
    story_id: u16,
) -> String {
    let label = pe_health_amenities_newspaper_label(story_id).unwrap_or("City report");
    match edition {
        NewspaperEdition::Extra => format!("Extra: {label}"),
        NewspaperEdition::Courier | NewspaperEdition::Subscription | NewspaperEdition::Founding => {
            label.to_string()
        }
    }
}

pub(super) fn pe_health_amenities_newspaper_story(city_name: &str, story_id: u16) -> String {
    match story_id {
        0x10 => format!("Residents of {city_name} report crime rates climbing across the city."),
        0x11 => format!("Commuters in {city_name} say traffic congestion has become unbearable."),
        0x12 => format!("Smog and waste foul {city_name} as pollution readings stay high."),
        0x13 => format!(
            "Teachers in {city_name} warn that education scores have slipped below the city's needs."
        ),
        0x14 => format!(
            "Doctors in {city_name} warn that life expectancy has fallen and clinics are overloaded."
        ),
        0x15 => format!(
            "Job seekers in {city_name} say unemployment is leaving too many residents idle."
        ),
        0x3d => format!("Police in {city_name} report crime has fallen to welcome lows."),
        0x3e => format!("Drivers in {city_name} enjoy lighter traffic on city roads."),
        0x3f => format!("Clean air returns to {city_name} as pollution readings improve."),
        0x40 => {
            format!("Schools in {city_name} report rising education scores across the workforce.")
        }
        0x41 => {
            format!("Residents of {city_name} celebrate longer, healthier lives across the city.")
        }
        0x42 => format!("Employers in {city_name} report strong hiring as unemployment stays low."),
        0x2e => format!("Brownouts hit {city_name} as power plants struggle to meet demand."),
        0x2f => {
            format!("Commuters in {city_name} demand more roads and rail to keep traffic moving.")
        }
        0x30 => {
            format!("Crime fears rise in {city_name} as residents demand more police coverage.")
        }
        0x31 => format!("Firefighters in {city_name} say stations cannot cover the growing city."),
        0x32 => {
            format!("Taps run dry across {city_name} as the water network falls behind demand.")
        }
        0x33 => format!(
            "Patients in {city_name} say hospital beds and staff cannot keep up with demand."
        ),
        0x34 => {
            format!("Parents in {city_name} demand more schools for a growing student population.")
        }
        0x35 => format!("Industry leaders in {city_name} say the city still needs a seaport."),
        0x36 => format!("Business travelers in {city_name} demand airport capacity for commerce."),
        0x37 => format!(
            "Families in {city_name} want a zoo so recreation keeps pace with residential growth."
        ),
        0x38 => format!(
            "Sports fans in {city_name} demand a stadium as residential tax receipts climb."
        ),
        0x39 => format!(
            "Boat owners in {city_name} say the city still lacks a marina for waterfront leisure."
        ),
        0x3a => format!(
            "Neighbors in {city_name} petition for more parks as residential density rises."
        ),
        0x3b => format!(
            "Factories in {city_name} need better freight connections to keep industry growing."
        ),
        0x3c => format!(
            "Shopkeepers in {city_name} say commercial districts still lack transit connections."
        ),
        _ => format!("City report from {city_name}."),
    }
}
