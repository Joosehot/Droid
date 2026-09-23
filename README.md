# AIGE - AI-native game engine

Describe a game. The AI designs it, draws the pixel art, writes the logic. You play it
immediately, then change it by talking while it runs.

Rust + wgpu. 2D and 3D. A game is one `game.json` file: scene, sprites, physics, UI and a
Rhai script. The engine runs it directly and hot-reloads it whenever the file changes.

## Setup

```
cp .env.example .env      # put ANTHROPIC_API_KEY=... in .env (gitignored)
cargo build --release
```

Local model instead of Claude: set `AIGE_BACKEND=local` and `AIGE_LOCAL_URL` to any
OpenAI-compatible server (llama.cpp `/v1`).

## Use

```
aige play invaders "Space Invaders with pixel art aliens, score and lives"
```

Generates `games/invaders/game.json`, opens the window, and starts the chat in the terminal.
Type change requests there ("make the aliens faster", "add a boss every 5 waves"). The AI
rewrites the file and the window reloads in place. Type `fix` to have it fix the error shown on
screen.

| Command | What it does |
|---|---|
| `aige new <name> "<prompt>"` | Generate a game |
| `aige run <name>` | Run it (F5 restarts, edits to `game.json` hot-reload) |
| `aige chat <name>` | Run it with the AI chat |
| `aige edit <name> "<request>"` | One-off AI change |
| `aige check <name>` | Validate JSON + script |
| `aige sim <name> --frames 600` | Headless run, prints UI texts, errors and entity positions |

`AIGE_EXIT_AFTER=5 aige run <name>` quits after 5 seconds and prints the UI state (automation).

## How the AI loop works

1. The system prompt is `docs/ENGINE_API.md`: the file format, the script API, and one example game.
2. The model returns the full JSON. The engine parses it, compiles the Rhai script, and sends any
   error back to the model (up to 3 repairs) before you ever see it.
3. Runtime script errors are shown on screen and attached to your next chat request.

## Layout

- `src/spec.rs` game file format (serde)
- `src/world.rs` ECS world (hecs), input, camera, texts
- `src/physics.rs` AABB physics, triggers, collision events
- `src/script.rs` Rhai bindings (the API the AI uses)
- `src/render.rs` wgpu renderer: instanced box/sphere/plane, sprite atlas, bitmap-font UI
- `src/ai.rs` Claude Messages API (raw HTTP, streaming) and OpenAI-compatible backends
- `src/game.rs` frame step + hot reload, `src/app.rs` winit window, `src/main.rs` CLI
- `games/` generated games, `docs/ENGINE_API.md` the AI's reference
