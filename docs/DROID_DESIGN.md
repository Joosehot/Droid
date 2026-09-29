# Droid design (the contract every part is built against)

Droid is a game engine where a whole game comes from one sentence. It follows the HotMotor
design (https://github.com/Joosehot/HotMotor): closed vocabulary, parser, rules with variants,
beam search with judges, checks before output. **No neural network runs at generation time.**
The same sentence and the same rules give byte-identical files.

```
"make a hard platformer with coins, slimes and bats in a night forest"
   │
   ▼  Droid front (src/hot/)  lexicon → parser → Request → rules → beam search → checks
   │     decides WHAT: genre, level layout, art from the asset library, speeds, goal, lives
   │     writes: game/ data  +  behavior.txt (the game's behaviour as English sentences)
   ▼
   Senne (../Senne, module senne::game)  behavior.txt → Rust source
   │     decides HOW it is coded: loops, helpers, guards (safety / brevity / perf)
   ▼
   games/<slug>/  a Rust crate: src/main.rs (Senne's code) + game/ (data) → cargo build → run
```

Nobody writes Rhai for new games. `legacy/` keeps the old script games runnable; do not delete it.

## 1. Repositories and who owns what

| repo | path | notes |
|---|---|---|
| Droid | `C:\Users\joose\AIgame-engine` | crate `droid` (lib + bin `droid`), workspace members `games/*` |
| Senne | `C:\Users\joose\Senne` | crate `senne`; game work happens on branch `game-domain`; `main` must keep passing its 70 tests |

Droid depends on Senne as `senne = { path = "../../Senne" }` (relative to Droid's Cargo.toml it is `../Senne`).
Never push to GitHub. Commit locally when a part is done, ending the message with
`Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
Never kill Python processes (trading bots run in the background on this machine).
Shell: Git Bash on Windows. Use `python` for multi-line file patches if needed; prefer the Write/Edit tools for files.

## 2. The runtime API games are written against (Droid, `src/api.rs`, authoritative)

```rust
pub trait Game {
    fn init(&mut self, cx: &mut Ctx) {}                     // start and after every restart (fresh Default instance)
    fn update(&mut self, cx: &mut Ctx, dt: f32) {}          // every frame, before physics
    fn on_collision(&mut self, cx: &mut Ctx, a: &str, b: &str) {} // every touching pair, called for (a,b) and (b,a)
    fn status(&self) -> Status { Status::Playing }          // Playing / Won / Lost
}
```

`Ctx` (read `src/api.rs` for exact signatures): time, key, key_pressed, axis_x, axis_y, random,
exists, pos, vel, grounded, has_tag, find_tag (sorted names), count_tag, set_pos, move_by,
set_vel, set_vel_x/y/z, set_scale, set_color, set_flip_x, set_solid, set_sprite, add_tag,
animate_sprite, squash, spawn_sprite, spawn_shape, respawn_tag, destroy, destroy_tag,
play_anim, text, text_at, text_color, remove_text, play_sound, play_music, restart.
`use droid::prelude::*;` brings Game, Ctx, Status, Vec3, vec2, vec3, BodyKind.

Conventions the data and the code share:
- The player entity is named `player`. Things carry their kind as a tag (`coin`, `slime`) plus a
  role tag (`pickup`, `enemy`, `hazard`, `exit`). Player projectiles are tagged `shot`, enemy ones `bomb`.
- Y is up. 2D lives on the XY plane. 3D arenas move on the XZ plane ("across the ground").
- Keys: `space enter shift r x z`, letters, `left right up down`; axis_x/axis_y cover arrows and WASD.
- Sounds: `shoot laser hit explosion jump coin pickup powerup hurt click win lose bounce step`.
- Headless check: running a game binary with `DROID_SIM=<seconds>` prints one JSON line:
  `{"title","seconds","status","ended_at":{seconds,status}|null,"error","player":[x,y,z]|null,"entities","texts"}`.
- `games/coin_runner/src/main.rs` is the hand-written reference for the shape of Senne's output.

## 3. The game sentence language (Senne's input, `behavior.txt`)

Plain English, closed vocabulary; a word Senne doesn't know is an error with a "did you mean"
hint (`--lenient` skips it). Sentences end with `.`. A sentence is either a **standing behaviour**
(no trigger, runs every frame) or **trigger, actions** (comma after the trigger). Actions split on
`and`, `,`, `then`. A verbless piece joins the action before it ("twice at 12 units per second").

Things (closed list, singular/plural): coin gem star apple heart key slime spider skeleton zombie
blob bat ghost alien spike saw lava door flag portal exit enemy pickup hazard shot bomb shield.
A quoted word is a custom tag or a sprite name.

Triggers:
```
At the start, ...
Every frame, ...
Every 1.6 seconds, ...
When the player touches a coin, ...            (it = the coin)
When the player touches the door and no coins are left, ...
When a shot hits an alien, ...                  (the shot / the alien by name; "both")
When the player lands on a slime, ...           (player falling and above it; excludes "touches" that frame)
When no aliens are left, ...                    (edge-triggered: fires once, re-arms when false)
When the score reaches 50, ...
When the time reaches 60 seconds, ...
When the player falls below -12, ...
When an alien falls below -6, ...
When the player presses r, ...                  (runs even after the game is over)
```

Standing behaviours:
```
Move the player sideways with the arrow keys at 7.5 units per second and face where it moves.
Move the player with the arrow keys at 7 units per second.              (2D, both axes)
Move the player across the ground with the arrow keys at 7 units per second.   (3D, XZ)
  + "smoothly" (eased), "within 12 units" (clamp x)
The player jumps with space at 14.5 units per second, twice at 12 units per second, higher while held, with the jump sound.
The player shoots up with space at 22 units per second every 0.3 seconds, using "shot", with the shoot sound.
  (up | down | forward; forward aims where the player last moved; "across the ground" in 3D)
Each zombie moves toward the player at 2.3 units per second.
  + "sideways" (x only), "across the ground", "within 9 units", "speeding up by 1.2 percent per second"
Each slime patrols 3 units to each side at 2.2 units per second.
Each bat flies in a wave 1.2 units high at 2 cycles per second.
Each blob wanders at 2 units per second, turning every 1.5 seconds.   (+ "across the ground")
Each alien marches sideways at 1.4 units per second within 12.5 units, stepping down 0.6 units,
  up to 2.5 times as fewer are left, 15 percent faster each wave.
Each ghost shoots at the player at 6 units per second every 2 seconds within 10 units, using "bomb".
Animate each slime with "slime_1" and "slime_2" at 4 frames per second.
Animate the player with "hero_1" and "hero_2" at 8 frames per second while it walks.
```

Event actions:
```
destroy it | destroy both | destroy the shot | destroy the bombs
add 1 to the score | add 10 points
the player has 3 lives                       (only "at the start")
the player loses a life, safe for 1.5 seconds   (blinks while safe; at 0 lives the player loses)
the player wins | the player wins "LEVEL CLEAR"
the player loses | the player loses "GAME OVER"
play the coin sound
play the music "C4 E4 G4 C5" at 132 beats per minute as a square wave   (square | saw | triangle | sine)
show "COINS {score} / 14" at the top left [in "#ffffff"]
  places: at the top left | at the top right | at the top | in the middle | at the bottom
  placeholders: {score} {lives} {wave} {time} {countdown 60} {left coin}
bounce the player up at 11 units per second
the player goes back to the start
squash it | squash the player
play the jump animation on the player        (3D models)
restart the game
start the next wave of aliens                (respawns the scene's aliens, wave += 1)
```

Modifiers `safely`, `quickly`, `simply` shift the code profile (safety / brevity / perf) of that sentence.

Semantics the generated code must have:
- The game struct holds typed state: `score: i64`, `lives: i64`, `wave: i64`, `status: Status`,
  timers as `f32` fields, edge flags as `bool` fields, `safe: f32`, etc. Nothing stringly typed.
- `update` runs key triggers first, then returns early unless `status == Playing`.
  `on_collision` returns early unless Playing. Winning/losing sets status, plays the win/lose
  sound and shows a banner "YOU WIN!" / "GAME OVER" plus " - PRESS R" when a restart key exists.
- Every emitted block is preceded by a comment with the sentence and `[pattern/variant]`.
- `fn main() { droid::run::<Name>(concat!(env!("CARGO_MANIFEST_DIR"), "/game")); }`
- Output compiles with no warnings (`#![deny(warnings)]` is NOT added to the file; the Droid
  check builds with `RUSTFLAGS=-D warnings` for that crate).

## 4. Senne's game domain (`senne::game`)

Same shape as Senne's Rust engine, separate module so the existing 19 patterns are untouched.
```rust
pub mod game {
    pub struct GameOptions { pub struct_name: String, pub lenient: bool, pub comments: bool }
    pub struct GameCompiled { pub code: String, pub explain: String }
    pub fn compile(text: &str, cfg: &crate::config::Config, opts: &GameOptions) -> Result<GameCompiled, Vec<crate::parser::Diag>>;
    pub fn vocabulary() -> Vec<&'static str>;
}
```
- `src/game/{lexicon,parser,search,codegen,explain}.rs`, `src/game/rules/<pattern>.rs` (one per
  pattern, each with one test where it applies and one where it doesn't), judges in `src/game/rules/judges.rs`.
- Variants are code-level choices with safety/brevity/perf axes in Senne's `rules.toml` under
  `[game.rules.<pattern>]` and `[game.judges.<name>]`; `[game.tuning]` holds every constant
  (smoothing rate, jump buffer, blink rate, shot size, text positions...). No numbers in pattern code.
  Examples of real variants: player movement normalized vs raw diagonal; jump on-press vs
  buffered; player position read once per frame (hoisted) vs per loop; win/lose/hurt as a
  method (helper) vs inline; patrol by stored bounds vs sine; wander own timer vs shared timer.
- Judges: `helper_reuse` (a helper used once costs, the same code inlined twice costs, mixing
  costs), `hoist_reuse` (hoisting pays from two users; mixing costs).
- Beam search over clauses, deterministic tie-break, width from `[search].beam`.
- CLI: `senne --game -f behavior.txt [--struct Name] [--explain]`.
- Tests: `examples/game/*.txt` → `examples/game/out/*.rs` golden files, each compiled 6 times
  in-process and byte-identical (`SENNE_BLESS=1` to accept). All 70 existing tests still pass.

## 5. The Droid front (`src/hot/`, the HotMotor part)

Input: one sentence, e.g. "make a hard platformer with 20 coins, slimes and bats, a door at the
end, in a night forest". Drafts exist: `rules.toml` (all numbers), `src/hot/config.rs`,
`src/hot/request.rs` — reuse and adapt them (they predate the Senne split; the script/Rhai parts
of that plan are dropped).

- Lexicon: closed, synonyms map to one token, unknown word = error + closest-match hint, `--lenient`.
- Parser → `Request` with provenance (the words behind each value). Unused values are errors ("2 what?").
- Genres v0: `platformer` (2D), `arena` (2D or 3D top-down), `shooter` (2D fixed screen).
- Rules, one decision per file, each with variants scored on [challenge, pace, simplicity]:
  genre, view, layout, controls, camera, enemy_behavior, enemy_attack, pickups, hazards, goal,
  health, ramp, palette, music, juice, hud, art (which library asset depicts each kind).
  Words "hard/easy/fast/slow/simple" shift the profile. Stated words give an evidence bonus.
- Judges for cross-rule interactions (see `rules.toml [judges]`). Beam search, deterministic ties.
- Build: level geometry from a PRNG seeded by a hash of the sentence + chosen variants
  (deterministic), entities into `scene.toml`, art from the asset library (recoloured by
  theme and colour words), camera/background into `game.toml`, and **`behavior.txt` in the
  Senne language** (speeds, ranges, counts filled in from rules.toml and the profile). Senne
  compiles it to `src/main.rs`.
- Checks on each finalist, best first; a failing finalist is dropped and the next tried; if none
  pass, print why and write nothing:
  1. Senne compiles the behaviour (its diagnostics are reported verbatim)
  2. reachability: platformer jump graph from spawn to every pickup and the exit (jump height and
     reach from the physics numbers x `checks.reach_margin`); arena flood fill; shooter trivial
  3. spawn safety (nothing hostile within `checks.spawn_clear`), chase fairness (chaser speed <=
     player speed x `checks.max_chase_ratio`), entity budget
  4. `cargo build -p <slug>` with `RUSTFLAGS=-D warnings` for that crate succeeds
  5. `DROID_SIM=<checks.sim_seconds>` on the built binary: no error, status not "lost", player exists
- Output `games/<slug>/`: `Cargo.toml`, `sentence.txt`, `behavior.txt`, `why.txt` (every piece →
  rule/variant → words), `src/main.rs`, `game/game.toml`, `game/scene.toml`, `game/sprites/*.txt`,
  `game/models/*.toml`. Written to a temp dir and moved into place only after checks pass.
- CLI (`droid`): `make <name> "<sentence>" [--explain] [--rules F] [--lenient] [--run] [--no-build]`,
  `edit <name> "<more words>"` (appends ", <more words>" to sentence.txt and remakes; later
  words override earlier ones), `explain "<sentence>"`, `vocabulary`, `assets [--check]`, and the
  legacy commands `run/check/sim/convert` for `legacy/` games.
- Remove the Claude-writes-the-game path: `ai.rs` generation, the `=== file:` bundle format in
  `project.rs`, the `new/edit/play/chat` commands and the AI worker in `app.rs`. (In the window,
  TAB chat may be removed or show "use droid edit".)
- Golden tests: `examples/*.txt` sentences → `golden/<slug>/` (all generated text files except
  build output), compiled twice in-process and byte-identical, `DROID_BLESS=1` to accept.
  At least: a platformer, a hard/fast platformer with hazards, a 2D arena survival, a 3D arena
  collector, a space shooter with waves. Plus tests: unknown word hint, impossible request error,
  modifiers change choices, rules.toml typo rejected.

## 6. The asset library (`assets/`, Droid)

Hand-made, versioned art the front chooses from (the "art" rule). Adding an asset is adding
files, no code.
```
assets/library.toml
assets/sprites/<file>.txt     palette lines `c = r g b` or `#rrggbb`, a `---` line, pixel rows ('.' = transparent)
assets/models/<file>.toml     model parts with animation groups (see legacy/demo3d/models/hero.toml)
```
`library.toml`:
```toml
[[sprite]]
id = "slime_round"
kinds = ["slime"]                  # lexicon kinds it can depict
frames = ["slime_round_1.txt", "slime_round_2.txt"]
fps = 4
main = "a"                         # palette char that takes the entity's colour (optional)
shade = "b"                        # palette char that takes a darker version (optional)
style = ["cute"]                   # free tags the art rule can prefer ("retro", "cute", "spooky")

[[model]]
id = "gnome"
kinds = ["gnome", "knight"]
file = "gnome.toml"
main_group = "body"                # group whose parts take the entity's colour (optional)
```
Coverage required (validated by `droid assets --check` and a test):
- sprites (2 frames for creatures, 1 for items): knight cat frog ship gnome (players); slime spider
  skeleton zombie blob bat ghost alien (enemies); coin gem star apple heart key (pickups); spike
  saw lava (hazards); door flag portal (exits); shot bomb (projectiles); decor: tree bush rock cloud.
  At least two different assets for slime, coin and the knight so the art rule has a real choice.
- models (3D arenas): gnome/knight/cat humanoid-ish players, zombie, skeleton, slime/blob, ghost,
  bat, spider; coin gem star heart key; tree rock; door portal flag; spike.
Rust: `src/assets.rs` with `Library::load(dir)`, `Library::builtin()` (the repo's `assets/` via
`env!("CARGO_MANIFEST_DIR")`), lookups by kind, `recolor(asset, main_rgb) -> Vec<SpriteSpec>`,
validation (every file parses, frames of one asset share a size, every required kind covered).

## 7. Definition of done

- `cargo test --workspace` passes in Droid; `cargo test` passes in Senne (old + new tests).
- `droid make` produces, builds, checks and runs games for the five golden sentences; each game
  opens a window that renders (screenshot) and its `DROID_SIM` report shows no error.
- README describes Droid, the sentence → game flow, the vocabulary, the asset library and the
  Senne split. Both repos committed locally, nothing pushed.
