//! GnomeGarden: written by Senne from the game's sentences (behavior.txt).
//! Each block is commented with the sentence that asked for it and the [pattern/variant]
//! that wrote it. Change the sentences, not this file.

use droid::prelude::*;

struct GnomeGarden {
    status: Status,
    lives: i64,
    score: i64,
    wander_blob: f32,
    safe: f32,
    start: Vec3,
    no_gem_left: bool,
}

impl Default for GnomeGarden {
    fn default() -> Self {
        GnomeGarden {
            status: Status::Playing,
            lives: 3,
            score: 0,
            wander_blob: 0.0,
            safe: 0.0,
            start: Vec3::ZERO,
            no_gem_left: false,
        }
    }
}

impl Game for GnomeGarden {
    fn init(&mut self, cx: &mut Ctx) {
        self.start = cx.pos("player");
        // "At the start, the player has 3 lives, play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave, show "GEMS {score} / 8" at the top left in "#263326" and show "LIVES {lives}" at the top right in "#263326"." [start/init]
        // the player has 3 lives [lives/set]
        self.lives = 3;
        // play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave [music/play]
        cx.play_music(132.0, "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4", "square");
        // show "GEMS {score} / 8" at the top left in "#263326" [show/every_frame]
        cx.text_at("text_0", &format!("GEMS {} / 8", self.score), 0.02, 0.03, 24.0);
        cx.text_color("text_0", vec3(0.149, 0.2, 0.149));
        // show "LIVES {lives}" at the top right in "#263326" [show/every_frame]
        cx.text_at("text_1", &format!("LIVES {}", self.lives), 0.78, 0.03, 24.0);
        cx.text_color("text_1", vec3(0.149, 0.2, 0.149));
    }

    fn update(&mut self, cx: &mut Ctx, dt: f32) {
        // "When the player presses r, restart the game." [key_press/pressed]
        if cx.key_pressed("r") {
            // restart the game [restart/request]
            cx.restart();
            return;
        }
        // "At the start, the player has 3 lives, play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave, show "GEMS {score} / 8" at the top left in "#263326" and show "LIVES {lives}" at the top right in "#263326"." [show/every_frame]
        cx.text_at("text_0", &format!("GEMS {} / 8", self.score), 0.02, 0.03, 24.0);
        // "At the start, the player has 3 lives, play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave, show "GEMS {score} / 8" at the top left in "#263326" and show "LIVES {lives}" at the top right in "#263326"." [show/every_frame]
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
        // "Move the player across the ground with the arrow keys at 7 units per second and face where it moves." [move_player/normalized]
        let want = vec2(cx.axis_x(), cx.axis_y()).normalize_or_zero() * 7.0;
        cx.set_vel_x("player", want.x);
        cx.set_vel_z("player", -want.y);
        if want.x != 0.0 {
            cx.set_flip_x("player", want.x < 0.0);
        }
        // "Each blob wanders across the ground at 2 units per second, turning every 1.6 seconds." [wander/shared_timer]
        self.wander_blob -= dt;
        if self.wander_blob <= 0.0 {
            self.wander_blob += 1.6;
            for n in cx.find_tag("blob") {
                let a = cx.random(0.0, std::f32::consts::TAU);
                cx.set_vel_x(&n, a.cos() * 2.0);
                cx.set_vel_z(&n, a.sin() * 2.0);
            }
        }
        // "When the player falls below -12, the player goes back to the start." [falls_below/check]
        if cx.pos("player").y < -12.0 {
            // the player goes back to the start [back_to_start/reset]
            cx.set_pos("player", self.start);
            cx.set_vel("player", Vec3::ZERO);
        }
        // "When no gems are left, the player wins "ALL COLLECTED"." [none_left/edge]
        let none = cx.count_tag("gem") == 0;
        if none && !self.no_gem_left {
            // the player wins "ALL COLLECTED" [win/inline]
            if self.status == Status::Playing {
                self.status = Status::Won;
                cx.play_sound("win");
                cx.text_at("banner", "ALL COLLECTED - PRESS R", 0.3, 0.4, 44.0);
            }
        }
        self.no_gem_left = none;
    }

    fn on_collision(&mut self, cx: &mut Ctx, a: &str, b: &str) {
        if self.status != Status::Playing {
            return;
        }
        // "When the player touches a gem, destroy it, add 1 to the score, play the spin animation on the player and play the pickup sound." [touch/has_tag]
        if a == "player" && cx.has_tag(b, "gem") {
            // destroy it [destroy/despawn]
            cx.destroy(b);
            // add 1 to the score [score/add]
            self.score += 1;
            // play the spin animation on the player [play_anim/oneshot]
            cx.play_anim("player", "spin");
            // play the pickup sound [sound/effect]
            cx.play_sound("pickup");
        }
        // "When the player touches a blob, the player loses a life, safe for 1.5 seconds." [touch/has_tag]
        if a == "player" && cx.has_tag(b, "blob") {
            // the player loses a life, safe for 1.5 seconds [hurt/inline]
            if self.safe <= 0.0 {
                self.lives -= 1;
                cx.play_sound("hurt");
                if self.lives <= 0 {
                    if self.status == Status::Playing {
                        self.status = Status::Lost;
                        cx.play_sound("lose");
                        cx.text_at("banner", "GAME OVER - PRESS R", 0.3, 0.4, 44.0);
                    }
                } else {
                    self.safe = 1.5;
                }
            }
        }
    }

    fn status(&self) -> Status {
        self.status
    }
}

fn main() {
    droid::run::<GnomeGarden>(concat!(env!("CARGO_MANIFEST_DIR"), "/game"));
}
