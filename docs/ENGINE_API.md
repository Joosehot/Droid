# AIGE engine reference

A game is a folder of small plain-text files:

```
game.toml          title, view, camera, gravity, background
scene.toml         [[entities]] and [[texts]]
script.rhai        behaviour (Rhai). Optional extra files scripts/*.rhai are appended.
sprites/NAME.txt   pixel-art sprite (palette + rows). Referenced as sprite = "NAME"
models/NAME.toml   3D model built from blended parts. Referenced as model = "NAME"
```

## game.toml

```toml
title = "Coin Runner"
description = "one line"
view = "2d"                     # "2d" or "3d"
background = [0.08, 0.09, 0.12] # rgb 0..1
gravity = [0.0, -30.0, 0.0]     # world units / s^2; [0,0,0] for top-down

[camera]
position = [0.0, 2.0, 20.0]     # 2d: x,y = view centre. 3d: eye position
look_at = [0.0, 0.0, 0.0]       # 3d only
zoom = 18.0                     # 2d only: world units visible vertically
follow = "player"               # optional: entity to keep centred
```

## scene.toml

```toml
[[entities]]
name = "ground"                 # unique; duplicates get _N suffix
shape = "box"                   # "box" | "sphere" | "plane"
position = [0.0, -1.0, 0.0]
scale = [60.0, 2.0, 1.0]        # box/plane: full width/height/depth. sphere: diameter
color = [0.3, 0.5, 0.3]
[entities.physics]
body = "static"                 # "none" (default) | "static" | "kinematic" | "dynamic"

[[entities]]
name = "player"
tags = ["player"]
sprite = "hero"                 # draws sprites/hero.txt as a quad of size `scale`
position = [-5.0, 1.0, 0.0]
scale = [1.2, 1.4, 1.0]
[entities.physics]
body = "dynamic"
velocity = [0.0, 0.0, 0.0]
bounciness = 0.0                # 0..1
solid = true                    # false = trigger: collision events only, no blocking

[[entities]]
name = "gnome"
model = "gnome"                 # draws models/gnome.toml (3D)
position = [0.0, 0.0, 0.0]
scale = [1.0, 1.0, 1.0]         # multiplies the model
rotation = [0.0, 0.0, 0.0]      # euler degrees
[entities.physics]
body = "dynamic"

[[texts]]
id = "score"
text = "Score: 0"
x = 0.02                        # screen 0..1 from top-left
y = 0.02
size = 28.0                     # pixels
color = [1.0, 1.0, 1.0]
```

## sprites/NAME.txt (pixel art you draw)

You are the artist. A palette of single characters, a `---` line, then one row of characters per pixel row (top row first). `.` = transparent. Colours are `r g b` in 0..1 or `#rrggbb`.

```
k = 0.10 0.10 0.12
s = 0.95 0.80 0.65
r = #d93333
b = 0.20 0.35 0.80
---
..kkkk..
.kssssk.
.ksksks.
..ssss..
.rrrrrr.
r.rrrr.r
..bbbb..
..kk.kk.
```

Typical sizes: 8x8 items, 12x16 characters, 16x16 to 32x32 for bigger things. Rendered crisp with no smoothing, so use bold silhouettes, 3 to 6 colours and a 1px dark outline. Keep the entity `scale` in the same aspect as the pixel grid so pixels stay square. Sprites ignore lighting and work in 2D and 3D. Physics uses the box of `scale`.
Animation: make several sprites (`hero_walk1`, `hero_walk2`) and switch with `set_sprite`; mirror with `set_flip_x`.

## models/NAME.toml (3D models you sculpt)

A model is a list of parts blended into one smooth surface (a signed distance field meshed with smooth normals; no visible facets). Parts are in model units, centred where you place them; the entity `scale` multiplies the whole model.

