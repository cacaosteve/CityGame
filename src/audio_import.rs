//! Native normalization of classic Mac sampled sounds and DOS VOC/XMIDI files.

use anyhow::{Context, Result, bail, ensure};
use std::fs;
use std::path::{Path, PathBuf};

pub fn import_mac(source: &Path, output: &Path) -> Result<(usize, usize)> {
    let sound_dir = output.join("catalog/sounds");
    let mut effects = 0;
    let mut music = 0;
    let snd_dir = find_dir(source, "snd");
    if let Some(snd_dir) = snd_dir {
        for path in files_with_prefix(&snd_dir, "snd_") {
            let Some(id) = resource_id(&path, "snd_") else {
                continue;
            };
            if id >= 10_000 {
                continue;
            }
            let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            let (samples, rate) = decode_mac_sound(&data)
                .with_context(|| format!("decoding Mac sound {}", path.display()))?;
            write_wav(&sound_dir.join(format!("{id}.WAV")), &samples, rate)?;
            effects += 1;
        }
    }
    if let Some(midi_dir) = find_dir(source, "MIDI") {
        for path in files_with_prefix(&midi_dir, "MIDI_") {
            let Some(id) = resource_id(&path, "MIDI_") else {
                continue;
            };
            let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            if data.starts_with(b"MThd") {
                fs::create_dir_all(&sound_dir)?;
                fs::write(sound_dir.join(format!("{id}.MID")), data)?;
                music += 1;
            }
        }
    }
    Ok((effects, music))
}

pub fn import_dos(archive_raw: &Path, output: &Path) -> Result<(usize, usize)> {
    let sound_dir = output.join("catalog/sounds");
    let mut effects = 0;
    let mut music = 0;
    if archive_raw.is_dir() {
        for path in files_with_extension(archive_raw, "VOC") {
            let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            let (samples, rate) = decode_voc(&data)
                .with_context(|| format!("decoding DOS sound {}", path.display()))?;
            write_wav(
                &sound_dir.join(format!(
                    "{}.WAV",
                    path.file_stem().unwrap_or_default().to_string_lossy()
                )),
                &samples,
                rate,
            )?;
            effects += 1;
        }
        for cue in 10_000..=10_018 {
            let path = archive_raw.join(format!("{cue}.XMI"));
            if !path.is_file() {
                continue;
            }
            let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            let midi =
                xmi_to_midi(&data).with_context(|| format!("converting {}", path.display()))?;
            fs::create_dir_all(&sound_dir)?;
            fs::write(sound_dir.join(format!("{cue}.MID")), midi)?;
            music += 1;
        }
    }
    Ok((effects, music))
}

fn decode_mac_sound(data: &[u8]) -> Result<(Vec<u8>, u32)> {
    ensure!(data.len() >= 4, "Mac sound header is truncated");
    let format = be_u16(data, 0)?;
    ensure!(
        format == 1 || format == 2,
        "unsupported Mac sound format {format}"
    );
    let modifiers = if format == 1 {
        be_u16(data, 2)? as usize
    } else {
        0
    };
    let command_count_at = 4usize
        .checked_add(
            modifiers
                .checked_mul(6)
                .context("Mac sound modifier overflow")?,
        )
        .context("Mac sound command offset overflow")?;
    ensure!(
        be_u16(data, command_count_at)? == 1,
        "expected one Mac sound command"
    );
    let command = command_count_at + 2;
    ensure!(
        be_u16(data, command)? == 0x8051,
        "expected sampled Mac buffer command"
    );
    let header = be_u32(data, command + 4)? as usize;
    let frames = be_u32(data, header + 4)? as usize;
    let rate_fixed = be_u32(data, header + 8)?;
    let rate = ((rate_fixed as f64 / 65_536.0).round()) as u32;
    ensure!(
        data.get(header + 20) == Some(&0) && rate > 0,
        "unsupported Mac sound encoding"
    );
    let start = header
        .checked_add(22)
        .context("Mac sound sample offset overflow")?;
    let end = start
        .checked_add(frames)
        .context("Mac sound sample size overflow")?;
    ensure!(end <= data.len(), "Mac sound samples are truncated");
    Ok((data[start..end].to_vec(), rate))
}

