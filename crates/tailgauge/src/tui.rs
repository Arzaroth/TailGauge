//! `tailgauge tui` - the panel, in a terminal.
//!
//! The same panel the three desktop frontends draw: `panel_spec` decides the
//! rows, their order and what each one does, and this draws them. What it adds
//! over the widgets is that a terminal has a keyboard, so every row is
//! reachable without a pointer.
//!
//! Gathering and acting both block - a daemon call takes tens of milliseconds
//! and `up` can take seconds - so both happen on a worker thread and the draw
//! loop never waits on either.

mod render;

use std::io;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tailgauge_core::panel::{Panel, PanelRow, panel_spec};
use tailgauge_core::panel_state::PanelState;

use crate::gather::{self, Ui};

/// How often the panel is re-read when nothing is happening. The desktop
/// widgets poll at three seconds with the menu open, and a terminal that is
/// open is a menu that is open.
const TICK: Duration = Duration::from_secs(3);

/// After a command, the daemon needs a moment before it has anything new to
/// say. Asking immediately reports the state the command was about to change.
const SETTLE: Duration = Duration::from_millis(600);

pub fn run() -> Result<()> {
    // Everything this runs from here on has to keep its output to itself.
    crate::ctl::take_over_terminal();
    let mut terminal = enter()?;
    let outcome = App::new().run(&mut terminal);
    leave(&mut terminal)?;
    outcome
}

type Screen = Terminal<CrosstermBackend<io::Stdout>>;

fn enter() -> Result<Screen> {
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(out))?)
}

/// Always runs, including on the way out of an error: a process that dies in
/// raw mode leaves the shell without an echo.
fn leave(terminal: &mut Screen) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// the worker
// ---------------------------------------------------------------------------

enum Job {
    /// Boxed because it is an order of magnitude larger than the others, and
    /// every job in the channel would otherwise be that size.
    Gather(Box<(Ui, Options)>),
    Act(Act, String),
    Stop,
}

/// What only this side knows, handed to `panel_spec` with the gathered state.
#[derive(Clone, Default)]
struct Options {
    expanded_peer_id: String,
    mullvad_picker_open: bool,
    recent_regions: Vec<String>,
}

enum Act {
    Toggle,
    SwitchAccount(String),
    /// The node to route through, or "off" to stop. Never `None`: that is
    /// `ctl`'s query path, which prints the current node to stdout.
    SetExitNode(String),
    SelectNetwork {
        id: String,
        join: bool,
    },
    Authorize,
    Update,
    Copy(String),
}

enum Done {
    Panel(Box<PanelState>, Options),
    Acted(Option<String>),
}

fn worker(jobs: Receiver<Job>, out: Sender<Done>) {
    while let Ok(job) = jobs.recv() {
        match job {
            Job::Stop => return,
            Job::Gather(what) => {
                let (ui, options) = *what;
                let state = gather::panel_state(&ui);
                if out.send(Done::Panel(Box::new(state), options)).is_err() {
                    return;
                }
            }
            Job::Act(act, provider) => {
                let failed = perform(act, &provider);
                if out.send(Done::Acted(failed)).is_err() {
                    return;
                }
            }
        }
    }
}

/// Runs the command against the provider the panel is showing, and answers
/// with what went wrong, or nothing.
fn perform(act: Act, provider_id: &str) -> Option<String> {
    use tailgauge_core::providers;
    if let Act::Copy(text) = act {
        return crate::copy::run(&text).err().map(|e| e.to_string());
    }
    if let Act::Update = act {
        return match crate::update::apply(&crate::project::TAILGAUGE, &cache()) {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };
    }

    // The one being shown, not the first one installed. Taking the first sent
    // every command to Tailscale on a machine that has both, so switching to
    // NetBird and connecting drove the wrong daemon and reported nothing.
    let provider = match providers::provider_by_id(provider_id) {
        Some(provider) => provider,
        None => match providers::PROVIDERS
            .iter()
            .find(|p| selvedge::proc::has(p.cli))
        {
            Some(provider) => provider,
            None => return Some("no VPN CLI on PATH".into()),
        },
    };
    if !selvedge::proc::has(provider.cli) {
        return Some(format!("{} is not on PATH", provider.cli));
    }

    let outcome = match act {
        Act::Toggle => crate::ctl::toggle(provider),
        Act::SwitchAccount(id) => crate::ctl::switch_account(provider, &id),
        Act::SetExitNode(target) => crate::ctl::exit_node(provider, Some(target.as_str())),
        Act::SelectNetwork { id, join } => crate::ctl::select_network(provider, &id, join),
        Act::Authorize => crate::ctl::authorize(provider),
        Act::Copy(_) | Act::Update => unreachable!("handled above"),
    };
    match outcome {
        crate::ctl::Outcome::Failed(why) => Some(why),
        _ => None,
    }
}

