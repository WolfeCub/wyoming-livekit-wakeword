use std::time::{Duration, Instant};

use livekit_wakeword::WakeWordModel;

#[derive(Clone, Copy)]
pub struct Settings {
    pub threshold: f32,
    pub trigger_level: u32,
    pub refractory: Duration,
}

/// Turns one model's scores into detections.
pub struct Detector {
    pub name: String,
    model: WakeWordModel,
    settings: Settings,
    triggers_left: u32,
    last_triggered: Option<Instant>,
}

impl Detector {
    pub fn new(name: String, model: WakeWordModel, settings: Settings) -> Self {
        Self {
            name,
            model,
            settings,
            triggers_left: settings.trigger_level,
            last_triggered: None,
        }
    }

    pub fn reset(&mut self) -> anyhow::Result<()> {
        self.model.reset()?;
        self.triggers_left = self.settings.trigger_level;
        self.last_triggered = None;
        Ok(())
    }

    /// Returns the score when this chunk completes a detection.
    pub fn feed(&mut self, samples: &[i16]) -> anyhow::Result<Option<f32>> {
        // Fed during the refractory period too, so its state keeps up.
        let score = self.model.predict(samples)?[&self.name];

        if self
            .last_triggered
            .is_some_and(|t| t.elapsed() < self.settings.refractory)
        {
            return Ok(None);
        }
        if score <= self.settings.threshold {
            self.triggers_left = self.settings.trigger_level;
            return Ok(None);
        }
        self.triggers_left = self.triggers_left.saturating_sub(1);
        if self.triggers_left > 0 {
            return Ok(None);
        }

        self.model.reset()?;
        self.triggers_left = self.settings.trigger_level;
        self.last_triggered = Some(Instant::now());
        Ok(Some(score))
    }
}
