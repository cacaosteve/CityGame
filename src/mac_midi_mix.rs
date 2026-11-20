//! Selected original Mac MDRV voice-mixing paths.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MacEndingVoice {
    pub position: usize,
    pub fraction: u16,
    pub integer_step: u16,
    pub fractional_step: u16,
}

/// End prediction and non-looping dispatch from MDRV 0x1f9c..0x1fee.
pub(crate) fn mac_nonlooping_voice_ends(
    position: u32,
    last: u32,
    step: u32,
    frames: u16,
    cached_distance: &mut u32,
) -> bool {
    if *cached_distance == 0 {
        let frames = u32::from(frames);
        *cached_distance = (step >> 16)
            .wrapping_mul(frames)
            .wrapping_add(((step & 0xffff) * frames) >> 16);
    }
    (last.wrapping_sub(position).wrapping_sub(*cached_distance) as i32) <= 0
}

/// MDRV 0x214e..0x231e: unfiltered, non-looping voice at its end.
/// The caller selects this path only when the original dispatcher's end test
/// says the sample ends within this refill. `mix` holds existing word sums.
pub(crate) fn mix_ending_voice(
    voice: &mut MacEndingVoice,
    samples: &[u8],
    mix: &mut [u16],
) -> Result<(), &'static str> {
    let last = samples.len().checked_sub(1).ok_or("empty sample")?;
    for output in mix {
        if voice.position >= last {
            *output = output.wrapping_add(0x80);
            continue;
        }
        *output = output.wrapping_add(u16::from(samples[voice.position]));
        let (fraction, carry) = voice.fraction.overflowing_add(voice.fractional_step);
        voice.fraction = fraction;
        let displacement = voice.integer_step.wrapping_add(u16::from(carry)) as i16;
        voice.position = voice
            .position
            .checked_add_signed(isize::from(displacement))
            .ok_or("sample position underflow")?;
    }
    Ok(())
}

/// MDRV 0x20ac..0x2122, for an unfiltered saved tail. The driver clears
/// the tail flag and does not persist the temporary tail cursor.
pub(crate) fn mix_saved_tail(
    tail_flag: &mut u16,
    mut tail: MacEndingVoice,
    samples: &[u8],
    mix: &mut [u16],
    direct_mode: bool,
) -> Result<(), &'static str> {
    if mix.is_empty() || mix.len() % 5 != 0 {
        return Err("mixer requires five-frame blocks");
    }
    let flag = *tail_flag;
    if flag == 0 {
        return Err("no saved Mac voice tail");
    }
    *tail_flag = 0;
    if flag as i16 >= 0 {
        for block in mix.chunks_exact_mut(5) {
            for i in [0, 2] {
                let existing = (u32::from(block[i]) << 16) | u32::from(block[i + 1]);
                let sum = existing.wrapping_add(0x0080_0080);
                block[i] = (sum >> 16) as u16;
                block[i + 1] = sum as u16;
            }
            block[4] = block[4].wrapping_add(0x80);
        }
        return Ok(());
    }
    if direct_mode {
        let step = ((u32::from(tail.integer_step) << 16) | u32::from(tail.fractional_step)) >> 1;
        tail.integer_step = (step >> 16) as u16;
        tail.fractional_step = step as u16;
    }
    let mut previous = *samples
        .get(tail.position)
        .ok_or("tail position outside sound")?;
    let mut fade = None;
    for output in mix {
        let sample = if let Some(value) = fade {
            value
        } else {
            let current = *samples
                .get(tail.position)
                .ok_or("tail position outside sound")?;
            if (current ^ previous) & 0x80 != 0 {
                let value = ((u16::from(current) + u16::from(previous)) >> 1) as u8;
                fade = Some(value);
                value
            } else {
                previous = current;
                let (fraction, carry) = tail.fraction.overflowing_add(tail.fractional_step);
                tail.fraction = fraction;
                let displacement = tail.integer_step.wrapping_add(u16::from(carry)) as i16;
                tail.position = tail
                    .position
                    .checked_add_signed(isize::from(displacement))
                    .ok_or("tail position underflow")?;
                current
            }
        };
        *output = output.wrapping_add(u16::from(sample));
        if let Some(value) = fade {
            let centered = value.wrapping_sub(0x80) as i8;
            fade = Some(((centered >> 1) as u8).wrapping_add(0x80));
        }
    }
    Ok(())
}

/// MDRV 0x1ff2..0x203e and 0x2126..0x214a, before the sample end.
/// The driver processes five frames per block and uses long additions for
/// the first two pairs, allowing a carry between adjacent mix words.
pub(crate) fn mix_active_voice(
    voice: &mut MacEndingVoice,
    samples: &[u8],
    mix: &mut [u16],
) -> Result<(), &'static str> {
    if mix.len() % 5 != 0 {
        return Err("active mixer requires five-frame blocks");
    }
    for block in mix.chunks_exact_mut(5) {
        let mut values = [0_u16; 5];
        let mut displacement = 0_u16;
        for value in &mut values {
            let index = if voice.fractional_step == 0 {
                voice.position
            } else {
                voice
                    .position
                    .checked_add_signed(isize::from(displacement as i16))
                    .ok_or("sample position underflow")?
            };
            *value = u16::from(*samples.get(index).ok_or("sample position outside sound")?);
            if voice.fractional_step == 0 {
                voice.position = voice
                    .position
                    .checked_add_signed(isize::from(voice.integer_step as i16))
                    .ok_or("sample position underflow")?;
            } else {
                let (fraction, carry) = voice.fraction.overflowing_add(voice.fractional_step);
                voice.fraction = fraction;
                displacement = displacement
                    .wrapping_add(voice.integer_step)
                    .wrapping_add(u16::from(carry));
            }
        }
        if voice.fractional_step != 0 {
            voice.position = voice
                .position
                .checked_add_signed(isize::from(displacement as i16))
                .ok_or("sample position underflow")?;
        }
        for i in [0, 2] {
            let existing = (u32::from(block[i]) << 16) | u32::from(block[i + 1]);
            let addition = (u32::from(values[i]) << 16) | u32::from(values[i + 1]);
            let sum = existing.wrapping_add(addition);
            block[i] = (sum >> 16) as u16;
            block[i + 1] = sum as u16;
        }
        block[4] = block[4].wrapping_add(values[4]);
    }
    Ok(())
}

