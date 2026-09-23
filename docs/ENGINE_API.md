# AIGE engine reference

A game is ONE JSON document (`game.json`) with this shape:

```json
{
  "title": "My Game",
  "description": "one line",
  "view": "2d",                       // "2d" or "3d"
  "camera": {
    "position": [0, 0, 20],           // 2d: x,y = view centre. 3d: eye position
    "look_at": [0, 0, 0],             // 3d only
    "zoom": 20,                       // 2d only: world units visible vertically
    "follow": "player"                // optional entity name to keep centred
  },
  "background": [0.08, 0.09, 0.12],   // rgb 0..1
  "gravity": [0, -20, 0],             // world units / s^2 (set [0,0,0] for top-down)
  "sprites": { "hero": { ...SpriteSpec... } },
  "entities": [ ...EntitySpec... ],
  "texts": [ { "id": "score", "text": "Score: 0", "x": 0.02, "y": 0.02, "size": 24, "color": [1,1,1] } ],
  "script": "... Rhai source code ..."
}
```

EntitySpec:

```json
{
  "name": "player",                   // unique; duplicates get _N suffix
  "tags": ["player"],
  "shape": "box",                     // "box" | "sphere" | "plane"
  "sprite": "hero",                   // optional: draw as this pixel-art sprite (shape becomes a textured quad)
  "position": [0, 0, 0],
  "rotation": [0, 0, 0],              // euler degrees
  "scale": [1, 1, 1],                 // box/plane: full width/height/depth. sphere: diameter
  "color": [0.2, 0.8, 0.3],
  "physics": {
    "body": "dynamic",                // "none" | "static" | "kinematic" | "dynamic"
    "velocity": [0, 0, 0],
    "bounciness": 0.0,                // 0..1
    "solid": true                     // false = trigger: collision events only, no blocking
  }
}
```

## Sprites (pixel art you draw)
You are the artist. Draw pixel art as text: a `palette` maps single characters to RGB, and `pixels` is one string per row (top row first). `.` = transparent. Rows should have equal length. Typical sizes: 8x8 (items), 12x16 (characters), 16x16 to 32x32 (bigger things). Rendered crisp (no smoothing), so bold silhouettes and 3-6 colours read best; add a 1px dark outline for contrast.

```json
"sprites": {
  "hero": {
    "palette": { "k": [0.1,0.1,0.12], "s": [0.95,0.8,0.65], "r": [0.85,0.2,0.2], "b": [0.2,0.35,0.8] },
    "pixels": [
      "..kkkk..",
      ".kssssk.",
      ".ksksks.",
      "..ssss..",
      ".rrrrrr.",
      "r.rrrr.r",
      "..bbbb..",
      "..kk.kk."
    ]
  }
}
```

An entity with `"sprite": "hero"` is drawn as a flat quad of its `scale` (x = width, y = height in world units) showing that sprite; `color` tints it (keep `[1,1,1]` for the original colours). Sprites are unaffected by lighting and work in both 2D and 3D. Physics still uses the box of `scale`.
For animation, define several sprites (`hero_walk1`, `hero_walk2`, ...) and switch with `set_sprite`. Use `set_flip_x(name, true)` to face left. Keep the aspect of `scale` equal to the pixel aspect so pixels stay square.

## Conventions
- Y is up. In 2D everything lives on the XY plane (z=0). Camera looks down -Z.
- 2D: `zoom` is the visible height in world units; width = zoom * aspect (~16:9). A 20-unit zoom shows y in [-10, 10] around the camera.
- 3D: perspective 60° FOV, directional light from upper right. Use a big flat box as floor, e.g. scale [40, 1, 40] at y=-0.5.
- Physics is AABB (axis-aligned boxes) for all shapes; rotation does not affect collision.
- Body kinds: `static` never moves. `kinematic` moves only via script/velocity and is not affected by gravity or pushes. `dynamic` gets gravity, velocity, is pushed out of solid bodies. `none` = decoration only.
- Colours are 0..1 RGB. Texts use screen coordinates 0..1 from top-left; size is pixels.
- Visuals are pixel-art sprites (preferred for characters, items, enemies in 2D) plus coloured primitives (ground, walls, platforms, 3D scenes). Build 3D characters from several primitives (body box + head sphere) and move them together in script.

## Script (Rhai)
The `script` string is Rhai (Rust-like syntax: `let`, `fn`, `if`, `for x in arr`, `while`, `+=`, `"strings"`, `[arrays]`, `#{maps}`, closures `|x| x*2`). No semicolons needed after `}`. Integers and floats mix freely in engine calls.

Define these functions (all optional):
- `fn init()` runs once at start and after every restart.
- `fn update(dt)` runs every frame; `dt` is seconds since last frame.
- `fn on_collision(a, b)` called for each colliding pair, once per ordering (both (a,b) and (b,a)), so you can just check `a == "player"`.

