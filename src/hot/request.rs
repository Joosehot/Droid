//! The structured request a sentence parses into. Every value remembers the words that asked for it.

macro_rules! kinds {
    ($name:ident { $($var:ident = $s:literal),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name { $($var),* }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),*];
            pub fn name(self) -> &'static str { match self { $($name::$var => $s),* } }
            #[allow(dead_code)]
            pub fn from_name(s: &str) -> Option<$name> { $name::ALL.iter().copied().find(|k| k.name() == s) }
        }
    };
}

kinds!(Genre { Platformer = "platformer", Arena = "arena", Shooter = "shooter" });
kinds!(ViewReq { Flat = "2d", Depth = "3d" });
kinds!(Mod { Hard = "hard", Easy = "easy", Fast = "fast", Slow = "slow", Simple = "simple" });
kinds!(Theme {
    Forest = "forest", Cave = "cave", Space = "space", Desert = "desert", Snow = "snow",
    Garden = "garden", Ocean = "ocean", Candy = "candy", Castle = "castle", City = "city", Lava = "lava",
});
kinds!(LayoutReq {
    FlatRun = "flat_run", Steps = "steps", Stacked = "stacked", Open = "open", Pillars = "pillars", Rooms = "rooms", Shields = "shields",
});
kinds!(ArtStyle { Pixel = "pixel", Vector = "vector", Lowpoly = "lowpoly" });
kinds!(TimeOfDay { Day = "day", Night = "night" });
kinds!(Color {
    Red = "red", Orange = "orange", Yellow = "yellow", Green = "green", Blue = "blue", Purple = "purple",
    Pink = "pink", Black = "black", White = "white", Grey = "grey", Brown = "brown",
});
kinds!(PlayerKind { Knight = "knight", Cat = "cat", Frog = "frog", Ship = "ship", Gnome = "gnome" });
kinds!(EnemyKind {
    Slime = "slime", Spider = "spider", Skeleton = "skeleton", Zombie = "zombie", Blob = "blob",
    Bat = "bat", Ghost = "ghost", Alien = "alien",
});
kinds!(PickupKind { Coin = "coin", Gem = "gem", Star = "star", Apple = "apple", Heart = "heart", Key = "key" });
kinds!(HazardKind { Spike = "spike", Saw = "saw", Lava = "lava" });
kinds!(ExitKind { Door = "door", Flag = "flag", Portal = "portal" });
kinds!(Behavior { Patrol = "patrol", Chase = "chase", Wander = "wander", Fly = "fly", March = "march" });
kinds!(Ability { Jump = "jump", DoubleJump = "double jump", Shoot = "shoot" });
kinds!(Target { Enemies = "enemies", Pickups = "pickups", Player = "player", Level = "level", Hazards = "hazards" });
kinds!(Change { Faster = "faster", Slower = "slower", More = "more", Fewer = "fewer", Bigger = "bigger", Smaller = "smaller" });

/// How many of something: an exact number, or "lots of" / "a few" (turned into numbers by rules.toml).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amount {
    N(u32),
    Many,
    Few,
}

/// A value plus the words that produced it.
#[derive(Clone, Debug, PartialEq)]
pub struct Val<T> {
    pub v: T,
    pub words: String,
}

impl<T> Val<T> {
    pub fn new(v: T, words: impl Into<String>) -> Self {
        Val { v, words: words.into() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GoalReq {
    CollectAll,
    Reach,
    /// seconds, if given
    Survive(Option<u32>),
    DestroyAll,
}

#[derive(Clone, Debug)]
pub struct EnemyReq {
    pub kind: Val<EnemyKind>,
    pub count: Option<Val<Amount>>,
    pub rows: Option<Val<u32>>,
    pub color: Option<Val<Color>>,
    pub behaviors: Vec<Val<Behavior>>,
    pub shoots: Option<Val<()>>,
}

#[derive(Clone, Debug)]
pub struct PickupReq {
    pub kind: Val<PickupKind>,
    pub count: Option<Val<Amount>>,
    pub color: Option<Val<Color>>,
}

#[derive(Clone, Debug)]
pub struct HazardReq {
    pub kind: Val<HazardKind>,
    pub count: Option<Val<Amount>>,
}

#[derive(Clone, Debug, Default)]
pub struct Request {
    pub sentence: String,
    pub genre: Option<Val<Genre>>,
    pub view: Option<Val<ViewReq>>,
    pub player: Option<Val<PlayerKind>>,
    pub player_color: Option<Val<Color>>,
    pub enemies: Vec<EnemyReq>,
    /// "with enemies" without naming a kind: the genre's default enemy.
    pub generic_enemies: Option<Val<Option<Amount>>>,
    /// Behaviours said about enemies without a kind ("enemies chase the player").
    pub generic_behaviors: Vec<Val<Behavior>>,
    /// "enemies that shoot back" without a kind.
    pub generic_shoots: Option<Val<()>>,
    pub no_enemies: Option<Val<()>>,
    pub pickups: Vec<PickupReq>,
    pub generic_pickups: Option<Val<Option<Amount>>>,
    pub no_pickups: Option<Val<()>>,
    pub hazards: Vec<HazardReq>,
    pub exit: Option<Val<ExitKind>>,
    pub goals: Vec<Val<GoalReq>>,
    pub abilities: Vec<Val<Ability>>,
    pub theme: Option<Val<Theme>>,
    pub time: Option<Val<TimeOfDay>>,
    /// "pixel", "cartoon", "low poly": which family of library art to draw with.
    pub style: Option<Val<ArtStyle>>,
    /// "with rooms", "on platforms", "with shields": the level's shape.
    pub layout: Option<Val<LayoutReq>>,
    pub mods: Vec<Val<Mod>>,
    pub lives: Option<Val<u32>>,
    pub edits: Vec<Val<(Target, Change)>>,
}

impl Request {
    pub fn has_ability(&self, a: Ability) -> Option<&Val<Ability>> {
        self.abilities.iter().find(|x| x.v == a)
    }

    pub fn goal(&self, pred: impl Fn(&GoalReq) -> bool) -> Option<&Val<GoalReq>> {
        self.goals.iter().rev().find(|g| pred(&g.v))
    }

    /// Net number of steps for an edit: (+1 per `up`, -1 per `down`) on a target.
    pub fn edit_steps(&self, target: Target, up: Change, down: Change) -> i32 {
        self.edits.iter().map(|e| if e.v.0 != target { 0 } else if e.v.1 == up { 1 } else if e.v.1 == down { -1 } else { 0 }).sum()
    }

    pub fn edit_words(&self, target: Target) -> String {
        self.edits.iter().filter(|e| e.v.0 == target).map(|e| e.words.clone()).collect::<Vec<_>>().join(", ")
    }

    pub fn any_enemy_shoots(&self) -> Option<String> {
        self.enemies.iter().find_map(|e| e.shoots.as_ref().map(|s| s.words.clone()))
    }
}
