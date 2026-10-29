//! Play statistics. Every finished session is appended to `sessions.jsonl`
//! (game, system, start, length); the stats screen sums them by day, week
//! and year. All-time totals per game come from the user data Lumen already
//! keeps, so they include play from before sessions were logged.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{BufRead, Write},
    path::Path,
};

const FILE: &str = "sessions.jsonl";
const DAY: u64 = 86_400;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub game_id: String,
    pub system_id: String,
    /// Unix seconds.
    pub start: u64,
    pub seconds: u64,
}

pub fn record(data_dir: &Path, session: &Session) -> anyhow::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir.join(FILE))?;
    writeln!(file, "{}", serde_json::to_string(session)?)?;
    Ok(())
}

pub fn load(data_dir: &Path) -> Vec<Session> {
    let Ok(file) = std::fs::File::open(data_dir.join(FILE)) else {
        return Vec::new();
    };
    std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    /// Unix seconds at the day's local midnight.
    pub start: u64,
    pub seconds: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Total {
    pub id: String,
    pub seconds: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// The last 7 days, oldest first; the last one is today.
    pub days: Vec<Day>,
    pub week_seconds: u64,
    pub previous_week_seconds: u64,
    pub year: i32,
    pub year_seconds: u64,
    pub year_sessions: usize,
    pub year_games: usize,
    /// All-time, most played first.
    pub top_games: Vec<Total>,
    pub top_systems: Vec<Total>,
}

/// `today` is the Unix time of today's local midnight; `year_start` that of
/// January 1st (both from the caller, which knows the local time zone).
pub fn summarize(
    sessions: &[Session],
    all_time: &HashMap<String, (String, u64)>,
    today: u64,
    year: i32,
    year_start: u64,
) -> Stats {
    let sum = |from: u64, to: u64| -> u64 {
        sessions
            .iter()
            .filter(|s| s.start >= from && s.start < to)
            .map(|s| s.seconds)
            .sum()
    };
    let days: Vec<Day> = (0..7)
        .rev()
        .map(|back| {
            let start = today - back * DAY;
            Day {
                start,
                seconds: sum(start, start + DAY),
            }
        })
        .collect();
    let week_start = today - 6 * DAY;
    let this_year: Vec<&Session> = sessions.iter().filter(|s| s.start >= year_start).collect();
    let mut year_games: Vec<&str> = this_year.iter().map(|s| s.game_id.as_str()).collect();
    year_games.sort();
    year_games.dedup();

    let mut top_games: Vec<Total> = all_time
        .iter()
        .filter(|(_, (_, seconds))| *seconds > 0)
        .map(|(id, (_, seconds))| Total {
            id: id.clone(),
            seconds: *seconds,
        })
        .collect();
    top_games.sort_by(|a, b| b.seconds.cmp(&a.seconds).then(a.id.cmp(&b.id)));
    let mut systems: HashMap<&str, u64> = HashMap::new();
    for (system, seconds) in all_time.values() {
        *systems.entry(system).or_default() += seconds;
    }
    let mut top_systems: Vec<Total> = systems
        .into_iter()
        .filter(|(_, s)| *s > 0)
        .map(|(id, seconds)| Total {
            id: id.to_string(),
            seconds,
        })
        .collect();
    top_systems.sort_by(|a, b| b.seconds.cmp(&a.seconds).then(a.id.cmp(&b.id)));
    top_games.truncate(5);
    top_systems.truncate(5);

    Stats {
        days,
        week_seconds: sum(week_start, today + DAY),
        previous_week_seconds: sum(week_start - 7 * DAY, week_start),
        year,
        year_seconds: this_year.iter().map(|s| s.seconds).sum(),
        year_sessions: this_year.len(),
        year_games: year_games.len(),
        top_games,
        top_systems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(game: &str, start: u64, seconds: u64) -> Session {
        Session {
            game_id: game.into(),
            system_id: "snes".into(),
            start,
            seconds,
        }
    }

    #[test]
    fn sums_days_weeks_and_the_year() {
        let today = 400 * DAY;
        let sessions = vec![
            session("a", today + 100, 600),        // today
            session("a", today - DAY + 5, 300),    // yesterday
            session("b", today - 6 * DAY, 120),    // first day of this week
            session("b", today - 7 * DAY, 60),     // previous week
            session("c", today - 300 * DAY, 1000), // last year
        ];
        let all_time = HashMap::from([
            ("a".to_string(), ("snes".to_string(), 900)),
            ("b".to_string(), ("gba".to_string(), 180)),
            ("c".to_string(), ("gba".to_string(), 1000)),
        ]);
        let stats = summarize(&sessions, &all_time, today, 2026, today - 200 * DAY);
        assert_eq!(stats.days.len(), 7);
        assert_eq!(stats.days[6].seconds, 600);
        assert_eq!(stats.days[5].seconds, 300);
        assert_eq!(stats.days[0].seconds, 120);
        assert_eq!(stats.week_seconds, 1020);
        assert_eq!(stats.previous_week_seconds, 60);
        assert_eq!(
            (stats.year_seconds, stats.year_sessions, stats.year_games),
            (1080, 4, 2)
        );
        assert_eq!(stats.top_games[0].id, "c");
        assert_eq!(
            stats.top_systems[0],
            Total {
                id: "gba".into(),
                seconds: 1180
            }
        );
    }

    #[test]
    fn records_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("lumen-stats-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = session("a", 10, 20);
        record(&dir, &s).unwrap();
        record(&dir, &s).unwrap();
        assert_eq!(load(&dir), vec![s.clone(), s]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
