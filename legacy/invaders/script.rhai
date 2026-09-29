fn init() {
  set("score", 0);
  set("lives", 3);
  set("level", 1);
  set("over", 0);
  set("cool", 0.0);
  set("frame", -1.0);
  let i = 0;
  while i < 40 {
    spawn_star();
    i += 1;
  }
  build_wave();
  text("score", "Score: 0");
  text("lives", "Lives: 3");
  text("level", "Wave 1");
}

fn spawn_star() {
  let b = rand(0.45, 1.0);
  let x = rand(-17.5, 17.5);
  let y = rand(-9.0, 10.0);
  let p = vec3(x, y, -2.0);
  let s = vec3(0.09, 0.09, 1.0);
  let c = [b * 0.8, b * 0.85, b];
  spawn_tagged("star", ["star"], "box", p, s, c, "none");
}

fn build_wave() {
  destroy_tag("alien");
  destroy_tag("shield");
  destroy_tag("pb");
  destroy_tag("eb");
  set("dir", 1.0);
  set("etimer", 1.6);
  set("frame", -1.0);
  let i = 0;
  while i < 24 {
    spawn_alien(i);
    i += 1;
  }
  let j = 0;
  while j < 40 {
    spawn_shield(j);
    j += 1;
  }
}

fn spawn_alien(i) {
  let r = i / 8;
  let c = i % 8;
  let x = to_float(c) * 2.6;
  x = x - 9.1;
  let y = to_float(r) * 1.8;
  y = 7.2 - y;
  let sp = "alienA1";
  if r == 1 { sp = "alienB1"; }
  if r == 2 { sp = "alienC1"; }
  let p = vec3(x, y, 0.0);
  let s = vec3(1.5, 1.5, 1.0);
  let n = spawn_sprite("alien", ["alien"], sp, p, s, "none");
  add_tag(n, "row" + r);
}

fn spawn_shield(j) {
  let k = j % 10;
  if k == 7 { return; }
  let blk = j / 10;
  let xs = [-10.8, -3.6, 3.6, 10.8];
  let bx = xs[blk];
  let row = k / 5;
  let col = k % 5;
  let x = to_float(col) - 2.0;
  x = x * 0.64;
  x = x + bx;
  let y = to_float(row) * 0.64;
  y = -5.0 - y;
  let p = vec3(x, y, 0.0);
  let s = vec3(0.6, 0.6, 1.0);
  let c = [0.25, 0.9, 0.45];
  spawn_tagged("shield", ["shield"], "box", p, s, c, "none");
}

fn boom_at(p) {
  let bp = vec3(p.x, p.y, 0.0);
  let bs = vec3(1.8, 1.8, 1.0);
  spawn_sprite("boom", ["boom"], "boom", bp, bs, "none");
}

fn fade_booms(dt) {
  let d = 5.0 * dt;
  for n in find_tag("boom") {
    fade_one(n, d);
  }
}

fn fade_one(n, d) {
  let s = scale_of(n).x - d;
  if s <= 0.2 {
    destroy(n);
    return;
  }
  let v = vec3(s, s, 1.0);
  set_scale(n, v);
}

fn game_over(m) {
  set("over", 1);
  text_at("msg", m, 0.30, 0.42, 40);
  text_color("msg", [1.0, 0.8, 0.3]);
  text_at("msg2", "press R to play again", 0.35, 0.52, 24);
  text_color("msg2", [0.8, 0.85, 1.0]);
}

fn hit_player() {
  let p = pos("player");
  boom_at(p);
  destroy_tag("eb");
  let l = add("lives", -1);
  if l < 0 { l = 0; }
  text("lives", "Lives: " + l);
  if l <= 0 { game_over("GAME OVER"); }
}

fn next_wave() {
  let lv = add("level", 1);
  let sc = add("score", 100);
  text("score", "Score: " + sc);
  text("level", "Wave " + lv);
  build_wave();
}

fn fire(px) {
  let bp = vec3(px, -7.1, 0.0);
  let bs = vec3(0.18, 0.8, 1.0);
  let bc = [1.0, 1.0, 0.45];
  spawn_tagged("pb", ["pb"], "box", bp, bs, bc, "none");
}

fn move_player(dt) {
  let ax = axis_x();
  let px = pos("player").x;
  px = px + ax * 15.0 * dt;
  px = clamp(px, -15.5, 15.5);
  let p = vec3(px, -8.0, 0.0);
  set_pos("player", p);
  let cool = get("cool") - dt;
  let shoot = key("space");
  if mouse_down() { shoot = true; }
  if shoot && cool <= 0.0 {
    fire(px);
    cool = 0.38;
  }
  set("cool", cool);
}

fn move_bullets(dt) {
  let up = vec3(0.0, 28.0 * dt, 0.0);
  for n in find_tag("pb") {
    move_by(n, up);
    cull_high(n);
  }
  let lvf = to_float(get("level"));
  let esp = 9.0 + lvf * 0.7;
  let dy = -esp * dt;
  let dn = vec3(0.0, dy, 0.0);
  for n in find_tag("eb") {
    move_by(n, dn);
    cull_low(n);
  }
}

fn cull_high(n) {
  let y = pos(n).y;
  if y > 10.0 { destroy(n); }
}

fn cull_low(n) {
  let y = pos(n).y;
  if y < -9.6 { destroy(n); }
}

