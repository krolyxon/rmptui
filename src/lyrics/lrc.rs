//! Parser for LRC (synced lyrics) files.

use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricLine {
    pub time: Duration,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lyrics {
    /// Lines with timestamps, sorted by time.
    Synced(Vec<LyricLine>),
    Plain(Vec<String>),
}

impl Lyrics {
    /// Parses LRC text. Text without any timestamps is treated as plain
    /// lyrics.
    pub fn parse(text: &str) -> Self {
        let mut offset_ms: i64 = 0;
        let mut synced = Vec::new();
        for raw in text.lines() {
            let mut rest = raw.trim();
            let mut times = Vec::new();
            while let Some(tag) = rest.strip_prefix('[').and_then(|r| r.split_once(']')) {
                let (inner, tail) = tag;
                if let Some(time) = parse_timestamp(inner) {
                    times.push(time);
                } else if let Some(value) = inner.strip_prefix("offset:") {
                    offset_ms = value.trim().parse().unwrap_or(0);
                } else if !is_id_tag(inner) {
                    break;
                }
                rest = tail;
            }
            let text = rest.trim().to_owned();
            synced.extend(times.into_iter().map(|time| (time, text.clone())));
        }
        if synced.is_empty() {
            return Self::Plain(text.lines().map(|l| l.trim_end().to_owned()).collect());
        }
        // A positive offset makes lyrics appear sooner.
        let mut lines: Vec<LyricLine> = synced
            .into_iter()
            .map(|(time, text)| LyricLine {
                time: shift(time, -offset_ms),
                text,
            })
            .collect();
        lines.sort_by_key(|line| line.time);
        Self::Synced(lines)
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Synced(lines) => lines.is_empty(),
            Self::Plain(lines) => lines.iter().all(|l| l.trim().is_empty()),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Synced(lines) => lines.len(),
            Self::Plain(lines) => lines.len(),
        }
    }

    /// The index of the line being sung at `elapsed`.
    pub fn current_line(&self, elapsed: Duration) -> Option<usize> {
        match self {
            Self::Synced(lines) => lines
                .partition_point(|line| line.time <= elapsed)
                .checked_sub(1),
            Self::Plain(_) => None,
        }
    }

    pub fn line(&self, index: usize) -> &str {
        match self {
            Self::Synced(lines) => lines.get(index).map_or("", |l| l.text.as_str()),
            Self::Plain(lines) => lines.get(index).map_or("", String::as_str),
        }
    }
}

/// `mm:ss`, `mm:ss.xx` or `mm:ss.xxx`.
fn parse_timestamp(tag: &str) -> Option<Duration> {
    let (minutes, seconds) = tag.split_once(':')?;
    let minutes: u64 = minutes.trim().parse().ok()?;
    let (secs, fraction) = seconds.split_once(['.', ':']).unwrap_or((seconds, "0"));
    let secs: u64 = secs.trim().parse().ok()?;
    if secs >= 60 || fraction.is_empty() || !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let millis = match fraction.len() {
        1 => fraction.parse::<u64>().ok()? * 100,
        2 => fraction.parse::<u64>().ok()? * 10,
        _ => fraction[..3].parse::<u64>().ok()?,
    };
    Some(Duration::from_millis((minutes * 60 + secs) * 1000 + millis))
}

fn is_id_tag(tag: &str) -> bool {
    tag.split_once(':')
        .is_some_and(|(key, _)| !key.is_empty() && key.chars().all(|c| c.is_ascii_alphabetic()))
}

fn shift(time: Duration, ms: i64) -> Duration {
    let shifted = time.as_millis() as i64 + ms;
    Duration::from_millis(shifted.max(0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    #[test]
    fn parses_synced_lyrics() {
        let lyrics = Lyrics::parse(
            "[ar:Someone]\n[ti:Song]\n[00:12.50]First\n[00:05.1][01:00.000]Chorus\n\n[00:20]Last",
        );
        let Lyrics::Synced(lines) = &lyrics else {
            panic!("expected synced lyrics");
        };
        let times: Vec<_> = lines.iter().map(|l| l.time).collect();
        assert_eq!(times, [ms(5_100), ms(12_500), ms(20_000), ms(60_000)]);
        assert_eq!(lines[0].text, "Chorus");
        assert_eq!(lyrics.current_line(ms(0)), None);
        assert_eq!(lyrics.current_line(ms(12_500)), Some(1));
        assert_eq!(lyrics.current_line(ms(90_000)), Some(3));
    }

    #[test]
    fn applies_offset() {
        let lyrics = Lyrics::parse("[offset:+500]\n[00:01.00]a\n[00:00.20]b");
        let Lyrics::Synced(lines) = lyrics else {
            panic!("expected synced lyrics");
        };
        assert_eq!(lines[0].time, ms(0));
        assert_eq!(lines[1].time, ms(500));
    }

    #[test]
    fn falls_back_to_plain() {
        let lyrics = Lyrics::parse("Just some\nwords [not a tag]\n");
        assert_eq!(
            lyrics,
            Lyrics::Plain(vec!["Just some".into(), "words [not a tag]".into()])
        );
        assert_eq!(lyrics.current_line(ms(1000)), None);
    }

    #[test]
    fn rejects_bad_timestamps() {
        assert_eq!(parse_timestamp("1:75.00"), None);
        assert_eq!(parse_timestamp("ab:10"), None);
        assert_eq!(parse_timestamp("02:03.4567"), Some(ms(123_456)));
    }
}
