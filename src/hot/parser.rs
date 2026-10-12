//! A sentence -> [`Request`]. The grammar is small on purpose:
//!
//! - Clauses split on "and", ",", "then", and "with" / "where" / "while" (which start a feature list).
//! - A number, a colour, "no" or "lots of" belongs to the next thing named in the same clause.
//! - A verb after a thing ("zombies chase the player", "aliens that shoot back") is that thing's
//!   behaviour; a behaviour word before a thing ("flying ghosts") is too.
//! - "collect", "avoid" and "shoot" take the things after them; a clause without a verb repeats
//!   the previous clause's verb ("collect 10 coins and 3 gems").
//! - "faster", "more", "bigger" edit the thing named in the same clause ("make the enemies
//!   faster"); alone, "faster"/"slower" shift the pace and "bigger"/"smaller" resize the level.
//! - Later words override earlier ones (so `droid edit` can append a change).
//! - A value nothing uses is an error ("20 what?"). An unknown word is an error with a hint.

use super::lexicon::{self, Tok, Token, Verb};
use super::request::*;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct Diag {
    pub message: String,
    pub hint: Option<String>,
}

impl Diag {
    pub fn new(m: impl Into<String>) -> Diag {
        Diag { message: m.into(), hint: None }
    }
    pub fn hint(mut self, h: impl Into<String>) -> Diag {
        self.hint = Some(h.into());
        self
    }
}

impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}", self.message)?;
        if let Some(h) = &self.hint {
            write!(f, "\n  hint: {h}")?;
        }
        Ok(())
    }
}

/// What a clause is about so far, for verbs and edits that come after it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Subject {
    Player,
    Enemy(EnemyKind),
    Enemies,
    Pickups,
    Hazards,
    Level,
}

impl Subject {
    fn target(self) -> Target {
        match self {
            Subject::Player => Target::Player,
            Subject::Enemy(_) | Subject::Enemies => Target::Enemies,
            Subject::Pickups => Target::Pickups,
            Subject::Hazards => Target::Hazards,
            Subject::Level => Target::Level,
        }
    }
}

#[derive(Default)]
struct Clause {
    num: Option<(u32, String)>,
    amount: Option<(Amount, String)>,
    color: Option<(Color, String)>,
    rows: Option<(u32, String)>,
    no: Option<String>,
    change: Option<(Change, String)>,
    behavior: Option<(Behavior, String)>,
    subject: Option<Subject>,
    verb: Option<(Verb, String)>,
    /// The verb came from the clause before ("collect coins and gems").
    inherited: bool,
    survive: Option<String>,
}

pub fn parse(sentence: &str, lenient: bool) -> Result<Request, Vec<Diag>> {
    let (tokens, unknown) = lexicon::tokenize(sentence);
    let mut diags: Vec<Diag> = Vec::new();
    if !lenient {
        for w in &unknown {
            let d = Diag::new(format!("no rule knows the word \"{w}\""));
            diags.push(match lexicon::closest(w) {
                Some(c) => d.hint(format!("did you mean \"{c}\"? (--lenient skips unknown words; `droid vocabulary` lists them all)")),
                None => d.hint("rephrase with known words (`droid vocabulary` lists them), or pass --lenient to skip it"),
            });
        }
    }
    let mut req = Request { sentence: sentence.trim().to_string(), ..Default::default() };
    let mut prev_verb: Option<(Verb, String)> = None;
    for clause in split(&tokens) {
        if let Err(d) = parse_clause(&clause, &mut req, &mut prev_verb) {
            diags.push(d);
        }
    }
    if diags.is_empty() && is_empty(&req) {
        diags.push(Diag::new("the sentence doesn't describe a game").hint("e.g. \"make a platformer with coins and slimes\""));
    }
    if diags.is_empty() {
        Ok(req)
    } else {
        Err(diags)
    }
}

fn is_empty(r: &Request) -> bool {
    r.genre.is_none()
        && r.view.is_none()
        && r.player.is_none()
        && r.enemies.is_empty()
        && r.generic_enemies.is_none()
        && r.pickups.is_empty()
        && r.generic_pickups.is_none()
        && r.hazards.is_empty()
        && r.exit.is_none()
        && r.goals.is_empty()
        && r.abilities.is_empty()
        && r.theme.is_none()
        && r.layout.is_none()
        && r.mods.is_empty()
        && r.edits.is_empty()
}

