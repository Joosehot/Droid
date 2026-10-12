//! SlimeRun: written by Senne from the game's sentences (behavior.txt).
//! Each block is commented with the sentence that asked for it and the [pattern/variant]
//! that wrote it. Change the sentences, not this file.

use droid::prelude::*;
use std::collections::BTreeMap;

struct SlimeRun {
    status: Status,
    lives: i64,
    score: i64,
    jump_buffer: f32,
    coyote: f32,
    patrol_slime: BTreeMap<String, (f32, f32)>,
    safe: f32,
    start: Vec3,
    no_coin_left: bool,
}

impl Default for SlimeRun {
    fn default() -> Self {
        SlimeRun {
            status: Status::Playing,
            lives: 3,
            score: 0,
            jump_buffer: 0.0,
            coyote: 0.0,
            patrol_slime: BTreeMap::new(),
            safe: 0.0,
            start: Vec3::ZERO,
            no_coin_left: false,
        }
    }
}

impl SlimeRun {
    fn hurt(&mut self, cx: &mut Ctx, safe: f32) {
        if self.safe > 0.0 {
            return;
        }
        self.lives -= 1;
        cx.play_sound("hurt");
        if self.lives <= 0 {
            if self.status == Status::Playing {
                self.status = Status::Lost;
                cx.play_sound("lose");
                cx.text_at("banner", "GAME OVER - PRESS R", 0.3, 0.4, 44.0);
            }
        } else {
            self.safe = safe;
        }
    }
}

impl Game for SlimeRun {
    fn init(&mut self, cx: &mut Ctx) {
        self.start = cx.pos("player");
        // "At the start, the player has 3 lives, play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave, show "COINS {score} / 14" at the top left in "#ffffff" and show "LIVES {lives}" at the top right in "#ffffff"." [start/init]
        // the player has 3 lives [lives/set]
        self.lives = 3;
        // play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave [music/play]
        cx.play_music(132.0, "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4", "square");
        // show "COINS {score} / 14" at the top left in "#ffffff" [show/every_frame]
        cx.text_at("text_0", &format!("COINS {} / 14", self.score), 0.02, 0.03, 24.0);
        cx.text_color("text_0", vec3(1.0, 1.0, 1.0));
        // show "LIVES {lives}" at the top right in "#ffffff" [show/every_frame]
        cx.text_at("text_1", &format!("LIVES {}", self.lives), 0.78, 0.03, 24.0);
        cx.text_color("text_1", vec3(1.0, 1.0, 1.0));
        // "Animate each slime with "slime_1" and "slime_2" at 4 frames per second." [animate/engine]
        for n in cx.find_tag("slime") {
            cx.animate_sprite(&n, &["slime_1", "slime_2"], 4.0);
        }
        // "Animate each coin with "coin_1" and "coin_2" at 4 frames per second." [animate/engine]
        for n in cx.find_tag("coin") {
            cx.animate_sprite(&n, &["coin_1", "coin_2"], 4.0);
        }
    }

