mod audio;
mod connection;
mod describe;
mod detector;
mod models;
mod wyoming;

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use livekit_wakeword::FRAME_SAMPLES;
use log::{debug, info, warn};

use crate::detector::Settings;
use crate::models::Models;

// Not the Cargo version: bumping that rebuilds every dependency in Nix.
const VERSION: &str = include_str!("../VERSION").trim_ascii();

/// Wyoming wake word server for LiveKit WakeWord models.
#[derive(Parser)]
#[command(version = VERSION)]
struct Args {
    /// Address to listen on.
    #[arg(long, env = "LISTEN", default_value = "0.0.0.0:10400")]
    listen: String,

    /// Directory of LiveKit WakeWord v2 .onnx models, named by file stem.
    #[arg(long, env = "MODEL_DIR")]
    model_dir: PathBuf,

    /// Score a model has to exceed to count as an activation.
    #[arg(long, env = "THRESHOLD", default_value_t = 0.5)]
    threshold: f32,

    /// Consecutive 80 ms activations needed for a detection.
    #[arg(long, env = "TRIGGER_LEVEL", default_value_t = 1,
          value_parser = clap::value_parser!(u32).range(1..))]
    trigger_level: u32,

    /// Seconds before the same wake word can be detected again.
    #[arg(long, env = "REFRACTORY_SECONDS", default_value_t = 2.0)]
    refractory_seconds: f64,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();
    let settings = Settings {
        threshold: args.threshold,
        trigger_level: args.trigger_level,
        refractory_frames: (args.refractory_seconds * f64::from(audio::RATE) / FRAME_SAMPLES as f64)
            .ceil() as u32,
    };
    info!(
        "wyoming-livekit-wakeword {VERSION}: threshold {}, trigger level {}, refractory {} s",
        args.threshold, args.trigger_level, args.refractory_seconds
    );

    let models = Arc::new(Models::scan(&args.model_dir)?);
    let listener =
        TcpListener::bind(&args.listen).with_context(|| format!("binding {}", args.listen))?;
    info!("listening on {}", args.listen);

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                // E.g. out of file descriptors, which won't clear right away.
                warn!("accepting a connection: {err}");
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        let models = models.clone();
        std::thread::spawn(move || {
            let peer = stream
                .peer_addr()
                .map(|a| a.to_string())
                .unwrap_or_default();
            debug!("{peer} connected");
            match connection::handle(stream, &peer, &models, settings) {
                Ok(()) => debug!("{peer} disconnected"),
                Err(err) => warn!("{peer}: {err:#}"),
            }
        });
    }
    Ok(())
}