fn split(tokens: &[Token]) -> Vec<Vec<Token>> {
    let mut out = vec![Vec::new()];
    for t in tokens {
        match t.tok {
            Tok::And | Tok::Comma | Tok::Then => out.push(Vec::new()),
            // "with" starts a new clause but is kept: it marks a feature list
            Tok::With => out.push(vec![t.clone()]),
            _ => out.last_mut().unwrap().push(t.clone()),
        }
    }
    out.into_iter().filter(|c| c.iter().any(|t| t.tok != Tok::Filler && t.tok != Tok::With)).collect()
}

fn words(c: &[Token]) -> String {
    c.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ")
}

fn parse_clause(toks: &[Token], req: &mut Request, prev_verb: &mut Option<(Verb, String)>) -> Result<(), Diag> {
    let all = words(toks);
    let mut c = Clause::default();
    let has_verb = toks.iter().any(|t| matches!(t.tok, Tok::Verb(_)));
    let feature_list = toks.first().is_some_and(|t| t.tok == Tok::With);
    if !has_verb && !feature_list {
        if let Some((v, w)) = prev_verb.clone() {
            if matches!(v, Verb::Collect | Verb::Avoid | Verb::Shoot) {
                c.verb = Some((v, w));
                c.inherited = true;
            }
        }
    }
    for t in toks {
        let w = t.text.clone();
        match t.tok {
            Tok::Make | Tok::Add | Tok::Filler | Tok::With => {}
            Tok::Genre(g) => {
                // "top-down arena": both words name the same genre; keep them all as provenance
                let words = match &req.genre {
                    Some(old) if old.v == g && !old.words.contains(&w) => format!("{} {w}", old.words),
                    _ => w,
                };
                req.genre = Some(Val::new(g, words));
            }
            Tok::View(v) => req.view = Some(Val::new(v, w)),
            Tok::Mod(m) => req.mods.push(Val::new(m, w)),
            Tok::Theme(th) => req.theme = Some(Val::new(th, w)),
            Tok::Time(ti) => req.time = Some(Val::new(ti, w)),
            Tok::Style(st) => req.style = Some(Val::new(st, w)),
            Tok::Layout(l) => req.layout = Some(Val::new(l, w)),
            Tok::Color(col) => {
                if c.color.is_some() {
                    return Err(Diag::new(format!("\"{all}\" has two colours for one thing")));
                }
                c.color = Some((col, w));
            }
            Tok::Num(n) => {
                if let Some((m, mw)) = &c.num {
                    return Err(Diag::new(format!("\"{all}\": two numbers ({m} from \"{mw}\" and {n}) for one thing")));
                }
                c.num = Some((n, w));
            }
            Tok::Many => c.amount = Some((Amount::Many, w)),
            Tok::Few => c.amount = Some((Amount::Few, w)),
            Tok::No => c.no = Some(w),
            Tok::Rows => {
                let (n, nw) = c.num.take().ok_or_else(|| Diag::new(format!("\"{all}\": how many rows?")).hint("e.g. \"3 rows of aliens\""))?;
                c.rows = Some((n, format!("{nw} {w}")));
            }
            Tok::Lives => {
                let (n, nw) = c.num.take().ok_or_else(|| Diag::new(format!("\"{all}\": how many lives?")).hint("e.g. \"with 3 lives\""))?;
                if n == 0 {
                    return Err(Diag::new(format!("\"{all}\": the player needs at least 1 life")));
                }
                req.lives = Some(Val::new(n, format!("{nw} {w}")));
            }
            Tok::Seconds | Tok::Minutes => {
                let (n, nw) = c.num.take().ok_or_else(|| Diag::new(format!("\"{all}\": how many {w}?")))?;
                let secs = if t.tok == Tok::Minutes { n * 60 } else { n };
                let sw = c.survive.clone().or_else(|| matches!(c.verb, Some((Verb::Survive, _))).then(|| c.verb.clone().unwrap().1));
                match sw {
                    Some(sw) => req.goals.push(Val::new(GoalReq::Survive(Some(secs)), format!("{sw} {nw} {w}"))),
                    None => return Err(Diag::new(format!("\"{all}\": {nw} {w} of what?")).hint("e.g. \"survive 60 seconds\"")),
                }
                c.survive = None;
            }
            Tok::Change(ch) => match c.subject {
                // "make the enemies faster": the thing came first
                Some(s) => req.edits.push(Val::new((s.target(), ch), format!("{} {w}", subject_words(toks, t)))),
                None => c.change = Some((ch, w)),
            },
            Tok::PlayerWord | Tok::Player(_) => {
                if let Tok::Player(k) = t.tok {
                    req.player = Some(Val::new(k, w.clone()));
                }
                if let Some((col, cw)) = c.color.take() {
                    req.player_color = Some(Val::new(col, format!("{cw} {w}")));
                }
                if let Some((ch, cw)) = c.change.take() {
                    req.edits.push(Val::new((Target::Player, ch), format!("{cw} {w}")));
                }
                // "the player shoots": the player is the subject; "zombies chase the player": an object
                if c.subject.is_none() {
                    c.subject = Some(Subject::Player);
                }
            }
            Tok::LevelWord => {
                if let Some((ch, cw)) = c.change.take() {
                    req.edits.push(Val::new((Target::Level, ch), format!("{cw} {w}")));
                }
                c.subject.get_or_insert(Subject::Level);
            }
            Tok::Enemy(kind) => {
                enemy(req, &mut c, Some(kind), &w, &all)?;
            }
            Tok::EnemiesWord => {
                enemy(req, &mut c, None, &w, &all)?;
            }
            Tok::Pickup(kind) => pickup(req, &mut c, Some(kind), &w, &all)?,
            Tok::PickupsWord => pickup(req, &mut c, None, &w, &all)?,
            Tok::Hazard(kind) => {
                if let Some(nw) = c.no.take() {
                    req.hazards.retain(|h| h.kind.v != kind);
                    let _ = nw;
                } else {
                    let count = take_count(&mut c).map(|(a, aw)| Val::new(a, format!("{aw} {w}")));
                    match req.hazards.iter_mut().find(|h| h.kind.v == kind) {
                        Some(h) => {
                            if count.is_some() {
                                h.count = count;
                            }
                        }
                        None => req.hazards.push(HazardReq { kind: Val::new(kind, w.clone()), count }),
                    }
                }
                if let Some((ch, cw)) = c.change.take() {
                    req.edits.push(Val::new((Target::Hazards, ch), format!("{cw} {w}")));
                }
                c.subject.get_or_insert(Subject::Hazards);
            }
            Tok::Exit(kind) => {
                req.exit = Some(Val::new(kind, w.clone()));
                if let Some((Verb::Reach, vw)) = &c.verb {
                    req.goals.push(Val::new(GoalReq::Reach, format!("{vw} {w}")));
                }
            }
            Tok::Verb(v) => verb(req, &mut c, v, &w, &all)?,
            Tok::And | Tok::Comma | Tok::Then => {}
        }
    }
    // leftovers
    if let Some((n, nw)) = c.num {
        return Err(Diag::new(format!("\"{all}\": {n} what?")).hint(format!("put a thing after the number, e.g. \"{nw} coins\"")));
    }
    if let Some((_, aw)) = c.amount {
        return Err(Diag::new(format!("\"{all}\": {aw} what?")));
    }
    if let Some((_, cw)) = c.color {
        return Err(Diag::new(format!("\"{all}\": {cw} what?")).hint(format!("put a thing after the colour, e.g. \"{cw} slimes\"")));
    }
    if let Some((_, rw)) = c.rows {
        return Err(Diag::new(format!("\"{all}\": {rw} of what?")).hint("e.g. \"3 rows of aliens\""));
    }
    if let Some(nw) = c.no {
        return Err(Diag::new(format!("\"{all}\": {nw} what?")));
    }
    if let Some((b, bw)) = c.behavior {
        return Err(Diag::new(format!("\"{all}\": {bw} what?")).hint(format!("name who does it, e.g. \"{} bats\"", b.name())));
    }
    if let Some((ch, cw)) = c.change {
        match ch {
            Change::Faster => req.mods.push(Val::new(Mod::Fast, cw)),
            Change::Slower => req.mods.push(Val::new(Mod::Slow, cw)),
            Change::Bigger | Change::Smaller => req.edits.push(Val::new((Target::Level, ch), cw)),
            Change::More | Change::Fewer => return Err(Diag::new(format!("\"{all}\": {cw} what?")).hint("e.g. \"more coins\", \"fewer enemies\"")),
        }
    }
    if let Some(sw) = c.survive {
        req.goals.push(Val::new(GoalReq::Survive(None), sw));
    }
    if let Some((v, vw)) = &c.verb {
        if !c.inherited {
            *prev_verb = Some((*v, vw.clone()));
        }
    } else {
        *prev_verb = None;
    }
    Ok(())
}

