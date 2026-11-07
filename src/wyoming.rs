use std::io::{self, BufRead, Write};

use serde::Deserialize;
use serde_json::{Map, Value, json};

const VERSION: &str = "1.11.0";

pub struct Event {
    pub kind: String,
    pub data: Map<String, Value>,
    pub payload: Vec<u8>,
}

#[derive(Deserialize)]
struct Header {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    data: Option<Map<String, Value>>,
    data_length: Option<usize>,
    payload_length: Option<usize>,
}

/// Returns `None` once the peer closes the connection.
pub fn read(reader: &mut impl BufRead) -> io::Result<Option<Event>> {
    let mut line = Vec::new();
    if reader.read_until(b'\n', &mut line)? == 0 {
        return Ok(None);
    }
    let header: Header = serde_json::from_slice(&line)?;

    let mut data = header.data.unwrap_or_default();
    if let Some(len) = header.data_length.filter(|&n| n > 0) {
        let mut buf = vec![0; len];
        reader.read_exact(&mut buf)?;
        data.extend(serde_json::from_slice::<Map<String, Value>>(&buf)?);
    }

    let mut payload = vec![0; header.payload_length.unwrap_or(0)];
    reader.read_exact(&mut payload)?;

    Ok(Some(Event {
        kind: header.kind,
        data,
        payload,
    }))
}

pub fn write(writer: &mut impl Write, kind: &str, data: Value) -> io::Result<()> {
    let data = serde_json::to_vec(&data)?;
    let header = json!({ "type": kind, "version": VERSION, "data_length": data.len() });
    serde_json::to_writer(&mut *writer, &header)?;
    writer.write_all(b"\n")?;
    writer.write_all(&data)?;
    writer.flush()
}
