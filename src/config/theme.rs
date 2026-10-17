use std::str::FromStr;

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Deserializer, de};

/// Colors used throughout the UI.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    #[serde(deserialize_with = "color")]
    pub text: Color,
    #[serde(deserialize_with = "color")]
    pub dim: Color,
    #[serde(deserialize_with = "color")]
    pub border: Color,
    #[serde(deserialize_with = "color")]
    pub selection_fg: Color,
    #[serde(deserialize_with = "color")]
    pub selection_bg: Color,
    #[serde(deserialize_with = "color")]
    pub playing: Color,
    #[serde(deserialize_with = "color")]
    pub marked: Color,
    #[serde(deserialize_with = "color")]
    pub directory: Color,
    #[serde(deserialize_with = "color")]
    pub error: Color,
    #[serde(deserialize_with = "color")]
    pub artist: Color,
    #[serde(deserialize_with = "color")]
    pub track: Color,
    #[serde(deserialize_with = "color")]
    pub album: Color,
    #[serde(deserialize_with = "color")]
    pub duration: Color,
    #[serde(deserialize_with = "color", rename = "progress")]
    pub progress_filled: Color,
    #[serde(deserialize_with = "colors")]
    pub visualizer: Vec<Color>,
}

impl Theme {
    pub fn text(&self) -> Style {
        Style::new().fg(self.text)
    }

    pub fn dim(&self) -> Style {
        Style::new().fg(self.dim)
    }

    pub fn border(&self) -> Style {
        Style::new().fg(self.border)
    }

    /// Titles and column headers.
    pub fn header(&self) -> Style {
        self.text().add_modifier(Modifier::BOLD)
    }

    /// The selected row of the focused list.
    pub fn selection(&self) -> Style {
        if self.selection_fg == Color::Reset && self.selection_bg == Color::Reset {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new().fg(self.selection_fg).bg(self.selection_bg)
        }
    }

    /// The selected row of a list without focus.
    pub fn selection_inactive(&self) -> Style {
        self.selection().add_modifier(Modifier::DIM)
    }

    pub fn playing(&self) -> Style {
        Style::new().fg(self.playing).add_modifier(Modifier::BOLD)
    }

    pub fn marked(&self) -> Style {
        Style::new().fg(self.marked)
    }

    pub fn directory(&self) -> Style {
        Style::new().fg(self.directory)
    }

    pub fn error(&self) -> Style {
        Style::new().fg(self.error)
    }
}

fn parse_color<E: de::Error>(value: &str) -> Result<Color, E> {
    Color::from_str(value).map_err(|_| E::custom(format!("invalid color `{value}`")))
}

fn color<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Color, D::Error> {
    parse_color(&String::deserialize(deserializer)?)
}

fn colors<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Color>, D::Error> {
    Vec::<String>::deserialize(deserializer)?
        .iter()
        .map(|value| parse_color(value))
        .collect()
}