fn cache() -> std::path::PathBuf {
    selvedge::state::update_cache_file(&crate::project::TAILGAUGE)
}

// ---------------------------------------------------------------------------
// the app
// ---------------------------------------------------------------------------

struct App {
    ui: Ui,
    options: Options,
    panel: Panel,
    /// Which section of the menu is under the cursor, and where the cursor
    /// sits inside it once the pane is open.
    section: usize,
    row: usize,
    open: bool,
    /// Scope being typed into, and the query per scope.
    editing: Option<String>,
    queries: Vec<(String, String)>,
    message: String,
    working: bool,
    /// Until the first gather lands there is nothing to say. The empty state
    /// resolves to "no supported VPN CLI", which is a claim about the machine
    /// that nothing has looked at yet.
    landed: bool,
    quit: bool,
    next_gather: Instant,
}

impl App {
    fn new() -> Self {
        let ui = Ui {
            version: env!("CARGO_PKG_VERSION").into(),
            ..Ui::default()
        };
        let options = Options::default();
        App {
            panel: panel_spec(&PanelState::default(), &resolve(&ui, &options)),
            ui,
            options,
            section: 0,
            row: 0,
            open: false,
            editing: None,
            queries: Vec::new(),
            message: String::new(),
            working: false,
            landed: false,
            quit: false,
            next_gather: Instant::now(),
        }
    }

    fn run(mut self, terminal: &mut Screen) -> Result<()> {
        let (jobs, job_rx) = channel();
        let (done_tx, done) = channel();
        let hand = thread::spawn(move || worker(job_rx, done_tx));

        self.ask(&jobs);
        while !self.quit {
            terminal.draw(|f| render::draw(f, &self))?;

            if event::poll(Duration::from_millis(120))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.key(key, &jobs);
            }

            loop {
                match done.try_recv() {
                    Ok(Done::Panel(state, options)) => self.landed(*state, options),
                    Ok(Done::Acted(failed)) => {
                        self.working = false;
                        self.message = failed.unwrap_or_default();
                        self.next_gather = Instant::now() + SETTLE;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.quit = true;
                        break;
                    }
                }
            }

            if Instant::now() >= self.next_gather {
                self.ask(&jobs);
            }
        }