/// MDRV 0x21a4..0x2238 and 0x223a..0x225a: an unfiltered voice
/// crosses its loop boundary during this refill.
pub(crate) fn mix_loop_crossing_voice(
    voice: &mut MacEndingVoice,
    samples: &[u8],
    loop_range: std::ops::Range<usize>,
    mix: &mut [u16],
) -> Result<(), &'static str> {
    if loop_range.start >= loop_range.end || loop_range.end > samples.len() {
        return Err("invalid sample loop");
    }
    if mix.is_empty() || mix.len() % 5 != 0 {
        return Err("mixer requires five-frame blocks");
    }
    for output in mix {
        if voice.position >= loop_range.end {
            voice.position = voice.position - loop_range.end + loop_range.start;
        }
        *output = output.wrapping_add(u16::from(
            *samples
                .get(voice.position)
                .ok_or("sample position outside sound")?,
        ));
        let (fraction, carry) = voice.fraction.overflowing_add(voice.fractional_step);
        voice.fraction = fraction;
        let displacement = voice.integer_step.wrapping_add(u16::from(carry)) as i16;
        voice.position = voice
            .position
            .checked_add_signed(isize::from(displacement))
            .ok_or("sample position underflow")?;
    }
    Ok(())
}

/// Select the unfiltered active or loop-crossing path for an active voice.
pub(crate) fn mix_looping_voice(
    voice: &mut MacEndingVoice,
    cached_distance: &mut u32,
    samples: &[u8],
    loop_range: std::ops::Range<usize>,
    mix: &mut [u16],
) -> Result<(), &'static str> {
    let frames = u16::try_from(mix.len()).map_err(|_| "too many mixer frames")?;
    if frames == 0 || mix.len() % 5 != 0 {
        return Err("mixer requires five-frame blocks");
    }
    if loop_range.start >= loop_range.end || loop_range.end > samples.len() {
        return Err("invalid sample loop");
    }
    let position = u32::try_from(voice.position).map_err(|_| "sample position too large")?;
    let end = u32::try_from(loop_range.end).map_err(|_| "sample loop too large")?;
    let step = (u32::from(voice.integer_step) << 16) | u32::from(voice.fractional_step);
    // Reuse the driver's identical cache calculation, ignoring the end result.
    mac_nonlooping_voice_ends(position, end, step, frames, cached_distance);
    if position.wrapping_add(*cached_distance) >= end {
        mix_loop_crossing_voice(voice, samples, loop_range, mix)
    } else {
        mix_active_voice(voice, samples, mix)
    }
}

/// Build the MDRV output conversion table for its configured voice counts.
pub(crate) fn mac_output_lookup(
    main: u16,
    secondary: u16,
    divisor: u16,
) -> Result<Vec<u8>, &'static str> {
    let total = u32::from(main) + 2 * u32::from(secondary);
    if total == 0 || total > 0x7fff || divisor == 0 || u32::from(divisor) > total {
        return Err("invalid Mac output voice counts");
    }
    let padding = ((total - u32::from(divisor)) * 128) as usize;
    let mut table = Vec::with_capacity(total as usize * 256);
    table.resize(padding, 0);
    for value in 0..=255_u8 {
        table.extend(std::iter::repeat_n(value, usize::from(divisor)));
    }
    table.resize(table.len() + padding, 255);
    Ok(table)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MacOutputMode {
    Interpolated,
    Direct,
}

/// Initialize the driver's mix buffer from ordinary and secondary voice
/// statuses. The final `secondary_count` slots contribute twice when idle.
pub(crate) fn mac_init_mix_buffer(
    statuses: &[i16],
    secondary_count: usize,
    mode: MacOutputMode,
    high_water: &mut u16,
) -> Result<(Vec<u16>, usize), &'static str> {
    if statuses.is_empty() || statuses.len() > 16 || secondary_count > statuses.len() {
        return Err("invalid Mac mixer voice counts");
    }
    let main_count = statuses.len() - secondary_count;
    let mut silence_weight = 0_u16;
    let mut active = 0_usize;
    for (index, &status) in statuses.iter().enumerate() {
        if status < 0 {
            silence_weight += if index >= main_count { 2 } else { 1 };
        } else {
            active += 1;
        }
    }
    if active != 0 {
        *high_water = (*high_water).max(active as u16);
    }
    let count = match mode {
        MacOutputMode::Interpolated => 185,
        MacOutputMode::Direct => 375,
    };
    Ok((vec![silence_weight * 128; count], active))
}

