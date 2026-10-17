use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Row},
};

use super::{Popup, PopupState, frame_popup};
use crate::{
    action::{Action, Context},
    config::KeyCombo,
    screens::Ctx,
    ui::list::ListView,
};

enum HelpRow {
    Heading(String),
    Binding {
        keys: String,
        description: &'static str,
    },
}

/// Lists the key bindings, starting with the active screen's.
pub struct HelpPopup {
    rows: Vec<HelpRow>,
    list: ListView,
}

impl HelpPopup {
    pub fn new(screen: Context, ctx: &Ctx) -> Self {
        let mut order = vec![screen, Context::Navigation, Context::Global];
        let rest: Vec<Context> = Context::ALL
            .iter()
            .copied()
            .filter(|c| !order.contains(c) && !is_screen_context(*c))
            .collect();
        order.extend(rest);
        let mut rows = Vec::new();
        for context in order {
            let Some(actions) = ctx.config.keymap.bindings().get(&context) else {
                continue;
            };
            let bound: Vec<_> = actions
                .iter()
                .filter(|(_, keys)| !keys.is_empty())
                .collect();
            if bound.is_empty() {
                continue;
            }
            if !rows.is_empty() {
                rows.push(HelpRow::Heading(String::new()));
            }
            rows.push(HelpRow::Heading(heading(context)));
            for (action, keys) in bound {
                rows.push(HelpRow::Binding {
                    keys: keys
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    description: action.description(),
                });
            }
        }
        Self {
            rows,
            list: ListView::default(),
        }
    }
}

fn is_screen_context(context: Context) -> bool {
    !matches!(
        context,
        Context::Global | Context::Navigation | Context::Popup | Context::Input
    )
}

fn heading(context: Context) -> String {
    let name = context.name().replace('_', " ");
    let mut chars = name.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

impl Popup for HelpPopup {
    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let inner = frame_popup(
            frame,
            area,
            70,
            area.height.saturating_sub(4),
            "Key bindings",
            ctx,
        );
        let theme = ctx.theme();
        let rows = &self.rows;
        self.list.render(
            frame,
            inner,
            Block::new(),
            None,
            &[Constraint::Length(24), Constraint::Fill(1)],
            rows.len(),
            true,
            theme,
            |i| match &rows[i] {
                HelpRow::Heading(title) => Row::new([Span::styled(title.clone(), theme.accent())]),
                HelpRow::Binding { keys, description } => Row::new([
                    Span::styled(format!("  {keys}"), theme.text()),
                    Span::styled(*description, theme.dim()),
                ]),
            },
        );
    }

    fn handle_key(&mut self, key: KeyCombo, ctx: &mut Ctx) -> PopupState {
        let keymap = &ctx.config.keymap;
        match keymap.resolve(&[Context::Popup, Context::Navigation], key) {
            Some(Action::Close) => PopupState::Closed,
            Some(action) => {
                self.list.navigate(action, self.rows.len());
                PopupState::Open
            }
            None if keymap.resolve(&[Context::Global], key) == Some(Action::Help) => {
                PopupState::Closed
            }
            None => PopupState::Open,
        }
    }
}