```toml
resolution = 40                  # 8..96 grid cells along the longest axis (32-48 is good)

[[parts]]
shape = "capsule"                # "sphere" | "box" | "capsule" | "cylinder" | "cone"
position = [0.0, 0.6, 0.0]
size = [0.9, 1.2, 0.9]           # full extents; capsule/cylinder/cone: x,z = diameter, y = height
color = [0.2, 0.5, 0.9]
blend = 0.25                     # smooth-merge radius with neighbours; 0 = hard edge

[[parts]]
shape = "sphere"
position = [0.0, 1.45, 0.0]
size = [0.8, 0.8, 0.8]
color = [0.98, 0.85, 0.7]
blend = 0.2

[[parts]]
shape = "cone"                   # hat
position = [0.0, 1.95, 0.0]
size = [0.9, 0.7, 0.9]
rotation = [0.0, 0.0, 0.0]       # euler degrees
color = [0.9, 0.25, 0.25]
blend = 0.1

[[parts]]
shape = "sphere"                 # carve a mouth
position = [0.0, 1.3, 0.38]
size = [0.25, 0.12, 0.2]
subtract = true
```

Colours blend across the joins. Use `blend` 0.2-0.5 for organic shapes (trees, animals, characters) and 0-0.05 for mechanical ones. Keep parts under ~14 per model.

The model is fitted into the entity's collision box: feet at the bottom, height = `scale.y`, uniform scaling. So for a character give the entity `scale = [0.9, 2.2, 0.9]` (box width, character height, depth) and place `position.y` at half the height.

### Animation groups (characters and creatures)
Give each part a `group`. Parts in the same group are blended together and move as one rigid piece; the engine animates the groups automatically:

| group | animated as |
|---|---|
| `body` | torso: bobs while walking, breathes when idle (default group) |
| `head` (also hats, eyes: put them in `head`) | nods and tilts |
| `arm_l`, `arm_r` | swing opposite to the legs, raised in the air |
| `leg_l`, `leg_r` | walk cycle |
| `tail`, `ear_l`, `ear_r` | wag / flap |

Pivots are automatic (top of legs and arms, bottom of head) or set with `pivot = [x, y, z]` on one part of the group. A model entity turns to face its movement direction in 3D; the engine chooses walk / idle / air poses from its velocity and grounded state. Trees, rocks and props need no groups.

```toml
[[parts]]
group = "leg_l"
shape = "capsule"
position = [-0.2, 0.3, 0.0]
size = [0.3, 0.65, 0.3]
color = [0.15, 0.2, 0.4]
blend = 0.05
```

## Conventions
- Y is up. 2D lives on the XY plane (z = 0); the camera looks down -Z. `zoom` is the visible height; width = zoom * aspect (about 16:9).
- 3D: perspective 60 degree FOV, sun from upper right. Use a big flat box as the floor, e.g. scale [40, 1, 40] at y = -0.5.
- Physics is axis-aligned boxes for everything; rotation does not affect collision.
- `static` never moves. `kinematic` moves only by script/velocity and ignores gravity and pushes. `dynamic` gets gravity, velocity and is pushed out of solid bodies. `none` = decoration.
- Colours are 0..1 RGB. Texts use screen coordinates 0..1 from top-left; size is pixels.
- 2D characters, items and enemies: sprites. 3D characters and props: models. Walls, floors, platforms: plain boxes.
- TAB is reserved for the engine's chat box; do not use it in games.

## script.rhai

Rhai syntax is Rust-like: `let`, `fn`, `if`, `for x in arr`, `while`, `+=`, `"strings"`, `[arrays]`, `#{maps}`, closures `|x| x*2`. Integers and floats mix freely in engine calls.

Define these (all optional):
- `fn init()` runs at start and after every restart.
- `fn update(dt)` runs every frame; `dt` = seconds since last frame.
- `fn on_collision(a, b)` called for each colliding pair, both orderings, so `if a == "player"` is enough.

Functions cannot see top-level `let` variables. Persist state with `set("score", 0)`, `get("score")`, `add("score", 1)` (returns the new value), `get_or("lives", 3)`.

