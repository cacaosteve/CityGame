//! Saved newspaper selection and PE MISC paper/story-list initialization.
//! Event conditions and authored story presentation live in separate modules.

use super::*;

#[derive(Default)]
pub(super) struct MacPaperRuntime {
    pub(super) tick_word: Option<i16>,
    pub(super) data: Option<std::sync::Arc<crate::mac_newspaper::MacPaperTokens>>,
    pub(super) font: Option<std::sync::Arc<crate::mac_font_metrics::MacFontFamily>>,
    pub(super) strings: Option<std::sync::Arc<crate::mac_newspaper::MacPaperDisplayStrings>>,
    pub(super) pictures: Option<[bool; 12]>,
    pub(super) paint: Option<MacPaperPaint>,
    pub(super) headline: Option<String>,
    pub(super) article: Option<String>,
    pub(super) detail_open: bool,
    pub(super) detail_scroll: usize,
    detail_choices: Option<[u8; 3]>,
}

pub(super) struct PePaperArticle {
    pub(super) headline: String,
    pub(super) body: String,
    pub(super) scroll: usize,
}

pub(super) struct MacPaperPaint {
    pub(super) family: usize,
    pub(super) initial_seed: u32,
    pub(super) lead: crate::mac_newspaper::MacLeadLayout,
    pub(super) stories: Vec<crate::mac_newspaper::MacShortStory>,
    pub(super) weather: crate::mac_newspaper::MacAuxiliaryBox,
    pub(super) survey: crate::mac_newspaper::MacAuxiliaryBox,
}

impl ViewerState {
    pub(super) fn configure_original_newspaper_data(
        &mut self,
        profile: &crate::asset_profile::AssetProfile,
    ) {
        self.mac_system_font = profile.mac_system_chicago_path().and_then(|path| {
            let result = std::fs::read(&path).map_err(anyhow::Error::from)
                .and_then(|bytes| crate::mac_font_metrics::MacFontFamily::from_suitcase(&bytes, 0));
            match result {
                Ok(mut family) => family.bitmap.remove(&12).map(std::sync::Arc::new),
                Err(error) => { log::warn!("could not load Macintosh system font {}: {error:#}", path.display()); None }
            }
        });
        self.pe_registered_mayor_name = profile.win95_mayor_name().to_owned();
        self.pe_new_city_reserved_labels = profile.win95_new_city_labels_path().and_then(|path| {
            let labels = std::fs::read(&path)
                .map_err(anyhow::Error::from)
                .and_then(|bytes| serde_json::from_slice::<[String; 5]>(&bytes).map_err(Into::into));
            match labels {
                Ok(labels) if labels.iter().all(|label| !label.contains('\0')) => Some(labels),
                Ok(_) => {
                    log::warn!("Win95 new-city labels contain a NUL: {}", path.display());
                    None
                }
                Err(error) => {
                    log::warn!("could not load Win95 new-city labels {}: {error:#}", path.display());
                    None
                }
            }
        });
        self.mac_newspaper.pictures = profile.mac_newspaper_picture_index_path().and_then(|path| {
            let bytes = std::fs::read(path).ok()?;
            let values: [u8; 12] = bytes.try_into().ok()?;
            values.iter().all(|&n| n <= 1).then(|| values.map(|n| n != 0))
        });
        self.mac_newspaper.strings = profile.mac_newspaper_strings_path().and_then(|path| {
            match std::fs::read(&path).map_err(anyhow::Error::from)
                .and_then(|bytes| crate::mac_newspaper::MacPaperDisplayStrings::from_strs(&bytes)) {
                Ok(strings) => Some(std::sync::Arc::new(strings)),
                Err(error) => { log::warn!("could not load Macintosh newspaper strings {}: {error:#}", path.display()); None }
            }
        });
        self.mac_newspaper.font = profile.mac_newspaper_font_path().and_then(|path| {
            let font = std::fs::read(&path).map_err(anyhow::Error::from)
                .and_then(|bytes| crate::mac_font_metrics::MacFontFamily::from_suitcase(&bytes, 2));
            match font {
                Ok(font) => Some(std::sync::Arc::new(font)),
                Err(error) => {
                    log::warn!("could not load Macintosh newspaper font {}: {error:#}", path.display());
                    None
                }
            }
        });
        self.mac_newspaper.data = profile.mac_newspaper_data_paths().and_then(|paths| {
            match crate::mac_newspaper::MacPaperTokens::load(&paths) {
                Ok(data) => Some(std::sync::Arc::new(data)),
                Err(error) => {
                    log::warn!("could not load Macintosh newspaper data: {error:#}");
                    None
                }
            }
        });
        self.mac_newspaper.headline = None;
        self.mac_newspaper.article = None;
        self.mac_newspaper.detail_open = false;
        self.mac_newspaper.paint = None;
        self.pe_newspaper_data = profile
            .win95_newspaper_data_paths()
            .and_then(|(data, index)| {
                match crate::win95_newspaper::PeNewspaperData::load(&data, &index) {
                    Ok(data) => Some(std::sync::Arc::new(data)),
                    Err(error) => {
                        if self.engine_fidelity.uses_win95()
                            && let Some(open_error) = error.downcast_ref::<crate::win95_newspaper::PeArchiveOpenError>()
                        {
                            crate::win95_file_error::record_open_io_error(&open_error.path, &open_error.error);
                        }
                        log::warn!("could not load Win95 newspaper data: {error:#}");
                        None
                    }
                }
            });
        self.pe_newspaper_paint = None;
        self.pe_newspaper_article = None;
        self.pe_newspaper_paint_attempted = false;
    }

