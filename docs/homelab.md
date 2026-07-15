# Homelab services

The canonical API documentation is in `~/Desktop/Dev/docs`. Read its
`README.md`, then the relevant service's `llm.txt` and `openapi.yml` before using
an endpoint. The `.local` hosts are trusted-LAN services and must not be exposed
publicly.

## Media workflow

- `http://ai.local` is the preferred AI gateway. It provides OpenAI-compatible
  chat, speech, transcription, image generation, video generation, and music.
- `http://larynx.local` is the voice studio. It provides voice discovery,
  profiles, captures, effects, stories, transcription, and cloning workflows.
- `http://tts.local/tts` is the direct Kokoro speech endpoint. Send JSON with
  `text`, `voice`, and `speed`; it returns 24 kHz mono WAV audio.
- `http://clone.local` provides XTTS v2 voice cloning. Register a clean reference
  clip before using a speaker identifier.
- `http://whisper.local/inference` provides whisper.cpp transcription.
- `http://image.local` provides the ComfyUI graph API. Prefer `ai.local/image`
  for ordinary text-to-image work.
- `http://posts.local` provides automated show discovery, scripting, voice,
  rendering, publishing, media hosting, SSE, and RSS.

The site video renderer uses `tts.local` directly so narration speed and voice
selection are deterministic. Course narration uses Kokoro `af_heart`; trailer
narration uses Kokoro `am_onyx`.

## Other services

- `http://llm.local` provides Ollama's native and OpenAI-compatible APIs.
- `http://castle.local` manages containers, LXC, hosts, apps, routes, users,
  databases, OIDC, WebSockets, and MCP. Most operations require a Bearer JWT.
- `http://storage.local` provides authenticated REST, S3-compatible, and WebDAV
  storage.
- `http://outvie.local` and `http://arcade.local` provide the retro-game library.
  Open mode still requires the freely minted Bearer token.

## Security

Never embed credentials or returned tokens in source, generated media, logs, or
documentation. `ai`, `larynx`, `posts`, and upstream AI services have no LAN
authentication. Treat the network boundary as part of their security model.
