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
