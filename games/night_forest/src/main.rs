//! NightForest: written by Senne from the game's sentences (behavior.txt).
//! Each block is commented with the sentence that asked for it and the [pattern/variant]
//! that wrote it. Change the sentences, not this file.

use droid::prelude::*;

struct NightForest {
    status: Status,
    score: i64,
    jump_buffer: f32,
    coyote: f32,
}

impl Default for NightForest {
    fn default() -> Self {
        NightForest {
            status: Status::Playing,
            score: 0,
            jump_buffer: 0.0,
            coyote: 0.0,
        }
    }
}

impl NightForest {
    fn lose(&mut self, cx: &mut Ctx, banner: &str) {
        if self.status != Status::Playing {
            return;
        }
        self.status = Status::Lost;
        cx.play_sound("lose");
        cx.text_at("banner", banner, 0.3, 0.4, 44.0);
    }
}

impl Game for NightForest {
    fn init(&mut self, cx: &mut Ctx) {
        // "At the start, play the music "A3/2 A3/2 C4/2 A3/2 E4/2 A3/2 D4/2 C4/2 A3/2 A3/2 C4/2 A3/2 G4/2 E4/2 D4 -" at 150 beats per minute as a saw wave and show "COINS {score} / 20" at the top left in "#ffffff"." [start/init]
        // play the music "A3/2 A3/2 C4/2 A3/2 E4/2 A3/2 D4/2 C4/2 A3/2 A3/2 C4/2 A3/2 G4/2 E4/2 D4 -" at 150 beats per minute as a saw wave [music/play]
        cx.play_music(150.0, "A3/2 A3/2 C4/2 A3/2 E4/2 A3/2 D4/2 C4/2 A3/2 A3/2 C4/2 A3/2 G4/2 E4/2 D4 -", "saw");
        // show "COINS {score} / 20" at the top left in "#ffffff" [show/every_frame]
        cx.text_at("text_0", &format!("COINS {} / 20", self.score), 0.02, 0.03, 24.0);
        cx.text_color("text_0", vec3(1.0, 1.0, 1.0));
        // "Animate each slime with "slime_1" and "slime_2" at 4 frames per second." [animate/engine]
        for n in cx.find_tag("slime") {
            cx.animate_sprite(&n, &["slime_1", "slime_2"], 4.0);
        }
        // "Animate each bat with "bat_1" and "bat_2" at 6 frames per second." [animate/engine]
        for n in cx.find_tag("bat") {
            cx.animate_sprite(&n, &["bat_1", "bat_2"], 6.0);
        }
        // "Animate each coin with "coin_1" and "coin_2" at 4 frames per second." [animate/engine]
        for n in cx.find_tag("coin") {
            cx.animate_sprite(&n, &["coin_1", "coin_2"], 4.0);
        }
        // "Animate each flag with "flag_1" and "flag_2" at 4 frames per second." [animate/engine]
        for n in cx.find_tag("flag") {
            cx.animate_sprite(&n, &["flag_1", "flag_2"], 4.0);
        }
    }

    fn update(&mut self, cx: &mut Ctx, dt: f32) {
        // "When the player presses r, restart the game." [key_press/pressed]
        if cx.key_pressed("r") {
            // restart the game [restart/request]
            cx.restart();
            return;
        }
        // "At the start, play the music "A3/2 A3/2 C4/2 A3/2 E4/2 A3/2 D4/2 C4/2 A3/2 A3/2 C4/2 A3/2 G4/2 E4/2 D4 -" at 150 beats per minute as a saw wave and show "COINS {score} / 20" at the top left in "#ffffff"." [show/every_frame]
        cx.text_at("text_0", &format!("COINS {} / 20", self.score), 0.02, 0.03, 24.0);
        if self.status != Status::Playing {
            return;
        }
        let player = cx.pos("player");
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
        // "Each slime moves toward the player sideways at 2.51 units per second within 9 units." [chase/hoisted]
        for n in cx.find_tag("slime") {
            let to = player - cx.pos(&n);
            if to.length() > 9.0 {
                cx.set_vel_x(&n, 0.0);
                continue;
            }
            if to.x.abs() > 0.1 {
                cx.set_vel_x(&n, to.x.signum() * 2.51);
                cx.set_flip_x(&n, to.x < 0.0);
            } else {
                cx.set_vel_x(&n, 0.0);
            }
        }
        // "Each bat moves toward the player at 3.65 units per second within 9 units." [chase/hoisted]
        for n in cx.find_tag("bat") {
            let to = player - cx.pos(&n);
            if to.length() > 9.0 {
                cx.set_vel(&n, Vec3::ZERO);
                continue;
            }
            cx.set_vel(&n, vec2(to.x, to.y).normalize_or_zero() * 3.65);
        }
        // "When the player falls below -12, the player loses." [falls_below/check]
        if cx.pos("player").y < -12.0 {
            // the player loses [lose/helper]
            self.lose(cx, "GAME OVER - PRESS R");
        }
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
        // "When the player lands on a bat, destroy it, bounce the player up at 11 units per second, squash the player and play the bounce sound." [lands_on/above]
        if a == "player" && cx.has_tag(b, "bat") && cx.pos(a).y - cx.pos(b).y > 0.25 {
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
        // "When the player touches a slime, the player loses." [touch/has_tag]
        if a == "player" && cx.has_tag(b, "slime") {
            // the player loses [lose/helper]
            self.lose(cx, "GAME OVER - PRESS R");
        }
        // "When the player touches a bat, the player loses." [touch/has_tag]
        if a == "player" && cx.has_tag(b, "bat") {
            // the player loses [lose/helper]
            self.lose(cx, "GAME OVER - PRESS R");
        }
        // "When the player touches a spike, the player loses." [touch/has_tag]
        if a == "player" && cx.has_tag(b, "spike") {
            // the player loses [lose/helper]
            self.lose(cx, "GAME OVER - PRESS R");
        }
        // "When the player touches the flag, the player wins "LEVEL CLEAR"." [touch/has_tag]
        if a == "player" && cx.has_tag(b, "flag") {
            // the player wins "LEVEL CLEAR" [win/inline]
            if self.status == Status::Playing {
                self.status = Status::Won;
                cx.play_sound("win");
                cx.text_at("banner", "LEVEL CLEAR - PRESS R", 0.3, 0.4, 44.0);
            }
        }
    }

    fn status(&self) -> Status {
        self.status
    }
}

fn main() {
    droid::run::<NightForest>(concat!(env!("CARGO_MANIFEST_DIR"), "/game"));
}
