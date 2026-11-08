# wyoming-livekit-wakeword

[Wyoming](https://github.com/OHF-Voice/wyoming) wake word server for [LiveKit WakeWord](https://github.com/livekit/livekit-wakeword) models.

## Running

```sh
docker run -p 10400:10400 ghcr.io/wolfecub/wyoming-livekit-wakeword
```

The image includes `hey_jarvis`. To use other models, mount a directory of v2 `.onnx` models and set `MODEL_DIR`:

```sh
docker run -p 10400:10400 -v ./models:/models -e MODEL_DIR=/models ghcr.io/wolfecub/wyoming-livekit-wakeword
```

Models are named by file stem, so `hey_jarvis.onnx` is `hey_jarvis`.

In Home Assistant, add the Wyoming Protocol integration with this host and port 10400.

## Options

| Flag | Env | Default | |
|---|---|---|---|
| `--listen` | `LISTEN` | `0.0.0.0:10400` | Address to listen on |
| `--model-dir` | `MODEL_DIR` | | Directory of v2 `.onnx` models |
| `--threshold` | `THRESHOLD` | `0.5` | Score a model has to exceed to count as an activation |
| `--trigger-level` | `TRIGGER_LEVEL` | `1` | Consecutive 80 ms activations needed for a detection |
| `--refractory-seconds` | `REFRACTORY_SECONDS` | `2` | Seconds before the same wake word can be detected again |

Audio must be 16 kHz 16-bit mono.

## Logging

Set `RUST_LOG` to `debug` for connections and model loads, or `wyoming_livekit_wakeword=trace` for every 80 ms score.

## Building

```sh
nix build .#wyoming-livekit-wakeword   # binary
nix build .#image                      # container image
nix run .#image.copyToDockerDaemon     # load the image into Docker
```
