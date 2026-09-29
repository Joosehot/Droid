//! The closed vocabulary for describing a whole game in one sentence. Synonyms map to one
//! token. A word the table doesn't know is an error with a closest-match hint (or skipped with
//! `--lenient`); the engine never guesses.

use super::request::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tok {
    Make,
    Add,
    Genre(Genre),
    View(ViewReq),
    Mod(Mod),
    Theme(Theme),
    Time(TimeOfDay),
    Color(Color),
    Player(PlayerKind),
    /// "the player", "you", "hero": the player without a kind.
    PlayerWord,
    Enemy(EnemyKind),
    /// "enemies", "monsters": enemies without a kind.
    EnemiesWord,
    Pickup(PickupKind),
    /// "items", "collectibles".
    PickupsWord,
    Hazard(HazardKind),
    Exit(ExitKind),
    LevelWord,
    Verb(Verb),
    Change(Change),
    Rows,
    Lives,
    Seconds,
    Minutes,
    Many,
    Few,
    No,
    Num(u32),
    And,
    Comma,
    Then,
    With,
    Filler,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Collect,
    Avoid,
    Chase,
    Patrol,
    Wander,
    Fly,
    March,
    Shoot,
    Jump,
    DoubleJump,
    Survive,
    Reach,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub tok: Tok,
    /// The words as written.
    pub text: String,
}

