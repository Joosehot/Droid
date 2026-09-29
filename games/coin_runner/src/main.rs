//! Coin Runner. The reference for the code Senne writes: one block per sentence, each
//! commented with the sentence that asked for it.

use droid::prelude::*;

#[derive(Default)]
struct CoinRunner {
    score: u32,
    won: bool,
}

impl Game for CoinRunner {
    fn init(&mut self, cx: &mut Ctx) {
        // "there are 8 coins in a row, 3 units apart, starting at -8 1.5"
        for i in 0..8 {
            let n = cx.spawn_sprite("coin", &["coin"], "coin", vec3(-8.0 + i as f32 * 3.0, 1.5, 0.0), vec3(0.7, 0.7, 0.7), BodyKind::Static);
            cx.set_solid(&n, false);
        }
        for n in cx.find_tag("coin") {
            cx.set_solid(&n, false);
        }
        // "play the music "C4 E4 G4 C5 - G4 E4 C4" at 140 beats per minute as a square wave"
        cx.play_music(140.0, "C4 E4 G4 C5 - G4 E4 C4", "square");
    }

    fn update(&mut self, cx: &mut Ctx, _dt: f32) {
        // "when the player presses r, restart the game"
        if cx.key_pressed("r") {
            cx.restart();
            return;
        }
        if self.won {
            return;
        }
        // "move the player sideways with the arrow keys at 8 units per second and face where it moves"
        let ax = cx.axis_x();
        cx.set_vel_x("player", ax * 8.0);
        if ax < 0.0 {
            cx.set_flip_x("player", true);
        }
        if ax > 0.0 {
            cx.set_flip_x("player", false);
        }
        // "animate the player with "hero_walk1" and "hero_walk2" at 8 frames per second while it walks"
        let on_ground = cx.grounded("player");
        if ax != 0.0 && on_ground {
            let frame = if (cx.time() * 8.0) as i32 % 2 == 0 { "hero_walk1" } else { "hero_walk2" };
            cx.set_sprite("player", frame);
        } else {
            cx.set_sprite("player", "hero_walk1");
        }
        // "the player jumps with space at 14 units per second with the jump sound"
        if cx.key_pressed("space") && on_ground {
            cx.set_vel_y("player", 14.0);
            cx.play_sound("jump");
        }
        // "when the player falls below -10, restart the game"
        if cx.pos("player").y < -10.0 {
            cx.restart();
        }
        // "each coin spins"
        let t = cx.time();
        for n in cx.find_tag("coin") {
            cx.set_scale(&n, vec3(0.7 * (t * 3.0).cos().abs() + 0.1, 0.7, 0.7));
        }
    }

    fn on_collision(&mut self, cx: &mut Ctx, a: &str, b: &str) {
        // "when the player touches a coin, destroy it, add 1 to the score and play the coin sound"
        if a == "player" && cx.has_tag(b, "coin") {
            cx.destroy(b);
            self.score += 1;
            cx.text("score", &format!("Coins: {}", self.score));
            cx.play_sound("coin");
            cx.squash("player", 0.2);
            // "when no coins are left, the player wins"
            if cx.count_tag("coin") == 0 {
                self.won = true;
                cx.play_sound("win");
                cx.text_at("win", "YOU WIN! R to restart", 0.35, 0.45, 40.0);
            }
        }
    }
}

fn main() {
    droid::run::<CoinRunner>(concat!(env!("CARGO_MANIFEST_DIR"), "/game"));
}