### Vectors
`vec3(x,y,z)`, `vec2(x,y)`, fields `.x .y .z` (read/write), `+ - * /`, `length(v)`, `normalize(v)`, `dot(a,b)`, `lerp(a,b,t)`. A `[x,y,z]` array works anywhere a vector is expected.

### Input
`key("space")` held, `key_pressed("space")` this frame, `key_released(k)`. Keys: `a`-`z`, `0`-`9`, `left right up down space enter escape shift ctrl alt backspace`.
`axis_x()` -1..1 from A/D or arrows, `axis_y()` from W/S or arrows. `mouse_down()`, `mouse_pressed()`, `mouse_pos()` (world position on z = 0 in 2D).

### Entities
- Read: `pos(name)`, `vel(name)`, `scale_of(name)`, `rotation_of(name)`, `exists(name)`, `grounded(name)`.
- Move: `set_pos(name, v)`, `move_by(name, v)`, `set_vel(name, v)`, `add_vel(name, v)`, `set_vel_x/y/z(name, f)`.
- Look: `set_scale`, `set_rotation`, `rotate(name, degrees_vec)`, `set_color(name, rgb)`, `set_sprite(name, sprite)`, `set_flip_x(name, bool)`, `set_model(name, model)`, `look_at_2d(name, target_vec)`.
- Physics: `set_body(name, "dynamic")`, `set_solid(name, bool)`.
- Tags: `has_tag`, `add_tag`, `remove_tag`, `find_tag(tag)` -> names, `count_tag(tag)`, `nearest_tag(from, tag)` -> name or `()`, `distance(a, b)`.
- Spawn: `spawn(name, shape, pos, scale, color, body)`, `spawn_tagged(name, [tags], shape, pos, scale, color, body)`, `spawn_sprite(name, [tags], sprite, pos, scale, body)`, `spawn_model(name, [tags], model, pos, scale, body)`. All return the actual (unique) name.
- `destroy(name)`, `destroy_tag(tag)`.

### Animation
- Models: `play_anim(name, "jump" | "attack" | "hit" | "spin" | "wave" | "nod")` one-shot on top of the automatic walk/idle. `set_anim(name, "walk" | "idle" | "air" | "none" | "auto")` forces a loop. `set_part(name, group, rot_degrees_vec, offset_vec)` manual pose (e.g. aim an arm). `face_velocity(name, false)` to stop auto-turning.
- Sprites: `animate_sprite(name, ["run1", "run2", "run3"], 8)` cycles frames at 8 fps; `animate_sprite_once(...)` plays through; `stop_sprite_anim(name)`.
- Tweens (smooth over seconds): `tween_pos(name, v, secs)`, `tween_scale`, `tween_rotation`, `tween_color`. `squash(name, 0.3)` = instant squash-and-stretch that springs back (landings, hits, pickups).

### Sound (all procedural, no files)
- `play_sound(name)`: `shoot laser hit explosion jump coin pickup powerup hurt click win lose bounce step`.
- `play_tone(freq_hz, seconds, wave)` with wave `sine square saw triangle noise`.
- `play_music(bpm, "C4 E4 G4 C5 - A3 C4*2 E4/2 F4/2", wave)` loops a note sequence; `-` = rest, `*2` = two beats, `/2` = half beat. `stop_music()`.

### UI, camera, game
- `text(id, value)` create or update a text. `text_at(id, value, x, y, size)`, `text_color(id, rgb)`, `remove_text(id)`.
- `camera_follow(name)` / `camera_follow("")`, `camera_set(pos, look_at)`, `camera_zoom(units)`, `camera_pos()`.
- `set_gravity(v)`, `set_background(rgb)`, `restart()`.
- `time()`, `delta()`, `rand(lo, hi)`, `rand_int(lo, hi)`, `clamp`, `abs`, `sqrt`, `sin`, `cos`, `atan2`, `floor`, `round`, `min`, `max`, `to_float`, `print(str)`.

