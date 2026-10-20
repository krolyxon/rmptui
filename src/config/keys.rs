//! Key specifications (`Ctrl+x`, `Space`, `F5`, …) and the context keymap.

use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    str::FromStr,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Deserializer, de};

use crate::action::{Action, Context};

/// A key with modifiers, normalized so that lookups ignore how terminals
/// report shifted characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyCombo {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        let mut modifiers =
            modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT);
        let code = match code {
            // `Shift+a` and `A` are the same key; the case carries the shift.
            KeyCode::Char(c) if modifiers.contains(KeyModifiers::SHIFT) => {
                modifiers.remove(KeyModifiers::SHIFT);
                KeyCode::Char(c.to_uppercase().next().unwrap_or(c))
            }
            KeyCode::Tab if modifiers.contains(KeyModifiers::SHIFT) => {
                modifiers.remove(KeyModifiers::SHIFT);
                KeyCode::BackTab
            }
            KeyCode::BackTab => {
                modifiers.remove(KeyModifiers::SHIFT);
                KeyCode::BackTab
            }
            other => other,
        };
        Self { code, modifiers }
    }

    /// Whether this is a printable character without Ctrl/Alt, i.e. text.
    pub fn as_text(&self) -> Option<char> {
        match self.code {
            KeyCode::Char(c)
                if !self
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                Some(c)
            }
            _ => None,
        }
    }
}

impl From<KeyEvent> for KeyCombo {
    fn from(event: KeyEvent) -> Self {
        Self::new(event.code, event.modifiers)
    }
}

impl FromStr for KeyCombo {
    type Err = String;

    fn from_str(spec: &str) -> Result<Self, Self::Err> {
        let mut modifiers = KeyModifiers::NONE;
        let mut rest = spec;
        // A lone `+` (or a trailing one, as in `Ctrl++`) is the plus key.
        while let Some((modifier, tail)) = rest.split_once('+').filter(|(m, _)| !m.is_empty()) {
            match modifier.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "alt" | "meta" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                _ => return Err(format!("unknown modifier `{modifier}` in `{spec}`")),
            }
            rest = tail;
        }
        if rest.is_empty() {
            return Err(format!("missing key in `{spec}`"));
        }
        let lower = rest.to_ascii_lowercase();
        let code = match lower.as_str() {
            "esc" | "escape" => KeyCode::Esc,
            "enter" | "return" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "backspace" => KeyCode::Backspace,
            "delete" | "del" => KeyCode::Delete,
            "insert" | "ins" => KeyCode::Insert,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" | "pgup" => KeyCode::PageUp,
            "pagedown" | "pgdn" => KeyCode::PageDown,
            "space" => KeyCode::Char(' '),
            _ => {
                if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
                    if (1..=24).contains(&n) {
                        KeyCode::F(n)
                    } else {
                        return Err(format!("unknown key `{rest}` in `{spec}`"));
                    }
                } else {
                    let mut chars = rest.chars();
                    match (chars.next(), chars.next()) {
                        (Some(c), None) => KeyCode::Char(c),
                        _ => return Err(format!("unknown key `{rest}` in `{spec}`")),
                    }
                }
            }
        };
        Ok(Self::new(code, modifiers))
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            f.write_str("Ctrl+")?;
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            f.write_str("Alt+")?;
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            f.write_str("Shift+")?;
        }
        match self.code {
            KeyCode::Char(' ') => f.write_str("Space"),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::F(n) => write!(f, "F{n}"),
            KeyCode::BackTab => f.write_str("Shift+Tab"),
            KeyCode::PageUp => f.write_str("PageUp"),
            KeyCode::PageDown => f.write_str("PageDown"),
            other => write!(f, "{other:?}"),
        }
    }
}

impl<'de> Deserialize<'de> for KeyCombo {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(de::Error::custom)
    }
}

/// The `[keys.*]` tables as written in the config file.
pub type Bindings = BTreeMap<Context, BTreeMap<Action, Vec<KeyCombo>>>;

/// Resolved bindings: one lookup table per context.
#[derive(Clone, Debug, Default)]
pub struct Keymap {
    contexts: HashMap<Context, HashMap<KeyCombo, Action>>,
    bindings: Bindings,
}

