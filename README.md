# Droid

A game engine where a whole game comes from one sentence.

```
droid make forest_run "make a platformer with coins, slimes and bats, a flag at the end, in a forest"
cargo run -p forest_run
```

No neural network runs when a game is made. Droid follows the
[HotMotor](https://github.com/Joosehot/HotMotor) design: a closed vocabulary, a parser that
remembers which words asked for what, design rules with variants, a beam search with judges,
and checks before anything is written. The same sentence and the same `rules.toml` give
byte-identical files. The full contract is `docs/DROID_DESIGN.md`.

## Sentence to game

```
"make a hard platformer with coins, slimes and bats in a night forest"
   |
   |  Droid front (src/hot/)
   |    lexicon -> parser -> Request (every value keeps its words)
   |    17 rules (genre, view, layout, controls, camera, enemy_behavior, enemy_attack, pickups,
   |    hazards, goal, health, ramp, palette, art, music, juice, hud), each with variants
   |    scored on [challenge, pace, simplicity]; judges score choices together; beam search
   |    -> finalists, best first. For each finalist:
   |       design (numbers from rules.toml) -> art (asset library) -> level (seeded PRNG)
   |       -> behavior.txt (the game's behaviour as English sentences)
   |       -> checks; the first finalist that passes is written
   v
   Senne (../Senne, senne::game): behavior.txt -> src/main.rs (Rust against Droid's Ctx API)
   v
   games/<name>/   Cargo.toml, sentence.txt, behavior.txt, why.txt, src/main.rs, game/
```

Droid decides *what* the game is (genre, level, art, speeds, goal, lives). Senne decides *how*
it is coded (loops, helpers, guards). `why.txt` lists every decision with the rule, the variant
and the words behind it; `droid explain "<sentence>"` shows the same without writing anything.

Checks on each finalist: Senne compiles the behaviour; every pickup and the exit can be
reached (a jump graph for platformers, a flood fill for arenas); nothing hostile near the
start; chasers slower than the player; an entity budget; the crate builds with
`-D warnings`; a headless run (`DROID_SIM`) ends without error, not lost, with a player.
If no finalist passes, nothing is written and every reason is printed.

## Commands

| command | what it does |
|---|---|
| `droid make <name> "<sentence>"` | make `games/<name>/` (`--explain`, `--rules F`, `--lenient`, `--run`, `--no-build`) |
| `droid edit <name> "<more words>"` | append words to the game's sentence and make it again; later words win |
| `droid explain "<sentence>"` | how the sentence is understood, what every rule weighed, the finalists |
| `droid vocabulary` | every word a sentence may use |
| `droid assets [--check]` | list / validate the asset library |
| `droid run|check|sim|convert <name>` | the old Rhai script games in `legacy/` |

Games are ordinary crates in the workspace: `cargo run -p <name>`. In the window, `R`
restarts, `F5` reloads, and data files hot-reload.

## The vocabulary

Closed and small on purpose. An unknown word is an error with a "did you mean" hint
(`--lenient` skips it); a value nothing uses is an error ("20 what?").

- genres: platformer, arena / top-down, shooter / space shooter; `3d` (arenas)
- the player: knight, cat, frog, ship, gnome (or "the player", "you")
- enemies: slime, spider, skeleton, zombie, blob, bat, ghost, alien; behaviours chase,
  patrol, wander, fly, march, shoot ("zombies chase the player", "aliens that shoot back")
- pickups: coin, gem, star, apple, heart, key; hazards: spikes, saws, lava pits;
  exits: door, flag, portal
- goals: collect, reach, survive N seconds, shoot the X; abilities: jump, double jump, shoot
- counts: numbers, "lots of", "a few", "3 rows of", "with 5 lives"
- themes: forest, cave, space, desert, snow, garden, ocean, candy, castle, city, lava;
  night / day; colours (red coins); art style pixel / cartoon / low poly
- level shapes: steps, platforms, pillars, rooms, shields, open
- profile: hard, easy, fast, slow, simple; edits: faster, slower, more, fewer, bigger, smaller

## rules.toml

Every number the front uses: variant weights and axes, judges, physics per control style,
level layout sizes, enemy speeds and sizes, counts, checks, colours, themes, music, art
sizes. Typos fail at load. `droid make --rules my.toml ...` experiments with a copy.

## The asset library

`assets/` holds real CC0 art downloaded from the internet (Kenney packs and Quaternius
models): 2D sprites, tilesets by theme, and low-poly glTF models. `assets/packs.toml`
records every pack's url, licence and sha256; `assets/fetch.sh` re-downloads them;
`assets/CREDITS.md` lists the authors. The art rule picks, for each kind in the game, an asset
of the chosen style (then colour, then animation), and the files are copied into the game's
`game/` folder so each game crate is self-contained. Levels are drawn with tiles by theme.

## Runtime

- Renderer: wgpu; sprite atlas (PNG sprites, sheets, tiling sprites for ground and walls,
  text pixel art), imported glTF/OBJ meshes baked to vertex colours with limb groups for
  procedural walk animation, SDF-blended procedural models, bitmap UI font.
- Physics: AABB bodies (static, kinematic, dynamic), triggers, grounded detection.
- Audio: procedural synth effects and a note sequencer for music.
- Games implement `droid::api::Game` (`init`, `update`, `on_collision`, `status`) against
  `Ctx`; `DROID_SIM=<seconds>` runs one headless and prints a JSON report,
  `DROID_EXIT_AFTER=<seconds>` closes the window after a while.

## Layout

- `src/hot/` the front: `lexicon`, `parser`, `request`, `rules/` (one decision per file,
  `judges.rs`), `search`, `design`, `paint` (art), `level`, `behavior`, `checks`, `mod.rs` (make)
- `src/api.rs` the game API, `src/game.rs` loop + hot reload, `src/app.rs` window
- `src/world.rs`, `src/physics.rs`, `src/render.rs`, `src/models.rs`, `src/sound.rs`, `src/font.rs`
- `src/assets.rs` the asset library, `src/project.rs` game folders, `src/spec.rs` data model
- `src/script.rs` Rhai for the legacy games in `legacy/`
- `games/` generated games (workspace members), `rules.toml`, `docs/DROID_DESIGN.md`