        let _ = jobs.send(Job::Stop);
        let _ = hand.join();
        Ok(())
    }

    fn ask(&mut self, jobs: &Sender<Job>) {
        self.next_gather = Instant::now() + TICK;
        let _ = jobs.send(Job::Gather(Box::new((
            self.ui.clone(),
            self.options.clone(),
        ))));
    }

    /// A fresh panel replaces the old one. The menu keeps the section it was
    /// on by name and the pane the row by id, because both come and go on
    /// every poll: a machine dropping off the tailnet must not move the
    /// cursor onto something else.
    fn landed(&mut self, state: PanelState, options: Options) {
        let section_id = self.menu().get(self.section).map(|s| s.id.clone());
        let row_id = self.pane_rows().get(self.row).map(|r| r.id.clone());

        self.landed = true;
        self.panel = panel_spec(&state, &resolve(&self.ui, &options));
        // The daemon caught up with an optimistic toggle, so stop overriding.
        if self
            .ui
            .active
            .is_some_and(|a| a == self.panel.header.toggle_checked)
        {
            self.ui.active = None;
        }

        self.section = section_id
            .and_then(|id| self.menu().iter().position(|s| s.id == id))
            .unwrap_or(0)
            .min(self.menu().len().saturating_sub(1));
        self.row = row_id
            .and_then(|id| self.pane_rows().iter().position(|r| r.id == id))
            .unwrap_or(self.row)
            .min(self.pane_rows().len().saturating_sub(1));
    }

    // ---- what is on screen ------------------------------------------------

    fn query(&self, scope: &str) -> String {
        self.queries
            .iter()
            .find(|(s, _)| s == scope)
            .map(|(_, q)| q.trim().to_lowercase())
            .unwrap_or_default()
    }

    /// The rule the desktop frontends follow, and `parity.rs` forbids
    /// reinventing: a row with a scope is drawn when the query is a substring
    /// of its `searchKey`, and the one row per scope with no key is the
    /// message shown when nothing matched.
    fn matches(&self, row: &PanelRow) -> bool {
        let query = self.query(&row.search_scope);
        query.is_empty() || row.search_key.contains(&query)
    }

    fn shown(&self, row: &PanelRow) -> bool {
        if row.search_scope.is_empty() {
            return true;
        }
        if !row.search_key.is_empty() {
            return self.matches(row);
        }
        let scope = &row.search_scope;
        !self.query(scope).is_empty()
            && !self
                .rows()
                .any(|r| r.search_scope == *scope && !r.search_key.is_empty() && self.matches(r))
    }

    fn rows(&self) -> impl Iterator<Item = &PanelRow> {
        self.panel
            .sections
            .iter()
            .filter(|s| s.visible)
            .flat_map(|s| s.rows.iter())
            .flat_map(|r| std::iter::once(r).chain(r.children.iter()))
    }

    /// The sections the menu offers: the ones the panel is drawing that have a
    /// name to put in a list.
    pub(super) fn menu(&self) -> Vec<&tailgauge_core::panel::PanelSection> {
        self.panel
            .sections
            .iter()
            .filter(|s| s.visible && !s.title.is_empty())
            .collect()
    }

    /// What the selected section holds, after any live search.
    ///
    /// Owned rather than borrowed because this composes: the panel answers a
    /// widget's menu, where "This device" is one line you click to copy, and a
    /// pane with a screenful of room wants the name, the addresses and what
    /// the link is doing. Everything added here is built out of the row's own
    /// payload, so nothing is invented - but it is this frontend's view, and
    /// the other three do not have it.
    pub(super) fn pane_rows(&self) -> Vec<PanelRow> {
        let Some(section) = self.menu().get(self.section).copied() else {
            return Vec::new();
        };
        let rows: Vec<PanelRow> = section
            .rows
            .iter()
            .flat_map(|r| std::iter::once(r).chain(r.children.iter().filter(|_| r.expanded)))
            .filter(|r| self.shown(r))
            .cloned()
            .collect();

        match section.id.as_str() {
            "self" => Self::device_rows(&rows),
            "exitNodes" => Self::exit_node_rows(&rows),
            "machines" => Self::grouped_by_owner(rows),
            _ => rows,
        }
    }

    /// A caption, and the value under it: not something to land on.
    fn caption(label: &str) -> PanelRow {
        PanelRow {
            id: format!("caption:{label}"),
            label: label.to_string(),
            navigable: false,
            ..PanelRow::default()
        }
    }

    fn text(label: &str, value: &str) -> PanelRow {
        PanelRow {
            id: format!("text:{label}"),
            label: label.to_string(),
            sublabel: value.to_string(),
            navigable: false,
            ..PanelRow::default()
        }
    }

    fn blank() -> PanelRow {
        PanelRow {
            id: "blank".into(),
            navigable: false,
            ..PanelRow::default()
        }
    }

    /// The machine this is running on, opened out. The panel hands one row
    /// with a name and a subtitle; the peer behind it carries the rest.
    fn device_rows(rows: &[PanelRow]) -> Vec<PanelRow> {
        let Some(row) = rows.first() else {
            return Vec::new();
        };
        let Ok(peer) = serde_json::from_value::<tailgauge_core::Peer>(row.payload.clone()) else {
            return rows.to_vec();
        };

        let mut out = vec![Self::caption("Name")];
        out.push(Self::text(&tailgauge_core::panel::peer_address(&peer), ""));

        let addresses: Vec<&String> = peer.ipv4.iter().chain(peer.ipv6.iter()).collect();
        if !addresses.is_empty() {
            out.push(Self::blank());
            out.push(Self::caption("IPs"));
            for address in addresses {
                out.push(Self::text(address, ""));
            }
        }

        let details = tailgauge_core::panel::peer_detail_rows(&peer, selvedge::state::now_ms());
        if !details.is_empty() {
            out.push(Self::blank());
            out.push(Self::caption("Link"));
            out.extend(details);
        }

        // The row the panel gave, kept last and still actionable: it is the
        // one that copies.
        out.push(Self::blank());
        out.push(row.clone());
        out
    }

    /// tsui opens with the node in use, or with None when there is none, so
    /// that turning one off is a row rather than a thing you have to know.
    fn exit_node_rows(rows: &[PanelRow]) -> Vec<PanelRow> {
        let using = rows.iter().any(|r| r.current && r.kind == "exitNode");
        let mut out = vec![PanelRow {
            id: "exitNode:none".into(),
            kind: "exitNode".into(),
            label: "None".into(),
            action: "clearExitNode".into(),
            current: !using,
            ..PanelRow::default()
        }];
        out.push(Self::blank());
        out.extend(rows.iter().cloned());
        out
    }

    /// tsui lists devices under the person who owns them, with what each one
    /// runs beside it. The owner and the OS are both on the peer the row
    /// carries, so this is a sort, a caption and a different second column.
    fn grouped_by_owner(rows: Vec<PanelRow>) -> Vec<PanelRow> {
        fn peer_of(row: &PanelRow) -> Option<tailgauge_core::Peer> {
            serde_json::from_value(row.payload.clone()).ok()
        }
        fn owner(row: &PanelRow) -> String {
            peer_of(row).and_then(|p| p.user_name).unwrap_or_default()
        }

        let (mut peers, others): (Vec<PanelRow>, Vec<PanelRow>) =
            rows.into_iter().partition(|r| r.kind == "peer");
        // The panel sorts by name, which is what a flat list wants. Grouped,
        // it has to be by owner first or a person appears twice.
        peers.sort_by_key(|r| (owner(r).to_lowercase(), r.label.to_lowercase()));

        let mut out = others;
        let mut last = String::new();
        let mut started = false;
        for mut row in peers {
            let who = owner(&row);
            if who != last || !started {
                if started {
                    out.push(Self::blank());
                }
                last = who.clone();
                started = true;
                out.push(Self::caption(if who.is_empty() {
                    "Other devices"
                } else {
                    who.as_str()
                }));
            }
            // The owner is the caption now, so the column beside the name says
            // what the machine runs instead of repeating it.
            if let Some(peer) = peer_of(&row) {
                row.sublabel = os_label(&peer.os);
            }
            out.push(row);
        }
        out
    }

    /// The muted value beside a section's name, the way tsui summarises a
    /// submenu without opening it.
    pub(super) fn section_value(&self, section: &tailgauge_core::panel::PanelSection) -> String {
        let current = section.rows.iter().find(|r| r.current);
        match section.id.as_str() {
            "exitNodes" => current
                .map(|r| r.label.clone())
                .unwrap_or_else(|| "None".into()),
            "providers" => current.map(|r| r.label.clone()).unwrap_or_default(),
            "machines" => {
                let shown = self
                    .pane_rows_of(section)
                    .iter()
                    .filter(|r| r.kind == "peer")
                    .count();
                format!("{shown} visible")
            }
            "networks" => {
                let joined = section.rows.iter().filter(|r| r.current).count();
                format!("{joined} joined")
            }
            // Not a choice between rows, so the summary is what it is about:
            // the machine this is running on.
            "self" => section
                .rows
                .first()
                .map(|r| r.label.clone())
                .unwrap_or_default(),
            "connections" => section
                .rows
                .first()
                .map(|r| r.label.clone())
                .unwrap_or_default(),
            _ => current.map(|r| r.label.clone()).unwrap_or_default(),
        }
    }

    fn pane_rows_of<'a>(
        &'a self,
        section: &'a tailgauge_core::panel::PanelSection,
    ) -> Vec<&'a PanelRow> {
        section.rows.iter().filter(|r| self.shown(r)).collect()
    }

    /// The exit node in use, for the status badge.
    pub(super) fn current_exit_node(&self) -> Option<String> {
        self.panel
            .sections
            .iter()
            .find(|s| s.id == "exitNodes")?
            .rows
            .iter()
            .find(|r| r.current && r.kind == "exitNode")
            .map(|r| r.label.clone())
    }

    /// Who this machine is logged in as.
    pub(super) fn account(&self) -> String {
        self.panel
            .sections
            .iter()
            .find(|s| s.id == "self")
            .and_then(|s| s.rows.first())
            .map(|r| {
                if r.sublabel.is_empty() {
                    r.label.clone()
                } else {
                    r.sublabel.clone()
                }
            })
            .unwrap_or_default()
    }

    pub(super) fn provider_label(&self) -> String {
        self.panel
            .sections
            .iter()
            .find(|s| s.id == "providers")
            .and_then(|s| s.rows.iter().find(|r| r.current))
            .map(|r| r.label.clone())
            .unwrap_or_else(|| "(none)".into())
    }

    fn current(&self) -> Option<PanelRow> {
        self.pane_rows().get(self.row).cloned()
    }

    // ---- keys -------------------------------------------------------------

    fn key(&mut self, key: KeyEvent, jobs: &Sender<Job>) {
        if self.editing.is_some() {
            self.typing(key, jobs);
            return;
        }
        self.message.clear();
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Char('q') => self.quit = true,
            // Esc closes the pane, and only quits when there is none open -
            // the way a menu backs out before it gives up.
            KeyCode::Esc => {
                if self.open {
                    self.open = false;
                } else {
                    self.quit = true;
                }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('s') => self.step(1),
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => self.step(-1),
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('a') => self.open = false,
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('d') => self.enter(),
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.open {
                    self.act(jobs);
                } else {
                    self.enter();
                }
            }
            // tsui's shortcut, and the one thing worth reaching without
            // opening anything.
            KeyCode::Char('.') => self.toggle(jobs),
            KeyCode::Char('r') => self.ask(jobs),
            KeyCode::Char('/') => self.start_search(),
            _ => {}
        }
    }

    /// Open the section under the cursor, if it has anything in it.
    fn enter(&mut self) {
        if self.pane_rows().is_empty() {
            return;
        }
        self.open = true;
        self.row = self.row.min(self.pane_rows().len().saturating_sub(1));
    }

    fn step(&mut self, by: isize) {
        let len = if self.open {
            self.pane_rows().len()
        } else {
            self.menu().len()
        };
        if len == 0 {
            return;
        }
        let slot = if self.open { self.row } else { self.section };
        let mut next = (slot as isize + by).clamp(0, len as isize - 1) as usize;
        if self.open {
            // Captions, blanks and the rows a panel draws but cannot act on
            // are passed over rather than landed on.
            let rows = self.pane_rows();
            let step = if by >= 0 { 1isize } else { -1 };
            while rows
                .get(next)
                .is_some_and(|r| !r.navigable && r.search_placeholder.is_empty())
            {
                let after = next as isize + step;
                if after < 0 || after >= len as isize {
                    break;
                }
                next = after as usize;
            }
            self.row = next;
        } else {
            self.section = next;
            // A different section holds different rows, so the pane's cursor
            // starts at its top rather than wherever the last one left it.
            self.row = 0;
        }
    }

    fn typing(&mut self, key: KeyEvent, jobs: &Sender<Job>) {
        let Some(scope) = self.editing.clone() else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.set_query(&scope, String::new());
                self.editing = None;
            }
            KeyCode::Enter => self.editing = None,
            KeyCode::Backspace => {
                let mut q = self.raw_query(&scope);
                q.pop();
                self.set_query(&scope, q);
            }
            KeyCode::Char(c) => {
                let mut q = self.raw_query(&scope);
                q.push(c);
                self.set_query(&scope, q);
            }
            _ => {
                self.editing = None;
                self.key(key, jobs);
                return;
            }
        }
        // A query that hides the row the cursor was on leaves it past the end.
        self.row = self.row.min(self.pane_rows().len().saturating_sub(1));
    }

    /// Connect or disconnect without opening anything, the one move tsui puts
    /// on a key of its own.
    fn toggle(&mut self, jobs: &Sender<Job>) {
        if !self.panel.header.toggle_visible {
            return;
        }
        self.ui.active = Some(!self.panel.header.toggle_checked);
        self.working = true;
        let _ = jobs.send(Job::Act(Act::Toggle, self.ui.active_provider_id.clone()));
    }

    fn raw_query(&self, scope: &str) -> String {
        self.queries
            .iter()
            .find(|(s, _)| s == scope)
            .map(|(_, q)| q.clone())
            .unwrap_or_default()
    }

    fn set_query(&mut self, scope: &str, value: String) {
        match self.queries.iter_mut().find(|(s, _)| s == scope) {
            Some(slot) => slot.1 = value,
            None => self.queries.push((scope.to_string(), value)),
        }
    }

    /// Which scope a search field filters.
    ///
    /// The panel does not say. A field row carries a `searchPlaceholder` and
    /// the rows it filters carry the `searchScope`, so the link between them
    /// is a thing every frontend knows for itself - GNOME keys it off the row
    /// kind here too. Worth pushing into the model rather than into a fourth
    /// copy of this.
    pub(super) fn field_scope(row: &PanelRow) -> Option<&'static str> {
        match row.kind.as_str() {
            "machineSearch" => Some("machines"),
            "mullvadPicker" => Some("mullvad"),
            _ => None,
        }
    }

    /// Start typing into the section's search field, wherever the cursor is
    /// inside it.
    fn start_search(&mut self) {
        let Some(section) = self.menu().get(self.section).copied() else {
            return;
        };
        let scope = section.rows.iter().find_map(Self::field_scope);
        if let Some(scope) = scope {
            self.open = true;
            self.editing = Some(scope.to_string());
        }
    }

    // ---- acting -----------------------------------------------------------

    fn act(&mut self, jobs: &Sender<Job>) {
        let Some(row) = self.current() else {
            return;
        };
        let act = match row.action.as_str() {
            "toggle" => {
                // Show the click on the frame it was pressed, and let the next
                // gather reconcile it.
                let on = !self.panel.header.toggle_checked;
                self.ui.active = Some(on);
                Some(Act::Toggle)
            }
            "switchProvider" => {
                self.ui.active_provider_id = string_field(&row, "id");
                self.panel = panel_spec(&PanelState::default(), &resolve(&self.ui, &self.options));
                self.section = 0;
                self.row = 0;
                self.open = false;
                self.landed = false;
                self.ask(jobs);
                return;
            }
            "switchAccount" => Some(Act::SwitchAccount(string_field(&row, "id"))),
            "setExitNode" => Some(match crate::ctl::target_of(&row.payload.to_string()) {
                // The panel answers with an empty target for a node already in
                // use: the click is a disconnection.
                Ok(target) if target.is_empty() => Act::SetExitNode("off".into()),
                Ok(target) => Act::SetExitNode(target),
                Err(why) => {
                    self.message = why;
                    return;
                }
            }),
            // "off" is what every provider takes for "stop using one".
            "clearExitNode" => Some(Act::SetExitNode("off".into())),
            "selectNetwork" => Some(Act::SelectNetwork {
                id: string_field(&row, "id"),
                join: !row.current,
            }),
            "authorize" => Some(Act::Authorize),
            "update" => Some(Act::Update),
            "copy" => row
                .copy_options
                .first()
                .map(|_| Act::Copy(row.label.clone())),
            "togglePicker" => {
                self.options.mullvad_picker_open = !self.options.mullvad_picker_open;
                self.ask(jobs);
                return;
            }
            _ => {
                // Enter on the search field starts typing into it, rather
                // than being the one row in the pane that answers to nothing.
                if let Some(scope) = Self::field_scope(&row) {
                    self.editing = Some(scope.to_string());
                    return;
                }
                // A machine row opens onto what it is doing.
                if row.kind == "peer" {
                    self.options.expanded_peer_id = if self.options.expanded_peer_id == row.id {
                        String::new()
                    } else {
                        row.id
                    };
                    self.ask(jobs);
                }
                return;
            }
        };
        if let Some(act) = act {
            self.working = true;
            self.message.clear();
            let _ = jobs.send(Job::Act(act, self.ui.active_provider_id.clone()));
        }
    }
}

