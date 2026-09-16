//! Drawing the panel, in the shape tsui taught: a wordmark and a status badge
//! across the top, a menu of sections down the left with each one's value
//! beside it, the selected one opening a pane to its right, and a status bar
//! along the bottom.
//!
//! Nothing here decides anything. Which sections exist, which rows they hold,
//! what each row says and what it does is `panel_spec`'s answer; this arranges
//! it.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use tailgauge_core::panel::PanelRow;

use super::App;

// tsui's palette, by index, so both agree on a terminal that has opinions
// about what "magenta" means.
const PRIMARY: Color = Color::Indexed(207);
const RED: Color = Color::Indexed(203);
const BLUE: Color = Color::Indexed(39);
const GREEN: Color = Color::Indexed(40);
const YELLOW: Color = Color::Indexed(214);
const DARK_GRAY: Color = Color::Indexed(237);
const BLACK: Color = Color::Indexed(16);

/// Label and value share this, and the arrow sits after it, so every arrow in
/// the menu lines up.
const MENU_WIDTH: usize = 35;

/// Where a row's second column starts. tsui right-aligns its values, which
/// works when they are all "Windows" or "8ms"; a row here carries an address
/// and an owner, and right-aligning those leaves the column ragged down its
/// left edge. So they begin together instead.
const VALUE_COLUMN: usize = 26;

const WORDMARK: [&str; 2] = [
    "▀█▀ ▄▀█ █ █   █▀▀ ▄▀█ █ █ █▀▀ █▀▀",
    " █  █▀█ █ █▄▄ █▄█ █▀█ █▄█ █▄█ █▄▄",
];

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

pub fn draw(frame: &mut Frame, app: &App) {
    let areas = Layout::vertical([
        Constraint::Length(3), // wordmark, byline, status
        Constraint::Length(2), // breathing room
        Constraint::Min(3),    // menu and pane
        Constraint::Length(1), // status bar
    ])
    .split(frame.area());

    header(frame, areas[0], app);
    if app.landed {
        body(frame, areas[2], app);
    } else {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("  Reading the machine…", dim()))),
            areas[2],
        );
    }
    status_bar(frame, areas[3], app);
}

// ---------------------------------------------------------------------------
// the top
// ---------------------------------------------------------------------------

fn header(frame: &mut Frame, area: Rect, app: &App) {
    let mark = cells(WORDMARK[0]) as u16 + 4;
    let versions = 24;
    // A narrow terminal drops the decoration before it drops the information:
    // the wordmark goes first, the versions second, and the status stays.
    let columns = if area.width >= mark + 50 + versions {
        Layout::horizontal([
            Constraint::Length(mark),
            Constraint::Min(24),
            Constraint::Length(versions),
        ])
        .split(area)
        .to_vec()
    } else if area.width >= 50 + versions {
        let split =
            Layout::horizontal([Constraint::Min(24), Constraint::Length(versions)]).split(area);
        vec![Rect::new(area.x, area.y, 0, 0), split[0], split[1]]
    } else {
        vec![
            Rect::new(area.x, area.y, 0, 0),
            area,
            Rect::new(area.x, area.y, 0, 0),
        ]
    };

    if columns[0].width > 0 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(WORDMARK[0], Style::default().fg(PRIMARY))),
                Line::from(Span::styled(WORDMARK[1], Style::default().fg(PRIMARY))),
                Line::from(Span::styled("by arzaroth", dim().fg(PRIMARY)))
                    .alignment(Alignment::Center),
            ]),
            columns[0],
        );
    }

    let h = &app.panel.header;
    let (badge, colour) = if !app.landed {
        ("Loading...".to_string(), BLUE)
    } else if h.warning && !h.meta.is_empty() {
        (h.meta.clone(), YELLOW)
    } else if h.toggle_checked {
        match app.current_exit_node() {
            Some(node) => (format!("Connected - {node}"), GREEN),
            None => ("Connected".to_string(), GREEN),
        }
    } else {
        ("Not Connected".to_string(), RED)
    };

    let hint = if !app.landed {
        ""
    } else if h.toggle_checked {
        " (press . to disconnect)"
    } else {
        " (press . to connect)"
    };

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::raw("Status: "),
                Span::styled(format!(" {badge} "), Style::default().bg(colour).fg(BLACK)),
                Span::styled(hint, dim()),
            ]),
            Line::from(Span::styled(
                match app.account() {
                    account if account.is_empty() => "--".to_string(),
                    account => account,
                },
                dim(),
            )),
        ]),
        columns[1],
    );

    if columns[2].width > 0 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(format!("tailgauge: {}", app.ui.version)),
                Line::from(format!("provider:  {}", app.provider_label())),
            ])
            .style(dim()),
            columns[2],
        );
    }
}

// ---------------------------------------------------------------------------
// the menu and the pane it opens
// ---------------------------------------------------------------------------

fn body(frame: &mut Frame, area: Rect, app: &App) {
    let columns = Layout::horizontal([
        Constraint::Length(MENU_WIDTH as u16 + 3),
        Constraint::Length(2), // gutter, not a rule: tsui draws no line here
        Constraint::Min(20),
    ])
    .split(area);
    menu(frame, columns[0], app);
    pane(frame, columns[2], app);
}