/// Execute the output-table lookup for one 370-frame Sound Manager buffer.
/// `table_origin` is the original table pointer within `table`, allowing safe
/// representation of the driver's signed-WORD lookup indices.
pub(crate) fn mac_convert_output(
    mode: MacOutputMode,
    mix: &[u16],
    table: &[u8],
    table_origin: usize,
) -> Result<Vec<u8>, &'static str> {
    let expected = match mode {
        MacOutputMode::Interpolated => 185,
        MacOutputMode::Direct => 370,
    };
    if mix.len() != expected {
        return Err("invalid Mac output buffer length");
    }
    let lookup = |word: u16| -> Result<u8, &'static str> {
        let index = table_origin
            .checked_add_signed(isize::from(word as i16))
            .ok_or("Mac output table index outside bounds")?;
        table
            .get(index)
            .copied()
            .ok_or("Mac output table index outside bounds")
    };
    let mut output = Vec::with_capacity(370);
    match mode {
        MacOutputMode::Direct => {
            for &word in mix {
                output.push(lookup(word)?);
            }
        }
        MacOutputMode::Interpolated => {
            for pair in mix.windows(2) {
                output.push(lookup(pair[0])?);
                output.push(lookup(pair[0].wrapping_add(pair[1]) >> 1)?);
            }
            output.push(lookup(mix[184])?);
            output.push(lookup(mix[184])?);
        }
    }
    Ok(output)
}

/// A controlled unfiltered voice after the driver's envelope/service step.
pub(crate) struct MacRefillVoice<'a> {
    pub status: i16,
    pub cursor: MacEndingVoice,
    pub cached_distance: u32,
    pub samples: &'a [u8],
    pub loop_range: Option<std::ops::Range<usize>>,
}

/// Compose a half-rate MDRV refill for ordinary unfiltered voices. Envelope
/// and MIDI servicing are caller responsibilities; release voices and
/// secondary voice slots are not supported by this worker yet.
pub(crate) fn mac_mix_controlled_refill(
    voices: &mut [MacRefillVoice<'_>],
    divisor: u16,
    high_water: &mut u16,
) -> Result<Vec<u8>, &'static str> {
    if voices.iter().any(|v| v.status == 0) {
        return Err("Mac release envelope is not yet supported");
    }
    let statuses: Vec<i16> = voices.iter().map(|v| v.status).collect();
    let (mut mix, _) = mac_init_mix_buffer(&statuses, 0, MacOutputMode::Interpolated, high_water)?;
    let table = mac_output_lookup(voices.len() as u16, 0, divisor)?;
    for voice in voices {
        if voice.status < 0 {
            continue;
        }
        if let Some(loop_range) = voice.loop_range.clone() {
            mix_looping_voice(
                &mut voice.cursor,
                &mut voice.cached_distance,
                voice.samples,
                loop_range,
                &mut mix,
            )?;
        } else if mix_nonlooping_voice(
            &mut voice.cursor,
            &mut voice.cached_distance,
            voice.samples,
            &mut mix,
        )? {
            voice.status = -1;
        }
    }
    mac_convert_output(MacOutputMode::Interpolated, &mix, &table, 0)
}

/// Ordinary unfiltered voice using the terminal 0x8000 instrument envelope.
pub(crate) struct MacTerminalRefillVoice<'a> {
    pub voice: MacRefillVoice<'a>,
    pub release_timer: u16,
    pub pending_tail: Option<MacPendingTail<'a>>,
}

pub(crate) struct MacPendingTail<'a> {
    pub flag: u16,
    pub cursor: MacEndingVoice,
    pub samples: &'a [u8],
}

/// Capture the old voice before installing a new note when no tail is pending.
pub(crate) fn mac_capture_replacement_tail<'a>(
    old: &MacRefillVoice<'a>,
    driver_flags: u16,
) -> Result<Option<MacPendingTail<'a>>, &'static str> {
    use crate::mac_midi_voice::{
        MacTerminalVoiceService, MacVoiceTail, mac_prepare_voice_replacement,
    };
    let position = u32::try_from(old.cursor.position).map_err(|_| "sample position too large")?;
    let last = if old.status < 0 {
        0
    } else {
        u32::try_from(old.samples.len().checked_sub(1).ok_or("empty sample")?)
            .map_err(|_| "sample too large")?
    };
    let step = (u32::from(old.cursor.integer_step) << 16) | u32::from(old.cursor.fractional_step);
    let mut state = MacTerminalVoiceService {
        status: old.status,
        release_timer: 0,
        tail_flag: 0,
        position,
        last,
        step,
        fraction_words: u32::from(old.cursor.fraction) << 16,
        volume_words: 0,
        volume_table: 0,
        cached_distance: old.cached_distance,
        tail: MacVoiceTail::default(),
    };
    mac_prepare_voice_replacement(&mut state, driver_flags);
    if state.tail_flag == 0 {
        return Ok(None);
    }
    Ok(Some(MacPendingTail {
        flag: state.tail_flag,
        cursor: old.cursor,
        samples: old.samples,
    }))
}

/// Run terminal-envelope servicing, unfiltered mixing and interpolated output.
pub(crate) fn mac_mix_terminal_refill(
    voices: &mut [MacTerminalRefillVoice<'_>],
    driver_flags: u16,
    divisor: u16,
    high_water: &mut u16,
) -> Result<Vec<u8>, &'static str> {
    mac_mix_terminal_refill_configured(
        voices,
        driver_flags,
        divisor,
        0,
        MacOutputMode::Interpolated,
        high_water,
    )
}