fn decode_voc(data: &[u8]) -> Result<(Vec<u8>, u32)> {
    ensure!(
        data.starts_with(b"Creative Voice File\x1a"),
        "not a Creative VOC file"
    );
    ensure!(data.len() >= 22, "VOC header is truncated");
    let mut at = u16::from_le_bytes([data[20], data[21]]) as usize;
    let mut samples = Vec::new();
    let mut rate = None;
    while at < data.len() {
        let kind = data[at];
        at += 1;
        if kind == 0 {
            break;
        }
        ensure!(at + 3 <= data.len(), "VOC block header is truncated");
        let size =
            data[at] as usize | ((data[at + 1] as usize) << 8) | ((data[at + 2] as usize) << 16);
        at += 3;
        let end = at.checked_add(size).context("VOC block size overflow")?;
        ensure!(end <= data.len(), "VOC block data is truncated");
        let chunk = &data[at..end];
        at = end;
        match kind {
            1 => {
                ensure!(chunk.len() >= 2, "VOC sound block is truncated");
                ensure!(chunk[1] == 0, "compressed VOC audio is unsupported");
                let chunk_rate =
                    (1_000_000.0 / f64::from(256u16 - u16::from(chunk[0]))).round() as u32;
                if let Some(existing) = rate {
                    ensure!(existing == chunk_rate, "VOC contains mixed sample rates");
                }
                rate = Some(chunk_rate);
                samples.extend_from_slice(&chunk[2..]);
            }
            2 => {
                ensure!(
                    rate.is_some(),
                    "VOC continuation block has no preceding audio"
                );
                samples.extend_from_slice(chunk);
            }
            6 | 7 => {} // Repeat markers surround 528.VOC; emit its samples once.
            other => bail!("unsupported VOC block type {other}"),
        }
    }
    Ok((samples, rate.context("VOC contains no audio")?))
}

fn xmi_to_midi(data: &[u8]) -> Result<Vec<u8>> {
    let chunk_at = data
        .windows(4)
        .position(|window| window == b"EVNT")
        .context("XMIDI EVNT chunk is missing")?;
    ensure!(chunk_at + 8 <= data.len(), "XMIDI EVNT header is truncated");
    let size = be_u32(data, chunk_at + 4)? as usize;
    let end = (chunk_at + 8)
        .checked_add(size)
        .context("XMIDI event size overflow")?;
    ensure!(end <= data.len(), "XMIDI event data is truncated");
    let events = &data[chunk_at + 8..end];
    let mut at = 0usize;
    let mut tick = 0u64;
    let mut sequence = 0u64;
    let mut output: Vec<(u64, u64, Vec<u8>)> = Vec::new();
    while at < events.len() {
        while at < events.len() && events[at] < 128 {
            tick += u64::from(events[at]);
            at += 1;
        }
        if at == events.len() {
            break;
        }
        let status = events[at];
        at += 1;
        if status == 0xff {
            ensure!(at < events.len(), "XMIDI meta event is truncated");
            let kind = events[at];
            at += 1;
            let (length, next) = read_vlq(events, at)?;
            at = next;
            let length = usize::try_from(length).context("XMIDI meta-event too large")?;
            ensure!(
                at + length <= events.len(),
                "XMIDI meta event data is truncated"
            );
            if kind == 0x2f {
                break;
            }
            if kind != 0x51 {
                let mut body = vec![0xff, kind];
                body.extend(write_vlq(length as u32));
                body.extend_from_slice(&events[at..at + length]);
                output.push((tick, sequence, body));
            }
            at += length;
        } else if status == 0xf0 || status == 0xf7 {
            let (length, next) = read_vlq(events, at)?;
            at = next;
            let length = usize::try_from(length).context("XMIDI sysex event too large")?;
            ensure!(at + length <= events.len(), "XMIDI sysex data is truncated");
            let mut body = vec![status];
            body.extend(write_vlq(length as u32));
            body.extend_from_slice(&events[at..at + length]);
            output.push((tick, sequence, body));
            at += length;
        } else {
            match status & 0xf0 {
                0x90 => {
                    ensure!(at + 2 <= events.len(), "XMIDI note event is truncated");
                    let key = events[at];
                    let velocity = events[at + 1];
                    at += 2;
                    let (duration, next) = read_vlq(events, at)?;
                    at = next;
                    output.push((tick, sequence, vec![status, key, velocity]));
                    output.push((
                        tick + u64::from(duration),
                        sequence + 1_000_000,
                        vec![(status & 0x0f) | 0x80, key, 0],
                    ));
                }
                0x80 | 0xa0 | 0xb0 | 0xe0 => {
                    ensure!(at + 2 <= events.len(), "XMIDI channel event is truncated");
                    output.push((tick, sequence, vec![status, events[at], events[at + 1]]));
                    at += 2;
                }
                0xc0 | 0xd0 => {
                    ensure!(at < events.len(), "XMIDI channel event is truncated");
                    output.push((tick, sequence, vec![status, events[at]]));
                    at += 1;
                }
                _ => bail!("unsupported XMIDI event {status:#x}"),
            }
        }
        sequence += 1;
    }
    output.sort_by_key(|(event_tick, order, _)| (*event_tick, *order));
    let mut track = vec![0, 0xff, 0x51, 3, 0x0f, 0x42, 0x40];
    let mut last_tick = 0u64;
    for (event_tick, _, body) in output {
        let delta = u32::try_from(event_tick.saturating_sub(last_tick)).unwrap_or(u32::MAX);
        track.extend(write_vlq(delta));
        track.extend(body);
        last_tick = event_tick;
    }
    track.extend([0, 0xff, 0x2f, 0]);
    let track_len = u32::try_from(track.len()).context("MIDI track too large")?;
    let mut midi = b"MThd\0\0\0\x06\0\0\0\x01".to_vec();
    midi.extend_from_slice(&120u16.to_be_bytes());
    midi.extend_from_slice(b"MTrk");
    midi.extend_from_slice(&track_len.to_be_bytes());
    midi.extend(track);
    Ok(midi)
}