    /// Prepare the local preview and execute the original first paint when all
    /// user-supplied newspaper and System resources are available.
    pub(super) fn prepare_mac_paper_headline(&mut self) {
        self.compose_mac_paper_text(false, 0, 4);
        self.mac_newspaper.paint = None;
        if let Err(error) = self.prepare_mac_paper_first_paint() {
            log::debug!("Mac newspaper paint unavailable: {error:#}");
        }
    }

    fn prepare_mac_paper_first_paint(&mut self) -> anyhow::Result<()> {
        use crate::mac_newspaper::{MacPaperSlot, layout_lead_and_picture, layout_short_stories, layout_weather_box, layout_survey_box};
        use anyhow::{Context, ensure};
        let data = self.mac_newspaper.data.clone().context("missing newspaper tokens")?;
        let font = self.mac_newspaper.font.clone().context("missing New York font")?;
        let strings = self.mac_newspaper.strings.clone().context("missing newspaper strings")?;
        let pictures = self.mac_newspaper.pictures.context("missing picture index")?;
        let tick = self.mac_newspaper.tick_word.context("missing newspaper TickCount")?;
        let page = if matches!(
            self.city_info_dialog,
            Some(CityInfoDialog::Newspaper(
                NewspaperEdition::MacStandard | NewspaperEdition::MacStoryWindow
            ))
        ) {
            self.city.mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET).unwrap_or(0) as u16 as usize
        } else { 0 };
        ensure!(page < 6, "invalid newspaper selection");
        let paper_field = |field: usize| -> anyhow::Result<usize> {
            Ok(self.city.mac_state_misc_u32_at(MISC_NEWSPAPER_LIST_OFFSET + page*20 + field*4)
                .context("missing newspaper record")? as u8 as usize)
        };
        let family = paper_field(1)?;
        let survey_heading = paper_field(3)?;
        let weather_heading = paper_field(4)?;
        let age = self.city.mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET).unwrap_or(0) as i32;
        let records: [[u32; 6]; 9] = std::array::from_fn(|i| self.city.mac_story_record(i));
        let mut slots: [MacPaperSlot; 9] = std::array::from_fn(|i| MacPaperSlot {
            value: records[i][2] as u8, choice5: records[i][3] as u8,
            choice6: records[i][4] as u8, number: records[i][5] as u8,
        });
        let groups: [usize; 9] = records.map(|row| row[0] as u16 as usize);
        let city_name = crate::city::mac_roman::encode(&self.city.name, 255);
        let mayor = crate::city::mac_roman::encode(self.city.label_at(0).unwrap_or(""), 24);
        let mut labels = vec![0u8; 507*25];
        for index in 0..=u8::MAX {
            let text = crate::city::mac_roman::encode(self.city.label_at(index).unwrap_or(""), 24);
            let offset = usize::from(index)*25;
            labels[offset] = text.len() as u8;
            labels[offset+1..offset+1+text.len()].copy_from_slice(&text);
        }
        // Work on local copies so an incomplete asset pack cannot leave a
        // partially advanced RNG or partially changed story caches.
        let mut rng = self.mac_process_rng;
        let initial_seed = rng.seed();
        let lead = layout_lead_and_picture(&data,&font,family,age,tick,page as i16,groups[0],&mut slots[0],
            &city_name,&mayor,&labels,&|id| (400..=411).contains(&id) && pictures[usize::from(id-400)],&mut rng)?;
        let mut short: [MacPaperSlot;4] = std::array::from_fn(|i| slots[i+1].clone());
        let stories = layout_short_stories(&data,&font,family,age,tick,page as i16,
            groups[1..5].try_into().unwrap(),&mut short,&city_name,&mayor,&labels,&mut rng)?;
        slots[1..5].clone_from_slice(&short);
        let weather_values = [MISC_HEAT_OFFSET, MISC_WIND_OFFSET, MISC_HUMID_OFFSET]
            .map(|offset| self.city.mac_state_misc_u32_at(offset).unwrap_or(0) as u8);
        let weather = layout_weather_box(&data,&font,&strings,family,weather_heading,age,tick,page as i16,
            groups[7],&mut slots[7],&city_name,&mayor,&labels,weather_values,&mut rng)?;
        let (approval, counts, order) = self.city.mac_opinion_poll_results();
        let survey = layout_survey_box(&data,&font,&strings,family,survey_heading,age,tick,page as i16,
            groups[8],&mut slots[8],&city_name,&mayor,&labels,approval,counts,order,&mut rng)?;
        for i in [0,1,2,3,4,7,8] {
            self.city.set_mac_story_choices(i,[slots[i].choice5,slots[i].choice6,slots[i].number]);
        }
        self.mac_process_rng = rng;
        self.mac_newspaper.paint = Some(MacPaperPaint { family, initial_seed, lead, stories, weather, survey });
        Ok(())
    }

    fn compose_mac_paper_text(&mut self, use_shared_rng: bool, story: usize, position: usize) {
        self.mac_newspaper.headline = None;
        self.mac_newspaper.article = None;
        self.mac_newspaper.detail_open = false;
        self.mac_newspaper.detail_scroll = 0;
        self.mac_newspaper.detail_choices = None;
        let (Some(data), Some(tick)) = (&self.mac_newspaper.data, self.mac_newspaper.tick_word)
        else {
            return;
        };
        let row = self.city.mac_story_record(story);
        let mut slot = crate::mac_newspaper::MacPaperSlot {
            value: row[2] as u8,
            choice5: row[3] as u8,
            choice6: row[4] as u8,
            number: row[5] as u8,
        };
        let mut groups = [255; 256];
        let city_name = crate::city::mac_roman::encode(&self.city.name, 255);
        let mayor_name = crate::city::mac_roman::encode(self.city.label_at(0).unwrap_or(""), 24);
        // The native label table has 256 Pascal strings of 25 bytes. `>`
        // indexes records 251 onward (the saved team-name labels).
        let mut labels = vec![0u8; 256 * 25];
        for index in 0..=u8::MAX {
            let text = crate::city::mac_roman::encode(self.city.label_at(index).unwrap_or(""), 24);
            let offset = usize::from(index) * 25;
            labels[offset] = text.len() as u8;
            labels[offset + 1..offset + 1 + text.len()].copy_from_slice(&text);
        }
        let mut context = crate::mac_newspaper::MacPaperContext {
            slot: &mut slot,
            group_choices: &mut groups,
            auxiliary: &city_name,
            mayor_name: &mayor_name,
            labels: &labels,
        };
        let age = self.city.mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET).unwrap_or(0) as i32;
        // CODE 2 and the subscription caller pass saved choice WORD 1027;
        // the disaster-completion caller explicitly passes page zero.
        let page = if matches!(
            self.city_info_dialog,
            Some(CityInfoDialog::Newspaper(
                NewspaperEdition::MacStandard | NewspaperEdition::MacStoryWindow
            ))
        ) {
            self.city.mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET).unwrap_or(0) as u16 as i16
        } else { 0 };
        if !(0..=5).contains(&page) { return; }
        let seed = crate::mac_newspaper::story_seed(age, tick, page, (position as i32)*7);
        let mut preview_rng = MacToolboxRng::new(seed);
        let rng = if use_shared_rng { &mut self.mac_process_rng } else { &mut preview_rng };
        *rng = MacToolboxRng::new(seed);
        match data.headline(row[0] as u16 as usize, &mut context, &mut || rng.random_u16()) {
            Ok(bytes) => self.mac_newspaper.headline = Some(crate::city::mac_roman::decode(&bytes)),
            Err(error) => {
                log::debug!("Mac newspaper headline unavailable: {error:#}");
                return;
            }
        }
        // FUN_33866 reseeds to the same position before generating the body;
        // it retains the headline's slot and group caches.
        *rng = MacToolboxRng::new(seed);
        match data.article(row[0] as u16 as usize, &mut context, &mut || rng.random_u16()) {
            Ok(bytes) => {
                self.mac_newspaper.article = Some(crate::city::mac_roman::decode(&bytes));
                self.mac_newspaper.detail_choices = Some([slot.choice5, slot.choice6, slot.number]);
            }
            Err(error) => log::debug!("Mac newspaper article unavailable: {error:#}"),
        }
    }

    /// CODE 7 FUN_33866 replays the headline and article on the process
    /// QuickDraw stream, then displays the article until the detail is closed.
    pub(super) fn open_mac_paper_detail(&mut self) {
        self.open_mac_paper_detail_at(4);
    }

    /// CODE 7 maps page positions 4–10 to story records 0,8,7,1,2,3,4.
    pub(super) fn open_mac_paper_detail_at(&mut self, position: usize) {
        let Some(&story) = [0usize,8,7,1,2,3,4].get(position.wrapping_sub(4)) else { return; };
        // Re-run both native workers at click time using current story state.
        self.compose_mac_paper_text(true, story, position);
        if self.mac_newspaper.article.is_none() { return; }
        if let Some(choices) = self.mac_newspaper.detail_choices {
            self.city.set_mac_story_choices(story, choices);
        }
        self.mac_newspaper.detail_open = true;
        self.mac_newspaper.detail_scroll = 0;
    }

    /// Closing the original detail window exposes and repaints the paper.
    pub(super) fn close_mac_paper_detail(&mut self) {
        self.mac_newspaper.detail_open = false;
        self.compose_mac_paper_text(false, 0, 4);
        if let Err(error) = self.prepare_mac_paper_first_paint() {
            log::debug!("Mac newspaper repaint unavailable: {error:#}");
        }
    }

    pub(super) fn scroll_mac_paper_detail(&mut self, delta: i32) -> bool {
        if !self.mac_newspaper.detail_open { return false; }
        let max = self.mac_newspaper.article.as_deref().map_or(0, newspaper::detail_scroll_max);
        self.mac_newspaper.detail_scroll = self.mac_newspaper.detail_scroll.saturating_add_signed(delta as isize).min(max);
        true
    }

    /// Open the clicked PE story through the recovered 0x479b90 worker.
    pub(super) fn open_pe_newspaper_article(&mut self, position: u16) -> bool {
        let Some(CityInfoDialog::Newspaper(edition)) = self.city_info_dialog else { return false; };
        if !self.engine_fidelity.uses_win95() || self.pe_newspaper_article.is_some() { return false; }
        let (Some(data), Some(tick), Some(paint)) =
            (self.pe_newspaper_data.clone(), self.pe_document_tick_word, self.pe_newspaper_paint.as_ref())
        else { return false; };
        let style = paint.style;
        let paper = if edition == NewspaperEdition::Founding { 0 } else {
            newspaper_choice_for_city(&self.city) as usize
        };
        let Some(days) = self.city.city_age_days() else { return false; };
        let Some(state) = crate::win95_newspaper::PeHeadlineContextState::from_city(&self.city) else { return false; };
        let state = std::cell::RefCell::new(state);
        let context = crate::win95_newspaper::PeHeadlineContext { city_name_oem: &[], labels_oem: &[], mutable_state: Some(&state) };
        let seed = crate::win95_newspaper::PeNewspaperSeed {
            days: days as i32, startup_tick: tick, paper: paper as i32, slot: position as i16,
        };
        let mut next_city = self.city.clone();
        let mut next_crt = self.pe_process_crt;
        let Some((headline, body)) = data.render_city_article_ascii(
            &mut next_city, style, seed, &mut next_crt, Some(&context),
        ) else { return false; };
        let (headline, body) = (crate::win95_oem::decode_oem(&headline), crate::win95_oem::decode_oem(&body));
        state.into_inner().apply_to_city(&mut next_city);
        self.city = next_city;
        self.pe_process_crt = next_crt;
        self.pe_newspaper_article = Some(PePaperArticle { headline, body: body.replace('\n', "\r"), scroll: 0 });
        true
    }

    pub(super) fn scroll_pe_newspaper_article(&mut self, delta: i32) -> bool {
        let Some(article) = self.pe_newspaper_article.as_mut() else { return false; };
        let max = newspaper::pe_detail_scroll_max(&article.body);
        article.scroll = article.scroll.saturating_add_signed(delta as isize).min(max);
        true
    }

    /// Compose at most once per opened dialog. SDL redraws at frame rate,
    /// whereas the PE's newspaper workers consume RNG on window paints.
    pub(super) fn prepare_pe_newspaper_first_paint(&mut self) {
        let Some(CityInfoDialog::Newspaper(edition)) = self.city_info_dialog else {
            return;
        };
        if !self.engine_fidelity.uses_win95() || self.pe_newspaper_paint_attempted {
            return;
        }
        self.pe_newspaper_paint_attempted = true;
        let (Some(data), Some(tick)) = (self.pe_newspaper_data.clone(), self.pe_document_tick_word)
        else {
            return;
        };
        let Some(state) = crate::win95_newspaper::PeHeadlineContextState::from_city(&self.city) else { return; };
        let state = std::cell::RefCell::new(state);
        let context = crate::win95_newspaper::PeHeadlineContext {
            city_name_oem: &[],
            labels_oem: &[],
            mutable_state: Some(&state),
        };
        let paper = if edition == NewspaperEdition::Founding {
            0
        } else {
            newspaper_choice_for_city(&self.city) as usize
        };
        self.pe_newspaper_paint = data.render_paper_region_ascii(
            &mut self.city,
            paper,
            tick,
            &mut self.pe_process_crt,
            None,
            Some(&context),
            |text| {
                let text = crate::city::win1252::decode(text);
                // A portable approximation until the selected native GDI font is recovered.
                crate::newspaper_font::width(&text, 10.0 / 7.2).ceil() as i32
            },
        );
        if self.pe_newspaper_paint.is_some() {
            state.into_inner().apply_to_city(&mut self.city);
        }
    }
}