## Design rules
1. Playable immediately: controls shown in a text, a visible goal, score or win/lose state, restart on `r`.
2. Physics for platformers and ball games; `kinematic` + `set_vel`/`move_by` for top-down and arcade movement.
3. Under ~300 entities alive; spawn waves over time.
4. Camera shows the whole playfield or follows the player.
5. Robust scripts: check `exists(name)` for things that can be destroyed; never divide by zero.
6. Use sounds for feedback (shoot, hit, coin, win, lose) and a short music loop.
7. Make things move with life: model characters get animation groups, sprites get 2-4 frames with `animate_sprite`, and `squash`/`play_anim` on jumps, hits and pickups.

## Example: minimal 2D platformer

=== file: game.toml ===
title = "Coin Runner"
view = "2d"
background = [0.08, 0.09, 0.12]
gravity = [0.0, -30.0, 0.0]

[camera]
position = [0.0, 2.0, 20.0]
zoom = 18.0
follow = "player"

=== file: scene.toml ===
[[entities]]
name = "ground"
shape = "box"
position = [0.0, -1.0, 0.0]
scale = [60.0, 2.0, 1.0]
color = [0.3, 0.5, 0.3]
[entities.physics]
body = "static"

[[entities]]
name = "platform"
shape = "box"
position = [6.0, 2.5, 0.0]
scale = [5.0, 0.6, 1.0]
color = [0.4, 0.4, 0.5]
[entities.physics]
body = "static"

[[entities]]
name = "player"
tags = ["player"]
sprite = "hero"
position = [-5.0, 1.0, 0.0]
scale = [1.0, 1.4, 1.0]
[entities.physics]
body = "dynamic"

[[texts]]
id = "score"
text = "Coins: 0"
x = 0.02
y = 0.02
size = 28.0

[[texts]]
id = "help"
text = "A/D move, SPACE jump, R restart"
x = 0.02
y = 0.08
size = 18.0
color = [0.7, 0.7, 0.7]

=== file: sprites/hero.txt ===
k = 0.10 0.10 0.12
s = 0.95 0.80 0.65
r = 0.85 0.20 0.20
b = 0.20 0.35 0.80
---
..kkkk..
.kssssk.
.ksksks.
..ssss..
.rrrrrr.
r.rrrr.r
..bbbb..
..kk.kk.

=== file: sprites/coin.txt ===
y = 0.85 0.65 0.10
Y = 1.00 0.85 0.20
---
..yyyy..
.yYYYYy.
yYYyyYYy
yYyYYyYy
yYyYYyYy
yYYyyYYy
.yYYYYy.
..yyyy..

=== file: script.rhai ===
fn init() {
  set("score", 0);
  for i in 0..8 {
    let n = spawn_sprite("coin", ["coin"], "coin", vec3(-8 + i * 3, 1.5, 0), vec3(0.7, 0.7, 0.7), "static");
    set_solid(n, false);
  }
  play_music(140, "C4 E4 G4 C5 - G4 E4 C4", "square");
}

fn update(dt) {
  if key_pressed("r") { restart(); return; }
  let ax = axis_x();
  set_vel_x("player", ax * 8);
  if ax < 0 { set_flip_x("player", true); }
  if ax > 0 { set_flip_x("player", false); }
  if key_pressed("space") && grounded("player") { set_vel_y("player", 14); play_sound("jump"); }
  if pos("player").y < -10 { play_sound("lose"); restart(); }
}

fn on_collision(a, b) {
  if a == "player" && has_tag(b, "coin") {
    destroy(b);
    play_sound("coin");
    let s = add("score", 1);
    text("score", "Coins: " + s);
    if count_tag("coin") == 0 { play_sound("win"); text_at("win", "YOU WIN! R to restart", 0.35, 0.45, 40); }
  }
}
