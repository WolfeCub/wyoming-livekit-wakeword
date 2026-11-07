use serde_json::{Value, json};

/// The response to a describe event.
pub fn info<'a>(names: impl Iterator<Item = &'a str>) -> Value {
    let attribution =
        json!({ "name": "LiveKit", "url": "https://github.com/livekit/livekit-wakeword" });
    let models: Vec<Value> = names
        .map(|name| {
            let phrase = phrase(name);
            json!({
                "name": name,
                "description": phrase,
                "phrase": phrase,
                "attribution": attribution,
                "installed": true,
                "languages": [],
                "version": null,
            })
        })
        .collect();

    json!({
        "asr": [], "tts": [], "handle": [], "intent": [], "mic": [], "snd": [],
        "wake": [{
            "name": "livekit-wakeword",
            "description": "LiveKit WakeWord",
            "attribution": attribution,
            "installed": true,
            "version": crate::VERSION,
            "models": models,
        }],
    })
}

/// "hey_jarvis" -> "Hey Jarvis"
fn phrase(name: &str) -> String {
    name.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars
                .next()
                .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn phrase() {
        assert_eq!(super::phrase("hey_jarvis"), "Hey Jarvis");
        assert_eq!(super::phrase("ok__nabu"), "Ok Nabu");
    }
}
