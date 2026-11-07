use anyhow::bail;

use crate::wyoming::Event;

pub const RATE: u32 = 16000;

/// Decodes an audio-chunk event. Home Assistant always sends 16 kHz 16-bit mono.
pub fn samples(event: &Event) -> anyhow::Result<Vec<i16>> {
    let format = ["rate", "width", "channels"].map(|k| event.data.get(k).and_then(|v| v.as_u64()));
    if format != [Some(RATE.into()), Some(2), Some(1)] {
        bail!("audio must be 16 kHz 16-bit mono, got rate/width/channels {format:?}");
    }
    let (samples, _) = event.payload.as_chunks();
    Ok(samples.iter().map(|&b| i16::from_le_bytes(b)).collect())
}

pub fn millis(samples: usize) -> u64 {
    samples as u64 * 1000 / u64::from(RATE)
}