fn write_wav(path: &Path, samples: &[u8], rate: u32) -> Result<()> {
    let data_size = u32::try_from(samples.len()).context("WAV sample data exceeds 4 GiB")?;
    let riff_size = data_size
        .checked_add(36)
        .context("WAV file size overflow")?;
    let mut wav = Vec::with_capacity(samples.len() + 44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_size.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&8u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(samples);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::write(path, wav).with_context(|| format!("writing {}", path.display()))
}

fn read_vlq(data: &[u8], mut at: usize) -> Result<(u32, usize)> {
    let mut value = 0u32;
    for _ in 0..4 {
        ensure!(at < data.len(), "XMIDI variable-length value is truncated");
        let byte = data[at];
        at += 1;
        value = value
            .checked_shl(7)
            .context("XMIDI variable-length value overflow")?
            | u32::from(byte & 0x7f);
        if byte < 128 {
            return Ok((value, at));
        }
    }
    bail!("XMIDI variable-length value is too long")
}

fn write_vlq(mut value: u32) -> Vec<u8> {
    let mut bytes = vec![(value & 0x7f) as u8];
    value >>= 7;
    while value != 0 {
        bytes.push(((value & 0x7f) | 0x80) as u8);
        value >>= 7;
    }
    bytes.reverse();
    bytes
}

fn be_u16(data: &[u8], at: usize) -> Result<u16> {
    let bytes = data.get(at..at + 2).context("resource data is truncated")?;
    Ok(u16::from_be_bytes(bytes.try_into().unwrap()))
}

fn be_u32(data: &[u8], at: usize) -> Result<u32> {
    let bytes = data.get(at..at + 4).context("resource data is truncated")?;
    Ok(u32::from_be_bytes(bytes.try_into().unwrap()))
}

fn find_dir(root: &Path, name: &str) -> Option<PathBuf> {
    fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|part| part.to_string_lossy().eq_ignore_ascii_case(name))
        })
}

fn files_with_prefix(root: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .is_some_and(|part| part.to_string_lossy().starts_with(prefix))
        })
        .collect();
    paths.sort();
    paths
}

fn files_with_extension(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|part| part.to_string_lossy().eq_ignore_ascii_case(extension))
        })
        .collect();
    paths.sort();
    paths
}

fn resource_id(path: &Path, prefix: &str) -> Option<u32> {
    path.file_name()?
        .to_string_lossy()
        .strip_prefix(prefix)?
        .split('_')
        .next()?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_unsigned_pcm_samples_as_wav() {
        let path =
            std::env::temp_dir().join(format!("sc2k-audio-import-{}.wav", std::process::id()));
        write_wav(&path, &[128, 129, 127], 11_025).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(
            u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            11_025
        );
        assert_eq!(&bytes[44..], &[128, 129, 127]);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn converts_minimal_xmidi_event_stream_to_standard_midi() {
        let events = [0x90, 60, 100, 10, 0];
        let mut xmi = b"FORM\0\0\0\x0dXMIDCAT\0\0\0\x05EVNT\0\0\0\x05".to_vec();
        xmi.extend_from_slice(&events);
        let midi = xmi_to_midi(&xmi).unwrap();
        assert_eq!(&midi[..4], b"MThd");
        assert_eq!(&midi[14..18], b"MTrk");
        assert!(midi.windows(3).any(|window| window == [0x90, 60, 100]));
        assert!(midi.windows(3).any(|window| window == [0x80, 60, 0]));
    }
}
