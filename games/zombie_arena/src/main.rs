//! ZombieArena: written by Senne from the game's sentences (behavior.txt).
//! Each block is commented with the sentence that asked for it and the [pattern/variant]
//! that wrote it. Change the sentences, not this file.

use droid::prelude::*;

struct ZombieArena {
    status: Status,
    lives: i64,
    score: i64,
    clock: f32,
    wave: i64,
    safe: f32,
    timer: f32,
    time_60_0: bool,
}

impl Default for ZombieArena {
    fn default() -> Self {
        ZombieArena {
            status: Status::Playing,
            lives: 3,
            score: 0,
            clock: 0.0,
            wave: 1,
            safe: 0.0,
            timer: 15.0,
            time_60_0: false,
        }
    }
}

impl Game for ZombieArena {
    fn init(&mut self, cx: &mut Ctx) {
        // "At the start, the player has 3 lives, play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave, show "SCORE {score}" at the top left in "#f2f2f2", show "LIVES {lives}" at the top right in "#f2f2f2", show "TIME {countdown 60}" at the top in "#f2f2f2" and show "WAVE {wave}" at the bottom in "#f2f2f2"." [start/init]
        // the player has 3 lives [lives/set]
        self.lives = 3;
        // play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave [music/play]
        cx.play_music(132.0, "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4", "square");
        // show "SCORE {score}" at the top left in "#f2f2f2" [show/every_frame]
        cx.text_at("text_0", &format!("SCORE {}", self.score), 0.02, 0.03, 24.0);
        cx.text_color("text_0", vec3(0.949, 0.949, 0.949));
        // show "LIVES {lives}" at the top right in "#f2f2f2" [show/every_frame]
        cx.text_at("text_1", &format!("LIVES {}", self.lives), 0.78, 0.03, 24.0);
        cx.text_color("text_1", vec3(0.949, 0.949, 0.949));
        // show "TIME {countdown 60}" at the top in "#f2f2f2" [show/every_frame]
        cx.text_at("text_2", &format!("TIME {}", (60.0 - self.clock).max(0.0).ceil() as i64), 0.42, 0.03, 24.0);
        cx.text_color("text_2", vec3(0.949, 0.949, 0.949));
        // show "WAVE {wave}" at the bottom in "#f2f2f2" [show/every_frame]
        cx.text_at("text_3", &format!("WAVE {}", self.wave), 0.4, 0.92, 24.0);
        cx.text_color("text_3", vec3(0.949, 0.949, 0.949));
    }