    fn update(&mut self, cx: &mut Ctx, dt: f32) {
        // "When the player presses r, restart the game." [key_press/pressed]
        if cx.key_pressed("r") {
            // restart the game [restart/request]
            cx.restart();
            return;
        }
        // "At the start, the player has 3 lives, play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave, show "COINS {score} / 14" at the top left in "#ffffff" and show "LIVES {lives}" at the top right in "#ffffff"." [show/every_frame]
        cx.text_at("text_0", &format!("COINS {} / 14", self.score), 0.02, 0.03, 24.0);
        // "At the start, the player has 3 lives, play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave, show "COINS {score} / 14" at the top left in "#ffffff" and show "LIVES {lives}" at the top right in "#ffffff"." [show/every_frame]
        cx.text_at("text_1", &format!("LIVES {}", self.lives), 0.78, 0.03, 24.0);
        if self.status != Status::Playing {
            return;
        }
        // blink while safe after losing a life
        if self.safe > 0.0 {
            self.safe -= dt;
            let lit = self.safe <= 0.0 || (self.safe * 12.0) as i64 % 2 == 0;
            cx.set_color("player", if lit { Vec3::ONE } else { Vec3::splat(0.35) });
        }
        // "Move the player sideways with the arrow keys at 7.5 units per second and face where it moves." [move_player/axis]
        let want = cx.axis_x() * 7.5;
        cx.set_vel_x("player", want);
        if want != 0.0 {
            cx.set_flip_x("player", want < 0.0);
        }
        // "The player jumps with space at 14.5 units per second, higher while held, with the jump sound." [jump/buffered]
        if cx.grounded("player") {
            self.coyote = 0.1;
        } else {
            self.coyote -= dt;
        }
        if cx.key_pressed("space") {
            self.jump_buffer = 0.12;
        } else {
            self.jump_buffer -= dt;
        }
        if self.jump_buffer > 0.0 && self.coyote > 0.0 {
            self.jump_buffer = 0.0;
            self.coyote = 0.0;
            cx.set_vel_y("player", 14.5);
            cx.play_sound("jump");
        }
        let rise = cx.vel("player").y;
        if rise > 0.0 && !cx.key("space") {
            cx.set_vel_y("player", rise - 40.0 * dt);
        }
        // "Animate the player with "hero_1" and "hero_2" at 8 frames per second while it walks." [animate/clock]
        let frame = ["hero_1", "hero_2"][(cx.time() * 8.0) as usize % 2];
        let v = cx.vel("player");
        let walking = v.x.abs() > 0.1 && cx.grounded("player");
        cx.set_sprite("player", if walking { frame } else { "hero_1" });
        // "Each slime patrols 2.5 units to each side at 2.2 units per second." [patrol/bounds]
        for n in cx.find_tag("slime") {
            let x = cx.pos(&n).x;
            let (home, dir) = *self.patrol_slime.entry(n.clone()).or_insert((x, 1.0));
            let dir = if x > home + 2.5 { -1.0 } else if x < home - 2.5 { 1.0 } else { dir };
            cx.set_vel_x(&n, dir * 2.2);
            cx.set_flip_x(&n, dir < 0.0);
            self.patrol_slime.insert(n, (home, dir));
        }
        // "When the player falls below -12, the player loses a life and goes back to the start." [falls_below/check]
        if cx.pos("player").y < -12.0 {
            // the player loses a life [hurt/helper]
            self.hurt(cx, 1.0);
            // goes back to the start [back_to_start/reset]
            cx.set_pos("player", self.start);
            cx.set_vel("player", Vec3::ZERO);
        }
        // "When no coins are left, the player wins "ALL COLLECTED"." [none_left/edge]
        let none = cx.count_tag("coin") == 0;
        if none && !self.no_coin_left {
            // the player wins "ALL COLLECTED" [win/inline]
            if self.status == Status::Playing {
                self.status = Status::Won;
                cx.play_sound("win");
                cx.text_at("banner", "ALL COLLECTED - PRESS R", 0.3, 0.4, 44.0);
            }
        }
        self.no_coin_left = none;
    }

    fn on_collision(&mut self, cx: &mut Ctx, a: &str, b: &str) {
        if self.status != Status::Playing {
            return;
        }
        // "When the player lands on a slime, destroy it, bounce the player up at 11 units per second, squash the player and play the bounce sound." [lands_on/above]
        if a == "player" && cx.has_tag(b, "slime") && cx.pos(a).y - cx.pos(b).y > 0.25 {
            if cx.vel(a).y <= 0.0 {
                // destroy it [destroy/despawn]
                cx.destroy(b);
                // bounce the player up at 11 units per second [bounce/set_vel]
                cx.set_vel_y("player", 11.0);
                // squash the player [squash/tween]
                cx.squash("player", 0.3);
                // play the bounce sound [sound/effect]
                cx.play_sound("bounce");
            }
            return;
        }
        // "When the player touches a coin, destroy it, add 1 to the score, squash the player and play the coin sound." [touch/has_tag]
        if a == "player" && cx.has_tag(b, "coin") {
            // destroy it [destroy/despawn]
            cx.destroy(b);
            // add 1 to the score [score/add]
            self.score += 1;
            // squash the player [squash/tween]
            cx.squash("player", 0.3);
            // play the coin sound [sound/effect]
            cx.play_sound("coin");
        }
        // "When the player touches a slime, the player loses a life, safe for 1.5 seconds." [touch/has_tag]
        if a == "player" && cx.has_tag(b, "slime") {
            // the player loses a life, safe for 1.5 seconds [hurt/helper]
            self.hurt(cx, 1.5);
        }
    }

    fn status(&self) -> Status {
        self.status
    }
}

fn main() {
    droid::run::<SlimeRun>(concat!(env!("CARGO_MANIFEST_DIR"), "/game"));
}