fn table() -> Vec<(&'static str, Tok)> {
    use Tok as T;
    let mut t: Vec<(&'static str, Tok)> = vec![
        // making
        ("make", T::Make),
        ("create", T::Make),
        ("build", T::Make),
        ("generate", T::Make),
        ("design", T::Make),
        ("give me", T::Make),
        ("i want", T::Make),
        ("add", T::Add),
        ("put", T::Add),
        // genres
        ("platformer", T::Genre(Genre::Platformer)),
        ("platform game", T::Genre(Genre::Platformer)),
        ("platforming game", T::Genre(Genre::Platformer)),
        ("jump and run", T::Genre(Genre::Platformer)),
        ("side scroller", T::Genre(Genre::Platformer)),
        ("sidescroller", T::Genre(Genre::Platformer)),
        ("arena", T::Genre(Genre::Arena)),
        ("top down game", T::Genre(Genre::Arena)),
        ("top down", T::Genre(Genre::Arena)),
        ("topdown", T::Genre(Genre::Arena)),
        ("survival game", T::Genre(Genre::Arena)),
        ("shooter", T::Genre(Genre::Shooter)),
        ("space shooter", T::Genre(Genre::Shooter)),
        ("shoot em up", T::Genre(Genre::Shooter)),
        ("shmup", T::Genre(Genre::Shooter)),
        ("space invaders", T::Genre(Genre::Shooter)),
        // view
        ("3d", T::View(ViewReq::Depth)),
        ("three dimensional", T::View(ViewReq::Depth)),
        ("2d", T::View(ViewReq::Flat)),
        // profile words
        ("hard", T::Mod(Mod::Hard)),
        ("difficult", T::Mod(Mod::Hard)),
        ("tough", T::Mod(Mod::Hard)),
        ("brutal", T::Mod(Mod::Hard)),
        ("challenging", T::Mod(Mod::Hard)),
        ("harder", T::Mod(Mod::Hard)),
        ("easy", T::Mod(Mod::Easy)),
        ("gentle", T::Mod(Mod::Easy)),
        ("casual", T::Mod(Mod::Easy)),
        ("cozy", T::Mod(Mod::Easy)),
        ("easier", T::Mod(Mod::Easy)),
        ("fast", T::Mod(Mod::Fast)),
        ("quick", T::Mod(Mod::Fast)),
        ("speedy", T::Mod(Mod::Fast)),
        ("frantic", T::Mod(Mod::Fast)),
        ("hectic", T::Mod(Mod::Fast)),
        ("slow", T::Mod(Mod::Slow)),
        ("relaxed", T::Mod(Mod::Slow)),
        ("calm", T::Mod(Mod::Slow)),
        ("chill", T::Mod(Mod::Slow)),
        ("peaceful", T::Mod(Mod::Slow)),
        ("simple", T::Mod(Mod::Simple)),
        ("minimal", T::Mod(Mod::Simple)),
        ("basic", T::Mod(Mod::Simple)),
        // themes
        ("forest", T::Theme(Theme::Forest)),
        ("woods", T::Theme(Theme::Forest)),
        ("jungle", T::Theme(Theme::Forest)),
        ("cave", T::Theme(Theme::Cave)),
        ("caves", T::Theme(Theme::Cave)),
        ("underground", T::Theme(Theme::Cave)),
        ("space", T::Theme(Theme::Space)),
        ("galaxy", T::Theme(Theme::Space)),
        ("desert", T::Theme(Theme::Desert)),
        ("snow", T::Theme(Theme::Snow)),
        ("ice", T::Theme(Theme::Snow)),
        ("winter", T::Theme(Theme::Snow)),
        ("snowy", T::Theme(Theme::Snow)),
        ("garden", T::Theme(Theme::Garden)),
        ("park", T::Theme(Theme::Garden)),
        ("ocean", T::Theme(Theme::Ocean)),
        ("sea", T::Theme(Theme::Ocean)),
        ("underwater", T::Theme(Theme::Ocean)),
        ("candy", T::Theme(Theme::Candy)),
        ("castle", T::Theme(Theme::Castle)),
        ("dungeon", T::Theme(Theme::Castle)),
        ("city", T::Theme(Theme::City)),
        ("town", T::Theme(Theme::City)),
        ("lava", T::Theme(Theme::Lava)),
        ("volcano", T::Theme(Theme::Lava)),
        // time of day
        ("night", T::Time(TimeOfDay::Night)),
        ("dark", T::Time(TimeOfDay::Night)),
        ("midnight", T::Time(TimeOfDay::Night)),
        ("day", T::Time(TimeOfDay::Day)),
        ("sunny", T::Time(TimeOfDay::Day)),
        ("bright", T::Time(TimeOfDay::Day)),
        // colours
        ("red", T::Color(Color::Red)),
        ("orange", T::Color(Color::Orange)),
        ("yellow", T::Color(Color::Yellow)),
        ("gold", T::Color(Color::Yellow)),
        ("golden", T::Color(Color::Yellow)),
        ("green", T::Color(Color::Green)),
        ("blue", T::Color(Color::Blue)),
        ("purple", T::Color(Color::Purple)),
        ("violet", T::Color(Color::Purple)),
        ("pink", T::Color(Color::Pink)),
        ("black", T::Color(Color::Black)),
        ("white", T::Color(Color::White)),
        ("grey", T::Color(Color::Grey)),
        ("gray", T::Color(Color::Grey)),
        ("brown", T::Color(Color::Brown)),
        // the player
        ("the player", T::PlayerWord),
        ("player", T::PlayerWord),
        ("you", T::PlayerWord),
        ("hero", T::PlayerWord),
        ("character", T::PlayerWord),
        ("knight", T::Player(PlayerKind::Knight)),
        ("cat", T::Player(PlayerKind::Cat)),
        ("kitten", T::Player(PlayerKind::Cat)),
        ("frog", T::Player(PlayerKind::Frog)),
        ("ship", T::Player(PlayerKind::Ship)),
        ("spaceship", T::Player(PlayerKind::Ship)),
        ("rocket", T::Player(PlayerKind::Ship)),
        ("gnome", T::Player(PlayerKind::Gnome)),
        // enemies
        ("enemy", T::EnemiesWord),
        ("enemies", T::EnemiesWord),
        ("monster", T::EnemiesWord),
        ("monsters", T::EnemiesWord),
        ("baddies", T::EnemiesWord),
        ("foes", T::EnemiesWord),
        // pickups
        ("items", T::PickupsWord),
        ("collectibles", T::PickupsWord),
        ("pickups", T::PickupsWord),
        ("treasure", T::PickupsWord),
        ("jewel", T::Pickup(PickupKind::Gem)),
        ("jewels", T::Pickup(PickupKind::Gem)),
        ("diamond", T::Pickup(PickupKind::Gem)),
        ("diamonds", T::Pickup(PickupKind::Gem)),
        ("crystal", T::Pickup(PickupKind::Gem)),
        ("crystals", T::Pickup(PickupKind::Gem)),
        ("fruit", T::Pickup(PickupKind::Apple)),
        // hazards (plain "lava" is the theme)
        ("lava pit", T::Hazard(HazardKind::Lava)),
        ("lava pits", T::Hazard(HazardKind::Lava)),
        ("lava pool", T::Hazard(HazardKind::Lava)),
        ("lava pools", T::Hazard(HazardKind::Lava)),
        ("saw blade", T::Hazard(HazardKind::Saw)),
        ("saw blades", T::Hazard(HazardKind::Saw)),
        ("buzzsaw", T::Hazard(HazardKind::Saw)),
        ("buzzsaws", T::Hazard(HazardKind::Saw)),
        // exits
        ("door", T::Exit(ExitKind::Door)),
        ("exit", T::Exit(ExitKind::Door)),
        ("flag", T::Exit(ExitKind::Flag)),
        ("goal flag", T::Exit(ExitKind::Flag)),
        ("portal", T::Exit(ExitKind::Portal)),
        // the level
        ("level", T::LevelWord),
        ("world", T::LevelWord),
        ("map", T::LevelWord),
        ("stage", T::LevelWord),
        // verbs
        ("collect", T::Verb(Verb::Collect)),
        ("collects", T::Verb(Verb::Collect)),
        ("gather", T::Verb(Verb::Collect)),
        ("gathers", T::Verb(Verb::Collect)),
        ("grab", T::Verb(Verb::Collect)),
        ("grabs", T::Verb(Verb::Collect)),
        ("pick up", T::Verb(Verb::Collect)),
        ("picks up", T::Verb(Verb::Collect)),
        ("find", T::Verb(Verb::Collect)),
        ("finds", T::Verb(Verb::Collect)),
        ("avoid", T::Verb(Verb::Avoid)),
        ("avoids", T::Verb(Verb::Avoid)),
        ("dodge", T::Verb(Verb::Avoid)),
        ("dodges", T::Verb(Verb::Avoid)),
        ("escape", T::Verb(Verb::Avoid)),
        ("escapes", T::Verb(Verb::Avoid)),
        ("chase", T::Verb(Verb::Chase)),
        ("chases", T::Verb(Verb::Chase)),
        ("hunt", T::Verb(Verb::Chase)),
        ("hunts", T::Verb(Verb::Chase)),
        ("follow", T::Verb(Verb::Chase)),
        ("follows", T::Verb(Verb::Chase)),
        ("patrol", T::Verb(Verb::Patrol)),
        ("patrols", T::Verb(Verb::Patrol)),
        ("walk", T::Verb(Verb::Patrol)),
        ("walks", T::Verb(Verb::Patrol)),
        ("wander", T::Verb(Verb::Wander)),
        ("wanders", T::Verb(Verb::Wander)),
        ("roam", T::Verb(Verb::Wander)),
        ("roams", T::Verb(Verb::Wander)),
        ("fly", T::Verb(Verb::Fly)),
        ("flies", T::Verb(Verb::Fly)),
        ("flying", T::Verb(Verb::Fly)),
        ("march", T::Verb(Verb::March)),
        ("marches", T::Verb(Verb::March)),
        ("marching", T::Verb(Verb::March)),
        ("shoot", T::Verb(Verb::Shoot)),
        ("shoots", T::Verb(Verb::Shoot)),
        ("shooting", T::Verb(Verb::Shoot)),
        ("fire", T::Verb(Verb::Shoot)),
        ("fires", T::Verb(Verb::Shoot)),
        ("blast", T::Verb(Verb::Shoot)),
        ("jump", T::Verb(Verb::Jump)),
        ("jumps", T::Verb(Verb::Jump)),
        ("jumping", T::Verb(Verb::Jump)),
        ("double jump", T::Verb(Verb::DoubleJump)),
        ("double jumps", T::Verb(Verb::DoubleJump)),
        ("double jumping", T::Verb(Verb::DoubleJump)),
        ("survive", T::Verb(Verb::Survive)),
        ("survives", T::Verb(Verb::Survive)),
        ("last", T::Verb(Verb::Survive)),
        ("reach", T::Verb(Verb::Reach)),
        ("reaches", T::Verb(Verb::Reach)),
        ("get to", T::Verb(Verb::Reach)),
        ("gets to", T::Verb(Verb::Reach)),
        // edits
        ("faster", T::Change(Change::Faster)),
        ("quicker", T::Change(Change::Faster)),
        ("slower", T::Change(Change::Slower)),
        ("more", T::Change(Change::More)),
        ("fewer", T::Change(Change::Fewer)),
        ("less", T::Change(Change::Fewer)),
        ("bigger", T::Change(Change::Bigger)),
        ("larger", T::Change(Change::Bigger)),
        ("longer", T::Change(Change::Bigger)),
        ("smaller", T::Change(Change::Smaller)),
        ("shorter", T::Change(Change::Smaller)),
        // counts and units
        ("row", T::Rows),
        ("rows", T::Rows),
        ("life", T::Lives),
        ("lives", T::Lives),
        ("second", T::Seconds),
        ("seconds", T::Seconds),
        ("sec", T::Seconds),
        ("secs", T::Seconds),
        ("minute", T::Minutes),
        ("minutes", T::Minutes),
        ("many", T::Many),
        ("lots of", T::Many),
        ("lot of", T::Many),
        ("plenty of", T::Many),
        ("loads of", T::Many),
        ("tons of", T::Many),
        ("a few", T::Few),
        ("few", T::Few),
        ("no", T::No),
        ("without", T::No),
        // connectives
        ("and", T::And),
        (",", T::Comma),
        ("then", T::Then),
        ("and then", T::Then),
        ("with", T::With),
        ("where", T::With),
        ("while", T::With),
    ];
    // kinds by name, singular and plural
    fn plural(s: &'static str) -> &'static str {
        Box::leak(format!("{s}s").into_boxed_str())
    }
    for k in EnemyKind::ALL {
        t.push((k.name(), Tok::Enemy(*k)));
        t.push((plural(k.name()), Tok::Enemy(*k)));
    }
    for k in PickupKind::ALL {
        t.push((k.name(), Tok::Pickup(*k)));
        t.push((plural(k.name()), Tok::Pickup(*k)));
    }
    t.push(("spike", Tok::Hazard(HazardKind::Spike)));
    t.push(("spikes", Tok::Hazard(HazardKind::Spike)));
    t.push(("saw", Tok::Hazard(HazardKind::Saw)));
    t.push(("saws", Tok::Hazard(HazardKind::Saw)));
    t.push(("flags", Tok::Exit(ExitKind::Flag)));
    t.push(("portals", Tok::Exit(ExitKind::Portal)));
    t.push(("doors", Tok::Exit(ExitKind::Door)));
    t.push(("invaders", Tok::Enemy(EnemyKind::Alien)));
    t.push(("invader", Tok::Enemy(EnemyKind::Alien)));
    for f in [
        "a", "an", "the", "of", "in", "at", "on", "to", "by", "some", "that", "which", "who", "is", "are", "be", "has", "have",
        "it", "its", "their", "them", "him", "her", "they", "can", "must", "should", "will", "all", "every", "each", "there",
        "as", "for", "from", "around", "across", "back", "end", "game", "games", "me", "my", "your", "into", "out", "over",
        "under", "only", "just", "also", "very", "really", "super", "so", "please", "one", "full", "along",
        "little", "big", "tiny", "huge", "cute", "scary", "spooky", "retro", "pixel", "classic", "fun", "cool",
        "about", "like", "kind", "sort", "style", "themed",
    ] {
        t.push((f, Tok::Filler));
    }
    t
}

fn phrases() -> &'static Vec<(Vec<&'static str>, Tok, &'static str)> {
    use std::sync::OnceLock;
    static P: OnceLock<Vec<(Vec<&'static str>, Tok, &'static str)>> = OnceLock::new();
    P.get_or_init(|| {
        let mut v: Vec<(Vec<&'static str>, Tok, &'static str)> = table().into_iter().map(|(p, t)| (p.split(' ').collect(), t, p)).collect();
        // longest first; stable within a length
        v.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        v
    })
}