    fn update(&mut self, cx: &mut Ctx, dt: f32) {
        // "When the player presses r, restart the game." [key_press/pressed]
        if cx.key_pressed("r") {
            // restart the game [restart/request]
            cx.restart();
            return;
        }
        // "At the start, the player has 3 lives, play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave, show "SCORE {score}" at the top left in "#f2f2f2", show "LIVES {lives}" at the top right in "#f2f2f2", show "TIME {countdown 60}" at the top in "#f2f2f2" and show "WAVE {wave}" at the bottom in "#f2f2f2"." [show/every_frame]
        cx.text_at("text_0", &format!("SCORE {}", self.score), 0.02, 0.03, 24.0);
        // "At the start, the player has 3 lives, play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave, show "SCORE {score}" at the top left in "#f2f2f2", show "LIVES {lives}" at the top right in "#f2f2f2", show "TIME {countdown 60}" at the top in "#f2f2f2" and show "WAVE {wave}" at the bottom in "#f2f2f2"." [show/every_frame]
        cx.text_at("text_1", &format!("LIVES {}", self.lives), 0.78, 0.03, 24.0);
        // "At the start, the player has 3 lives, play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave, show "SCORE {score}" at the top left in "#f2f2f2", show "LIVES {lives}" at the top right in "#f2f2f2", show "TIME {countdown 60}" at the top in "#f2f2f2" and show "WAVE {wave}" at the bottom in "#f2f2f2"." [show/every_frame]
        cx.text_at("text_2", &format!("TIME {}", (60.0 - self.clock).max(0.0).ceil() as i64), 0.42, 0.03, 24.0);
        // "At the start, the player has 3 lives, play the music "C4/2 E4/2 G4/2 E4/2 F4/2 A4/2 C5/2 A4/2 G4/2 B4/2 D5/2 B4/2 C5 - G4/2 E4/2 C4" at 132 beats per minute as a square wave, show "SCORE {score}" at the top left in "#f2f2f2", show "LIVES {lives}" at the top right in "#f2f2f2", show "TIME {countdown 60}" at the top in "#f2f2f2" and show "WAVE {wave}" at the bottom in "#f2f2f2"." [show/every_frame]
        cx.text_at("text_3", &format!("WAVE {}", self.wave), 0.4, 0.92, 24.0);
        if self.status != Status::Playing {
            return;
        }
        self.clock += dt;
        // blink while safe after losing a life
        if self.safe > 0.0 {
            self.safe -= dt;
            let lit = self.safe <= 0.0 || (self.safe * 12.0) as i64 % 2 == 0;
            cx.set_color("player", if lit { Vec3::ONE } else { Vec3::splat(0.35) });
        }
        // "Move the player with the arrow keys at 7 units per second and face where it moves." [move_player/normalized]
        let want = vec2(cx.axis_x(), cx.axis_y()).normalize_or_zero() * 7.0;
        cx.set_vel_x("player", want.x);
        cx.set_vel_y("player", want.y);
        if want.x != 0.0 {
            cx.set_flip_x("player", want.x < 0.0);
        }
        // "Animate the player with "hero_1" and "hero_2" at 8 frames per second while it walks." [animate/clock]
        let frame = ["hero_1", "hero_2"][(cx.time() * 8.0) as usize % 2];
        let v = cx.vel("player");
        let walking = vec2(v.x, v.y).length() > 0.1;
        cx.set_sprite("player", if walking { frame } else { "hero_1" });
        // "Each zombie moves toward the player at 2.3 units per second." [chase/per_loop]
        for n in cx.find_tag("zombie") {
            let to = cx.pos("player") - cx.pos(&n);
            cx.set_vel(&n, vec2(to.x, to.y).normalize_or_zero() * 2.3);
        }
        // "Animate each zombie with "zombie_1", "zombie_2" and "zombie_3" at 6 frames per second." [animate/clock]
        let frame = ["zombie_1", "zombie_2", "zombie_3"][(cx.time() * 6.0) as usize % 3];
        for n in cx.find_tag("zombie") {
            cx.set_sprite(&n, frame);
        }
        // "Every 15 seconds, start the next wave of zombies." [every/countdown]
        self.timer -= dt;
        if self.timer <= 0.0 {
            self.timer += 15.0;
            // start the next wave of zombies [next_wave/respawn]
            self.wave += 1;
            cx.respawn_tag("zombie");
        }
        // "When the time reaches 60 seconds, the player wins "YOU SURVIVED"." [reaches/edge]
        let reached = self.clock >= 60.0;
        if reached && !self.time_60_0 {
            // the player wins "YOU SURVIVED" [win/inline]
            if self.status == Status::Playing {
                self.status = Status::Won;
                cx.play_sound("win");
                cx.text_at("banner", "YOU SURVIVED - PRESS R", 0.3, 0.4, 44.0);
            }
        }
        self.time_60_0 = reached;
    }

    fn on_collision(&mut self, cx: &mut Ctx, a: &str, b: &str) {
        if self.status != Status::Playing {
            return;
        }
        // "When the player touches a gem, destroy it, add 5 points, squash the player and play the pickup sound." [touch/has_tag]
        if a == "player" && cx.has_tag(b, "gem") {
            // destroy it [destroy/despawn]
            cx.destroy(b);
            // add 5 points [score/add]
            self.score += 5;
            // squash the player [squash/tween]
            cx.squash("player", 0.3);
            // play the pickup sound [sound/effect]
            cx.play_sound("pickup");
        }
        // "When the player touches a zombie, the player loses a life, safe for 1.5 seconds." [touch/has_tag]
        if a == "player" && cx.has_tag(b, "zombie") {
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
    droid::run::<ZombieArena>(concat!(env!("CARGO_MANIFEST_DIR"), "/game"));
}