fn bullet_aliens() {
  for b in find_tag("pb") {
    hit_check(b);
  }
}

fn hit_check(b) {
  if !exists(b) { return; }
  let bp = pos(b);
  for a in find_tag("alien") {
    try_hit(b, a, bp);
    if !exists(b) { return; }
  }
}

fn try_hit(b, a, bp) {
  let ap = pos(a);
  let dx = abs(ap.x - bp.x);
  if dx > 0.85 { return; }
  let dy = abs(ap.y - bp.y);
  if dy > 0.85 { return; }
  let pts = 10;
  if has_tag(a, "row0") { pts = 30; }
  if has_tag(a, "row1") { pts = 20; }
  let sc = add("score", pts);
  text("score", "Score: " + sc);
  boom_at(ap);
  destroy(a);
  destroy(b);
}

fn bullets_shields(tag) {
  for b in find_tag(tag) {
    shield_check(b);
  }
}

fn shield_check(b) {
  if !exists(b) { return; }
  let bp = pos(b);
  for s in find_tag("shield") {
    try_shield(b, s, bp);
    if !exists(b) { return; }
  }
}

fn try_shield(b, s, bp) {
  let sp = pos(s);
  let dx = abs(sp.x - bp.x);
  if dx > 0.4 { return; }
  let dy = abs(sp.y - bp.y);
  if dy > 0.5 { return; }
  destroy(s);
  destroy(b);
}

fn bullets_player() {
  let px = pos("player").x;
  for b in find_tag("eb") {
    check_pb(b, px);
  }
}

fn check_pb(b, px) {
  if !exists(b) { return; }
  let bp = pos(b);
  let dx = abs(bp.x - px);
  if dx > 1.1 { return; }
  let dy = abs(bp.y + 8.0);
  if dy > 0.8 { return; }
  destroy(b);
  hit_player();
}

fn crush_shields() {
  if count_tag("alien") < 1 { return; }
  for s in find_tag("shield") {
    crush_one(s);
  }
}

fn crush_one(s) {
  let a = nearest_tag(s, "alien");
  let d = distance(s, a);
  if d < 1.1 { destroy(s); }
}

fn march(dt) {
  let aliens = find_tag("alien");
  let alive = aliens.len();
  if alive == 0 {
    next_wave();
    return;
  }
  let lvf = to_float(get("level"));
  let spd = 1.6 + lvf * 0.75;
  let gone = to_float(24 - alive);
  spd = spd + gone * 0.28;
  let dir = get("dir");
  let step = dir * spd * dt;
  let mv = vec3(step, 0.0, 0.0);
  for a in aliens {
    move_by(a, mv);
  }
  let minx = 99.0;
  let maxx = -99.0;
  let miny = 99.0;
  for a in aliens {
    let p = pos(a);
    if p.x < minx { minx = p.x; }
    if p.x > maxx { maxx = p.x; }
    if p.y < miny { miny = p.y; }
  }
  let turn = false;
  if dir > 0.0 && maxx > 15.0 { turn = true; }
  if dir < 0.0 && minx < -15.0 { turn = true; }
  if turn {
    set("dir", -dir);
    drop_down(aliens);
    miny = miny - 0.62;
  }
  if miny < -4.0 { crush_shields(); }
  if miny < -7.0 { land_lose(); }
}

fn drop_down(aliens) {
  let dwn = vec3(0.0, -0.62, 0.0);
  for a in aliens {
    move_by(a, dwn);
  }
}

fn land_lose() {
  let p = pos("player");
  boom_at(p);
  game_over("THE ALIENS LANDED");
}

fn enemy_fire(dt) {
  let et = get("etimer") - dt;
  if et > 0.0 {
    set("etimer", et);
    return;
  }
  let lvf = to_float(get("level"));
  let base = 1.5 - lvf * 0.12;
  base = max(0.35, base);
  let nt = base + rand(0.0, 0.5);
  set("etimer", nt);
  let aliens = find_tag("alien");
  let n = aliens.len();
  if n < 1 { return; }
  let hi = n - 1;
  let idx = rand_int(0, hi);
  let a = aliens[idx];
  let ap = pos(a);
  let bp = vec3(ap.x, ap.y - 0.9, 0.0);
  let bs = vec3(0.22, 0.75, 1.0);
  let bc = [1.0, 0.4, 0.35];
  spawn_tagged("eb", ["eb"], "box", bp, bs, bc, "none");
}

fn set_frame(a, suf) {
  if has_tag(a, "row0") {
    set_sprite(a, "alienA" + suf);
    return;
  }
  if has_tag(a, "row1") {
    set_sprite(a, "alienB" + suf);
    return;
  }
  set_sprite(a, "alienC" + suf);
}

fn animate() {
  let f = floor(time() * 2.5) % 2.0;
  if f == get("frame") { return; }
  set("frame", f);
  let suf = "1";
  if f > 0.5 { suf = "2"; }
  for a in find_tag("alien") {
    set_frame(a, suf);
  }
}

fn update(dt) {
  if key_pressed("r") {
    restart();
    return;
  }
  fade_booms(dt);
  if get("over") == 1 { return; }
  move_player(dt);
  move_bullets(dt);
  bullet_aliens();
  bullets_shields("pb");
  bullets_shields("eb");
  bullets_player();
  if get("over") == 1 { return; }
  march(dt);
  if get("over") == 1 { return; }
  enemy_fire(dt);
  animate();
}