/// How wide a string is on a terminal, which is not how many `char`s it has:
/// a CJK hostname is two cells per character and a combining mark is none, and
/// counting characters puts the column somewhere else for both.
fn cells(text: &str) -> usize {
    Line::from(text).width()
}

/// `left` padded so `right` starts at `column`, with a single space between
/// them when the label runs past it.
fn split(left: &str, right: &str, column: usize) -> String {
    if right.is_empty() {
        return left.to_string();
    }
    let gap = column.saturating_sub(cells(left)).max(1);
    format!("{left}{}{right}", " ".repeat(gap))
}

fn menu(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .menu()
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let selected = index == app.section;
            let style = if selected && app.open {
                Style::default().bg(DARK_GRAY)
            } else if selected {
                Style::default().bg(PRIMARY).fg(BLACK)
            } else if app.open {
                dim()
            } else {
                Style::default()
            };
            // The value is muted unless the whole row is, in which case the
            // highlight already carries it.
            let value_style = if selected && !app.open {
                style
            } else {
                style.add_modifier(Modifier::DIM)
            };

            let label = format!(" {}", section.title);
            let value = app.section_value(section);
            let gap = MENU_WIDTH.saturating_sub(cells(&label) + cells(&value));
            ListItem::new(Line::from(vec![
                Span::styled(label, style),
                Span::styled(" ".repeat(gap), style),
                Span::styled(value, value_style),
                Span::styled(" > ", style),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items), area);
}

fn pane(frame: &mut Frame, area: Rect, app: &App) {
    let menu = app.menu();
    let Some(section) = menu.get(app.section) else {
        return;
    };
    let rows = app.pane_rows();

    if rows.is_empty() {
        let empty = if section.empty.is_empty() {
            "Nothing here."
        } else {
            section.empty.as_str()
        };
        frame.render_widget(Paragraph::new(Line::from(Span::styled(empty, dim()))), area);
        return;
    }

    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| ListItem::new(line(row, app)))
        .collect();
    let mut state = ListState::default();
    if app.open {
        state.select(Some(app.row.min(rows.len().saturating_sub(1))));
    }

    frame.render_stateful_widget(
        List::new(items).highlight_style(Style::default().bg(PRIMARY).fg(BLACK)),
        area,
        &mut state,
    );
}

fn line<'a>(row: &'a PanelRow, app: &App) -> Line<'a> {
    // A row being typed into shows the query rather than its placeholder.
    if !row.search_placeholder.is_empty() {
        let scope = App::field_scope(row).unwrap_or_default();
        let query = app.raw_query(scope);
        let editing = app.editing.as_deref() == Some(scope);
        return Line::from(Span::styled(
            if query.is_empty() && !editing {
                format!(" {}", row.search_placeholder)
            } else {
                format!(" /{query}{}", if editing { "█" } else { "" })
            },
            if editing {
                Style::default().fg(PRIMARY)
            } else {
                dim()
            },
        ));
    }

    // A caption over a group, brighter than what it labels.
    if row.id.starts_with("caption:") {
        return Line::from(Span::styled(format!(" {}", row.label), Style::default()));
    }
    if row.id == "blank" {
        return Line::from("");
    }

    // tsui marks the one in use with an asterisk and indents the rest by the
    // width of it, so the labels stay in one column.
    let label = format!("{}{}", if row.current { "*" } else { " " }, row.label);
    let text = split(&label, &row.sublabel, VALUE_COLUMN);
    let offline = row.kind == "peer"
        && row
            .payload
            .get("Online")
            .and_then(|v| v.as_bool())
            .is_some_and(|online| !online);
    let style = if !row.navigable || offline {
        dim()
    } else if row.current {
        Style::default().fg(PRIMARY)
    } else {
        Style::default()
    };
    if row.busy {
        return Line::from(vec![
            Span::styled(text, style),
            Span::styled("  …", Style::default().fg(YELLOW)),
        ]);
    }
    Line::from(Span::styled(text, style))
}

// ---------------------------------------------------------------------------
// the bottom
// ---------------------------------------------------------------------------

fn status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let quit = "press q to quit";
    let columns = Layout::horizontal([
        Constraint::Min(10),
        Constraint::Length(cells(quit) as u16 + 1),
    ])
    .split(area);

    let middle = if !app.message.is_empty() {
        Line::from(vec![
            Span::styled(
                "Error: ",
                Style::default().fg(RED).add_modifier(Modifier::BOLD),
            ),
            Span::styled(app.message.clone(), Style::default().fg(RED)),
        ])
    } else if app.working {
        Line::from(Span::styled("Working…", Style::default().fg(YELLOW)))
    } else if app.editing.is_some() {
        Line::from(Span::styled(
            "type to filter · enter to keep · esc to clear",
            dim(),
        ))
    } else {
        Line::from(Span::styled(
            app.panel.status.text.clone(),
            match app.panel.status.tone.as_str() {
                "warning" => Style::default().fg(YELLOW),
                "error" => Style::default().fg(RED),
                _ => dim(),
            },
        ))
    };

    frame.render_widget(
        Paragraph::new(middle).alignment(Alignment::Center),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(quit, dim()))),
        columns[1],
    );
}