pub(super) fn newspaper_open_choice_for_edition(city: &City, edition: NewspaperEdition) -> u32 {
    let max_choice = newspaper_max_choice_for_city(city);
    match edition {
        NewspaperEdition::Courier if max_choice >= NEWSPAPER_COURIER_CHOICE => {
            NEWSPAPER_COURIER_CHOICE
        }
        _ => newspaper_choice_for_city(city).min(max_choice),
    }
}

pub(super) fn newspaper_paper_name(choice: u32) -> &'static str {
    NEWSPAPER_PAPER_NAMES[(choice.min(NEWSPAPER_CHOICE_MAX)) as usize]
}

/// The PE indexes its six name resources through byte zero of each saved
/// five-byte paper record (menu construction at 0x406d70).
pub(super) fn pe_newspaper_masthead(city: &City, paper: usize) -> Option<String> {
    const PE_PAPER_NAMES: [&str; 6] = ["Picayune", "Courier", "Herald", "Journal", "Times", "Chronicle"];
    let tables = city.pe_newspaper_tables()?;
    let name_index = (*tables.papers.get(paper.checked_mul(5)?)? as u8) as usize;
    Some(format!("{} {}", city.name, PE_PAPER_NAMES.get(name_index)?))
}

/// Win95 `0x479736`: paper record `i` is `[i, i%3, i%3, i, i]`.
pub(super) fn pe_newspaper_paper_list_identity_bytes() -> [u8; NEWSPAPER_LIST_MISC_DWORDS] {
    let mut recs = [0u8; NEWSPAPER_LIST_MISC_DWORDS];
    for index in 0..NEWSPAPER_LIST_RECORD_COUNT as u8 {
        let rec = usize::from(index) * NEWSPAPER_LIST_RECORD_BYTES;
        recs[rec] = index;
        recs[rec + 1] = index % 3;
        recs[rec + 2] = index % 3;
        recs[rec + 3] = index;
        recs[rec + 4] = index;
    }
    recs
}

