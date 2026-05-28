# SeedRelay

[![Rust](https://img.shields.io/badge/Rust-1.85+-000000.svg?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-WTFPL-FF4136.svg)](http://www.wtfpl.net/)

**OpenAI-compatible Realtime transcription bridge for Seed-ASR**

[中文](README_CN.md) | English

SeedRelay is a lightweight local server that exposes an OpenAI Realtime API endpoint and translates it to the Seed-ASR backend. Drop it into any tool that supports the OpenAI Realtime protocol — it handles device registration, audio resampling, Opus encoding, and streaming transcription.

## Features

- **OpenAI-compatible** — Implements the `/v1/realtime` WebSocket endpoint with session management, audio streaming, and transcript events
- **Streaming Transcription** — Real-time interim results and final transcripts
- **Audio Processing** — Automatic sample rate conversion and Opus encoding
- **Built-in Web UI** — Live testing interface with audio visualization (`--webui`)
- **API Key Auth** — Optional Bearer token or official WebSocket subprotocol enforcement
- **Zero-config Credentials** — Automatic device registration and token management

## Architecture

```
Client (OpenAI Realtime API)
  │  WebSocket /v1/realtime
  ▼
SeedRelay
  ├── JSON event parsing (session.update, input_audio_buffer.append, input_audio_buffer.commit)
  ├── PCM16 resampling → Opus encoding
  ├── Protobuf message construction
  ▼
Seed-ASR Backend
  │  WebSocket
  ▼
Streaming transcript events back to client
```

## Getting Started

### Prerequisites

- Rust toolchain (1.85+)
- Opus library (for audio encoding)

### Build

```bash
git clone https://github.com/27Aaron/SeedRelay.git
cd SeedRelay
cargo build --release
```

### Run

```bash
# Start with default settings
./target/release/seedrelay

# Start with web UI
./target/release/seedrelay --webui

# Custom host and port
./target/release/seedrelay --host 0.0.0.0 --port 8080

# With API key
./target/release/seedrelay --api-key your-secret-key --webui
```

First run will automatically register a device and obtain credentials. These are saved to `.seedrelay/credentials.json` in the current working directory, so keep that directory persistent if you run SeedRelay from a container or a service manager.

## CLI Reference

| Flag | Default | Description |
|------|---------|-------------|
| `--host <ADDR>` | `0.0.0.0` | Server listen address |
| `--port <PORT>` | `8000` | Server listen port |
| `--model <MODEL>` | `seed-asr` | ASR model identifier |
| `--api-key <KEY>` | *(disabled)* | Require this API key from clients |
| `--webui` | off | Enable built-in web testing UI |
| `--reset` | off | Reset device credentials and re-register |

## Docker

```bash
docker compose up -d
```

Customize via `command:` in `compose.yml`.

## API

### WebSocket Endpoint

```
ws://127.0.0.1:8000/v1/realtime?model=seed-asr
```

### OpenAI Compatibility Scope

SeedRelay implements the OpenAI Realtime transcription surface for live speech-to-text clients:

- `GET /v1/models`
- `GET /v1/models/{model}`
- `WS /v1/realtime?model=seed-asr`
- `Authorization: Bearer <api-key>` and `openai-insecure-api-key.<api-key>` WebSocket subprotocol auth
- `session.update`
- `input_audio_buffer.append`
- `input_audio_buffer.commit`
- `input_audio_buffer.clear`
- `conversation.item.input_audio_transcription.delta`
- `conversation.item.input_audio_transcription.completed`

SeedRelay does not implement chat completions, text generation, Responses API, file upload transcription, embeddings, assistants, batches, or fine-tuning endpoints.

### Client Events

| Event | Description |
|-------|-------------|
| `session.update` | Configure session (e.g. `session.audio.input.format.type`, `session.audio.input.format.rate`) |
| `input_audio_buffer.append` | Send base64-encoded PCM16 audio chunk |
| `input_audio_buffer.commit` | Signal end of audio, start transcription |
| `input_audio_buffer.clear` | Clear buffered audio |
| `session.close` | Close the session |

### Server Events

| Server Event | Description |
| --- | --- |
| `session.created` | Session established with model info |
| `session.updated` | Session configuration confirmed after `session.update` |
| `input_audio_buffer.committed` | Audio buffer committed for transcription |
| `input_audio_buffer.cleared` | Audio buffer cleared |
| `conversation.item.input_audio_transcription.delta` | Interim transcript fragment |
| `conversation.item.input_audio_transcription.completed` | Final transcript |
| `error` | OpenAI-style protocol or backend error |

## Tech Stack

| Category | Technology |
|----------|-----------|
| Runtime | Tokio (async) |
| WebSocket | tokio-tungstenite |
| HTTP Client | reqwest |
| Audio Codec | opus (Opus encoding) |
| Protocol Buffers | prost |
| CLI | clap |
| Serialization | serde / serde_json |

## Project Structure

```
src/
├── main.rs            # Entry point
├── cli.rs             # CLI argument definitions
├── config.rs          # Config resolution
├── server.rs          # WebSocket server & connection handling
├── client.rs          # Doubao backend WebSocket client
├── credentials.rs     # Device registration & token management
├── protocol.rs        # Protobuf message definitions
├── realtime.rs        # OpenAI Realtime protocol events
├── audio.rs           # PCM16 resampling & Opus encoding
├── response.rs        # Doubao response parsing
└── web.rs             # Embedded HTTP server for web UI
```

## License

[WTFPL](http://www.wtfpl.net/) — Do What The Fuck You Want To Public License