impl Keymap {
    /// Builds the keymap, rejecting actions bound in the wrong context and
    /// keys bound twice within one context.
    pub fn new(bindings: Bindings) -> Result<Self, String> {
        let mut contexts: HashMap<Context, HashMap<KeyCombo, Action>> = HashMap::new();
        for (&context, actions) in &bindings {
            let table = contexts.entry(context).or_default();
            for (&action, keys) in actions {
                if !action.contexts().contains(&context) {
                    let allowed: Vec<_> = action.contexts().iter().map(|c| c.name()).collect();
                    return Err(format!(
                        "action `{action}` cannot be bound in [keys.{context}] (allowed in: {})",
                        allowed.join(", ")
                    ));
                }
                for &key in keys {
                    if let Some(previous) = table.insert(key, action)
                        && previous != action
                    {
                        return Err(format!(
                            "key `{key}` is bound to both `{previous}` and `{action}` in [keys.{context}]; \
                             unbind one of them (an action can be disabled with `action = []`)"
                        ));
                    }
                }
            }
        }
        Ok(Self { contexts, bindings })
    }

    /// Finds the action for `key`, searching `contexts` in order.
    pub fn resolve(&self, contexts: &[Context], key: KeyCombo) -> Option<Action> {
        contexts
            .iter()
            .find_map(|context| self.contexts.get(context)?.get(&key).copied())
    }

    /// The keys bound to `action` in `context`, for help texts.
    pub fn keys_for(&self, context: Context, action: Action) -> &[KeyCombo] {
        self.bindings
            .get(&context)
            .and_then(|actions| actions.get(&action))
            .map_or(&[], Vec::as_slice)
    }

    pub fn bindings(&self) -> &Bindings {
        &self.bindings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(spec: &str) -> KeyCombo {
        spec.parse().unwrap()
    }

    #[test]
    fn parses_specs() {
        assert_eq!(
            key("q"),
            KeyCombo::new(KeyCode::Char('q'), KeyModifiers::NONE)
        );
        assert_eq!(
            key("Ctrl+c"),
            KeyCombo::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
        );
        assert_eq!(
            key("+"),
            KeyCombo::new(KeyCode::Char('+'), KeyModifiers::NONE)
        );
        assert_eq!(
            key("Ctrl++"),
            KeyCombo::new(KeyCode::Char('+'), KeyModifiers::CONTROL)
        );
        assert_eq!(key("Space").code, KeyCode::Char(' '));
        assert_eq!(key("F5").code, KeyCode::F(5));
        assert_eq!(key("Shift+Tab").code, KeyCode::BackTab);
        assert_eq!(key("Shift+g"), key("G"));
        assert!("Hyper+x".parse::<KeyCombo>().is_err());
        assert!("abc".parse::<KeyCombo>().is_err());
        assert!("F99".parse::<KeyCombo>().is_err());
    }

    #[test]
    fn normalizes_terminal_events() {
        // Terminals report `G` with or without SHIFT.
        let shifted = KeyCombo::from(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert_eq!(shifted, key("G"));
        let backtab = KeyCombo::from(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(backtab, key("Shift+Tab"));
    }

    #[test]
    fn displays_specs() {
        for spec in ["q", "Ctrl+c", "Space", "F5", "Enter", "Shift+Tab", "PageUp"] {
            assert_eq!(key(spec).to_string(), spec);
        }
    }

    #[test]
    fn resolves_by_context_priority() {
        let mut bindings = Bindings::new();
        bindings
            .entry(Context::Global)
            .or_default()
            .insert(Action::ToggleConsume, vec![key("R")]);
        bindings
            .entry(Context::Playlists)
            .or_default()
            .insert(Action::RenamePlaylist, vec![key("R")]);
        let keymap = Keymap::new(bindings).unwrap();
        assert_eq!(
            keymap.resolve(&[Context::Playlists, Context::Global], key("R")),
            Some(Action::RenamePlaylist)
        );
        assert_eq!(
            keymap.resolve(&[Context::Queue, Context::Global], key("R")),
            Some(Action::ToggleConsume)
        );
    }

    #[test]
    fn rejects_conflicts_and_misplaced_actions() {
        let mut bindings = Bindings::new();
        let global = bindings.entry(Context::Global).or_default();
        global.insert(Action::Quit, vec![key("q")]);
        global.insert(Action::Pause, vec![key("q")]);
        assert!(Keymap::new(bindings).unwrap_err().contains("bound to both"));

        let mut bindings = Bindings::new();
        bindings
            .entry(Context::Queue)
            .or_default()
            .insert(Action::Quit, vec![key("q")]);
        assert!(
            Keymap::new(bindings)
                .unwrap_err()
                .contains("cannot be bound")
        );
    }
}