/// The words of the subject before `t`, for provenance of "the enemies ... faster".
fn subject_words(toks: &[Token], t: &Token) -> String {
    let end = toks.iter().position(|x| std::ptr::eq(x, t)).unwrap_or(toks.len());
    toks[..end].iter().filter(|x| x.tok != Tok::Filler && x.tok != Tok::Make).map(|x| x.text.as_str()).collect::<Vec<_>>().join(" ")
}

fn take_count(c: &mut Clause) -> Option<(Amount, String)> {
    if let Some((n, w)) = c.num.take() {
        return Some((Amount::N(n), w));
    }
    c.amount.take()
}

fn enemy(req: &mut Request, c: &mut Clause, kind: Option<EnemyKind>, w: &str, all: &str) -> Result<(), Diag> {
    if let Some((Verb::Collect, vw)) = &c.verb {
        if !c.inherited {
            return Err(Diag::new(format!("\"{all}\": you can't {vw} enemies")).hint("collect pickups (coins, gems, stars ...); avoid or shoot enemies"));
        }
    }
    let subject = kind.map(Subject::Enemy).unwrap_or(Subject::Enemies);
    if let Some(nw) = c.no.take() {
        match kind {
            Some(k) => req.enemies.retain(|e| e.kind.v != k),
            None => {
                req.enemies.clear();
                req.generic_enemies = None;
                req.no_enemies = Some(Val::new((), format!("{nw} {w}")));
            }
        }
        return Ok(());
    }
    let count = take_count(c).map(|(a, aw)| Val::new(a, format!("{aw} {w}")));
    match kind {
        Some(k) => {
            req.no_enemies = None;
            let idx = match req.enemies.iter().position(|e| e.kind.v == k) {
                Some(i) => i,
                None => {
                    req.enemies.push(EnemyReq { kind: Val::new(k, w), count: None, rows: None, color: None, behaviors: vec![], shoots: None });
                    req.enemies.len() - 1
                }
            };
            let e = &mut req.enemies[idx];
            if count.is_some() {
                e.count = count;
            }
            if let Some((r, rw)) = c.rows.take() {
                e.rows = Some(Val::new(r, format!("{rw} {w}")));
            }
            if let Some((col, cw)) = c.color.take() {
                e.color = Some(Val::new(col, format!("{cw} {w}")));
            }
            if let Some((b, bw)) = c.behavior.take() {
                e.behaviors.push(Val::new(b, format!("{bw} {w}")));
            }
        }
        None => {
            // a count or a behaviour on "enemies" without a kind
            let prev = req.generic_enemies.as_ref().and_then(|g| g.v);
            req.generic_enemies = Some(Val::new(count.as_ref().map(|c| c.v).or(prev), count.map(|c| c.words).unwrap_or_else(|| w.to_string())));
            req.no_enemies = None;
            if let Some((b, bw)) = c.behavior.take() {
                req.generic_behaviors.push(Val::new(b, format!("{bw} {w}")));
            }
            if let Some((_, cw)) = c.color.take() {
                return Err(Diag::new(format!("\"{all}\": colour which enemies?")).hint(format!("name them, e.g. \"{cw} slimes\"")));
            }
        }
    }
    if let Some((ch, cw)) = c.change.take() {
        req.edits.push(Val::new((Target::Enemies, ch), format!("{cw} {w}")));
    }
    // "shoot the aliens": the player's weapon
    if let Some((Verb::Shoot, vw)) = &c.verb {
        if c.subject.is_none() || c.subject == Some(Subject::Player) {
            req.goals.push(Val::new(GoalReq::DestroyAll, format!("{vw} {w}")));
        }
    }
    if c.subject.is_none() {
        c.subject = Some(subject);
    }
    Ok(())
}