#[cfg(test)]
fn pe_newspaper_shuffle_seed(city: &City) -> u32 {
    let mut seed = 0x4797_3000u32;
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(super) trait PeNewspaperRandom {
    fn random_u16(&mut self) -> u16;
}

impl PeNewspaperRandom for MacToolboxRng {
    fn random_u16(&mut self) -> u16 {
        MacToolboxRng::random_u16(self)
    }
}

impl PeNewspaperRandom for Win95CrtRand {
    fn random_u16(&mut self) -> u16 {
        Win95CrtRand::random_u16(self)
    }
}

pub(super) fn pe_newspaper_random_mod<R: PeNewspaperRandom>(rng: &mut R, modulus: u16) -> usize {
    usize::from(rng.random_u16() & 0x7fff) % usize::from(modulus)
}

/// Win95 `0x47977c`: CRT-driven pairwise byte swaps via `0x402a90`.
pub(super) fn pe_newspaper_paper_list_shuffled_bytes<R: PeNewspaperRandom>(
    rng: &mut R,
) -> [u8; NEWSPAPER_LIST_MISC_DWORDS] {
    let mut recs = pe_newspaper_paper_list_identity_bytes();
    let bit = usize::from(rng.random_u16() & 1);
    recs.swap(3, 0x12 + bit * NEWSPAPER_LIST_RECORD_BYTES);
    if rng.random_u16() & 1 != 0 {
        recs.swap(1, 6);
    }
    for _ in 0..PE_NEWSPAPER_SHUFFLE_ROUNDS {
        let a = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES;
        let b = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES;
        recs.swap(a, b);
        let a = pe_newspaper_random_mod(rng, 5) * NEWSPAPER_LIST_RECORD_BYTES + 6;
        let b = pe_newspaper_random_mod(rng, 5) * NEWSPAPER_LIST_RECORD_BYTES + 6;
        recs.swap(a, b);
        let a = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES + 2;
        let b = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES + 2;
        recs.swap(a, b);
        let a = pe_newspaper_random_mod(rng, 5) * NEWSPAPER_LIST_RECORD_BYTES + 8;
        let b = pe_newspaper_random_mod(rng, 5) * NEWSPAPER_LIST_RECORD_BYTES + 8;
        recs.swap(a, b);
        let a = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES + 4;
        let b = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES + 4;
        recs.swap(a, b);
    }
    recs
}

pub(super) fn pe_newspaper_paper_list_misc_values_with_rng(
    crt: &mut Win95CrtRand,
) -> [u32; NEWSPAPER_LIST_MISC_DWORDS] {
    let recs = pe_newspaper_paper_list_shuffled_bytes(crt);
    let mut values = [0u32; NEWSPAPER_LIST_MISC_DWORDS];
    for (index, byte) in recs.iter().copied().enumerate() {
        values[index] = u32::from(byte);
    }
    values
}

#[cfg(test)]
pub(super) fn pe_newspaper_paper_list_misc_values(
    city: &City,
) -> [u32; NEWSPAPER_LIST_MISC_DWORDS] {
    let mut rng = Win95CrtRand::new(pe_newspaper_shuffle_seed(city));
    pe_newspaper_paper_list_misc_values_with_rng(&mut rng)
}

/// Win95 `0x479938`: story record `i` is word `i+0x0B`, word `0`, then `0, 0xFF, 0xFF, 0xFF`.
pub(super) fn pe_newspaper_story_list_misc_values() -> [u32; NEWSPAPER_STORY_LIST_MISC_DWORDS] {
    let mut values = [0u32; NEWSPAPER_STORY_LIST_MISC_DWORDS];
    for index in 0..NEWSPAPER_STORY_RECORD_COUNT as u32 {
        let rec = index as usize * NEWSPAPER_STORY_MISC_FIELDS;
        values[rec] = index + 0x0B;
        values[rec + 1] = 0;
        values[rec + 2] = 0;
        values[rec + 3] = 0xFF;
        values[rec + 4] = 0xFF;
        values[rec + 5] = 0xFF;
    }
    values
}

pub(super) fn initialize_pe_newspaper_tables(
    crt: &mut Win95CrtRand,
) -> crate::city::PeNewspaperTables {
    crate::city::PeNewspaperTables {
        papers: pe_newspaper_paper_list_misc_values_with_rng(crt),
        stories: pe_newspaper_story_list_misc_values(),
    }
}

/// A deterministic isolated fallback for sessions without the startup snapshot.
pub(super) fn default_pe_newspaper_tables() -> crate::city::PeNewspaperTables {
    initialize_pe_newspaper_tables(&mut Win95CrtRand::new(Win95CrtRand::DEFAULT_SEED))
}
