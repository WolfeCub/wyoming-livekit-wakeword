use std::io::{BufReader, BufWriter};
use std::net::TcpStream;

use log::{debug, info, warn};
use serde_json::{Value, json};

use crate::detector::{Detector, Settings};
use crate::models::Models;
use crate::{audio, describe, wyoming};

/// Serves one client until it disconnects.
pub fn handle(
    stream: TcpStream,
    peer: &str,
    models: &Models,
    settings: Settings,
) -> anyhow::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = BufWriter::new(stream);
    let mut session = Session {
        peer,
        models,
        settings,
        detectors: Vec::new(),
        detected: false,
        samples: 0,
    };

    while let Some(event) = wyoming::read(&mut reader)? {
        match event.kind.as_str() {
            "describe" => wyoming::write(&mut writer, "info", describe::info(models.names()))?,
            "detect" => session.detect(event.data.get("names"))?,
            "audio-start" => session.start()?,
            "audio-chunk" => {
                for detection in session.chunk(&audio::samples(&event)?)? {
                    wyoming::write(&mut writer, "detection", detection)?;
                }
            }
            "audio-stop" if !session.detected => {
                wyoming::write(&mut writer, "not-detected", json!({}))?;
            }
            "audio-stop" => {}
            other => debug!("{peer}: ignoring {other} event"),
        }
    }
    Ok(())
}

struct Session<'a> {
    peer: &'a str,
    models: &'a Models,
    settings: Settings,
    detectors: Vec<Detector>,
    detected: bool,
    /// Samples since audio-start.
    samples: u64,
}

impl Session<'_> {
    /// Switches to the requested models, or all of them when none are known.
    fn detect(&mut self, names: Option<&Value>) -> anyhow::Result<()> {
        let (mut wanted, unknown): (Vec<&str>, Vec<&str>) = names
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .partition(|name| self.models.contains(name));
        if !unknown.is_empty() {
            warn!("{}: no model for requested {unknown:?}", self.peer);
        }
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
            "{}: detecting {:?}",
            self.peer,
            self.detectors.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
        Ok(())
    }

    fn start(&mut self) -> anyhow::Result<()> {
        self.detected = false;
        self.samples = 0;
        for detector in &mut self.detectors {
            detector.reset()?;
        }
        Ok(())
    }

    /// Returns the detection events for this chunk.
    fn chunk(&mut self, samples: &[i16]) -> anyhow::Result<Vec<Value>> {
        // Some clients skip detect.
        if self.detectors.is_empty() {
            self.detect(None)?;
        }

        let mut detections = Vec::new();
        for detector in &mut self.detectors {
            for detection in detector.feed(samples)? {
                let timestamp = audio::millis(self.samples + detection.offset as u64);
                info!(
                    "{}: detected {} at {timestamp} ms (score {:.3})",
                    self.peer, detector.name, detection.score
                );
                detections.push(json!({ "name": detector.name, "timestamp": timestamp }));
            }
        }

        self.detected |= !detections.is_empty();
        self.samples += samples.len() as u64;
        Ok(detections)
    }
}