fn pickup(req: &mut Request, c: &mut Clause, kind: Option<PickupKind>, w: &str, all: &str) -> Result<(), Diag> {
    if let Some((Verb::Avoid | Verb::Shoot, vw)) = &c.verb {
        if !c.inherited {
            return Err(Diag::new(format!("\"{all}\": {vw} pickups?")).hint("pickups are collected; avoid or shoot enemies and hazards"));
        }
    }
    if let Some(nw) = c.no.take() {
        match kind {
            Some(k) => req.pickups.retain(|p| p.kind.v != k),
            None => {
                req.pickups.clear();
                req.generic_pickups = None;
                req.no_pickups = Some(Val::new((), format!("{nw} {w}")));
            }
        }
        return Ok(());
    }
    let count = take_count(c).map(|(a, aw)| Val::new(a, format!("{aw} {w}")));
    match kind {
        Some(k) => {
            req.no_pickups = None;
            let idx = match req.pickups.iter().position(|p| p.kind.v == k) {
                Some(i) => i,
                None => {
                    req.pickups.push(PickupReq { kind: Val::new(k, w), count: None, color: None });
                    req.pickups.len() - 1
                }
            };
            let p = &mut req.pickups[idx];
            if count.is_some() {
                p.count = count;
            }
            if let Some((col, cw)) = c.color.take() {
                p.color = Some(Val::new(col, format!("{cw} {w}")));
            }
        }
        None => {
            let prev = req.generic_pickups.as_ref().and_then(|g| g.v);
            req.generic_pickups = Some(Val::new(count.as_ref().map(|c| c.v).or(prev), count.map(|c| c.words).unwrap_or_else(|| w.to_string())));
            req.no_pickups = None;
        }
    }
    if let Some((ch, cw)) = c.change.take() {
        req.edits.push(Val::new((Target::Pickups, ch), format!("{cw} {w}")));
    }
    if let Some((Verb::Collect, vw)) = &c.verb {
        req.goals.push(Val::new(GoalReq::CollectAll, format!("{vw} {w}")));
    }
    c.subject.get_or_insert(Subject::Pickups);
    Ok(())
}

