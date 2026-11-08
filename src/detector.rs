use livekit_wakeword::{FRAME_SAMPLES, WakeWordModel};
use log::trace;

#[derive(Clone, Copy)]
pub struct Settings {
    pub threshold: f32,
    pub trigger_level: u32,
    /// In 80 ms frames.
    pub refractory_frames: u32,
}

pub struct Detection {
    pub score: f32,
    /// Samples into the chunk at which the detecting frame ended.
    pub offset: usize,
}

/// Runs one model over the audio, one 80 ms frame at a time.
pub struct Detector {
    pub name: String,
    model: WakeWordModel,
    trigger: Trigger,
    /// Samples short of a full frame, kept for the next chunk.
    pending: Vec<i16>,
}

impl Detector {
    pub fn new(name: String, model: WakeWordModel, settings: Settings) -> Self {
        Self {
            name,
            model,
            trigger: Trigger::new(settings),
            pending: Vec::with_capacity(FRAME_SAMPLES),
        }
    }

    pub fn reset(&mut self) -> anyhow::Result<()> {
        self.model.reset()?;
        self.trigger = Trigger::new(self.trigger.settings);
        self.pending.clear();
        Ok(())
    }

    /// Returns the detections this chunk completes.
    pub fn feed(&mut self, samples: &[i16]) -> anyhow::Result<Vec<Detection>> {
        // The model scores a chunk by the best of the frames it completes, and
        // scores 0 when it completes none, so feed it exactly one frame at a time.
        let carried = self.pending.len();
        self.pending.extend_from_slice(samples);
        let mut detections = Vec::new();
        let (frames, _) = self.pending.as_chunks::<FRAME_SAMPLES>();
        let used = frames.len() * FRAME_SAMPLES;
        for (i, frame) in frames.iter().enumerate() {
            let score = self.model.predict(frame)?[&self.name];
            trace!("{} {score:.3}", self.name);
            if self.trigger.frame(score) {
                self.model.reset()?;
                let offset = (i + 1) * FRAME_SAMPLES - carried;
                detections.push(Detection { score, offset });
            }
        }
        self.pending.drain(..used);
        Ok(detections)
    }
}

/// Turns per-frame scores into detections.
struct Trigger {
    settings: Settings,
    activations: u32,
    refractory_left: u32,
}

impl Trigger {
    fn new(settings: Settings) -> Self {
        Self {
            settings,
            activations: 0,
            refractory_left: 0,
        }
    }

    /// Returns whether this frame's score completes a detection.
    fn frame(&mut self, score: f32) -> bool {
        if self.refractory_left > 0 {
            self.refractory_left -= 1;
            return false;
        }
        if score <= self.settings.threshold {
            self.activations = 0;
            return false;
        }
        self.activations += 1;
        if self.activations < self.settings.trigger_level {
            return false;
        }
        self.activations = 0;
        self.refractory_left = self.settings.refractory_frames;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detections(settings: Settings, scores: &[f32]) -> Vec<usize> {
        let mut trigger = Trigger::new(settings);
        (0..scores.len())
            .filter(|&i| trigger.frame(scores[i]))
            .collect()
    }

    const SETTINGS: Settings = Settings {
        threshold: 0.5,
        trigger_level: 1,
        refractory_frames: 0,
    };

    #[test]
    fn threshold() {
        assert_eq!(detections(SETTINGS, &[0.1, 0.5, 0.6, 0.2]), [2]);
    }

    #[test]
    fn trigger_level() {
        let settings = Settings {
            trigger_level: 3,
            ..SETTINGS
        };
        let scores = [0.9, 0.9, 0.1, 0.9, 0.9, 0.9, 0.9, 0.9, 0.9];
        assert_eq!(detections(settings, &scores), [5, 8]);
    }

    #[test]
    fn refractory() {
        let settings = Settings {
            refractory_frames: 2,
            ..SETTINGS
        };
        assert_eq!(detections(settings, &[0.9; 7]), [0, 3, 6]);
    }
}