Script functions cannot see top-level `let` variables. Persist state with `set("score", 0)`, `get("score")`, `add("score", 1)` (returns the new value), `get_or("lives", 3)`.

### Vectors
`vec3(x,y,z)`, `vec2(x,y)`, fields `.x .y .z` (read/write), `+ - * /`, `length(v)`, `normalize(v)`, `dot(a,b)`, `lerp(a,b,t)`. A `[x,y,z]` array is accepted anywhere a vector is expected.

### Input
`key("space")` held, `key_pressed("space")` this frame, `key_released(k)`. Key names: `a`-`z`, `0`-`9`, `left right up down space enter escape tab shift ctrl alt backspace`.
`axis_x()` -1..1 from A/D or arrows, `axis_y()` from W/S or arrows.
`mouse_down()`, `mouse_pressed()`, `mouse_pos()` world position on the z=0 plane (2D).

### Entities
- `pos(name)`, `vel(name)`, `scale_of(name)`, `rotation_of(name)`, `exists(name)`, `grounded(name)` (dynamic body resting on something below).
- `set_pos(name, v)`, `move_by(name, v)`, `set_vel(name, v)`, `add_vel(name, v)`, `set_vel_x/set_vel_y/set_vel_z(name, f)`.
- `set_scale(name, v)`, `set_rotation(name, v)`, `rotate(name, v)` (degrees), `set_color(name, rgb)`, `set_body(name, "dynamic")`, `set_solid(name, bool)`.
- `look_at_2d(name, target_vec)` rotates around Z to face a point.
- `has_tag(name, tag)`, `add_tag`, `remove_tag`, `find_tag(tag)` -> array of names, `count_tag(tag)`, `nearest_tag(from_name, tag)` -> name or `()`, `distance(a, b)`.
- `spawn(name, shape, pos, scale, color, body)` -> actual name (unique). `spawn_tagged(name, [tags], shape, pos, scale, color, body)`.
- `spawn_sprite(name, [tags], sprite_name, pos, scale, body)` -> actual name. `set_sprite(name, sprite_name)` swaps frames, `set_flip_x(name, bool)` mirrors.
- `destroy(name)`, `destroy_tag(tag)`.

### UI / camera / game
- `text(id, value)` create or update a text (value may be any type). `text_at(id, value, x, y, size)`, `text_color(id, rgb)`, `remove_text(id)`.
- `camera_follow(name)` or `camera_follow("")`, `camera_set(pos, look_at)`, `camera_zoom(units)`, `camera_pos()`.
- `set_gravity(v)`, `set_background(rgb)`, `restart()` (rebuilds the whole scene and calls init again).
- `time()` seconds since start, `delta()`, `rand(lo, hi)` float, `rand_int(lo, hi)` inclusive, `clamp`, `abs`, `sqrt`, `sin`, `cos`, `atan2`, `floor`, `round`, `min`, `max`, `to_float`, `print(str)`.

## Design rules for generated games
1. The game must be playable immediately: controls explained in a text element, visible goal, score or win/lose state, and restart on `r` (`if key_pressed("r") { restart() }`).
2. Use physics for platformers/ball games; use `kinematic` + `set_vel` or `move_by` for top-down/arcade movement.
3. Keep entity counts modest (< 300). Spawn waves over time rather than all at once.
4. Set the camera so the whole playfield is visible, or follow the player.
5. Keep the script robust: check `exists(name)` before using entities that may be destroyed; never divide by zero.
6. Everything must be inside the single JSON document; the script is a JSON string, so escape quotes and newlines (`\n`) properly.

## Minimal example (2D platformer)

