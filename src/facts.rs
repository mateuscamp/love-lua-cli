//! What the game writes while it runs, for the panel to show.
//!
//! The games write one-line facts, `<GAME> <TYPE> key=value ...` (GAME is the folder in uppercase;
//! a README rule), with stdout line-buffered (`io.stdout:setvbuf("line")` at the top of each one's
//! `conf.lua`), so each fact arrives live through the pipe. The format belongs to the games; here
//! it is only read. What is not a fact (a traceback, a LÖVE warning) becomes the `texto` type or,
//! from stderr, `erro`.

use std::time::Duration;

/// Which pipe the line came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Stdout,
    Stderr,
}

/// A line from the game, parsed.
#[derive(Debug, PartialEq)]
pub enum GameLine {
    Fact {
        kind: String,
        fields: Vec<(String, String)>,
    },
    Text(String),
}

/// The type of the loose lines from stdout and stderr; in lowercase, so as not to collide with the
/// games' types, which are in uppercase.
pub const TEXT_KIND: &str = "texto";
pub const ERROR_KIND: &str = "erro";

/// Parses a line from the game: it is a fact if it starts with `prefix` (the game's name in
/// uppercase) followed by a type in uppercase; the rest is loose text. A word without `=` becomes a
/// field without a key.
pub fn parse_line(prefix: &str, line: &str) -> GameLine {
    let line = line.trim_end_matches(['\n', '\r']);
    let mut words = line.split_whitespace();
    let (Some(game), Some(kind)) = (words.next(), words.next()) else {
        return GameLine::Text(line.to_string());
    };
    if game != prefix || !is_kind(kind) {
        return GameLine::Text(line.to_string());
    }
    let fields = words
        .map(|word| match word.split_once('=') {
            Some((key, value)) => (key.to_string(), value.to_string()),
            None => (String::new(), word.to_string()),
        })
        .collect();
    GameLine::Fact {
        kind: kind.to_string(),
        fields,
    }
}

/// `MAPA`, `FIM_ANDAR`: an uppercase letter first, then uppercase letters, digits and `_`.
fn is_kind(word: &str) -> bool {
    word.starts_with(|letter: char| letter.is_ascii_uppercase())
        && word
            .chars()
            .all(|letter| letter.is_ascii_uppercase() || letter.is_ascii_digit() || letter == '_')
}

/// A fact type of the run: how many times it came, when it last came and with which fields.
#[derive(Debug, Clone, PartialEq)]
pub struct FactKind {
    pub name: String,
    pub count: u64,
    /// Since the game started.
    pub last_at: Duration,
    pub fields: Vec<(String, String)>,
}

/// The run's scoreboard: one `FactKind` per fact type, in the order each one appeared, so the
/// panel's lines do not swap places.
pub struct Scoreboard {
    prefix: String,
    kinds: Vec<FactKind>,
    /// From `CONFIG`: key, value and origin, the latest for each key.
    config: Vec<(String, String, String)>,
    total: u64,
    /// Facts per second since the game started, for the title's sparkline.
    per_second: Vec<u32>,
    first_error: Option<String>,
}

impl Scoreboard {
    /// The empty scoreboard of the game called `name` (the facts' prefix is the name in uppercase).
    pub fn new(name: &str) -> Scoreboard {
        Scoreboard {
            prefix: name.to_uppercase(),
            kinds: Vec::new(),
            config: Vec::new(),
            total: 0,
            per_second: Vec::new(),
            first_error: None,
        }
    }

    /// Records a line that arrived `at` (since the game started). A blank line does not count.
    pub fn record(&mut self, source: Source, line: &str, at: Duration) {
        if line.trim().is_empty() {
            return;
        }
        if source == Source::Stderr && self.first_error.is_none() {
            self.first_error = Some(line.trim_end().to_string());
        }
        match parse_line(&self.prefix, line) {
            GameLine::Fact { kind, fields } => {
                self.count_fact(at);
                if kind == "CONFIG" {
                    self.record_config(&fields);
                }
                self.update_kind(&kind, fields, at);
            }
            GameLine::Text(text) => {
                let kind = if source == Source::Stderr {
                    ERROR_KIND
                } else {
                    TEXT_KIND
                };
                self.update_kind(kind, vec![(String::new(), text)], at);
            }
        }
    }

    fn count_fact(&mut self, at: Duration) {
        self.total += 1;
        let second = at.as_secs() as usize;
        if self.per_second.len() <= second {
            self.per_second.resize(second + 1, 0);
        }
        self.per_second[second] += 1;
    }

    /// The games' `CONFIG` says `chave=`, `valor=` and `origem=`; the latest for each key stays.
    fn record_config(&mut self, fields: &[(String, String)]) {
        let field = |name: &str| {
            fields
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        };
        let (Some(key), Some(value), Some(origin)) =
            (field("chave"), field("valor"), field("origem"))
        else {
            return;
        };
        match self.config.iter_mut().find(|(seen, _, _)| *seen == key) {
            Some(entry) => (entry.1, entry.2) = (value, origin),
            None => self.config.push((key, value, origin)),
        }
    }

    fn update_kind(&mut self, name: &str, fields: Vec<(String, String)>, at: Duration) {
        match self.kinds.iter_mut().find(|kind| kind.name == name) {
            Some(kind) => {
                kind.count += 1;
                kind.last_at = at;
                kind.fields = fields;
            }
            None => self.kinds.push(FactKind {
                name: name.to_string(),
                count: 1,
                last_at: at,
                fields,
            }),
        }
    }

    pub fn kinds(&self) -> &[FactKind] {
        &self.kinds
    }