/// What a peer's `OS` field is called when a person reads it.
fn os_label(os: &str) -> String {
    match os.to_lowercase().as_str() {
        "linux" => "Linux".into(),
        "windows" => "Windows".into(),
        "macos" => "macOS".into(),
        "ios" => "iOS".into(),
        "android" => "Android".into(),
        "freebsd" => "FreeBSD".into(),
        "openbsd" => "OpenBSD".into(),
        "" => String::new(),
        other => other.to_string(),
    }
}

fn string_field(row: &PanelRow, key: &str) -> String {
    row.payload
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn resolve(ui: &Ui, options: &Options) -> tailgauge_core::panel::ResolveOptions {
    tailgauge_core::panel::ResolveOptions {
        phrase_index: ui.phrase_index,
        recent_regions: options.recent_regions.clone(),
        mullvad_picker_open: options.mullvad_picker_open,
        expanded_peer_id: options.expanded_peer_id.clone(),
        now_ms: selvedge::state::now_ms(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with(rows: Vec<PanelRow>) -> App {
        let mut app = App::new();
        app.panel.sections = vec![tailgauge_core::panel::PanelSection {
            id: "machines".into(),
            title: "Machines".into(),
            visible: true,
            empty: String::new(),
            rows,
        }];
        app
    }

    fn searchable(id: &str, key: &str) -> PanelRow {
        PanelRow {
            id: id.into(),
            search_scope: "machines".into(),
            search_key: key.into(),
            ..PanelRow::default()
        }
    }

    #[test]
    fn a_query_leaves_the_rows_it_matches() {
        let mut app = app_with(vec![
            searchable("a", "workstation"),
            searchable("b", "laptop"),
            searchable("c", ""),
        ]);
        app.set_query("machines", "work".into());

        let shown: Vec<&str> = app
            .rows()
            .filter(|r| app.shown(r))
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(shown, ["a"], "the empty-key row is the nothing-matched one");
    }

    #[test]
    fn a_query_that_matches_nothing_shows_the_row_that_says_so() {
        let mut app = app_with(vec![
            searchable("a", "workstation"),
            searchable("nothing", ""),
        ]);
        app.set_query("machines", "zebra".into());

        let shown: Vec<&str> = app
            .rows()
            .filter(|r| app.shown(r))
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(shown, ["nothing"]);
    }

    #[test]
    fn no_query_shows_everything_but_the_message() {
        let app = app_with(vec![
            searchable("a", "workstation"),
            searchable("b", "laptop"),
            searchable("nothing", ""),
        ]);
        let shown: Vec<&str> = app
            .rows()
            .filter(|r| app.shown(r))
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(
            shown,
            ["a", "b"],
            "nothing matched is not drawn when nothing was asked"
        );
    }

    #[test]
    fn the_query_is_trimmed_and_case_folded_the_way_the_widgets_do_it() {
        let mut app = app_with(vec![searchable("a", "workstation")]);
        app.set_query("machines", "  WORK ".into());
        assert!(app.shown(&searchable("a", "workstation")));
    }

    #[test]
    fn the_cursor_cannot_leave_the_rows_that_are_left() {
        let mut app = app_with(vec![searchable("a", "workstation")]);
        app.open = true;
        app.row = 5;
        app.step(1);
        assert!(app.row <= app.pane_rows().len().saturating_sub(1));
        app.step(-10);
        assert_eq!(app.row, 0);
    }

    /// The field row carries a placeholder and the rows it filters carry the
    /// scope, so the link is this side's to know. Getting it wrong types into
    /// a scope nothing reads, and the list never filters.
    #[test]
    fn a_search_field_filters_the_scope_its_rows_carry() {
        let field = PanelRow {
            id: "machines:search".into(),
            kind: "machineSearch".into(),
            search_placeholder: "Search machines".into(),
            ..PanelRow::default()
        };
        assert_eq!(App::field_scope(&field), Some("machines"));
        assert_eq!(
            App::field_scope(&PanelRow {
                kind: "mullvadPicker".into(),
                ..PanelRow::default()
            }),
            Some("mullvad")
        );
        assert_eq!(App::field_scope(&searchable("a", "workstation")), None);

        // And typing into it hides what does not match.
        let mut app = app_with(vec![
            field,
            searchable("a", "workstation"),
            searchable("b", "laptop"),
        ]);
        app.start_search();
        assert_eq!(app.editing.as_deref(), Some("machines"));
        for c in "lap".chars() {
            app.set_query("machines", format!("{}{c}", app.raw_query("machines")));
        }
        let rows = app.pane_rows();
        let left: Vec<&str> = rows
            .iter()
            .filter(|r| r.search_placeholder.is_empty())
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(left, ["b"]);
    }

    fn peer_row(id: &str, label: &str, owner: Option<&str>, os: &str, online: bool) -> PanelRow {
        let peer = tailgauge_core::Peer {
            id: id.into(),
            host_name: label.into(),
            display_name: label.into(),
            dns_name: format!("{label}.tail.ts.net."),
            user_name: owner.map(str::to_string),
            os: os.into(),
            online,
            ipv4: vec!["100.64.0.1".into()],
            ..tailgauge_core::Peer::default()
        };
        PanelRow {
            id: id.into(),
            kind: "peer".into(),
            label: label.into(),
            sublabel: "100.64.0.1 · somebody".into(),
            payload: serde_json::to_value(&peer).expect("a peer serialises"),
            ..PanelRow::default()
        }
    }

    /// The panel sorts machines by name, which is what a flat menu wants.
    /// Grouped under their owner, that order makes a person appear twice.
    #[test]
    fn machines_are_grouped_under_the_person_who_owns_them() {
        let rows = vec![
            peer_row("a", "araki", Some("axxone"), "linux", true),
            peer_row("b", "belfort", Some("Louise"), "windows", true),
            peer_row("c", "cassini", Some("axxone"), "linux", false),
        ];
        let grouped = App::grouped_by_owner(rows);
        let shape: Vec<&str> = grouped.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            shape,
            ["axxone", "araki", "cassini", "", "Louise", "belfort"],
            "each owner captions its own machines, once"
        );

        // And the column beside the name says what it runs, since the owner is
        // already the caption.
        let araki = grouped.iter().find(|r| r.label == "araki").expect("araki");
        assert_eq!(araki.sublabel, "Linux");
    }

    #[test]
    fn a_machine_with_no_owner_still_has_somewhere_to_go() {
        let grouped = App::grouped_by_owner(vec![peer_row("a", "araki", None, "linux", true)]);
        assert_eq!(
            grouped.first().map(|r| r.label.as_str()),
            Some("Other devices")
        );
    }

    /// Turning an exit node off is a row rather than something you have to
    /// know: the panel lists the nodes and nothing else.
    #[test]
    fn the_exit_nodes_pane_opens_with_the_way_out() {
        let node = PanelRow {
            id: "n1".into(),
            kind: "exitNode".into(),
            label: "UDM-LAN".into(),
            action: "setExitNode".into(),
            ..PanelRow::default()
        };
        let rows = App::exit_node_rows(std::slice::from_ref(&node));
        assert_eq!(rows[0].label, "None");
        assert_eq!(rows[0].action, "clearExitNode");
        assert!(
            rows[0].current,
            "nothing is in use, so None is what is in use"
        );

        let mut routing = node;
        routing.current = true;
        let rows = App::exit_node_rows(&[routing]);
        assert!(!rows[0].current, "None is not current while a node is");
    }

    /// The panel hands a widget one line for the machine it runs on. A pane
    /// has room for what the peer behind that line carries.
    #[test]
    fn this_device_opens_out_into_what_the_peer_knows() {
        let row = peer_row("self", "workstation", Some("me"), "linux", true);
        let rows = App::device_rows(&[row]);
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert!(labels.contains(&"Name"));
        assert!(labels.contains(&"IPs"));
        assert!(
            labels.iter().any(|l| l.contains("workstation.tail.ts.net")),
            "the name is the one the tailnet answers to: {labels:?}"
        );
        assert!(labels.contains(&"100.64.0.1"));
        // And the row the panel gave is still there, still the one that copies.
        assert!(rows.iter().any(|r| r.id == "self"));
    }

    #[test]
    fn a_pane_with_nothing_to_act_on_is_walked_over() {
        // Captions and blanks are not places to land.
        let mut app = app_with(Vec::new());
        app.panel.sections = vec![tailgauge_core::panel::PanelSection {
            id: "exitNodes".into(),
            title: "Exit nodes".into(),
            visible: true,
            empty: String::new(),
            rows: vec![PanelRow {
                id: "n1".into(),
                kind: "exitNode".into(),
                label: "UDM-LAN".into(),
                action: "setExitNode".into(),
                ..PanelRow::default()
            }],
        }];
        app.open = true;
        app.row = 0;
        app.step(1);
        assert_eq!(
            app.pane_rows()[app.row].label,
            "UDM-LAN",
            "the blank between None and the nodes is not a stop"
        );
    }

    /// `ctl::exit_node(provider, None)` is the query: it prints the current
    /// node to stdout. From here stdout is a drawn frame, so nothing may take
    /// that path - clearing is "off".
    #[test]
    fn clearing_an_exit_node_never_takes_the_path_that_prints() {
        let (jobs, rx) = channel();
        let mut routing = peer_row("n1", "UDM-LAN", None, "linux", true);
        routing.action = "setExitNode".into();
        // A peer that is already the exit node: the panel answers with an
        // empty target and the click means disconnect.
        let mut peer: tailgauge_core::Peer =
            serde_json::from_value(routing.payload.clone()).expect("a peer");
        peer.exit_node = true;
        routing.payload = serde_json::to_value(&peer).expect("a peer");

        let mut app = app_with(Vec::new());
        app.panel.sections = vec![tailgauge_core::panel::PanelSection {
            id: "exitNodes".into(),
            title: "Exit nodes".into(),
            visible: true,
            empty: String::new(),
            rows: vec![routing],
        }];
        app.open = true;
        app.row = app
            .pane_rows()
            .iter()
            .position(|r| r.label == "UDM-LAN")
            .expect("the node is in the pane");
        app.act(&jobs);

        match rx.try_recv() {
            Ok(Job::Act(Act::SetExitNode(target), _)) => assert_eq!(target, "off"),
            other => panic!("expected a clear, got something else: {}", other.is_ok()),
        }
    }

    /// With both CLIs installed, taking the first one sent every command to
    /// Tailscale - so connecting NetBird drove the wrong daemon and reported
    /// nothing, because the wrong daemon succeeded.
    #[test]
    fn a_command_names_the_provider_the_panel_is_showing() {
        let (jobs, rx) = channel();
        let mut app = app_with(Vec::new());
        app.ui.active_provider_id = "netbird".into();
        app.panel.header.toggle_visible = true;
        app.toggle(&jobs);

        match rx.try_recv() {
            Ok(Job::Act(_, provider)) => assert_eq!(provider, "netbird"),
            _ => panic!("the toggle named no provider"),
        }
    }

    #[test]
    fn the_menu_is_the_sections_that_have_a_name() {
        let mut app = app_with(vec![searchable("a", "workstation")]);
        // A section the panel is not drawing is not a place to navigate to.
        app.panel
            .sections
            .push(tailgauge_core::panel::PanelSection {
                id: "networks".into(),
                title: "Networks".into(),
                visible: false,
                empty: String::new(),
                rows: Vec::new(),
            });
        app.panel
            .sections
            .push(tailgauge_core::panel::PanelSection {
                id: "update".into(),
                title: String::new(),
                visible: true,
                empty: String::new(),
                rows: Vec::new(),
            });
        let names: Vec<&str> = app.menu().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(names, ["machines"], "an untitled section has no menu entry");
    }

    #[test]
    fn moving_between_sections_starts_the_pane_at_its_top() {
        let mut app = app_with(vec![searchable("a", "one"), searchable("b", "two")]);
        app.panel
            .sections
            .push(tailgauge_core::panel::PanelSection {
                id: "exitNodes".into(),
                title: "Exit Nodes".into(),
                visible: true,
                empty: String::new(),
                rows: vec![PanelRow {
                    id: "n1".into(),
                    ..PanelRow::default()
                }],
            });
        app.open = true;
        app.row = 1;
        app.open = false;
        app.step(1);
        assert_eq!(app.section, 1);
        assert_eq!(app.row, 0, "the previous section's row means nothing here");
    }

    #[test]
    fn a_section_with_nothing_in_it_does_not_open() {
        let mut app = app_with(Vec::new());
        app.enter();
        assert!(!app.open, "an empty pane is not somewhere to be");
    }

    #[test]
    fn escape_closes_the_pane_before_it_quits() {
        let (jobs, _rx) = channel();
        let mut app = app_with(vec![searchable("a", "one")]);
        app.open = true;
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &jobs);
        assert!(!app.open);
        assert!(!app.quit, "the first escape backs out rather than leaving");

        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &jobs);
        assert!(app.quit);
    }
}
