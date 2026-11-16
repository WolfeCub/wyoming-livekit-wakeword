use std::io::{BufReader, BufWriter};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use anyhow::{Context, bail};
use serde_json::json;

use crate::{audio, wyoming};

const TIMEOUT: Duration = Duration::from_secs(2);

/// A Wyoming service that confirms or rejects a detection from the audio
/// leading up to it, such as wyoming-wake-kaldi.
pub struct Verifier {
    pub address: String,
    /// Samples before the detection to send.
    pub window: usize,
}

impl Verifier {
    /// Whether the verifier accepts `name` in `samples`.
    pub fn verify(&self, name: &str, samples: &[i16]) -> anyhow::Result<bool> {
        let address = self
            .address
            .to_socket_addrs()?
            .next()
            .with_context(|| format!("resolving {}", self.address))?;
        let stream = TcpStream::connect_timeout(&address, TIMEOUT)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut writer = BufWriter::new(stream);

        let format = json!({ "rate": audio::RATE, "width": 2, "channels": 1 });
        let payload: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        wyoming::write(&mut writer, "detect", json!({ "names": [name] }))?;
        wyoming::write(&mut writer, "audio-start", format.clone())?;
        wyoming::write_payload(&mut writer, "audio-chunk", format, &payload)?;
        wyoming::write(&mut writer, "audio-stop", json!({}))?;
        loop {
            match wyoming::read(&mut reader)? {
                Some(event) if event.kind == "detection" => return Ok(true),
                Some(event) if event.kind == "not-detected" => return Ok(false),
                Some(_) => {}
                None => bail!("verifier closed the connection"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};

    use super::*;
    use crate::wyoming::Event;

    /// A verifier that answers `reply` once the audio stops, or hangs up
    /// without answering. Joining it returns the events it received.
    fn fake(reply: Option<&'static str>) -> (Verifier, JoinHandle<Vec<Event>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let verifier = Verifier {
            address: listener.local_addr().unwrap().to_string(),
            window: 0,
        };
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&stream);
            let mut events = Vec::new();
            while let Some(event) = wyoming::read(&mut reader).unwrap() {
                let stopped = event.kind == "audio-stop";
                events.push(event);
                if stopped {
                    break;
                }
            }
            if let Some(kind) = reply {
                wyoming::write(&mut &stream, kind, json!({})).unwrap();
            }
            events
        });
        (verifier, server)
    }

    #[test]
    fn accepted() {
        let (verifier, server) = fake(Some("detection"));
        assert!(verifier.verify("hey_jarvis", &[1, -2, i16::MAX]).unwrap());

        let events = server.join().unwrap();
        let kinds: Vec<_> = events.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(
            kinds,
            ["detect", "audio-start", "audio-chunk", "audio-stop"]
        );
        assert_eq!(events[0].data["names"], json!(["hey_jarvis"]));
        assert_eq!(audio::samples(&events[2]).unwrap(), [1, -2, i16::MAX]);
    }

    #[test]
    fn rejected() {
        let (verifier, server) = fake(Some("not-detected"));
        assert!(!verifier.verify("hey_jarvis", &[0; 16]).unwrap());
        server.join().unwrap();
    }

    #[test]
    fn hung_up() {
        let (verifier, server) = fake(None);
        assert!(verifier.verify("hey_jarvis", &[0; 16]).is_err());
        server.join().unwrap();
    }
}