    /// How many facts came (the loose lines are left out).
    pub fn total(&self) -> u64 {
        self.total
    }

    /// The first stderr line: in a game that dies at boot, it is the one that says why.
    pub fn first_error(&self) -> Option<&str> {
        self.first_error.as_deref()
    }

    /// The `CONFIG` keys whose origin is not `padrao`, with the value, in the order they came.
    pub fn off_default(&self) -> Vec<(&str, &str)> {
        self.config
            .iter()
            .filter(|(_, _, origin)| origin != "padrao")
            .map(|(key, value, _)| (key.as_str(), value.as_str()))
            .collect()
    }

    /// Facts per second over the last `windows` up to the second of `now`, the oldest first; before
    /// the game started, zero.
    pub fn recent_rate(&self, now: Duration, windows: usize) -> Vec<u32> {
        let end = now.as_secs() as i64 + 1;
        (end - windows as i64..end)
            .map(|second| {
                usize::try_from(second)
                    .ok()
                    .and_then(|second| self.per_second.get(second).copied())
                    .unwrap_or(0)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    fn at(seconds: f64) -> Duration {
        Duration::from_secs_f64(seconds)
    }

    #[test]
    fn parse_line_tells_a_game_fact_from_loose_text() {
        assert_eq!(
            parse_line(
                "CAMPCRAWLER",
                "CAMPCRAWLER MAPA numero=1 linhas=9 esperado=9"
            ),
            GameLine::Fact {
                kind: "MAPA".to_string(),
                fields: fields(&[("numero", "1"), ("linhas", "9"), ("esperado", "9")]),
            }
        );
        assert_eq!(
            parse_line("CAMPCRAWLER", "CAMPCRAWLER FIM_ANDAR solto estado=won\r"),
            GameLine::Fact {
                kind: "FIM_ANDAR".to_string(),
                fields: fields(&[("", "solto"), ("estado", "won")]),
            }
        );
        for text in [
            "BOMBERBOOM INICIO motor=love",
            "Error: Could not initialize SDL video subsystem (x11 not available)",
            "CAMPCRAWLER",
            "CAMPCRAWLER minusculo a=1",
            "\t[C]: in function 'xpcall'",
        ] {
            assert_eq!(
                parse_line("CAMPCRAWLER", text),
                GameLine::Text(text.to_string()),
                "{text:?}"
            );
        }
    }

    #[test]
    fn the_scoreboard_counts_each_type_in_the_order_it_appeared() {
        let mut scoreboard = Scoreboard::new("CampCrawler");
        scoreboard.record(Source::Stdout, "CAMPCRAWLER PARTIDA semente=1", at(0.1));
        scoreboard.record(
            Source::Stdout,
            "CAMPCRAWLER VEZ soldado=1 texto=1º",
            at(0.5),
        );
        scoreboard.record(Source::Stdout, "", at(0.6));
        scoreboard.record(Source::Stdout, "loose LÖVE warning", at(0.7));
        scoreboard.record(
            Source::Stdout,
            "CAMPCRAWLER VEZ soldado=2 texto=2º",
            at(1.2),
        );
        scoreboard.record(Source::Stderr, "Error: boot.lua:479", at(3.9));
        scoreboard.record(Source::Stderr, "stack traceback:", at(3.9));

        let kinds: Vec<(&str, u64)> = scoreboard
            .kinds()
            .iter()
            .map(|kind| (kind.name.as_str(), kind.count))
            .collect();
        assert_eq!(
            kinds,
            [("PARTIDA", 1), ("VEZ", 2), (TEXT_KIND, 1), (ERROR_KIND, 2)]
        );
        let turn = &scoreboard.kinds()[1];
        assert_eq!(turn.fields, fields(&[("soldado", "2"), ("texto", "2º")]));
        assert_eq!(turn.last_at, at(1.2));
        assert_eq!(
            scoreboard.kinds()[3].fields,
            fields(&[("", "stack traceback:")])
        );
        assert_eq!(scoreboard.total(), 3, "only the facts");
        assert_eq!(scoreboard.first_error(), Some("Error: boot.lua:479"));
    }

    #[test]
    fn the_scoreboard_keeps_the_config_off_its_default() {
        let mut scoreboard = Scoreboard::new("BomberBoom");
        let lines = [
            "BOMBERBOOM CONFIG chave=seed valor=17 origem=plataforma",
            "BOMBERBOOM CONFIG chave=autoplay valor=0 origem=padrao",
            "BOMBERBOOM CONFIG chave=autoplay valor=1 origem=argumento",
            "BOMBERBOOM CONFIG chave=mode valor=- origem=padrao",
        ];
        for line in lines {
            scoreboard.record(Source::Stdout, line, at(0.0));
        }
        assert_eq!(
            scoreboard.off_default(),
            [("seed", "17"), ("autoplay", "1")]
        );
        assert_eq!(scoreboard.kinds()[0].count, 4);
    }

    #[test]
    fn recent_rate_counts_the_facts_per_second_up_to_now() {
        let mut scoreboard = Scoreboard::new("Boomlitude");
        for seconds in [0.1, 0.5, 1.2, 3.9] {
            scoreboard.record(Source::Stdout, "BOOMLITUDE LANCE aceito=sim", at(seconds));
        }
        scoreboard.record(Source::Stdout, "loose text does not count", at(2.5));
        assert_eq!(scoreboard.recent_rate(at(4.0), 5), [2, 1, 0, 1, 0]);
        assert_eq!(scoreboard.recent_rate(at(4.0), 3), [0, 1, 0]);
        assert_eq!(scoreboard.recent_rate(at(1.5), 4), [0, 0, 2, 1]);
    }
}
