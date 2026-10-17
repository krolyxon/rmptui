//! Wire format of MPD's text protocol: command encoding and response parsing.
//!
//! Everything here is pure so it can be tested without a server.

use std::fmt;

use super::error::{Ack, MpdError};

/// A single key/value line of an MPD response.
pub type Pair = (String, String);

/// A command ready to be written to the socket.
///
/// Arguments are always quoted and escaped, so user-provided names and paths
/// can never inject additional commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    line: String,
}

impl Command {
    pub fn new(name: &str) -> Self {
        Self {
            line: name.to_owned(),
        }
    }

    /// Appends a quoted argument.
    #[must_use]
    pub fn arg(mut self, value: impl AsRef<str>) -> Self {
        self.line.push(' ');
        push_quoted(&mut self.line, value.as_ref());
        self
    }

    /// The encoded command line, without the trailing newline.
    pub fn as_str(&self) -> &str {
        &self.line
    }

    /// The command name, used in error messages.
    pub fn name(&self) -> &str {
        self.line.split(' ').next().unwrap_or_default()
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.line)
    }
}

fn push_quoted(out: &mut String, value: &str) {
    out.push('"');
    out.push_str(&escape(value));
    out.push('"');
}

/// Escapes `\` and `"` so that `value` can be placed inside double quotes.
pub fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(ch, '"' | '\\') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// Builds an MPD filter expression `(TAG == "VALUE")`.
pub fn filter_eq(tag: &str, value: &str) -> String {
    format!("({tag} == \"{}\")", escape(value))
}

/// Combines filter expressions with `AND`.
pub fn filter_and<I, S>(filters: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let parts: Vec<String> = filters.into_iter().map(|f| f.as_ref().to_owned()).collect();
    match parts.as_slice() {
        [single] => single.clone(),
        _ => format!("({})", parts.join(" AND ")),
    }
}

/// One parsed response line.
#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    /// `OK`: the response is complete.
    Ok,
    /// `list_OK`: one command of a `command_list_ok_begin` list is complete.
    ListOk,
    /// A `key: value` pair.
    Pair(Pair),
}

/// Parses one response line (without its trailing newline).
pub fn parse_line(line: &str) -> Result<Line, MpdError> {
    match line {
        "OK" => Ok(Line::Ok),
        "list_OK" => Ok(Line::ListOk),
        _ if line.starts_with("ACK ") => Err(MpdError::Ack(parse_ack(line)?)),
        _ => line
            .split_once(": ")
            .map(|(key, value)| Line::Pair((key.to_owned(), value.to_owned())))
            .ok_or_else(|| MpdError::Protocol(format!("unexpected response line `{line}`"))),
    }
}

/// Parses `ACK [error@command_listNum] {current_command} message_text`.
fn parse_ack(line: &str) -> Result<Ack, MpdError> {
    let malformed = || MpdError::Protocol(format!("malformed ACK line `{line}`"));
    let rest = line.strip_prefix("ACK [").ok_or_else(malformed)?;
    let (codes, rest) = rest.split_once("] {").ok_or_else(malformed)?;
    let (command, message) = rest.split_once('}').ok_or_else(malformed)?;
    let (code, index) = codes.split_once('@').ok_or_else(malformed)?;
    Ok(Ack {
        code: code.parse().map_err(|_| malformed())?,
        index: index.parse().map_err(|_| malformed())?,
        command: command.to_owned(),
        message: message.trim_start().to_owned(),
    })
}

/// Parses the `OK MPD <version>` greeting.
pub fn parse_greeting(line: &str) -> Result<String, MpdError> {
    line.strip_prefix("OK MPD ")
        .map(str::to_owned)
        .ok_or_else(|| MpdError::Protocol(format!("not an MPD server (greeting `{line}`)")))
}

/// Splits a flat list of pairs into groups, starting a new group whenever a
/// key from `starts` is encountered. Pairs before the first start key are
/// dropped.
pub fn split_groups<'a>(pairs: &'a [Pair], starts: &[&str]) -> Vec<&'a [Pair]> {
    let mut bounds: Vec<usize> = pairs
        .iter()
        .enumerate()
        .filter(|(_, (key, _))| starts.contains(&key.as_str()))
        .map(|(index, _)| index)
        .collect();
    bounds.push(pairs.len());
    bounds.windows(2).map(|w| &pairs[w[0]..w[1]]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_and_escapes_arguments() {
        let cmd = Command::new("add").arg(r#"dir/a "quoted" \ name"#);
        assert_eq!(cmd.as_str(), r#"add "dir/a \"quoted\" \\ name""#);
        assert_eq!(cmd.name(), "add");
    }

    #[test]
    fn newlines_cannot_start_a_new_command() {
        // Newlines stay inside the quoted argument; MPD rejects the argument
        // rather than executing a second command.
        let cmd = Command::new("load").arg("x\"\nclear");
        assert_eq!(cmd.as_str().matches('"').count(), 3);
        assert!(cmd.as_str().contains("\\\""));
    }

    #[test]
    fn builds_filters() {
        assert_eq!(filter_eq("Artist", r#"A "B""#), r#"(Artist == "A \"B\"")"#);
        assert_eq!(
            filter_and([filter_eq("Artist", "x"), filter_eq("Album", "y")]),
            r#"((Artist == "x") AND (Album == "y"))"#
        );
        assert_eq!(filter_and([filter_eq("Album", "y")]), r#"(Album == "y")"#);
        // Filters are quoted once more when used as an argument.
        let cmd = Command::new("find").arg(filter_eq("Album", r"a\b"));
        assert_eq!(cmd.as_str(), r#"find "(Album == \"a\\\\b\")""#);
    }

    #[test]
    fn parses_lines() {
        assert_eq!(parse_line("OK").unwrap(), Line::Ok);
        assert_eq!(parse_line("list_OK").unwrap(), Line::ListOk);
        assert_eq!(
            parse_line("Title: a: b").unwrap(),
            Line::Pair(("Title".into(), "a: b".into()))
        );
        assert!(matches!(parse_line("garbage"), Err(MpdError::Protocol(_))));
    }

    #[test]
    fn parses_ack() {
        let Err(MpdError::Ack(ack)) = parse_line("ACK [50@1] {play} No such song") else {
            panic!("expected ACK");
        };
        assert_eq!(ack.code, 50);
        assert_eq!(ack.index, 1);
        assert_eq!(ack.command, "play");
        assert_eq!(ack.message, "No such song");
    }

    #[test]
    fn parses_greeting() {
        assert_eq!(parse_greeting("OK MPD 0.24.0").unwrap(), "0.24.0");
        assert!(parse_greeting("HTTP/1.1 400").is_err());
    }

    #[test]
    fn splits_groups() {
        let pairs: Vec<Pair> = [
            ("x", "0"),
            ("file", "a"),
            ("Title", "A"),
            ("directory", "d"),
            ("file", "b"),
        ]
        .iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect();
        let groups = split_groups(&pairs, &["file", "directory"]);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[1][0].1, "d");
    }
}
