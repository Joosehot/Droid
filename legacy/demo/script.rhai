fn init() {
  set("score", 0);
  for i in 0..8 {
    spawn_sprite("coin", ["coin"], "coin", vec3(-8 + i * 3, 1.5, 0), vec3(0.7, 0.7, 0.7), "static");
    set_solid("coin_" + i, false);
  }
  for n in find_tag("coin") { set_solid(n, false); }
}

fn update(dt) {
  if key_pressed("r") { restart(); return; }
  let ax = axis_x();
  set_vel_x("player", ax * 8);
  if ax < 0 { set_flip_x("player", true); }
  if ax > 0 { set_flip_x("player", false); }
  if ax != 0 && grounded("player") {
    if floor(time() * 8) % 2 == 0 { set_sprite("player", "hero_walk1"); } else { set_sprite("player", "hero_walk2"); }
  } else {
    set_sprite("player", "hero_walk1");
  }
  if key_pressed("space") && grounded("player") { set_vel_y("player", 14); }
  if pos("player").y < -10 { restart(); }
  for n in find_tag("coin") { set_scale(n, vec3(0.7 * abs(cos(time() * 3)) + 0.1, 0.7, 0.7)); }
}

fn on_collision(a, b) {
  if a == "player" && has_tag(b, "coin") {
    destroy(b);
    let s = add("score", 1);
    text("score", "Coins: " + s);
    if count_tag("coin") == 0 { text_at("win", "YOU WIN! R to restart", 0.35, 0.45, 40); }
  }
}