/// Main music voices plus inactive secondary effect slots, at either driver rate.
pub(crate) fn mac_mix_terminal_refill_configured(
    voices: &mut [MacTerminalRefillVoice<'_>],
    driver_flags: u16,
    divisor: u16,
    inactive_secondary: usize,
    mode: MacOutputMode,
    high_water: &mut u16,
) -> Result<Vec<u8>, &'static str> {
    use crate::mac_midi_voice::{
        MacTerminalVoiceService, MacVoiceTail, mac_service_terminal_voice,
    };
    let mut statuses: Vec<i16> = voices.iter().map(|v| v.voice.status).collect();
    statuses.extend(std::iter::repeat_n(-1, inactive_secondary));
    let (mut mix, _) = mac_init_mix_buffer(&statuses, inactive_secondary, mode, high_water)?;
    let table = mac_output_lookup(voices.len() as u16, inactive_secondary as u16, divisor)?;
    let frames = match mode {
        MacOutputMode::Interpolated => 185,
        MacOutputMode::Direct => 370,
    };
    let mix = &mut mix[..frames];
    for item in voices {
        let voice = &mut item.voice;
        if voice.status < 0 {
            continue;
        }
        if let Some(mut pending) = item.pending_tail.take() {
            let step = (u32::from(voice.cursor.integer_step) << 16)
                | u32::from(voice.cursor.fractional_step);
            mac_nonlooping_voice_ends(0, 0, step, frames as u16, &mut voice.cached_distance);
            mix_saved_tail(
                &mut pending.flag,
                pending.cursor,
                pending.samples,
                mix,
                mode == MacOutputMode::Direct,
            )?;
            continue;
        }
        let last = u32::try_from(voice.samples.len().checked_sub(1).ok_or("empty sample")?)
            .map_err(|_| "sample too large")?;
        let position =
            u32::try_from(voice.cursor.position).map_err(|_| "sample position too large")?;
        let step =
            (u32::from(voice.cursor.integer_step) << 16) | u32::from(voice.cursor.fractional_step);
        let mut service = MacTerminalVoiceService {
            status: voice.status,
            release_timer: item.release_timer,
            tail_flag: 0,
            position,
            last,
            step,
            fraction_words: u32::from(voice.cursor.fraction) << 16,
            volume_words: 0,
            volume_table: 0,
            cached_distance: voice.cached_distance,
            tail: MacVoiceTail::default(),
        };
        mac_service_terminal_voice(&mut service, driver_flags)?;
        voice.status = service.status;
        item.release_timer = service.release_timer;
        if service.tail_flag != 0 {
            let saved = service.tail;
            let tail = MacEndingVoice {
                position: saved.position as usize,
                fraction: (saved.fraction_words >> 16) as u16,
                integer_step: (saved.step >> 16) as u16,
                fractional_step: saved.step as u16,
            };
            mix_saved_tail(
                &mut service.tail_flag,
                tail,
                voice.samples,
                mix,
                mode == MacOutputMode::Direct,
            )?;
            continue;
        }
        // Direct mode mixes twice as many frames at half the stored step, but
        // the native dispatcher's cached-distance calculation uses the full step.
        mac_nonlooping_voice_ends(0, 0, step, frames as u16, &mut voice.cached_distance);
        let mut mix_cursor = voice.cursor;
        if mode == MacOutputMode::Direct {
            let effective = step >> 1;
            mix_cursor.integer_step = (effective >> 16) as u16;
            mix_cursor.fractional_step = effective as u16;
        }
        if voice.status != 0 && voice.loop_range.is_some() {
            mix_looping_voice(
                &mut mix_cursor,
                &mut voice.cached_distance,
                voice.samples,
                voice.loop_range.clone().unwrap(),
                mix,
            )?;
        } else if mix_nonlooping_voice(
            &mut mix_cursor,
            &mut voice.cached_distance,
            voice.samples,
            mix,
        )? {
            voice.status = -1;
        }
        voice.cursor.position = mix_cursor.position;
        voice.cursor.fraction = mix_cursor.fraction;
    }
    mac_convert_output(
        mode,
        &mix[..match mode {
            MacOutputMode::Interpolated => 185,
            MacOutputMode::Direct => 370,
        }],
        &table,
        0,
    )
}

