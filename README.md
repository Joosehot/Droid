# AIGE - AI-native game engine

Describe a game. The AI designs it, draws the pixel art, sculpts the 3D models, writes the
logic and the sounds. You play it immediately, then change it by talking to it while it runs.

Rust + wgpu. 2D and 3D. A game is a folder of small plain-text files the engine runs
directly and hot-reloads whenever one of them changes.

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

Generates `games/invaders/` and opens it. In the window, press **TAB**, type a change
("make the aliens shoot back", "add a boss every 5 waves"), press Enter. The AI edits the
files and the game reloads in place. Type `fix` to have it fix the error shown on screen.

| Command | What it does |
|---|---|
| `aige new <name> "<prompt>"` | Generate a game into `games/<name>/` |
| `aige run <name>` | Run it (TAB chat, F5 restart, file edits hot-reload) |
| `aige edit <name> "<request>"` | One-off AI change from the terminal |
| `aige chat <name>` | Run with an extra terminal chat |
| `aige check <name>` | Validate files + script |
| `aige sim <name> --frames 600` | Headless run: prints UI texts, errors, entity positions |
| `aige convert <name>` | Convert a legacy single `game.json` to the folder format |

`AIGE_EXIT_AFTER=5 aige run <name>` quits after 5 seconds and prints the UI state (automation).
`AIGE_EFFORT=low|medium|high` overrides the model effort (default: high for new games, medium for edits).

## Game folder

```
games/<name>/
  game.toml          title, view (2d/3d), camera, gravity, background
  scene.toml         [[entities]] and [[texts]]
  script.rhai        behaviour: init(), update(dt), on_collision(a, b)
  sprites/*.txt      pixel art as text: palette lines, ---, then rows of characters
  models/*.toml      3D models: primitive parts blended into one smooth surface
```

Everything is human-editable. Change a sprite file while the game runs and it swaps in
without restarting; change the script or scene and the game restarts with the new version.
The full format and script API is in `docs/ENGINE_API.md`, which is also the AI's reference.

## How the AI loop works

1. The system prompt is `docs/ENGINE_API.md` plus the file-bundle format.
2. For a new game the model writes every file. For a change it writes only the files that
   change, or FIND/REPLACE edit blocks inside a file, so edits are fast and cheap.
3. The engine assembles the files, validates them (TOML, sprite palettes, references,
   Rhai compile) and sends any error back to the model, up to 3 repairs, before you see it.
4. Runtime script errors are shown on screen and attached to your next chat request.

## Engine features

- Renderer: wgpu, instanced primitives, Lambert lighting in 3D, sprite atlas with
  nearest-neighbour sampling and alpha cut-out, built-in bitmap font for UI.
- 3D models: parts (sphere, box, capsule, cylinder, cone, subtractive) combined as a signed
  distance field with smooth-min blending, meshed with surface nets, smooth normals from
  the field gradient, per-vertex colour blending.
- Physics: AABB bodies (static, kinematic, dynamic), triggers, grounded detection, bounce.
- Scripting: Rhai with ~80 engine functions (input, entities, tags, spawning, camera, UI,
  sounds, music).
- Audio: procedural synth effects and a note-sequencer for looping music. No asset files.

## Layout

- `src/spec.rs` data model, `src/project.rs` folder format + AI bundle parser
- `src/world.rs` ECS world (hecs), `src/physics.rs`, `src/script.rs` Rhai bindings
- `src/models.rs` SDF sculpting + meshing, `src/sound.rs` synth, `src/font.rs`
- `src/render.rs` wgpu renderer, `src/app.rs` window + chat overlay, `src/game.rs` loop + hot reload
- `src/ai.rs` Claude / OpenAI-compatible client, `src/main.rs` CLI
