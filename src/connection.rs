use std::io::{BufReader, BufWriter};
use std::net::TcpStream;

use log::{debug, info};
use serde_json::{Value, json};

use crate::detector::{Detector, Settings};
use crate::models::Models;
use crate::{audio, describe, wyoming};

/// Serves one client until it disconnects.
pub fn handle(stream: TcpStream, models: &Models, settings: Settings) -> anyhow::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = BufWriter::new(stream);
    let mut session = Session {
        models,
        settings,
        detectors: Vec::new(),
        detected: false,
        timestamp_ms: 0,
    };

    while let Some(event) = wyoming::read(&mut reader)? {
        match event.kind.as_str() {
            "describe" => wyoming::write(&mut writer, "info", describe::info(models.names()))?,
            "detect" => session.detect(event.data.get("names"))?,
            "audio-start" => session.start()?,
            "audio-chunk" => {
                for name in session.chunk(&audio::samples(&event)?)? {
                    let data = json!({ "name": name, "timestamp": session.timestamp_ms });
                    wyoming::write(&mut writer, "detection", data)?;
                }
            }
            "audio-stop" if !session.detected => {
                wyoming::write(&mut writer, "not-detected", json!({}))?;
            }
            "audio-stop" => {}
            other => debug!("ignoring {other} event"),
        }
    }
    Ok(())
}

struct Session<'a> {
    models: &'a Models,
    settings: Settings,
    detectors: Vec<Detector>,
    detected: bool,
    timestamp_ms: u64,
}

impl Session<'_> {
    /// Switches to the requested models, or all of them when none are known.
    fn detect(&mut self, names: Option<&Value>) -> anyhow::Result<()> {
        let mut wanted: Vec<&str> = names
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|name| self.models.contains(name))
            .collect();
        if wanted.is_empty() {
            wanted = self.models.names().collect();
        }

        self.detectors.retain(|d| wanted.contains(&d.name.as_str()));
        for name in wanted {
            if !self.detectors.iter().any(|d| d.name == name) {
                let model = self.models.load(name)?;
                self.detectors
                    .push(Detector::new(name.to_owned(), model, self.settings));
            }
        }
        debug!(
            "detecting {:?}",
            self.detectors.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
        Ok(())
    }

    fn start(&mut self) -> anyhow::Result<()> {
        self.detected = false;
        self.timestamp_ms = 0;
        for detector in &mut self.detectors {
            detector.reset()?;
        }
        Ok(())
    }

    /// Returns the names detected in this chunk.
    fn chunk(&mut self, samples: &[i16]) -> anyhow::Result<Vec<String>> {
        // Some clients skip detect.
        if self.detectors.is_empty() {
            self.detect(None)?;
        }

        let mut detections = Vec::new();
        for detector in &mut self.detectors {
            if let Some(score) = detector.feed(samples)? {
                info!("detected {} (score {score:.3})", detector.name);
                detections.push(detector.name.clone());
            }
        }

        self.detected |= !detections.is_empty();
        self.timestamp_ms += audio::millis(samples.len());
        Ok(detections)
    }
}