```json
{
  "title": "Coin Runner",
  "view": "2d",
  "camera": {
    "position": [
      0,
      2,
      20
    ],
    "zoom": 18,
    "follow": "player"
  },
  "gravity": [
    0,
    -30,
    0
  ],
  "entities": [
    {
      "name": "ground",
      "shape": "box",
      "position": [
        0,
        -1,
        0
      ],
      "scale": [
        60,
        2,
        1
      ],
      "color": [
        0.3,
        0.5,
        0.3
      ],
      "physics": {
        "body": "static"
      }
    },
    {
      "name": "platform",
      "shape": "box",
      "position": [
        6,
        2.5,
        0
      ],
      "scale": [
        5,
        0.6,
        1
      ],
      "color": [
        0.4,
        0.4,
        0.5
      ],
      "physics": {
        "body": "static"
      }
    },
    {
      "name": "player",
      "tags": [
        "player"
      ],
      "shape": "box",
      "position": [
        -5,
        1,
        0
      ],
      "scale": [
        1.2,
        1.4,
        1
      ],
      "color": [
        1,
        1,
        1
      ],
      "physics": {
        "body": "dynamic"
      },
      "sprite": "hero_walk1"
    },
    {
      "name": "coin",
      "tags": [
        "coin"
      ],
      "shape": "sphere",
      "position": [
        6,
        3.6,
        0
      ],
      "scale": [
        0.7,
        0.7,
        0.7
      ],
      "color": [
        1,
        1,
        1
      ],
      "physics": {
        "body": "static",
        "solid": false
      },
      "sprite": "coin"
    }
  ],
  "texts": [
    {
      "id": "score",
      "text": "Coins: 0",
      "x": 0.02,
      "y": 0.02,
      "size": 28
    },
    {
      "id": "help",
      "text": "A/D move, SPACE jump, R restart",
      "x": 0.02,
      "y": 0.08,
      "size": 18,
      "color": [
        0.7,
        0.7,
        0.7
      ]
    }
  ],
  "script": "fn init() {\n  set(\"score\", 0);\n  for i in 0..8 {\n    spawn_sprite(\"coin\", [\"coin\"], \"coin\", vec3(-8 + i * 3, 1.5, 0), vec3(0.7, 0.7, 0.7), \"static\");\n    set_solid(\"coin_\" + i, false);\n  }\n  for n in find_tag(\"coin\") { set_solid(n, false); }\n}\n\nfn update(dt) {\n  if key_pressed(\"r\") { restart(); return; }\n  let ax = axis_x();\n  set_vel_x(\"player\", ax * 8);\n  if ax < 0 { set_flip_x(\"player\", true); }\n  if ax > 0 { set_flip_x(\"player\", false); }\n  if ax != 0 && grounded(\"player\") {\n    if floor(time() * 8) % 2 == 0 { set_sprite(\"player\", \"hero_walk1\"); } else { set_sprite(\"player\", \"hero_walk2\"); }\n  } else {\n    set_sprite(\"player\", \"hero_walk1\");\n  }\n  if key_pressed(\"space\") && grounded(\"player\") { set_vel_y(\"player\", 14); }\n  if pos(\"player\").y < -10 { restart(); }\n  for n in find_tag(\"coin\") { set_scale(n, vec3(0.7 * abs(cos(time() * 3)) + 0.1, 0.7, 0.7)); }\n}\n\nfn on_collision(a, b) {\n  if a == \"player\" && has_tag(b, \"coin\") {\n    destroy(b);\n    let s = add(\"score\", 1);\n    text(\"score\", \"Coins: \" + s);\n    if count_tag(\"coin\") == 0 { text_at(\"win\", \"YOU WIN! R to restart\", 0.35, 0.45, 40); }\n  }\n}",
  "sprites": {
    "hero_walk1": {
      "palette": {
        "k": [
          0.1,
          0.1,
          0.12
        ],
        "s": [
          0.95,
          0.8,
          0.65
        ],
        "h": [
          0.55,
          0.3,
          0.15
        ],
        "r": [
          0.85,
          0.2,
          0.2
        ],
        "b": [
          0.2,
          0.35,
          0.8
        ],
        "w": [
          1,
          1,
          1
        ]
      },
      "pixels": [
        "....hhhhh...",
        "...hhhhhhh..",
        "...hsssss...",
        "...skssks...",
        "...sssss....",
        "....sss.....",
        "..rrrrrrr...",
        ".rrrrrrrrr..",
        ".rsrrrrrsr..",
        ".s.rrrrr.s..",
        "...bbbbb....",
        "...bb.bb....",
        "...bb.bb....",
        "..kk...kk..."
      ]
    },
    "hero_walk2": {
      "palette": {
        "k": [
          0.1,
          0.1,
          0.12
        ],
        "s": [
          0.95,
          0.8,
          0.65
        ],
        "h": [
          0.55,
          0.3,
          0.15
        ],
        "r": [
          0.85,
          0.2,
          0.2
        ],
        "b": [
          0.2,
          0.35,
          0.8
        ],
        "w": [
          1,
          1,
          1
        ]
      },
      "pixels": [
        "....hhhhh...",
        "...hhhhhhh..",
        "...hsssss...",
        "...skssks...",
        "...sssss....",
        "....sss.....",
        "..rrrrrrr...",
        ".rrrrrrrrr..",
        ".rsrrrrrsr..",
        ".s.rrrrr.s..",
        "...bbbbb....",
        "..bb...bb...",
        ".bb.....bb..",
        "kk.......kk."
      ]
    },
    "coin": {
      "palette": {
        "y": [
          0.85,
          0.65,
          0.1
        ],
        "Y": [
          1,
          0.85,
          0.2
        ],
        "o": [
          0.95,
          0.55,
          0.05
        ]
      },
      "pixels": [
        "..yyyy..",
        ".yYYYYy.",
        "yYYooYYy",
        "yYoYYoYy",
        "yYoYYoYy",
        "yYYooYYy",
        ".yYYYYy.",
        "..yyyy.."
      ]
    }
  }
}
```