/// Dispatch an unfiltered, non-looping voice between the two verified paths.
/// Returns true when the driver has finished the voice.
pub(crate) fn mix_nonlooping_voice(
    voice: &mut MacEndingVoice,
    cached_distance: &mut u32,
    samples: &[u8],
    mix: &mut [u16],
) -> Result<bool, &'static str> {
    let frames = u16::try_from(mix.len()).map_err(|_| "too many mixer frames")?;
    if frames == 0 || mix.len() % 5 != 0 {
        return Err("mixer requires five-frame blocks");
    }
    let position = u32::try_from(voice.position).map_err(|_| "sample position too large")?;
    let last = u32::try_from(samples.len().checked_sub(1).ok_or("empty sample")?)
        .map_err(|_| "sample too large")?;
    let step = (u32::from(voice.integer_step) << 16) | u32::from(voice.fractional_step);
    if mac_nonlooping_voice_ends(position, last, step, frames, cached_distance) {
        mix_ending_voice(voice, samples, mix)?;
        Ok(true)
    } else {
        mix_active_voice(voice, samples, mix)?;
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_ending_voice_and_preserves_fraction_after_end() {
        let mut voice = MacEndingVoice {
            position: 0,
            fraction: 0,
            integer_step: 1,
            fractional_step: 0,
        };
        let mut output = [0xfffe, 0, 1, 2];
        mix_ending_voice(&mut voice, &[1, 2, 3], &mut output).unwrap();
        assert_eq!(output, [0xffff, 2, 129, 130]);
        assert_eq!((voice.position, voice.fraction), (2, 0));
    }

    #[test]
    fn saved_tail_fades_after_crossing_midpoint() {
        let tail = MacEndingVoice {
            position: 0,
            fraction: 0,
            integer_step: 1,
            fractional_step: 0,
        };
        let mut flag = 0xff00;
        let mut mix = [0; 5];
        mix_saved_tail(&mut flag, tail, &[200, 100, 0, 0, 0], &mut mix, false).unwrap();
        assert_eq!(mix, [200, 150, 139, 133, 130]);
        assert_eq!(flag, 0);
    }

    #[test]
    #[ignore = "requires original-instruction saved-tail mixer fixture"]
    fn saved_tail_mixer_matches_original_instructions() {
        let bytes = std::fs::read(std::env::var_os("CITYGAME_MAC_TAIL_MIX").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 3600);
        let patterns: [&[u8]; 5] = [
            &[128],
            &[1, 100, 127, 128, 129, 200, 255],
            &[255, 200, 1, 0, 127, 128],
            &[200, 190, 180, 150, 129],
            &[0, 255],
        ];
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let pattern = patterns[n("pattern")];
            let samples: Vec<u8> = (0..1024).map(|i| pattern[i % pattern.len()]).collect();
            let step = n("step") as u32;
            let tail = MacEndingVoice {
                position: n("position"),
                fraction: n("fraction") as u16,
                integer_step: (step >> 16) as u16,
                fractional_step: step as u16,
            };
            let mut flag = n("flag") as u16;
            let mut mix: Vec<u16> = (0..n("blocks") * 5)
                .map(|i| (n("initial") + i * 37) as u16)
                .collect();
            mix_saved_tail(&mut flag, tail, &samples, &mut mix, n("mode") != 0).unwrap();
            let expected: Vec<u16> = row["output"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as u16)
                .collect();
            assert_eq!(mix, expected, "{row}");
            assert_eq!(flag, n("new_flag") as u16, "{row}");
        }
    }

    #[test]
    fn active_mixer_carries_between_packed_words() {
        let mut voice = MacEndingVoice {
            position: 0,
            fraction: 0,
            integer_step: 1,
            fractional_step: 0,
        };
        let mut mix = [0, 0xffff, 0, 0, 0];
        mix_active_voice(&mut voice, &[0, 1, 0, 0, 0, 0], &mut mix).unwrap();
        assert_eq!(mix, [1, 0, 0, 0, 0]);
        assert_eq!(voice.position, 5);
    }

    #[test]
    fn nonlooping_voice_dispatches_then_finishes() {
        let mut voice = MacEndingVoice {
            position: 0,
            fraction: 0,
            integer_step: 1,
            fractional_step: 0,
        };
        let mut cache = 0;
        let samples = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut first = [0; 5];
        assert!(!mix_nonlooping_voice(&mut voice, &mut cache, &samples, &mut first).unwrap());
        assert_eq!(first, [0, 1, 2, 3, 4]);
        let mut second = [0; 5];
        assert!(mix_nonlooping_voice(&mut voice, &mut cache, &samples, &mut second).unwrap());
        assert_eq!(second, [5, 6, 7, 8, 128]);
    }

    #[test]
    fn looping_voice_wraps_without_carrying_between_mix_words() {
        let mut voice = MacEndingVoice {
            position: 3,
            fraction: 0,
            integer_step: 1,
            fractional_step: 0,
        };
        let mut mix = [0, 0xffff, 0, 0, 0];
        mix_loop_crossing_voice(&mut voice, &[10, 11, 12, 13, 14, 15], 2..4, &mut mix).unwrap();
        assert_eq!(mix, [13, 11, 13, 12, 13]);
        assert_eq!(voice.position, 4);
    }

    #[test]
    fn output_interpolation_wraps_before_averaging() {
        let lookup: Vec<u8> = (0..65536).map(|i| (i >> 8) as u8).collect();
        let mut mix = [0_u16; 185];
        mix[0] = 0xffff;
        mix[1] = 3;
        let output = mac_convert_output(MacOutputMode::Interpolated, &mix, &lookup, 32768).unwrap();
        assert_eq!(output.len(), 370);
        assert_eq!((output[0], output[1], output[2]), (127, 128, 128));
        assert!(mac_convert_output(MacOutputMode::Direct, &mix, &lookup, 32768).is_err());
    }

    #[test]
    #[ignore = "requires original-instruction mix initialization fixture"]
    fn mix_initialization_matches_original_instructions() {
        let bytes = std::fs::read(std::env::var_os("CITYGAME_MAC_BUFFER_INIT").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 1638);
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let statuses: Vec<i16> = row["statuses"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_i64().unwrap() as i16)
                .collect();
            let mode = if row["direct"].as_bool().unwrap() {
                MacOutputMode::Direct
            } else {
                MacOutputMode::Interpolated
            };
            let mut high = n("high") as u16;
            let (mix, active) =
                mac_init_mix_buffer(&statuses, n("secondary"), mode, &mut high).unwrap();
            assert_eq!(mix.len(), n("count"), "{row}");
            assert!(mix.iter().all(|&x| x == n("baseline") as u16), "{row}");
            assert_eq!(
                (active != 0, high),
                (row["active"].as_bool().unwrap(), n("new_high") as u16),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires original-instruction controlled-refill fixture"]
    fn controlled_refill_matches_original_output() {
        let bytes =
            std::fs::read(std::env::var_os("CITYGAME_MAC_CONTROLLED_REFILL").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 144);
        let source: Vec<u8> = (0..512).map(|i| (i * 73 + 129) as u8).collect();
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let step = n("step") as u32;
            let cursor = MacEndingVoice {
                position: n("position"),
                fraction: n("fraction") as u16,
                integer_step: (step >> 16) as u16,
                fractional_step: step as u16,
            };
            let mut voices = [
                MacRefillVoice {
                    status: 5,
                    cursor,
                    cached_distance: 0,
                    samples: &source[..n("length")],
                    loop_range: row["looping"].as_bool().unwrap().then_some(20..60),
                },
                MacRefillVoice {
                    status: -1,
                    cursor,
                    cached_distance: 0,
                    samples: &[],
                    loop_range: None,
                },
            ];
            let mut high = 0;
            let output = mac_mix_controlled_refill(&mut voices, 2, &mut high).unwrap();
            let expected: Vec<u8> = row["output"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect();
            assert_eq!(output, expected, "{row}");
            assert_eq!(
                (
                    voices[0].cursor.position,
                    voices[0].cursor.fraction,
                    voices[0].cached_distance,
                    voices[0].status as u16,
                    high
                ),
                (
                    n("final_position"),
                    n("final_fraction") as u16,
                    n("cache") as u32,
                    n("status") as u16,
                    1
                ),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires original-instruction terminal-envelope refill fixture"]
    fn terminal_refill_matches_original_output() {
        let bytes =
            std::fs::read(std::env::var_os("CITYGAME_MAC_TERMINAL_REFILL").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 216);
        let source: Vec<u8> = (0..512).map(|i| (i * 73 + 129) as u8).collect();
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let step = n("step") as u32;
            let cursor = MacEndingVoice {
                position: n("position"),
                fraction: 0,
                integer_step: (step >> 16) as u16,
                fractional_step: step as u16,
            };
            let mut voices = [
                MacTerminalRefillVoice {
                    voice: MacRefillVoice {
                        status: n("status") as i16,
                        cursor,
                        cached_distance: 0,
                        samples: &source[..n("length")],
                        loop_range: row["looping"].as_bool().unwrap().then_some(20..60),
                    },
                    release_timer: n("timer") as u16,
                    pending_tail: None,
                },
                MacTerminalRefillVoice {
                    voice: MacRefillVoice {
                        status: -1,
                        cursor,
                        cached_distance: 0,
                        samples: &[],
                        loop_range: None,
                    },
                    release_timer: 0,
                    pending_tail: None,
                },
            ];
            let mut high = 0;
            let output = mac_mix_terminal_refill(&mut voices, 0x0200, 2, &mut high).unwrap();
            let expected: Vec<u8> = row["output"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect();
            assert_eq!(output, expected, "{row}");
            assert_eq!(
                (
                    voices[0].voice.cursor.position,
                    voices[0].voice.cursor.fraction,
                    voices[0].voice.cached_distance,
                    voices[0].voice.status as u16,
                    voices[0].release_timer,
                    high
                ),
                (
                    n("final_position"),
                    n("final_fraction") as u16,
                    n("cache") as u32,
                    n("new_status") as u16,
                    n("new_timer") as u16,
                    1
                ),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires original-instruction terminal-tail refill fixture"]
    fn terminal_tail_refill_matches_original_output() {
        let bytes =
            std::fs::read(std::env::var_os("CITYGAME_MAC_TERMINAL_TAIL_REFILL").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 864);
        let source: Vec<u8> = (0..512).map(|i| (i * 73 + 129) as u8).collect();
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let step = n("step") as u32;
            let cursor = MacEndingVoice {
                position: n("position"),
                fraction: 0,
                integer_step: (step >> 16) as u16,
                fractional_step: step as u16,
            };
            let mut voices = [
                MacTerminalRefillVoice {
                    voice: MacRefillVoice {
                        status: n("status") as i16,
                        cursor,
                        cached_distance: n("initial_cache") as u32,
                        samples: &source[..n("length")],
                        loop_range: row["looping"].as_bool().unwrap().then_some(20..60),
                    },
                    release_timer: n("timer") as u16,
                    pending_tail: None,
                },
                MacTerminalRefillVoice {
                    voice: MacRefillVoice {
                        status: -1,
                        cursor,
                        cached_distance: 0,
                        samples: &[],
                        loop_range: None,
                    },
                    release_timer: 0,
                    pending_tail: None,
                },
            ];
            let mut high = 0;
            let output =
                mac_mix_terminal_refill(&mut voices, n("flags") as u16, 2, &mut high).unwrap();
            let expected: Vec<u8> = row["output"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect();
            assert_eq!(output, expected, "{row}");
            assert_eq!(
                (
                    voices[0].voice.cursor.position,
                    voices[0].voice.cursor.fraction,
                    voices[0].voice.cached_distance,
                    voices[0].voice.status as u16,
                    voices[0].release_timer,
                    high
                ),
                (
                    n("final_position"),
                    n("final_fraction") as u16,
                    n("cache") as u32,
                    n("new_status") as u16,
                    n("new_timer") as u16,
                    1
                ),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires original-instruction replacement refill fixture"]
    fn replacement_refills_match_original_output() {
        let bytes =
            std::fs::read(std::env::var_os("CITYGAME_MAC_REPLACEMENT_REFILL").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 108);
        let old_samples: Vec<u8> = (0..512).map(|i| (i * 73 + 129) as u8).collect();
        let new_samples: Vec<u8> = (0..512).map(|i| (i * 31 + 17) as u8).collect();
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let oldstep = n("oldstep") as u32;
            let old_cursor = MacEndingVoice {
                position: n("oldpos"),
                fraction: n("fraction") as u16,
                integer_step: (oldstep >> 16) as u16,
                fractional_step: oldstep as u16,
            };
            let old_voice = MacRefillVoice {
                status: if n("flag") == 1 { -1 } else { 5 },
                cursor: old_cursor,
                cached_distance: (oldstep >> 16) * 185 + ((oldstep & 65535) * 185 >> 16),
                samples: &old_samples,
                loop_range: None,
            };
            let pending = mac_capture_replacement_tail(&old_voice, 0)
                .unwrap()
                .unwrap();
            assert_eq!(pending.flag, n("flag") as u16, "{row}");
            let newstep = n("newstep") as u32;
            let new_cursor = MacEndingVoice {
                position: 0,
                fraction: 0,
                integer_step: (newstep >> 16) as u16,
                fractional_step: newstep as u16,
            };
            let mut voices = [
                MacTerminalRefillVoice {
                    voice: MacRefillVoice {
                        status: 5,
                        cursor: new_cursor,
                        cached_distance: 0,
                        samples: &new_samples,
                        loop_range: None,
                    },
                    release_timer: 0,
                    pending_tail: Some(pending),
                },
                MacTerminalRefillVoice {
                    voice: MacRefillVoice {
                        status: -1,
                        cursor: new_cursor,
                        cached_distance: 0,
                        samples: &[],
                        loop_range: None,
                    },
                    release_timer: 0,
                    pending_tail: None,
                },
            ];
            let mut high = 0;
            for snapshot in row["snapshots"].as_array().unwrap() {
                let output = mac_mix_terminal_refill(&mut voices, 0, 2, &mut high).unwrap();
                let expected: Vec<u8> = snapshot["output"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
                    .chunks_exact(2)
                    .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                    .collect();
                assert_eq!(output, expected, "{row}");
                let ns = |key: &str| snapshot[key].as_u64().unwrap() as usize;
                assert_eq!(
                    (
                        voices[0].voice.cursor.position,
                        voices[0].voice.cursor.fraction,
                        voices[0].voice.cached_distance,
                        voices[0].voice.status as u16,
                        voices[0].pending_tail.is_some()
                    ),
                    (
                        ns("position"),
                        ns("fraction") as u16,
                        ns("cache") as u32,
                        ns("status") as u16,
                        ns("tail") != 0
                    ),
                    "{row}"
                );
            }
        }
    }

    #[test]
    #[ignore = "requires original-instruction output conversion fixture"]
    fn output_conversion_matches_original_instructions() {
        let bytes =
            std::fs::read(std::env::var_os("CITYGAME_MAC_OUTPUT_FIXTURE").unwrap()).unwrap();
        let root: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let decode_hex = |s: &str| -> Vec<u8> {
            s.as_bytes()
                .chunks_exact(2)
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect()
        };
        let tables = root["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 26);
        for row in tables {
            let n = |k: &str| row[k].as_u64().unwrap() as u16;
            assert_eq!(
                mac_output_lookup(n("main"), n("secondary"), n("divisor")).unwrap(),
                decode_hex(row["bytes"].as_str().unwrap()),
                "{row}"
            );
        }
        let lookup: Vec<u8> = (0..65536)
            .map(|i| (i * 73 + (i >> 8) * 11 + 129) as u8)
            .collect();
        let conversions = root["conversions"].as_array().unwrap();
        assert_eq!(conversions.len(), 48);
        for row in conversions {
            let mode = if row["direct"].as_bool().unwrap() {
                MacOutputMode::Direct
            } else {
                MacOutputMode::Interpolated
            };
            let words: Vec<u16> = row["words"]
                .as_array()
                .unwrap()
                .iter()
                .map(|w| w.as_u64().unwrap() as u16)
                .collect();
            let expected = decode_hex(row["output"].as_str().unwrap());
            assert_eq!(
                mac_convert_output(mode, &words, &lookup, 32768).unwrap(),
                expected,
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires original-instruction loop-mixer fixture"]
    fn loop_mixer_matches_original_instructions() {
        let bytes = std::fs::read(std::env::var_os("CITYGAME_MAC_LOOP_MIX").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 1728);
        let source: Vec<u8> = (0..128).map(|i| (i * 73 + 129) as u8).collect();
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let mut voice = MacEndingVoice {
                position: n("position"),
                fraction: n("fraction") as u16,
                integer_step: n("integer") as u16,
                fractional_step: n("fstep") as u16,
            };
            let mut mix: Vec<u16> = (0..n("blocks") * 5)
                .map(|i| (n("initial") + i * 37) as u16)
                .collect();
            mix_loop_crossing_voice(&mut voice, &source, n("start")..n("end"), &mut mix).unwrap();
            let expected: Vec<u16> = row["output"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect();
            assert_eq!(mix, expected, "{row}");
            assert_eq!(
                (voice.position, voice.fraction),
                (n("final_position"), n("final_fraction") as u16),
                "{row}"
            );
            assert_eq!(n("status"), 5, "{row}");
        }
    }

    #[test]
    #[ignore = "requires original-instruction looping refill fixture"]
    fn looping_refill_matches_original_instructions() {
        let bytes = std::fs::read(std::env::var_os("CITYGAME_MAC_LOOP_REFILL").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 576);
        let source: Vec<u8> = (0..128).map(|i| (i * 73 + 129) as u8).collect();
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let mut voice = MacEndingVoice {
                position: n("position"),
                fraction: n("fraction") as u16,
                integer_step: n("integer") as u16,
                fractional_step: n("fstep") as u16,
            };
            let mut mix: Vec<u16> = (0..n("blocks") * 5)
                .map(|i| (0xff80 + i * 37) as u16)
                .collect();
            let mut cache = 0;
            mix_looping_voice(
                &mut voice,
                &mut cache,
                &source,
                n("start")..n("end"),
                &mut mix,
            )
            .unwrap();
            let expected: Vec<u16> = row["output"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect();
            assert_eq!(mix, expected, "{row}");
            assert_eq!(
                (voice.position, voice.fraction, cache),
                (
                    n("final_position"),
                    n("final_fraction") as u16,
                    n("cache") as u32
                ),
                "{row}"
            );
            assert_eq!(n("status"), 5, "{row}");
        }
    }

    #[test]
    #[ignore = "requires original-instruction composed refill fixture"]
    fn nonlooping_refill_matches_original_instructions() {
        let bytes = std::fs::read(std::env::var_os("CITYGAME_MAC_NONLOOP_MIX").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 324);
        let source: Vec<u8> = (0..256).map(|i| (i * 73 + 129) as u8).collect();
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let mut voice = MacEndingVoice {
                position: n("start"),
                fraction: n("fraction") as u16,
                integer_step: n("integer") as u16,
                fractional_step: n("fstep") as u16,
            };
            let mut mix: Vec<u16> = (0..n("blocks") * 5)
                .map(|i| (0xff80 + i * 37) as u16)
                .collect();
            let mut cache = 0;
            let ended =
                mix_nonlooping_voice(&mut voice, &mut cache, &source[..n("length")], &mut mix)
                    .unwrap();
            let expected: Vec<u16> = row["output"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect();
            assert_eq!(mix, expected, "{row}");
            assert_eq!(
                (voice.position, voice.fraction, cache, ended),
                (
                    n("position"),
                    n("final_fraction") as u16,
                    n("cache") as u32,
                    row["ended"].as_bool().unwrap()
                ),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires original-instruction mixer dispatch fixture"]
    fn nonlooping_dispatch_matches_original_instructions() {
        let bytes = std::fs::read(std::env::var_os("CITYGAME_MAC_MIX_DISPATCH").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 720);
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as u32;
            let mut cache = n("cache");
            let ending = mac_nonlooping_voice_ends(
                n("position"),
                n("last"),
                n("step"),
                n("frames") as u16,
                &mut cache,
            );
            assert_eq!(
                (ending, cache),
                (row["ending"].as_bool().unwrap(), n("new_cache")),
                "{row}"
            );
        }
    }

    #[test]
    #[ignore = "requires original-instruction active-mixer fixture"]
    fn active_mixer_matches_original_instructions() {
        let bytes = std::fs::read(std::env::var_os("CITYGAME_MAC_ACTIVE_MIX").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 972);
        let source: Vec<u8> = (0..512).map(|i| (i * 73 + 129) as u8).collect();
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let mut voice = MacEndingVoice {
                position: n("start"),
                fraction: n("fraction") as u16,
                integer_step: n("integer") as u16,
                fractional_step: n("fstep") as u16,
            };
            let mut mix: Vec<u16> = (0..n("blocks") * 5)
                .map(|i| (n("initial") + i * 37) as u16)
                .collect();
            mix_active_voice(&mut voice, &source, &mut mix).unwrap();
            let expected: Vec<u16> = row["output"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect();
            assert_eq!(mix, expected, "{row}");
            assert_eq!(
                (voice.position, voice.fraction),
                (n("position"), n("final_fraction") as u16),
                "{row}"
            );
            assert_eq!(n("status"), 5, "{row}");
        }
    }

    #[test]
    #[ignore = "requires original-instruction ending-mixer fixture"]
    fn ending_mixer_matches_original_instructions() {
        let bytes = std::fs::read(std::env::var_os("CITYGAME_MAC_ENDING_MIX").unwrap()).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 972);
        let source = [0, 1, 127, 128, 200, 255, 23, 42, 81, 13, 5, 250];
        for row in rows {
            let n = |k: &str| row[k].as_u64().unwrap() as usize;
            let mut voice = MacEndingVoice {
                position: n("start"),
                fraction: n("fraction") as u16,
                integer_step: n("integer") as u16,
                fractional_step: n("fstep") as u16,
            };
            let mut mix = vec![n("initial") as u16; n("frames")];
            mix_ending_voice(&mut voice, &source[..n("length")], &mut mix).unwrap();
            let expected: Vec<_> = row["output"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as u16)
                .collect();
            assert_eq!(mix, expected, "{row}");
            assert_eq!(
                (voice.position, voice.fraction),
                (n("position"), n("final_fraction") as u16),
                "{row}"
            );
            assert_eq!(n("status"), 0xffff, "{row}");
        }
    }
}