fn verb(req: &mut Request, c: &mut Clause, v: Verb, w: &str, all: &str) -> Result<(), Diag> {
    let behavior = match v {
        Verb::Chase => Some(Behavior::Chase),
        Verb::Patrol => Some(Behavior::Patrol),
        Verb::Wander => Some(Behavior::Wander),
        Verb::Fly => Some(Behavior::Fly),
        Verb::March => Some(Behavior::March),
        _ => None,
    };
    if let Some(b) = behavior {
        match c.subject {
            Some(Subject::Enemy(k)) => {
                let e = req.enemies.iter_mut().find(|e| e.kind.v == k).expect("subject was added");
                e.behaviors.push(Val::new(b, format!("{} {w}", e.kind.words)));
            }
            Some(Subject::Enemies) => req.generic_behaviors.push(Val::new(b, w.to_string())),
            Some(Subject::Player) => {
                return Err(Diag::new(format!("\"{all}\": the player doesn't {w}; enemies do")).hint("e.g. \"zombies chase the player\""));
            }
            // "flying ghosts": the next enemy gets it
            _ => c.behavior = Some((b, w.to_string())),
        }
        c.verb = Some((v, w.to_string()));
        c.inherited = false;
        return Ok(());
    }
    match v {
        Verb::Shoot => match c.subject {
            Some(Subject::Enemy(k)) => {
                let e = req.enemies.iter_mut().find(|e| e.kind.v == k).expect("subject was added");
                e.shoots = Some(Val::new((), format!("{} {w}", e.kind.words)));
            }
            Some(Subject::Enemies) => req.generic_shoots = Some(Val::new((), w.to_string())),
            _ => req.abilities.push(Val::new(Ability::Shoot, w.to_string())),
        },
        Verb::Jump => req.abilities.push(Val::new(Ability::Jump, w.to_string())),
        Verb::DoubleJump => req.abilities.push(Val::new(Ability::DoubleJump, w.to_string())),
        Verb::Survive => c.survive = Some(w.to_string()),
        Verb::Collect | Verb::Avoid | Verb::Reach => {}
        _ => unreachable!(),
    }
    c.verb = Some((v, w.to_string()));
    c.inherited = false;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Request {
        parse(s, false).unwrap_or_else(|d| panic!("{s:?}: {d:?}"))
    }

    fn err(s: &str) -> Diag {
        parse(s, false).expect_err(s).remove(0)
    }

    #[test]
    fn a_plain_platformer() {
        let r = p("make a platformer with coins and slimes");
        assert_eq!(r.genre.as_ref().unwrap().v, Genre::Platformer);
        assert_eq!(r.pickups[0].kind.v, PickupKind::Coin);
        assert_eq!(r.enemies[0].kind.v, EnemyKind::Slime);
    }

    #[test]
    fn counts_colours_and_provenance() {
        let r = p("make a hard platformer with 20 red coins, bats and spikes, a door at the end, in a night forest");
        assert_eq!(r.pickups[0].count.as_ref().unwrap().v, Amount::N(20));
        assert_eq!(r.pickups[0].color.as_ref().unwrap().v, Color::Red);
        assert_eq!(r.pickups[0].count.as_ref().unwrap().words, "20 coins");
        assert_eq!(r.exit.as_ref().unwrap().v, ExitKind::Door);
        assert_eq!(r.theme.as_ref().unwrap().v, Theme::Forest);
        assert_eq!(r.time.as_ref().unwrap().v, TimeOfDay::Night);
        assert_eq!(r.mods[0].v, Mod::Hard);
        assert_eq!(r.hazards[0].kind.v, HazardKind::Spike);
    }

    #[test]
    fn behaviours_attach_to_the_thing_before_the_verb() {
        let r = p("make a top-down arena where zombies chase the player, survive 60 seconds");
        assert_eq!(r.enemies[0].behaviors[0].v, Behavior::Chase);
        assert!(r.goals.iter().any(|g| g.v == GoalReq::Survive(Some(60))));
        let r = p("make a space shooter with 3 rows of aliens that shoot back");
        assert_eq!(r.enemies[0].rows.as_ref().unwrap().v, 3);
        assert!(r.enemies[0].shoots.is_some());
        let r = p("make an arena with flying ghosts");
        assert_eq!(r.enemies[0].behaviors[0].v, Behavior::Fly);
    }

    #[test]
    fn verbs_carry_over_to_the_next_clause() {
        let r = p("make a 3d garden where a gnome collects 8 gems and avoids blobs");
        assert_eq!(r.view.as_ref().unwrap().v, ViewReq::Depth);
        assert_eq!(r.player.as_ref().unwrap().v, PlayerKind::Gnome);
        assert!(r.goals.iter().any(|g| g.v == GoalReq::CollectAll));
        assert_eq!(r.enemies[0].kind.v, EnemyKind::Blob);
        let r = p("collect 10 coins and 3 gems");
        assert_eq!(r.pickups.len(), 2);
    }

    #[test]
    fn edits() {
        let r = p("make a platformer with slimes, make the enemies faster, more coins, and make it harder");
        assert!(r.edits.iter().any(|e| e.v == (Target::Enemies, Change::Faster)));
        assert!(r.edits.iter().any(|e| e.v == (Target::Pickups, Change::More)));
        assert!(r.mods.iter().any(|m| m.v == Mod::Hard));
        let r = p("make a platformer, bigger level");
        assert!(r.edits.iter().any(|e| e.v == (Target::Level, Change::Bigger)));
    }

    #[test]
    fn later_words_override() {
        let r = p("make a platformer with bats, no bats");
        assert!(r.enemies.is_empty());
        let r = p("make a platformer, make it a shooter");
        assert_eq!(r.genre.as_ref().unwrap().v, Genre::Shooter);
    }

    #[test]
    fn unused_values_are_errors() {
        assert!(err("make a platformer with 20").message.contains("20 what?"));
        assert!(err("make a platformer with red").message.contains("red what?"));
        assert!(err("make a shooter with 3 rows").message.contains("of what?"));
        assert!(err("make a platformer with more").message.contains("more what?"));
    }

    #[test]
    fn nonsense_is_an_error() {
        assert!(err("make a platformer where you collect slimes").message.contains("can't"));
        assert!(err("make a platfromer").hint.unwrap().contains("platformer"));
        assert!(err("hello there").message.contains("no rule knows"));
        assert!(err("the the").message.contains("describe"));
    }

    #[test]
    fn lenient_skips_unknown_words() {
        assert!(parse("make a groovy platformer", true).is_ok());
        assert!(parse("make a groovy platformer", false).is_err());
    }
}