const NUMBER_WORDS: &[&str] = &[
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen",
    "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen", "twenty",
];
const TENS: &[(&str, u32)] = &[("thirty", 30), ("forty", 40), ("fifty", 50), ("sixty", 60), ("seventy", 70), ("eighty", 80), ("ninety", 90), ("hundred", 100)];

fn words(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<(String, String)>| {
        if !cur.is_empty() {
            out.push((cur.to_lowercase(), cur.clone()));
            cur.clear();
        }
    };
    for c in text.chars() {
        match c {
            ',' => {
                flush(&mut cur, &mut out);
                out.push((",".into(), ",".into()));
            }
            '-' | '_' | '/' => flush(&mut cur, &mut out),
            c if c.is_whitespace() => flush(&mut cur, &mut out),
            '.' | '!' | '?' | ';' | ':' | '"' | '\'' | '(' | ')' => flush(&mut cur, &mut out),
            c => cur.push(c),
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// Tokens, plus the words nobody knows (with their positions in the token stream).
pub fn tokenize(text: &str) -> (Vec<Token>, Vec<String>) {
    let ws = words(text);
    let mut out = Vec::new();
    let mut unknown = Vec::new();
    let mut i = 0;
    'outer: while i < ws.len() {
        let (lw, orig) = &ws[i];
        if let Ok(n) = lw.parse::<u32>() {
            out.push(Token { tok: Tok::Num(n), text: orig.clone() });
            i += 1;
            continue;
        }
        for (p, tok, _) in phrases() {
            if i + p.len() <= ws.len() && p.iter().enumerate().all(|(k, w)| ws[i + k].0 == *w) {
                let text = ws[i..i + p.len()].iter().map(|w| w.1.as_str()).collect::<Vec<_>>().join(" ");
                out.push(Token { tok: *tok, text });
                i += p.len();
                continue 'outer;
            }
        }
        if let Some(n) = NUMBER_WORDS.iter().position(|w| w == lw) {
            out.push(Token { tok: Tok::Num(n as u32), text: orig.clone() });
            i += 1;
            continue;
        }
        if let Some((_, n)) = TENS.iter().find(|(w, _)| w == lw) {
            out.push(Token { tok: Tok::Num(*n), text: orig.clone() });
            i += 1;
            continue;
        }
        unknown.push(orig.clone());
        i += 1;
    }
    // "one" is a number word but also a filler ("the red one"); a filler "one" is only kept
    // when no noun follows. The table lists it as filler, so re-check: "one coin" -> 1.
    let mut fixed = Vec::with_capacity(out.len());
    for (k, t) in out.iter().enumerate() {
        if t.tok == Tok::Filler && t.text.eq_ignore_ascii_case("one") {
            let next_is_thing = out.get(k + 1).is_some_and(|n| matches!(n.tok, Tok::Enemy(_) | Tok::Pickup(_) | Tok::Hazard(_) | Tok::Rows | Tok::Lives | Tok::Minutes | Tok::Seconds));
            if next_is_thing {
                fixed.push(Token { tok: Tok::Num(1), text: t.text.clone() });
                continue;
            }
        }
        fixed.push(t.clone());
    }
    (fixed, unknown)
}

/// Every phrase that means something (fillers left out), sorted.
pub fn vocabulary() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = phrases().iter().filter(|(_, t, _)| *t != Tok::Filler).map(|(_, _, p)| *p).collect();
    v.sort_unstable();
    v.dedup();
    v
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The closest known phrase, if reasonably close. Ties go alphabetically, so hints are deterministic.
pub fn closest(word: &str) -> Option<&'static str> {
    let w = word.to_lowercase();
    let limit = (w.chars().count() / 3).max(1) + 1;
    vocabulary().into_iter().map(|v| (levenshtein(&w, v), v)).filter(|(d, _)| *d <= limit).min().map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        let (t, u) = tokenize(s);
        assert!(u.is_empty(), "unknown words in {s:?}: {u:?}");
        t.into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn phrases_are_unique() {
        let mut seen: Vec<&str> = phrases().iter().map(|(_, _, p)| *p).collect();
        seen.sort_unstable();
        let n = seen.len();
        seen.dedup();
        assert_eq!(n, seen.len(), "a phrase is listed twice");
    }

    #[test]
    fn longest_phrase_wins() {
        assert_eq!(toks("space shooter"), vec![Tok::Genre(Genre::Shooter)]);
        assert_eq!(toks("double jump"), vec![Tok::Verb(Verb::DoubleJump)]);
        assert_eq!(toks("lava pits"), vec![Tok::Hazard(HazardKind::Lava)]);
        assert_eq!(toks("lava"), vec![Tok::Theme(Theme::Lava)]);
        assert_eq!(toks("top-down"), vec![Tok::Genre(Genre::Arena)]);
    }

    #[test]
    fn synonyms_share_a_token() {
        assert_eq!(toks("jewels"), toks("gems"));
        assert_eq!(toks("invaders"), toks("aliens"));
        assert_eq!(toks("tough"), toks("hard"));
    }

    #[test]
    fn numbers() {
        assert_eq!(toks("20 coins"), vec![Tok::Num(20), Tok::Pickup(PickupKind::Coin)]);
        assert_eq!(toks("three rows"), vec![Tok::Num(3), Tok::Rows]);
        assert_eq!(toks("sixty seconds"), vec![Tok::Num(60), Tok::Seconds]);
        assert_eq!(toks("one bat"), vec![Tok::Num(1), Tok::Enemy(EnemyKind::Bat)]);
    }

    #[test]
    fn unknown_words_are_reported_with_a_hint() {
        let (_, u) = tokenize("make a platfromer");
        assert_eq!(u, vec!["platfromer".to_string()]);
        assert_eq!(closest("platfromer"), Some("platformer"));
        assert_eq!(closest("xylophone"), None);
    }
}
