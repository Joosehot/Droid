fn init() {
  set("score", 0);
  for i in 0..8 {
    let a = i * 0.785;
    let n = spawn_model("gem", ["gem"], "gem", vec3(cos(a) * 8, 0.6, sin(a) * 8), vec3(1, 1, 1), "static");
    set_solid(n, false);
  }
  play_music(120, "C4 E4 G4 C5 G4 E4 C4 - A3 C4 E4 A4 E4 C4 A3 -", "triangle");
}

fn update(dt) {
  if key_pressed("r") { restart(); return; }
  let v = vel("player");
  set_vel("player", vec3(axis_x() * 7, v.y, -axis_y() * 7));
  if key_pressed("space") && grounded("player") { set_vel_y("player", 11); play_sound("jump"); }
  if pos("player").y < -10 { restart(); }
  for n in find_tag("gem") { rotate(n, vec3(0, 90 * dt, 0)); }
}

fn on_collision(a, b) {
  if a == "player" && has_tag(b, "gem") {
    destroy(b);
    play_sound("coin");
    let s = add("score", 1);
    text("score", "Gems: " + s + " / 8");
    if s == 8 { play_sound("win"); text_at("win", "ALL GEMS! R to restart", 0.3, 0.45, 40); }
  }
}
