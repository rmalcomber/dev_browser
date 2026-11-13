//! The backend: everything the shell may ask the app to do, decided here without any CEF. The host feeds it one message (or one key
//! command) at a time and executes what comes back: replies to send to the shell, and **effects** that only the host can carry out
//! (show a tab's page, destroy a view, delete a workspace's data, move focus).
//!
//! Messages: `ping`; `workspaces.list|create|rename|colour|delete|activate|expand`; `tabs.list|new|close|select|move|open|group`;
//! `groups.new|rename|collapse`; `command` (run a
//! registered command by id, the same path a keyboard shortcut takes). Every state-changing message is answered with the full
//! `workspaces` and `tabs` state, so the shell never patches its own copy. Invariants kept here:
//! - there is always at least one workspace, exactly one is active, and every workspace has at least one tab with one selected;
//! - ids from the shell are parsed (`Id::parse`) before use, and a tab must belong to the *active* workspace (a tab id from another
//!   workspace is "no such tab"): the shell cannot reach across workspaces;
//! - unknown fields, wrong types and oversized payloads are refused, not guessed at;
//! - which tab views are alive is decided here (the last `MAX_LIVE_VIEWS` shown), so views are cheap to discard and recreate;
//! - **a collapsed workspace is only a name and an icon**: the `tabs` reply carries tab and group data for the *expanded* workspaces
//!   only, so the titles and addresses of a collapsed workspace never reach the shell at all (S11). Activating a workspace expands it and
//!   collapses the others; the chevron expands or collapses one without switching.

use crate::commands::{self, Source};
use crate::data::{DataDir, DataError};
use crate::group;
use crate::id::Id;
use crate::ipc::{Refused, Reply, MAX_PAYLOAD_BYTES};
use crate::tab::{self, Tab};
use crate::{settings, url, workspace};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const PING: &str = "ping";
pub const LIST: &str = "workspaces.list";
pub const CREATE: &str = "workspaces.create";
pub const RENAME: &str = "workspaces.rename";
pub const COLOUR: &str = "workspaces.colour";
pub const DELETE: &str = "workspaces.delete";
pub const ACTIVATE: &str = "workspaces.activate";
pub const TABS_LIST: &str = "tabs.list";
pub const TABS_NEW: &str = "tabs.new";
pub const TABS_CLOSE: &str = "tabs.close";
pub const TABS_SELECT: &str = "tabs.select";
pub const TABS_MOVE: &str = "tabs.move";
pub const WORKSPACES_EXPAND: &str = "workspaces.expand";
pub const TABS_OPEN: &str = "tabs.open";
pub const TABS_GROUP: &str = "tabs.group";
pub const GROUPS_NEW: &str = "groups.new";
pub const GROUPS_RENAME: &str = "groups.rename";
pub const GROUPS_COLLAPSE: &str = "groups.collapse";
pub const COMMAND: &str = "command";

/// The name a new group starts with; the user renames it.
const NEW_GROUP_NAME: &str = "Group";

/// Colour and name of the workspace made on first run.
const FIRST_NAME: &str = "Default";
const FIRST_COLOUR: u32 = 0x2C5FA8;

/// How many tab views may exist at once, across workspaces. Showing a tab beyond this destroys the view shown longest ago; its record
/// stays and the view is recreated when the tab is selected again.
pub const MAX_LIVE_VIEWS: usize = 8;

/// Something only the host can do, in the order to do it.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Effect {
    /// Show this tab's page (creating the workspace's profile and the tab's view first if needed) and hide everything else.
    Show {
        workspace: Id,
        tab: Id,
        url: String,
        /// The workspace's accent colour (0xRRGGBB), drawn as a frame around its pages.
        colour: u32,
    },
    /// The workspace was recoloured: redraw the frame of its existing views.
    SetAccent { workspace: Id, colour: u32 },
    /// Destroy this tab's view. The record is kept (a discard) or already gone (a close): the host does not need to know which.
    DestroyView { workspace: Id, tab: Id },
    /// Close all of this workspace's views, release its profile, then remove its folders. The record is already gone.
    DeleteWorkspace(Id),
    /// Give keyboard focus to the visible page.
    FocusPage,
    /// Give keyboard focus to the sidebar.
    FocusShell,
    /// Give keyboard focus to the sidebar and open its "new workspace" name field.
    NewWorkspaceForm,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct Outcome {
    pub replies: Vec<Reply>,
    pub effects: Vec<Effect>,
}

#[derive(Serialize)]
struct WorkspaceOut {
    id: String,
    name: String,
    colour: u32,
    position: u32,
}

#[derive(Serialize)]
struct StateOut {
    workspaces: Vec<WorkspaceOut>,
    active: String,
}

#[derive(Serialize)]
struct TabOut {
    id: String,
    title: String,
    url: String,
    position: u32,
    group: Option<String>,
}

#[derive(Serialize)]
struct GroupOut {
    id: String,
    name: String,
    position: u32,
    collapsed: bool,
}

/// One expanded workspace's tabs and groups, in the order the sidebar shows them.
#[derive(Serialize)]
struct SectionOut {
    workspace: String,
    selected: Option<String>,
    groups: Vec<GroupOut>,
    tabs: Vec<TabOut>,
}

#[derive(Serialize)]
struct TabsOut {
    sections: Vec<SectionOut>,
}

#[derive(Serialize)]
struct ErrorOut<'a> {
    request: &'a str,
    message: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateIn {
    name: String,
    colour: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameIn {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ColourIn {
    id: String,
    colour: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdIn {
    id: String,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct NewTabIn {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveIn {
    id: String,
    position: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpandIn {
    id: String,
    expanded: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenIn {
    workspace: String,
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TabIn {
    tab: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupRenameIn {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupCollapseIn {
    id: String,
    collapsed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TabGroupIn {
    id: String,
    /// A group id, or null for "no group". Must be present: a missing field is refused rather than read as null.
    group: serde_json::Value,
}

/// A parsed, validated request.
enum Action {
    Ping(String),
    ListWorkspaces,
    CreateWorkspace { name: String, colour: u32 },
    Rename { id: Id, name: String },
    Colour { id: Id, colour: u32 },
    DeleteWorkspace(Id),
    Activate(Id),
    ListTabs,
    NewTab(Option<String>),
    CloseTab(Id),
    SelectTab(Id),
    MoveTab { id: Id, position: u32 },
    Expand { id: Id, expanded: bool },
    OpenTab { workspace: Id, id: Id },
    NewGroup(Id),
    RenameGroup { id: Id, name: String },
    CollapseGroup { id: Id, collapsed: bool },
    GroupTab { id: Id, group: Option<Id> },
    Command(&'static str),
}

/// A user-facing failure (shown in the shell), as opposed to a `Refused` protocol violation.
type Failure = String;

pub struct Backend {
    data: DataDir,
    global: Connection,
    active: Id,
    /// The workspaces shown expanded in the sidebar. Session state: the active workspace only, at start.
    expanded: HashSet<Id>,
    /// Open workspace databases, kept so a tab operation does not reopen and re-migrate a file. Dropped when the workspace is deleted.
    workspace_dbs: HashMap<Id, Connection>,
    /// The page a new tab opens (until the launcher and address bar exist).
    start_url: String,
    /// Tabs that have a view, least recently shown first, with their workspace.
    live: Vec<(Id, Id)>,
    orphans_removed: Vec<Id>,
}

impl Backend {
    /// Open the data folder's global database, make sure there is a workspace and an active one, and sweep orphaned workspace folders
    /// (left by a crash or a locked folder). Safe to call on every start. `start_url` is what a new tab opens.
    pub fn open(data: DataDir, start_url: &str) -> Result<Backend, DataError> {
        let mut global = data.open_global()?;
        if workspace::list(&global)?.is_empty() {
            data.create_workspace(&mut global, FIRST_NAME, FIRST_COLOUR)?;
        }
        let all = workspace::list(&global)?;
        let saved = settings::get(&global, settings::ACTIVE_WORKSPACE)?
            .and_then(|s| Id::parse(&s).ok())
            .filter(|id| all.iter().any(|w| &w.id == id));
        let active = saved.unwrap_or_else(|| all[0].id.clone());
        settings::set(&global, settings::ACTIVE_WORKSPACE, active.as_str())?;
        data.open_workspace(&active)?; // its folders and database exist before the host needs them
        let orphans_removed = data.remove_orphans(&global)?;
        let start_url = if url::is_restorable(start_url) {
            start_url.to_string()
        } else {
            "about:blank".to_string()
        };
        Ok(Backend {
            data,
            global,
            expanded: HashSet::from([active.clone()]),
            active,
            workspace_dbs: HashMap::new(),
            start_url,
            live: Vec::new(),
            orphans_removed,
        })
    }

    pub fn data(&self) -> &DataDir {
        &self.data
    }

    pub fn active(&self) -> &Id {
        &self.active
    }

    /// Remove the folders of workspaces that no longer exist. Call it when nothing holds those folders: before CEF starts (`open` does)
    /// and after CEF has shut down. The host sweeps again right after CEF shuts down, so anything a crash or a locked file left behind
    /// is gone when the app exits, not only at the next start.
    pub fn sweep_orphans(&self) -> Result<Vec<Id>, DataError> {
        self.data.remove_orphans(&self.global)
    }

    /// Orphaned workspace folders removed by `open`, for the log.
    pub fn orphans_removed(&self) -> &[Id] {
        &self.orphans_removed
    }

    // ---- replies ----

    /// The `workspaces` reply: the whole list in order and the active id.
    pub fn state_reply(&self) -> Reply {
        let workspaces = workspace::list(&self.global)
            .unwrap_or_default()
            .into_iter()
            .map(|w| WorkspaceOut {
                id: w.id.to_string(),
                name: w.name,
                colour: w.colour,
                position: w.position,
            })
            .collect();
        let state = StateOut {
            workspaces,
            active: self.active.to_string(),
        };
        Reply {
            name: "workspaces",
            payload: serde_json::to_string(&state).expect("state serialises"),
        }
    }

    /// The `tabs` reply: one section per **expanded** workspace (in workspace order) with its groups, its tabs in visual order and its
    /// selected tab. A collapsed workspace has no section, so nothing about it reaches the shell.
    pub fn tabs_reply(&mut self) -> Reply {
        let workspaces = workspace::list(&self.global).unwrap_or_default();
        let mut sections = Vec::new();
        for w in workspaces {
            if !self.expanded.contains(&w.id) {
                continue;
            }
            let _ = self.ensure_tabs(&w.id);
            let data = self.with_db(&w.id, |c| {
                Ok((tab::list(c)?, group::list(c)?, tab::selected(c)?))
            });
            let Ok((tabs, groups, selected)) = data else {
                continue;
            };
            sections.push(SectionOut {
                workspace: w.id.to_string(),
                selected: selected.map(|t| t.id.to_string()),
                groups: groups
                    .into_iter()
                    .map(|g| GroupOut {
                        id: g.id.to_string(),
                        name: g.name,
                        position: g.position,
                        collapsed: g.collapsed,
                    })
                    .collect(),
                tabs: tabs
                    .into_iter()
                    .map(|t| TabOut {
                        id: t.id.to_string(),
                        title: t.title,
                        url: t.url,
                        position: t.position,
                        group: t.group.map(|g| g.to_string()),
                    })
                    .collect(),
            });
        }
        Reply {
            name: "tabs",
            payload: serde_json::to_string(&TabsOut { sections }).expect("tabs serialise"),
        }
    }

    fn error_reply(request: &str, message: impl std::fmt::Display) -> Reply {
        Reply {
            name: "error",
            payload: serde_json::to_string(&ErrorOut {
                request,
                message: message.to_string(),
            })
            .expect("error serialises"),
        }
    }

    fn both(&mut self, effects: Vec<Effect>) -> Outcome {
        Outcome {
            replies: vec![self.state_reply(), self.tabs_reply()],
            effects,
        }
    }

    fn user_error(request: &str, message: impl std::fmt::Display) -> Outcome {
        Outcome {
            replies: vec![Self::error_reply(request, message)],
            effects: vec![],
        }
    }

    // ---- database access ----

    /// Run `f` on a workspace's database (opened and migrated on first use).
    fn with_db<T>(
        &mut self,
        ws: &Id,
        f: impl FnOnce(&Connection) -> Result<T, tab::TabError>,
    ) -> Result<T, Failure> {
        if !self.workspace_dbs.contains_key(ws) {
            let conn = self.data.open_workspace(ws).map_err(|e| e.to_string())?;
            self.workspace_dbs.insert(ws.clone(), conn);
        }
        f(&self.workspace_dbs[ws]).map_err(|e| e.to_string())
    }

    /// Give the workspace a first tab if it has none (a new workspace, or one made before tabs existed).
    fn ensure_tabs(&mut self, ws: &Id) -> Result<(), Failure> {
        let start = self.start_url.clone();
        self.with_db(ws, |c| {
            if tab::list(c)?.is_empty() {
                let first = tab::create(c, &start, None, None)?;
                tab::select(c, &first.id)?;
            }
            Ok(())
        })
    }

    /// The selected tab of a workspace (the first one if none is remembered).
    fn current(&mut self, ws: &Id) -> Result<Tab, Failure> {
        self.ensure_tabs(ws)?;
        self.with_db(ws, |c| match tab::selected(c)? {
            Some(t) => Ok(t),
            None => Ok(tab::list(c)?.remove(0)),
        })
    }

    /// Select `tab`, ask the host to show it, and keep the live-view list: at most `MAX_LIVE_VIEWS`, least recently shown destroyed first.
    fn show(&mut self, ws: &Id, tab: &Tab, effects: &mut Vec<Effect>) -> Result<(), Failure> {
        let id = tab.id.clone();
        self.with_db(ws, |c| tab::select(c, &id))?;
        let colour = workspace::get(&self.global, ws)
            .map(|w| w.colour)
            .unwrap_or(FIRST_COLOUR);
        effects.push(Effect::Show {
            workspace: ws.clone(),
            tab: tab.id.clone(),
            url: tab.url.clone(),
            colour,
        });
        self.live.retain(|(_, t)| t != &tab.id);
        self.live.push((ws.clone(), tab.id.clone()));
        while self.live.len() > MAX_LIVE_VIEWS {
            let (w, t) = self.live.remove(0);
            effects.push(Effect::DestroyView {
                workspace: w,
                tab: t,
            });
        }
        Ok(())
    }

    fn set_active(&mut self, id: &Id) -> Result<(), DataError> {
        self.data.open_workspace(id)?;
        settings::set(&self.global, settings::ACTIVE_WORKSPACE, id.as_str())?;
        self.active = id.clone();
        Ok(())
    }

    fn activate(&mut self, id: &Id) -> Result<Vec<Effect>, Failure> {
        if workspace::get(&self.global, id).is_err() {
            return Err("there is no such workspace".into());
        }
        self.set_active(id).map_err(|e| e.to_string())?;
        // the activated workspace is the one expanded; the chevron can open others without switching
        self.expanded.clear();
        self.expanded.insert(id.clone());
        let current = self.current(id)?;
        let mut effects = vec![];
        self.show(id, &current, &mut effects)?;
        Ok(effects)
    }

    // ---- what the host calls ----

    /// The first thing the host does once its window exists: show the active workspace's selected tab and focus it.
    pub fn startup(&mut self) -> Outcome {
        let ws = self.active.clone();
        let mut effects = vec![];
        let result = self
            .current(&ws)
            .and_then(|t| self.show(&ws, &t, &mut effects));
        if let Err(e) = result {
            return Self::user_error("startup", e);
        }
        effects.push(Effect::FocusPage);
        self.both(effects)
    }

    /// A page changed its title. Returns the `tabs` reply to push to the shell, if the tab is in the active workspace.
    pub fn tab_title(&mut self, ws: &Id, tab_id: &Id, title: &str) -> Option<Reply> {
        let id = tab_id.clone();
        self.with_db(ws, |c| tab::set_title(c, &id, title)).ok()?;
        self.expanded.contains(ws).then(|| self.tabs_reply())
    }

    /// A page navigated. Only an address that may be re-opened is stored (`url::is_restorable`).
    pub fn tab_url(&mut self, ws: &Id, tab_id: &Id, new_url: &str) -> Option<Reply> {
        let id = tab_id.clone();
        let stored = self
            .with_db(ws, |c| tab::set_url(c, &id, new_url))
            .unwrap_or(false);
        (stored && self.expanded.contains(ws)).then(|| self.tabs_reply())
    }

    /// A tab's view went away without the app asking (the page closed itself, with `window.close()` for example). If the tab still
    /// exists this is a close: returns what to do next (usually: show a neighbour). `None` if the app had destroyed the view itself.
    pub fn view_closed_by_page(&mut self, ws: &Id, tab_id: &Id) -> Option<Outcome> {
        let before = self.live.len();
        self.live.retain(|(_, t)| t != tab_id);
        if self.live.len() == before {
            return None; // the app destroyed it (a discard or a close it ordered): nothing to do
        }
        let outcome = self.close_tab(ws, tab_id, false);
        Some(outcome.unwrap_or_else(|e| Self::user_error("view.closed", e)))
    }

    // ---- messages ----

    /// Decide what to do with one message from the shell.
    pub fn handle(&mut self, name: &str, payload: &str) -> Result<Outcome, Refused> {
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(Refused::PayloadTooLarge(payload.len()));
        }
        let action = Self::parse_action(name, payload)?;
        Ok(match self.run(action, Source::Shell) {
            Ok(outcome) => outcome,
            Err(message) => Self::user_error(name, message),
        })
    }

    /// Run a registered command by id, as a keyboard shortcut or the shell does. `source` is where the key was pressed.
    pub fn run_command(&mut self, id: &str, source: Source) -> Outcome {
        match commands::find(id) {
            Some(cmd) => match self.command(cmd.id, source) {
                Ok(o) => o,
                Err(message) => Self::user_error(cmd.id, message),
            },
            None => Self::user_error(id, "unknown command"),
        }
    }

    fn from_json<T: for<'de> Deserialize<'de>>(message: &str, payload: &str) -> Result<T, Refused> {
        serde_json::from_str(payload).map_err(|e| Refused::BadPayload {
            message: message.to_string(),
            // never serde's own text: for a wrong type it quotes the offending value (a token, say), and this goes to the log
            why: format!(
                "{:?} error at line {} column {}",
                e.classify(),
                e.line(),
                e.column()
            ),
        })
    }

    fn parse_id(message: &str, raw: &str) -> Result<Id, Refused> {
        Id::parse(raw).map_err(|e| Refused::BadPayload {
            message: message.to_string(),
            why: format!("id: {e}"),
        })
    }

    fn no_payload(name: &str, payload: &str) -> Result<(), Refused> {
        if payload.is_empty() || payload == "{}" {
            Ok(())
        } else {
            Err(Refused::BadPayload {
                message: name.into(),
                why: "takes no payload".into(),
            })
        }
    }

    fn parse_action(name: &str, payload: &str) -> Result<Action, Refused> {
        Ok(match name {
            PING => Action::Ping(payload.to_string()),
            LIST => {
                Self::no_payload(name, payload)?;
                Action::ListWorkspaces
            }
            CREATE => {
                let r: CreateIn = Self::from_json(name, payload)?;
                Action::CreateWorkspace {
                    name: r.name,
                    colour: r.colour,
                }
            }
            RENAME => {
                let r: RenameIn = Self::from_json(name, payload)?;
                Action::Rename {
                    id: Self::parse_id(name, &r.id)?,
                    name: r.name,
                }
            }
            COLOUR => {
                let r: ColourIn = Self::from_json(name, payload)?;
                Action::Colour {
                    id: Self::parse_id(name, &r.id)?,
                    colour: r.colour,
                }
            }
            DELETE => {
                let r: IdIn = Self::from_json(name, payload)?;
                Action::DeleteWorkspace(Self::parse_id(name, &r.id)?)
            }
            ACTIVATE => {
                let r: IdIn = Self::from_json(name, payload)?;
                Action::Activate(Self::parse_id(name, &r.id)?)
            }
            TABS_LIST => {
                Self::no_payload(name, payload)?;
                Action::ListTabs
            }
            TABS_NEW => {
                let r: NewTabIn = if payload.is_empty() {
                    NewTabIn::default()
                } else {
                    Self::from_json(name, payload)?
                };
                Action::NewTab(r.url)
            }
            TABS_CLOSE => {
                let r: IdIn = Self::from_json(name, payload)?;
                Action::CloseTab(Self::parse_id(name, &r.id)?)
            }
            TABS_SELECT => {
                let r: IdIn = Self::from_json(name, payload)?;
                Action::SelectTab(Self::parse_id(name, &r.id)?)
            }
            TABS_MOVE => {
                let r: MoveIn = Self::from_json(name, payload)?;
                Action::MoveTab {
                    id: Self::parse_id(name, &r.id)?,
                    position: r.position,
                }
            }
            WORKSPACES_EXPAND => {
                let r: ExpandIn = Self::from_json(name, payload)?;
                Action::Expand {
                    id: Self::parse_id(name, &r.id)?,
                    expanded: r.expanded,
                }
            }
            TABS_OPEN => {
                let r: OpenIn = Self::from_json(name, payload)?;
                Action::OpenTab {
                    workspace: Self::parse_id(name, &r.workspace)?,
                    id: Self::parse_id(name, &r.id)?,
                }
            }
            TABS_GROUP => {
                let r: TabGroupIn = Self::from_json(name, payload)?;
                Action::GroupTab {
                    id: Self::parse_id(name, &r.id)?,
                    group: match r.group {
                        serde_json::Value::Null => None,
                        serde_json::Value::String(g) => Some(Self::parse_id(name, &g)?),
                        _ => {
                            return Err(Refused::BadPayload {
                                message: name.into(),
                                why: "group must be an id or null".into(),
                            })
                        }
                    },
                }
            }
            GROUPS_NEW => {
                let r: TabIn = Self::from_json(name, payload)?;
                Action::NewGroup(Self::parse_id(name, &r.tab)?)
            }
            GROUPS_RENAME => {
                let r: GroupRenameIn = Self::from_json(name, payload)?;
                Action::RenameGroup {
                    id: Self::parse_id(name, &r.id)?,
                    name: r.name,
                }
            }
            GROUPS_COLLAPSE => {
                let r: GroupCollapseIn = Self::from_json(name, payload)?;
                Action::CollapseGroup {
                    id: Self::parse_id(name, &r.id)?,
                    collapsed: r.collapsed,
                }
            }
            COMMAND => {
                let r: IdIn = Self::from_json(name, payload)?;
                match commands::find(&r.id) {
                    Some(c) => Action::Command(c.id),
                    None => return Err(Refused::UnknownMessage(r.id.chars().take(64).collect())),
                }
            }
            other => return Err(Refused::UnknownMessage(other.chars().take(64).collect())),
        })
    }

    fn run(&mut self, action: Action, source: Source) -> Result<Outcome, Failure> {
        match action {
            Action::Ping(text) => Ok(Outcome {
                replies: vec![Reply {
                    name: "pong",
                    payload: text,
                }],
                effects: vec![],
            }),
            Action::ListWorkspaces | Action::ListTabs => {
                // make sure the active workspace has its tab before reporting it
                let ws = self.active.clone();
                self.ensure_tabs(&ws)?;
                Ok(self.both(vec![]))
            }
            Action::CreateWorkspace { name, colour } => {
                let ws = self
                    .data
                    .create_workspace(&mut self.global, &name, colour)
                    .map_err(|e| e.to_string())?;
                let effects = self.activate(&ws.id)?;
                Ok(self.both(effects))
            }
            Action::Rename { id, name } => {
                workspace::rename(&self.global, &id, &name).map_err(|e| e.to_string())?;
                Ok(self.both(vec![]))
            }
            Action::Colour { id, colour } => {
                let w =
                    workspace::set_colour(&self.global, &id, colour).map_err(|e| e.to_string())?;
                Ok(self.both(vec![Effect::SetAccent {
                    workspace: id,
                    colour: w.colour,
                }]))
            }
            Action::Expand { id, expanded } => {
                if workspace::get(&self.global, &id).is_err() {
                    return Err("there is no such workspace".into());
                }
                if expanded {
                    self.expanded.insert(id);
                } else {
                    self.expanded.remove(&id);
                }
                Ok(self.both(vec![]))
            }
            Action::OpenTab { workspace, id } => {
                if workspace::get(&self.global, &workspace).is_err() {
                    return Err("there is no such workspace".into());
                }
                // the tab must be in that workspace's own database: another workspace's id is "no such tab"
                let tab = self.with_db(&workspace, |c| tab::get(c, &id))?;
                self.set_active(&workspace).map_err(|e| e.to_string())?;
                self.expanded.clear();
                self.expanded.insert(workspace.clone());
                let mut effects = vec![];
                self.show(&workspace, &tab, &mut effects)?;
                Ok(self.both(effects))
            }
            Action::NewGroup(tab_id) => {
                let ws = self.active.clone();
                self.tab_in_active(&tab_id)?;
                self.with_db(&ws, |c| group::create_for(c, &tab_id, NEW_GROUP_NAME))?;
                Ok(self.both(vec![]))
            }
            Action::RenameGroup { id, name } => {
                let ws = self.active.clone();
                self.with_db(&ws, |c| group::rename(c, &id, &name))?;
                Ok(self.both(vec![]))
            }
            Action::CollapseGroup { id, collapsed } => {
                let ws = self.active.clone();
                self.with_db(&ws, |c| group::set_collapsed(c, &id, collapsed))?;
                Ok(self.both(vec![]))
            }
            Action::GroupTab { id, group: g } => {
                let ws = self.active.clone();
                self.tab_in_active(&id)?;
                self.with_db(&ws, |c| group::assign(c, &id, g.as_ref()))?;
                Ok(self.both(vec![]))
            }
            Action::Activate(id) => {
                let effects = self.activate(&id)?;
                Ok(self.both(effects))
            }
            Action::DeleteWorkspace(id) => self.delete_workspace(&id),
            Action::NewTab(url) => self.new_tab(url, false),
            Action::CloseTab(id) => {
                let ws = self.active.clone();
                self.close_tab(&ws, &id, true)
            }
            Action::SelectTab(id) => {
                let ws = self.active.clone();
                let tab = self.tab_in_active(&id)?;
                let mut effects = vec![];
                self.show(&ws, &tab, &mut effects)?;
                Ok(self.both(effects))
            }
            Action::MoveTab { id, position } => {
                let ws = self.active.clone();
                self.tab_in_active(&id)?;
                self.with_db(&ws, |c| tab::move_to(c, &id, position))?;
                Ok(self.both(vec![]))
            }
            Action::Command(id) => self.command(id, source),
        }
    }

    /// A tab by id, which must be in the **active** workspace.
    fn tab_in_active(&mut self, id: &Id) -> Result<Tab, Failure> {
        let ws = self.active.clone();
        self.with_db(&ws, |c| tab::get(c, id))
    }

    fn delete_workspace(&mut self, id: &Id) -> Result<Outcome, Failure> {
        let all = workspace::list(&self.global).map_err(|e| e.to_string())?;
        let Some(index) = all.iter().position(|w| &w.id == id) else {
            return Err("there is no such workspace".into());
        };
        if all.len() == 1 {
            return Err("you cannot delete your only workspace".into());
        }
        workspace::delete_record(&mut self.global, id).map_err(|e| e.to_string())?;
        // release everything held for it: its views are the host's to destroy (DeleteWorkspace), its database file is closed here
        self.live.retain(|(w, _)| w != id);
        self.workspace_dbs.remove(id);
        self.expanded.remove(id);
        let mut effects = vec![Effect::DeleteWorkspace(id.clone())];
        if &self.active == id {
            // the workspace that took its place in the list, or the one before it if it was last
            let next = all
                .get(index + 1)
                .or_else(|| all.get(index.wrapping_sub(1)))
                .expect("there was more than one workspace")
                .id
                .clone();
            effects.extend(self.activate(&next)?);
        }
        Ok(self.both(effects))
    }

    /// Open a tab after the selected one and show it. `focus`: also give the page keyboard focus (keyboard commands do).
    fn new_tab(&mut self, url: Option<String>, focus: bool) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let url = url.unwrap_or_else(|| self.start_url.clone());
        // opened after the selected tab, and in its group, so a group's tabs stay together
        let current = self.current(&ws)?;
        let after = current.position + 1;
        let group = current.group.clone();
        let created = self.with_db(&ws, |c| tab::create(c, &url, Some(after), group.as_ref()))?;
        let mut effects = vec![];
        self.show(&ws, &created, &mut effects)?;
        if focus {
            effects.push(Effect::FocusPage);
        }
        Ok(self.both(effects))
    }

    /// Close a tab of workspace `ws`. If it was the selected one, show its neighbour (a fresh tab if it was the last), and only then
    /// destroy its view, so the window is never left without a page. `destroy_view`: false when the view is already gone.
    fn close_tab(&mut self, ws: &Id, id: &Id, destroy_view: bool) -> Result<Outcome, Failure> {
        let tabs = self.with_db(ws, tab::list)?;
        let index = tabs
            .iter()
            .position(|t| &t.id == id)
            .ok_or("there is no such tab")?;
        let was_selected = self.current(ws)?.id == *id;
        self.with_db(ws, |c| tab::close(c, id))?;
        let was_live = self.live.iter().any(|(_, t)| t == id);
        self.live.retain(|(_, t)| t != id);
        let mut effects = vec![];
        let mut focus = false;
        if was_selected && ws == &self.active {
            // a fresh tab if that was the last; otherwise the neighbour that now sits where it was, or the one before
            let next = self.current_after_close(ws, index)?;
            self.show(ws, &next, &mut effects)?;
            focus = true;
        } else if was_selected {
            self.ensure_tabs(ws)?;
        }
        if was_live && destroy_view {
            effects.push(Effect::DestroyView {
                workspace: ws.clone(),
                tab: id.clone(),
            });
        }
        // last, so the view that was just destroyed cannot take the focus back
        if focus {
            effects.push(Effect::FocusPage);
        }
        Ok(self.both(effects))
    }

    fn current_after_close(&mut self, ws: &Id, index: usize) -> Result<Tab, Failure> {
        self.ensure_tabs(ws)?;
        self.with_db(ws, |c| {
            let tabs = tab::list(c)?;
            Ok(tabs
                .get(index)
                .or_else(|| tabs.get(index.wrapping_sub(1)))
                .or(tabs.first())
                .expect("a workspace has a tab after ensure_tabs")
                .clone())
        })
    }

    /// Execute a command that the registry knows. Commands that change what is shown also focus the page: a hidden view cannot keep
    /// the focus, and the user pressed a key, so the keyboard should land somewhere useful.
    fn command(&mut self, id: &str, source: Source) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        match id {
            "tab.new" => self.new_tab(None, true),
            "tab.close" => {
                let current = self.current(&ws)?;
                self.close_tab(&ws, &current.id, true)
            }
            "tab.next" => self.step_tab(1),
            "tab.prev" => self.step_tab(-1),
            "tab.select.last" => {
                let n = self.with_db(&ws, |c| Ok(tab::list(c)?.len()))?;
                self.select_index(n.saturating_sub(1))
            }
            "tab.move.down" => self.move_selected(1),
            "tab.move.up" => self.move_selected(-1),
            "tab.discard" => self.discard_background(),
            "tab.group.new" => {
                let current = self.current(&ws)?;
                self.with_db(&ws, |c| group::create_for(c, &current.id, NEW_GROUP_NAME))?;
                Ok(self.both(vec![]))
            }
            "tab.ungroup" => {
                let current = self.current(&ws)?;
                self.with_db(&ws, |c| group::assign(c, &current.id, None))?;
                Ok(self.both(vec![]))
            }
            "workspace.new" => Ok(Outcome {
                replies: vec![],
                effects: vec![Effect::NewWorkspaceForm],
            }),
            "workspace.next" => self.step_workspace(1),
            "workspace.prev" => self.step_workspace(-1),
            "focus.cycle" => Ok(Outcome {
                replies: vec![],
                effects: vec![match source {
                    Source::Shell => Effect::FocusPage,
                    Source::Page => Effect::FocusShell,
                }],
            }),
            "focus.page" => Ok(Outcome {
                replies: vec![],
                effects: vec![Effect::FocusPage],
            }),
            other => match other.strip_prefix("tab.select.").map(str::parse::<usize>) {
                Some(Ok(n)) if n >= 1 => self.select_index(n - 1),
                _ => Err("that command is not available yet".into()),
            },
        }
    }

    fn step_tab(&mut self, delta: isize) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let tabs = self.with_db(&ws, tab::list)?;
        let current = self.current(&ws)?;
        let index = tabs.iter().position(|t| t.id == current.id).unwrap_or(0);
        let next = (index as isize + delta).rem_euclid(tabs.len() as isize) as usize;
        self.select_index(next)
    }

    fn select_index(&mut self, index: usize) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let tabs = self.with_db(&ws, tab::list)?;
        let mut effects = vec![];
        if let Some(t) = tabs.get(index) {
            self.show(&ws, t, &mut effects)?;
            effects.push(Effect::FocusPage);
        }
        Ok(self.both(effects))
    }

    fn move_selected(&mut self, delta: isize) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let current = self.current(&ws)?;
        let count = self.with_db(&ws, |c| Ok(tab::list(c)?.len()))?;
        let target = current.position as isize + delta;
        if target >= 0 && (target as usize) < count {
            self.with_db(&ws, |c| tab::move_to(c, &current.id, target as u32))?;
        }
        Ok(self.both(vec![]))
    }

    /// Destroy the views of every tab but the selected one. The records stay; selecting a tab recreates its view.
    fn discard_background(&mut self) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let current = self.current(&ws)?;
        let mut effects = vec![];
        let (keep, drop): (Vec<_>, Vec<_>) = std::mem::take(&mut self.live)
            .into_iter()
            .partition(|(_, t)| t == &current.id);
        self.live = keep;
        for (w, t) in drop {
            effects.push(Effect::DestroyView {
                workspace: w,
                tab: t,
            });
        }
        Ok(self.both(effects))
    }

    fn step_workspace(&mut self, delta: isize) -> Result<Outcome, Failure> {
        let all = workspace::list(&self.global).map_err(|e| e.to_string())?;
        let index = all.iter().position(|w| w.id == self.active).unwrap_or(0);
        let next = &all[(index as isize + delta).rem_euclid(all.len() as isize) as usize];
        let id = next.id.clone();
        let mut effects = self.activate(&id)?;
        effects.push(Effect::FocusPage);
        Ok(self.both(effects))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use serde_json::{json, Value};

    const START: &str = "https://start.example/";

    fn backend(tag: &str) -> (TempDir, Backend) {
        let t = TempDir::new(tag);
        let b = Backend::open(DataDir::open(t.path()).unwrap(), START).unwrap();
        (t, b)
    }

    fn state(b: &Backend) -> Value {
        serde_json::from_str(&b.state_reply().payload).unwrap()
    }

    /// The active workspace's section of the `tabs` reply: { workspace, selected, groups, tabs }.
    fn tabs(b: &mut Backend) -> Value {
        let all: Value = serde_json::from_str(&b.tabs_reply().payload).unwrap();
        let active = b.active().to_string();
        all["sections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["workspace"] == active)
            .cloned()
            .unwrap_or(Value::Null)
    }

    /// The workspaces that have a section in the `tabs` reply (the expanded ones), by name.
    fn expanded_names(b: &mut Backend) -> Vec<String> {
        let all: Value = serde_json::from_str(&b.tabs_reply().payload).unwrap();
        all["sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                let id = s["workspace"].as_str().unwrap();
                state(b)["workspaces"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|w| w["id"] == id)
                    .unwrap()["name"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    fn tab_urls(b: &mut Backend) -> Vec<String> {
        tabs(b)["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["url"].as_str().unwrap().to_string())
            .collect()
    }

    fn tab_ids(b: &mut Backend) -> Vec<String> {
        tabs(b)["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect()
    }

    fn selected(b: &mut Backend) -> String {
        tabs(b)["selected"].as_str().unwrap().to_string()
    }

    fn names(b: &Backend) -> Vec<String> {
        state(b)["workspaces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["name"].as_str().unwrap().to_string())
            .collect()
    }

    fn send(b: &mut Backend, name: &str, payload: Value) -> Outcome {
        b.handle(name, &payload.to_string()).unwrap()
    }

    fn first_reply(o: &Outcome) -> (&'static str, Value) {
        let r = &o.replies[0];
        (r.name, serde_json::from_str(&r.payload).unwrap())
    }

    fn id_of(b: &Backend, name: &str) -> String {
        state(b)["workspaces"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["name"] == name)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn id(s: &str) -> Id {
        Id::parse(s).unwrap()
    }

    fn shown(o: &Outcome) -> Vec<String> {
        o.effects
            .iter()
            .filter_map(|e| match e {
                Effect::Show { tab, .. } => Some(tab.to_string()),
                _ => None,
            })
            .collect()
    }

    fn destroyed(o: &Outcome) -> Vec<String> {
        o.effects
            .iter()
            .filter_map(|e| match e {
                Effect::DestroyView { tab, .. } => Some(tab.to_string()),
                _ => None,
            })
            .collect()
    }

    /// Open `n` more tabs with the keyboard command and return the new tab ids in order.
    fn open_tabs(b: &mut Backend, n: usize) -> Vec<String> {
        (0..n)
            .map(|_| {
                b.run_command("tab.new", Source::Page);
                selected(b)
            })
            .collect()
    }

    // ---- workspaces (as in M1.2, with the new effects) ----

    #[test]
    fn a_fresh_data_folder_gets_one_active_default_workspace_with_one_tab() {
        let (_t, mut b) = backend("fresh");
        assert_eq!(names(&b), vec!["Default"]);
        assert_eq!(state(&b)["active"], id_of(&b, "Default"));
        assert!(b.data().workspace_db_path(b.active()).is_file());
        assert!(b.data().chromium_profile(b.active()).is_dir());
        send(&mut b, TABS_LIST, json!({})); // makes the first tab
        assert_eq!(tab_urls(&mut b), vec![START]);
        assert!(!selected(&mut b).is_empty());
    }

    #[test]
    fn startup_shows_the_selected_tab_and_focuses_the_page() {
        let (_t, mut b) = backend("startup");
        let o = b.startup();
        let ws = b.active().clone();
        let sel = selected(&mut b);
        assert_eq!(
            o.effects,
            vec![
                Effect::Show {
                    workspace: ws,
                    tab: id(&sel),
                    url: START.into(),
                    colour: FIRST_COLOUR
                },
                Effect::FocusPage
            ]
        );
        assert_eq!(o.replies.len(), 2, "the shell gets workspaces and tabs");
    }

    #[test]
    fn ping_still_answers_pong() {
        let (_t, mut b) = backend("ping");
        let o = b.handle("ping", "hello").unwrap();
        assert_eq!(
            o.replies,
            vec![Reply {
                name: "pong",
                payload: "hello".into()
            }]
        );
        assert!(o.effects.is_empty());
    }

    #[test]
    fn creating_a_workspace_activates_it_with_its_own_first_tab() {
        let (_t, mut b) = backend("create");
        b.startup();
        let o = send(
            &mut b,
            CREATE,
            json!({"name": " Work ", "colour": 0x00AA00}),
        );
        let work = id_of(&b, "Work");
        assert_eq!(state(&b)["active"], work);
        assert_eq!(names(&b), vec!["Default", "Work"]);
        assert!(
            matches!(&o.effects[..], [Effect::Show { workspace, url, .. }] if workspace.as_str() == work && url == START)
        );
        assert_eq!(
            tabs(&mut b)["workspace"],
            work,
            "the tabs reply is for the new workspace"
        );
        assert!(b.data().chromium_profile(&id(&work)).is_dir());
    }

    #[test]
    fn rename_and_colour_change_the_record_only() {
        let (_t, mut b) = backend("rename");
        let wid = id_of(&b, "Default");
        let o = send(&mut b, RENAME, json!({"id": wid, "name": "Home"}));
        assert!(o.effects.is_empty());
        assert_eq!(names(&b), vec!["Home"]);
        let o = send(&mut b, COLOUR, json!({"id": wid, "colour": 255}));
        // the host redraws the accent frame of this workspace's views; nothing else happens
        assert_eq!(
            o.effects,
            vec![Effect::SetAccent {
                workspace: id(&wid),
                colour: 255
            }]
        );
        assert_eq!(state(&b)["workspaces"][0]["colour"], 255);
    }

    #[test]
    fn bad_names_and_colours_are_user_errors_that_change_nothing() {
        let (_t, mut b) = backend("baduser");
        let wid = id_of(&b, "Default");
        for (msg, p) in [
            (CREATE, json!({"name": "", "colour": 1})),
            (CREATE, json!({"name": "x", "colour": 0x1000000u32})),
            (RENAME, json!({"id": wid, "name": "  "})),
            (COLOUR, json!({"id": wid, "colour": 0x1000000u32})),
        ] {
            let o = send(&mut b, msg, p);
            let (name, e) = first_reply(&o);
            assert_eq!(name, "error", "{msg}");
            assert_eq!(e["request"], msg);
            assert!(o.effects.is_empty());
        }
        assert_eq!(names(&b), vec!["Default"]);
    }

    #[test]
    fn activate_switches_shows_that_workspaces_tab_and_survives_a_restart() {
        let t = TempDir::new("activate");
        let second;
        {
            let mut b = Backend::open(DataDir::open(t.path()).unwrap(), START).unwrap();
            send(&mut b, CREATE, json!({"name": "Two", "colour": 2}));
            let first = id_of(&b, "Default");
            second = id_of(&b, "Two");
            let o = send(&mut b, ACTIVATE, json!({"id": first}));
            assert!(
                matches!(&o.effects[..], [Effect::Show { workspace, .. }] if workspace.as_str() == first)
            );
            send(&mut b, ACTIVATE, json!({"id": second}));
        }
        let b = Backend::open(DataDir::open(t.path()).unwrap(), START).unwrap();
        assert_eq!(
            state(&b)["active"],
            second,
            "the last active workspace is restored"
        );
    }

    #[test]
    fn a_saved_active_workspace_that_no_longer_exists_falls_back_to_the_first() {
        let t = TempDir::new("fallback");
        let data = DataDir::open(t.path()).unwrap();
        {
            let g = data.open_global().unwrap();
            settings::set(&g, settings::ACTIVE_WORKSPACE, "ws-0000000000000001").unwrap();
        }
        let b = Backend::open(data, START).unwrap();
        assert_eq!(state(&b)["active"], id_of(&b, "Default"));
    }

    #[test]
    fn deleting_a_workspace_removes_the_record_and_tells_the_host_to_remove_the_data() {
        let (_t, mut b) = backend("delete");
        send(&mut b, CREATE, json!({"name": "Two", "colour": 2}));
        let first = id_of(&b, "Default");
        let second = id_of(&b, "Two");
        let o = send(&mut b, DELETE, json!({"id": first}));
        assert_eq!(o.effects, vec![Effect::DeleteWorkspace(id(&first))]);
        assert_eq!(names(&b), vec!["Two"]);
        assert_eq!(
            state(&b)["active"],
            second,
            "the active workspace did not change"
        );
    }

    #[test]
    fn deleting_the_active_workspace_activates_a_neighbour_and_forgets_its_views() {
        let (_t, mut b) = backend("deleteactive");
        b.startup();
        send(&mut b, CREATE, json!({"name": "B", "colour": 2}));
        send(&mut b, CREATE, json!({"name": "C", "colour": 3}));
        let (a, bb, c) = (id_of(&b, "Default"), id_of(&b, "B"), id_of(&b, "C"));
        assert_eq!(b.live.len(), 3);
        let o = send(&mut b, DELETE, json!({"id": c}));
        assert_eq!(o.effects[0], Effect::DeleteWorkspace(id(&c)));
        assert!(
            matches!(&o.effects[1], Effect::Show { workspace, .. } if workspace.as_str() == bb)
        );
        assert_eq!(state(&b)["active"], bb);
        assert_eq!(
            b.live.len(),
            2,
            "the deleted workspace's views are no longer tracked"
        );
        assert!(
            !b.workspace_dbs.contains_key(&id(&c)),
            "its database file is closed"
        );
        send(&mut b, ACTIVATE, json!({"id": a}));
        let o = send(&mut b, DELETE, json!({"id": a}));
        assert_eq!(o.effects[0], Effect::DeleteWorkspace(id(&a)));
        assert!(
            matches!(&o.effects[1], Effect::Show { workspace, .. } if workspace.as_str() == bb)
        );
        assert_eq!(names(&b), vec!["B"]);
    }

    #[test]
    fn the_only_workspace_cannot_be_deleted() {
        let (_t, mut b) = backend("onlyone");
        let wid = id_of(&b, "Default");
        let o = send(&mut b, DELETE, json!({"id": wid}));
        assert_eq!(first_reply(&o).0, "error");
        assert!(o.effects.is_empty());
        assert_eq!(names(&b), vec!["Default"]);
    }

    #[test]
    fn an_unknown_workspace_is_a_user_error() {
        let (_t, mut b) = backend("unknown");
        let ghost = Id::new("ws").to_string();
        for msg in [ACTIVATE, DELETE] {
            let o = send(&mut b, msg, json!({"id": ghost}));
            assert_eq!(first_reply(&o).0, "error", "{msg}");
            assert!(o.effects.is_empty());
        }
        let o = send(&mut b, RENAME, json!({"id": ghost, "name": "x"}));
        assert_eq!(first_reply(&o).0, "error");
    }

    // ---- tabs ----

    #[test]
    fn new_tab_opens_after_the_selected_one_shows_it_and_focuses_the_page() {
        let (_t, mut b) = backend("newtab");
        b.startup();
        let first = selected(&mut b);
        let o = b.run_command("tab.new", Source::Page);
        let second = selected(&mut b);
        assert_ne!(first, second);
        assert_eq!(shown(&o), vec![second.clone()]);
        assert_eq!(o.effects.last(), Some(&Effect::FocusPage));
        // opening another from the first tab inserts right after it, not at the end
        send(&mut b, TABS_SELECT, json!({"id": first}));
        b.run_command("tab.new", Source::Page);
        let third = selected(&mut b);
        assert_eq!(tab_ids(&mut b), vec![first, third, second]);
    }

    #[test]
    fn the_new_tab_message_takes_an_optional_restorable_url() {
        let (_t, mut b) = backend("newtabmsg");
        b.startup();
        send(&mut b, TABS_NEW, json!({"url": "https://given.example/"}));
        assert_eq!(tab_urls(&mut b), vec![START, "https://given.example/"]);
        send(&mut b, TABS_NEW, json!({}));
        assert_eq!(tab_urls(&mut b).last().unwrap(), START);
        let o = send(&mut b, TABS_NEW, json!({"url": "javascript:alert(1)"}));
        assert_eq!(first_reply(&o).0, "error");
        let o = send(&mut b, TABS_NEW, json!({"url": "app://shell/index.html"}));
        assert_eq!(first_reply(&o).0, "error");
        assert_eq!(tab_urls(&mut b).len(), 3, "refused addresses made no tab");
        assert!(
            b.handle(TABS_NEW, "").is_ok(),
            "no payload at all means the default page"
        );
    }

    #[test]
    fn closing_the_selected_tab_shows_its_neighbour_before_destroying_it() {
        let (_t, mut b) = backend("closesel");
        b.startup();
        let ids = open_tabs(&mut b, 2); // tabs: first, ids[0], ids[1]; selected ids[1]
        send(&mut b, TABS_SELECT, json!({"id": ids[0]}));
        let o = b.run_command("tab.close", Source::Page);
        // the one that took its place (ids[1]) is shown first, then the closed one's view is destroyed
        assert_eq!(shown(&o), vec![ids[1].clone()]);
        assert_eq!(destroyed(&o), vec![ids[0].clone()]);
        let show_at = o
            .effects
            .iter()
            .position(|e| matches!(e, Effect::Show { .. }))
            .unwrap();
        let destroy_at = o
            .effects
            .iter()
            .position(|e| matches!(e, Effect::DestroyView { .. }))
            .unwrap();
        assert!(
            show_at < destroy_at,
            "never destroy the visible view before showing another"
        );
        assert_eq!(selected(&mut b), ids[1]);
        assert_eq!(tab_urls(&mut b).len(), 2);
    }

    #[test]
    fn closing_the_last_tab_in_the_list_shows_the_one_before() {
        let (_t, mut b) = backend("closelast");
        b.startup();
        open_tabs(&mut b, 1);
        let first = tab_ids(&mut b)[0].clone();
        b.run_command("tab.close", Source::Page);
        assert_eq!(selected(&mut b), first);
        assert_eq!(tab_urls(&mut b).len(), 1);
    }

    #[test]
    fn closing_the_only_tab_opens_a_fresh_one() {
        let (_t, mut b) = backend("closeonly");
        b.startup();
        let only = selected(&mut b);
        let o = b.run_command("tab.close", Source::Page);
        assert_eq!(tab_urls(&mut b), vec![START]);
        let fresh = selected(&mut b);
        assert_ne!(fresh, only, "a new record, not the old one");
        assert_eq!(shown(&o), vec![fresh]);
        assert_eq!(destroyed(&o), vec![only]);
        assert_eq!(o.effects.last(), Some(&Effect::FocusPage));
    }

    #[test]
    fn closing_a_background_tab_destroys_its_view_and_changes_nothing_visible() {
        let (_t, mut b) = backend("closebg");
        b.startup();
        let ids = open_tabs(&mut b, 2);
        let o = send(&mut b, TABS_CLOSE, json!({"id": ids[0]}));
        assert!(shown(&o).is_empty());
        assert_eq!(destroyed(&o), vec![ids[0].clone()]);
        assert_eq!(selected(&mut b), ids[1], "the selection did not move");
        // a tab that never had a view needs no destroy
        send(&mut b, TABS_NEW, json!({}));
        let first = tab_ids(&mut b)[0].clone();
        b.run_command("tab.discard", Source::Page);
        let o = send(&mut b, TABS_CLOSE, json!({"id": first}));
        assert!(destroyed(&o).is_empty());
    }

    #[test]
    fn next_and_previous_wrap_around_and_focus_the_page() {
        let (_t, mut b) = backend("step");
        b.startup();
        let ids = open_tabs(&mut b, 2);
        let first = tab_ids(&mut b)[0].clone();
        assert_eq!(selected(&mut b), ids[1]);
        let o = b.run_command("tab.next", Source::Page);
        assert_eq!(selected(&mut b), first, "after the last comes the first");
        assert_eq!(o.effects.last(), Some(&Effect::FocusPage));
        b.run_command("tab.prev", Source::Page);
        assert_eq!(selected(&mut b), ids[1], "before the first comes the last");
        b.run_command("tab.prev", Source::Shell);
        assert_eq!(selected(&mut b), ids[0]);
    }

    #[test]
    fn numbered_and_last_tab_commands_select_by_position() {
        let (_t, mut b) = backend("numbered");
        b.startup();
        let ids = open_tabs(&mut b, 3);
        let first = tab_ids(&mut b)[0].clone();
        b.run_command("tab.select.1", Source::Page);
        assert_eq!(selected(&mut b), first);
        b.run_command("tab.select.3", Source::Page);
        assert_eq!(selected(&mut b), ids[1]);
        b.run_command("tab.select.last", Source::Page);
        assert_eq!(selected(&mut b), ids[2]);
        let o = b.run_command("tab.select.8", Source::Page);
        assert!(o.effects.is_empty(), "there is no 8th tab: nothing happens");
        assert_eq!(selected(&mut b), ids[2]);
    }

    #[test]
    fn move_commands_reorder_and_stop_at_the_ends() {
        let (_t, mut b) = backend("movecmd");
        b.startup();
        let ids = open_tabs(&mut b, 2);
        let first = tab_ids(&mut b)[0].clone();
        assert_eq!(selected(&mut b), ids[1]);
        b.run_command("tab.move.down", Source::Page); // already last
        assert_eq!(
            tab_ids(&mut b),
            vec![first.clone(), ids[0].clone(), ids[1].clone()]
        );
        b.run_command("tab.move.up", Source::Page);
        assert_eq!(
            tab_ids(&mut b),
            vec![first.clone(), ids[1].clone(), ids[0].clone()]
        );
        b.run_command("tab.move.up", Source::Page);
        b.run_command("tab.move.up", Source::Page); // already first
        assert_eq!(tab_ids(&mut b), vec![ids[1].clone(), first, ids[0].clone()]);
        // and the message form
        let o = send(&mut b, TABS_MOVE, json!({"id": ids[0], "position": 0}));
        assert_eq!(first_reply(&o).0, "workspaces");
        assert_eq!(tab_ids(&mut b)[0], ids[0]);
        let o = send(&mut b, TABS_MOVE, json!({"id": ids[0], "position": 9}));
        assert_eq!(first_reply(&o).0, "error");
    }

    #[test]
    fn only_the_last_few_views_stay_alive_and_a_discarded_tab_is_recreated_when_selected() {
        let (_t, mut b) = backend("live");
        b.startup();
        let mut destroyed_total = vec![];
        for _ in 0..(MAX_LIVE_VIEWS + 3) {
            let o = b.run_command("tab.new", Source::Page);
            destroyed_total.extend(destroyed(&o));
        }
        assert_eq!(
            b.live.len(),
            MAX_LIVE_VIEWS,
            "never more views than the limit"
        );
        assert_eq!(
            destroyed_total.len(),
            4,
            "12 tabs shown, 8 alive: 4 destroyed"
        );
        assert_eq!(
            tab_ids(&mut b).len(),
            MAX_LIVE_VIEWS + 4,
            "every record is still there"
        );
        // selecting a discarded tab asks the host to show it, which creates the view from the record
        let revived = &destroyed_total[0];
        let o = send(&mut b, TABS_SELECT, json!({"id": revived}));
        assert_eq!(shown(&o), vec![revived.clone()]);
        assert_eq!(b.live.len(), MAX_LIVE_VIEWS);
        assert!(b.live.iter().any(|(_, t)| t.as_str() == revived));
    }

    #[test]
    fn discard_destroys_every_background_view_and_keeps_the_visible_one() {
        let (_t, mut b) = backend("discard");
        b.startup();
        let ids = open_tabs(&mut b, 3);
        let o = b.run_command("tab.discard", Source::Page);
        assert_eq!(destroyed(&o).len(), 3);
        assert!(
            !destroyed(&o).contains(&ids[2]),
            "the visible tab keeps its view"
        );
        assert_eq!(b.live.len(), 1);
        assert_eq!(tab_urls(&mut b).len(), 4, "no record was removed");
    }

    #[test]
    fn a_page_closing_itself_is_a_close_and_a_view_the_app_destroyed_is_not() {
        let (_t, mut b) = backend("pageclose");
        b.startup();
        let ids = open_tabs(&mut b, 1);
        let ws = b.active().clone();
        // the app discards the first tab's view; the host later hears it closed, which must not count as the page closing itself
        b.run_command("tab.discard", Source::Page);
        let first = tab_ids(&mut b)[0].clone();
        assert!(b.view_closed_by_page(&ws, &id(&first)).is_none());
        assert_eq!(tab_urls(&mut b).len(), 2);
        // the visible tab's page calls window.close(): its tab is closed and a neighbour shown
        let o = b.view_closed_by_page(&ws, &id(&ids[0])).expect("a close");
        assert_eq!(tab_urls(&mut b).len(), 1);
        assert_eq!(shown(&o), vec![first.clone()]);
        assert!(destroyed(&o).is_empty(), "its view is already gone");
        // a second report for the same tab changes nothing
        assert!(b.view_closed_by_page(&ws, &id(&ids[0])).is_none());
    }

    #[test]
    fn tab_ids_from_another_workspace_are_not_reachable() {
        let (_t, mut b) = backend("crossws");
        b.startup();
        let in_first = selected(&mut b);
        send(&mut b, CREATE, json!({"name": "Two", "colour": 2}));
        // the active workspace is Two; a tab id from Default must be "no such tab" for every operation
        for (msg, p) in [
            (TABS_SELECT, json!({"id": in_first})),
            (TABS_CLOSE, json!({"id": in_first})),
            (TABS_MOVE, json!({"id": in_first, "position": 0})),
        ] {
            let o = send(&mut b, msg, p);
            assert_eq!(first_reply(&o).0, "error", "{msg}");
            assert!(o.effects.is_empty(), "{msg}");
        }
        let ws = id_of(&b, "Default");
        b.run_command("workspace.prev", Source::Page);
        assert_eq!(tabs(&mut b)["workspace"], ws);
        assert_eq!(
            selected(&mut b),
            in_first,
            "and nothing was changed in Default"
        );
    }

    #[test]
    fn titles_and_addresses_update_the_record_and_reach_the_shell_only_for_the_active_workspace() {
        let (_t, mut b) = backend("titles");
        b.startup();
        let ws = b.active().clone();
        let tid = id(&selected(&mut b));
        let reply = b
            .tab_title(&ws, &tid, "Hello\nthere")
            .expect("active workspace");
        let t: Value = serde_json::from_str(&reply.payload).unwrap();
        assert_eq!(t["sections"][0]["tabs"][0]["title"], "Hello there");
        let reply = b
            .tab_url(&ws, &tid, "https://moved.example/page")
            .expect("stored");
        assert!(reply.payload.contains("moved.example"));
        assert!(b.tab_url(&ws, &tid, "javascript:alert(1)").is_none());
        assert!(b.tab_url(&ws, &tid, "data:text/html,x").is_none());
        assert_eq!(tab_urls(&mut b), vec!["https://moved.example/page"]);
        // a background workspace's tab updates its record but pushes nothing
        send(&mut b, CREATE, json!({"name": "Two", "colour": 2}));
        assert!(b.tab_title(&ws, &tid, "Background").is_none());
        // a tab that no longer exists is ignored
        assert!(b.tab_title(&ws, &Id::new("tab"), "x").is_none());
    }

    #[test]
    fn tabs_and_the_selection_survive_a_restart() {
        let t = TempDir::new("restore");
        let (urls, sel);
        {
            let mut b = Backend::open(DataDir::open(t.path()).unwrap(), START).unwrap();
            b.startup();
            send(&mut b, TABS_NEW, json!({"url": "https://two.example/"}));
            send(&mut b, TABS_NEW, json!({"url": "https://three.example/"}));
            b.run_command("tab.prev", Source::Page);
            urls = tab_urls(&mut b);
            sel = selected(&mut b);
        }
        let mut b = Backend::open(DataDir::open(t.path()).unwrap(), START).unwrap();
        let o = b.startup();
        assert_eq!(tab_urls(&mut b), urls);
        assert_eq!(selected(&mut b), sel);
        assert_eq!(shown(&o), vec![sel], "the remembered tab is the one shown");
    }

    // ---- other commands ----

    #[test]
    fn workspace_commands_cycle_through_workspaces_and_focus_the_page() {
        let (_t, mut b) = backend("wscycle");
        b.startup();
        send(&mut b, CREATE, json!({"name": "Two", "colour": 2}));
        let (one, two) = (id_of(&b, "Default"), id_of(&b, "Two"));
        let o = b.run_command("workspace.next", Source::Page);
        assert_eq!(state(&b)["active"], one, "wraps from the last to the first");
        assert_eq!(o.effects.last(), Some(&Effect::FocusPage));
        b.run_command("workspace.prev", Source::Page);
        assert_eq!(state(&b)["active"], two);
    }

    #[test]
    fn focus_commands_depend_on_where_the_key_was_pressed() {
        let (_t, mut b) = backend("focus");
        assert_eq!(
            b.run_command("focus.cycle", Source::Page).effects,
            vec![Effect::FocusShell]
        );
        assert_eq!(
            b.run_command("focus.cycle", Source::Shell).effects,
            vec![Effect::FocusPage]
        );
        assert_eq!(
            b.run_command("focus.page", Source::Shell).effects,
            vec![Effect::FocusPage]
        );
    }

    #[test]
    fn every_registered_command_runs_without_error_on_a_fresh_backend() {
        for cmd in commands::COMMANDS {
            let (_t, mut b) = backend("allcmds");
            b.startup();
            let o = b.run_command(cmd.id, Source::Page);
            assert!(
                o.replies.iter().all(|r| r.name != "error"),
                "{} gave an error: {:?}",
                cmd.id,
                o.replies
            );
        }
    }

    #[test]
    fn the_command_message_runs_registered_commands_and_refuses_unknown_ones() {
        let (_t, mut b) = backend("cmdmsg");
        b.startup();
        let o = send(&mut b, COMMAND, json!({"id": "tab.new"}));
        assert_eq!(tab_urls(&mut b).len(), 2);
        assert_eq!(o.effects.last(), Some(&Effect::FocusPage));
        assert_eq!(
            b.handle(COMMAND, r#"{"id":"system.format-disk"}"#),
            Err(Refused::UnknownMessage("system.format-disk".into()))
        );
        assert_eq!(b.run_command("nope", Source::Page).replies[0].name, "error");
    }

    // ---- protocol ----

    #[test]
    fn protocol_violations_are_refused() {
        let (_t, mut b) = backend("refused");
        assert_eq!(
            b.handle("delete-everything", ""),
            Err(Refused::UnknownMessage("delete-everything".into()))
        );
        for (name, payload, why) in [
            (CREATE, "not json", "not json"),
            (CREATE, r#"{"name":"x"}"#, "a missing field"),
            (
                CREATE,
                r#"{"name":"x","colour":1,"extra":true}"#,
                "an unknown field",
            ),
            (CREATE, r#"{"name":5,"colour":1}"#, "a wrong type"),
            (DELETE, r#"{"id":"../../Windows"}"#, "a path trick in an id"),
            (LIST, r#"{"x":1}"#, "a payload where none is allowed"),
            (TABS_LIST, r#"{"x":1}"#, "a payload where none is allowed"),
            (TABS_CLOSE, r#"{"id":"tab-1","x":1}"#, "an unknown field"),
            (
                TABS_MOVE,
                r#"{"id":"tab-1","position":-1}"#,
                "a negative position",
            ),
            (TABS_NEW, r#"{"url":5}"#, "a wrong type"),
            (COMMAND, r#"{"id":5}"#, "a wrong type"),
        ] {
            assert!(
                matches!(b.handle(name, payload), Err(Refused::BadPayload { .. })),
                "{name}: {why}"
            );
        }
        let big = "a".repeat(MAX_PAYLOAD_BYTES + 1);
        assert_eq!(
            b.handle(PING, &big),
            Err(Refused::PayloadTooLarge(MAX_PAYLOAD_BYTES + 1))
        );
        assert_eq!(
            names(&b),
            vec!["Default"],
            "nothing was changed by any refusal"
        );
    }

    #[test]
    fn a_refused_message_never_echoes_the_payload() {
        let (_t, mut b) = backend("noecho");
        let secret = "s3cr3t-token-value";
        // the secret sits in a field of the wrong type, in an unknown field's name and value, and in a bad id
        for payload in [
            format!(r#"{{"name":"x","colour":"{secret}"}}"#),
            format!(r#"{{"name":"x","colour":1,"{secret}":"{secret}"}}"#),
            format!(r#"{{"name":{{"{secret}":1}},"colour":1}}"#),
        ] {
            let err = b.handle(CREATE, &payload).unwrap_err();
            assert!(!err.to_string().contains(secret), "{err}");
        }
        let err = b
            .handle(DELETE, &format!(r#"{{"id":"{secret}"}}"#))
            .unwrap_err();
        assert!(!err.to_string().contains(secret), "{err}");
    }

    // ---- data folder ----

    #[test]
    fn a_deleted_workspaces_folders_are_removed_by_the_sweep() {
        let (_t, mut b) = backend("sweep");
        send(&mut b, CREATE, json!({"name": "Two", "colour": 2}));
        let two = id(&id_of(&b, "Two"));
        let first = id_of(&b, "Default");
        send(&mut b, DELETE, json!({"id": two.as_str()}));
        // the host has not removed the folders (Chromium still holds them): they are still on disk, with no record
        assert!(b.data().workspace_dir(&two).is_dir() && b.data().chromium_profile(&two).is_dir());
        assert_eq!(b.sweep_orphans().unwrap(), vec![two.clone()]);
        assert!(
            !b.data().workspace_dir(&two).exists() && !b.data().chromium_profile(&two).exists()
        );
        assert!(
            b.data().workspace_dir(&id(&first)).is_dir(),
            "the remaining workspace is untouched"
        );
        assert!(
            b.sweep_orphans().unwrap().is_empty(),
            "a second sweep finds nothing"
        );
    }

    #[test]
    fn open_removes_orphaned_workspace_folders_and_keeps_foreign_ones() {
        let t = TempDir::new("orphans");
        let data = DataDir::open(t.path()).unwrap();
        let orphan = Id::new("ws");
        std::fs::create_dir_all(data.workspace_dir(&orphan)).unwrap();
        std::fs::create_dir_all(data.chromium_profile(&orphan)).unwrap();
        let chromium_own = data.chromium_root().join("hyphen-data"); // CEF's own folder
        std::fs::create_dir_all(&chromium_own).unwrap();
        // a folder whose name is a valid id of another kind is not a workspace either, and is never removed on a guess
        let other_kind = data.chromium_root().join(Id::new("tab").as_str());
        std::fs::create_dir_all(&other_kind).unwrap();
        let b = Backend::open(data, START).unwrap();
        assert_eq!(b.orphans_removed(), std::slice::from_ref(&orphan));
        assert!(!b.data().workspace_dir(&orphan).exists());
        assert!(
            chromium_own.is_dir(),
            "a folder that is not a workspace is never removed"
        );
        assert!(
            other_kind.is_dir(),
            "nor is one that is an id of another kind"
        );
    }

    #[test]
    fn an_unusable_start_url_falls_back_to_a_blank_page() {
        let t = TempDir::new("badstart");
        let mut b = Backend::open(DataDir::open(t.path()).unwrap(), "javascript:alert(1)").unwrap();
        b.startup();
        assert_eq!(tab_urls(&mut b), vec!["about:blank"]);
    }

    // ---- M1.4: expanded workspaces, privacy of collapsed ones, opening a tab elsewhere, groups, accent ----

    /// A backend with workspaces "Default", "B" and "C" (C active), each with a tab titled after its workspace.
    fn three_workspaces(tag: &str) -> (TempDir, Backend) {
        let (t, mut b) = backend(tag);
        b.startup();
        for (name, colour) in [("B", 0x00AA00u32), ("C", 0x0000AAu32)] {
            send(&mut b, CREATE, json!({"name": name, "colour": colour}));
            let ws = b.active().clone();
            let tid = id(&selected(&mut b));
            b.tab_title(&ws, &tid, &format!("secret title of {name}"));
            b.tab_url(
                &ws,
                &tid,
                &format!("https://{}.secret.example/", name.to_lowercase()),
            );
        }
        (t, b)
    }

    #[test]
    fn only_the_active_workspace_is_expanded_at_first() {
        let (_t, mut b) = backend("expanded0");
        b.startup();
        send(&mut b, CREATE, json!({"name": "Two", "colour": 2}));
        assert_eq!(
            expanded_names(&mut b),
            vec!["Two"],
            "creating a workspace activates and expands it, and collapses the rest"
        );
    }

    #[test]
    fn a_collapsed_workspaces_tabs_never_appear_in_the_reply() {
        // S11 at the source: the shell only ever receives data for expanded workspaces
        let (_t, mut b) = three_workspaces("s11core");
        assert_eq!(expanded_names(&mut b), vec!["C"]);
        let payload = b.tabs_reply().payload;
        for secret in [
            "secret title of B",
            "b.secret.example",
            "secret title of Default",
        ] {
            assert!(
                !payload.contains(secret),
                "the reply leaks {secret}: {payload}"
            );
        }
        assert!(
            payload.contains("secret title of C") && payload.contains("c.secret.example"),
            "the expanded one is there"
        );
        // and the workspaces reply carries names and colours only
        let ws = b.state_reply().payload;
        assert!(!ws.contains("secret") && !ws.contains("example"), "{ws}");
    }

    #[test]
    fn the_chevron_expands_and_collapses_without_switching() {
        let (_t, mut b) = three_workspaces("chevron");
        let b_id = id_of(&b, "B");
        let active_before = state(&b)["active"].clone();
        let o = send(
            &mut b,
            WORKSPACES_EXPAND,
            json!({"id": b_id, "expanded": true}),
        );
        assert!(o.effects.is_empty(), "no page changes");
        assert_eq!(
            state(&b)["active"],
            active_before,
            "the active workspace did not change"
        );
        assert_eq!(expanded_names(&mut b), vec!["B", "C"]);
        assert!(
            b.tabs_reply().payload.contains("secret title of B"),
            "expanding reveals its tabs"
        );
        send(
            &mut b,
            WORKSPACES_EXPAND,
            json!({"id": b_id, "expanded": false}),
        );
        assert_eq!(expanded_names(&mut b), vec!["C"]);
        assert!(
            !b.tabs_reply().payload.contains("secret title of B"),
            "collapsing hides them again"
        );
        // the active workspace can be collapsed too (its page stays)
        let c_id = id_of(&b, "C");
        send(
            &mut b,
            WORKSPACES_EXPAND,
            json!({"id": c_id, "expanded": false}),
        );
        assert!(expanded_names(&mut b).is_empty());
        let ghost = Id::new("ws").to_string();
        let o = send(
            &mut b,
            WORKSPACES_EXPAND,
            json!({"id": ghost, "expanded": true}),
        );
        assert_eq!(first_reply(&o).0, "error");
    }

    #[test]
    fn activating_a_workspace_expands_it_and_collapses_the_others() {
        let (_t, mut b) = three_workspaces("activate_collapse");
        let (a_id, b_id) = (id_of(&b, "Default"), id_of(&b, "B"));
        send(
            &mut b,
            WORKSPACES_EXPAND,
            json!({"id": b_id, "expanded": true}),
        );
        assert_eq!(expanded_names(&mut b), vec!["B", "C"]);
        send(&mut b, ACTIVATE, json!({"id": a_id}));
        assert_eq!(expanded_names(&mut b), vec!["Default"]);
        b.run_command("workspace.next", Source::Page);
        assert_eq!(expanded_names(&mut b), vec!["B"]);
    }

    #[test]
    fn opening_a_tab_in_another_workspace_switches_to_it_and_collapses_the_rest() {
        let (_t, mut b) = three_workspaces("opentab");
        let b_id = id_of(&b, "B");
        send(
            &mut b,
            WORKSPACES_EXPAND,
            json!({"id": b_id, "expanded": true}),
        );
        let all: Value = serde_json::from_str(&b.tabs_reply().payload).unwrap();
        let b_section = all["sections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["workspace"] == b_id)
            .unwrap()
            .clone();
        let b_tab = b_section["tabs"][0]["id"].as_str().unwrap().to_string();
        let o = send(&mut b, TABS_OPEN, json!({"workspace": b_id, "id": b_tab}));
        assert_eq!(state(&b)["active"], b_id);
        assert_eq!(expanded_names(&mut b), vec!["B"]);
        assert!(
            matches!(&o.effects[..], [Effect::Show { workspace, tab, colour, .. }] if workspace.as_str() == b_id && tab.as_str() == b_tab && *colour == 0x00AA00)
        );
        // a tab that belongs to a different workspace is "no such tab", whichever workspace the message names
        let c_tab = {
            let c_id = id_of(&b, "Default");
            send(&mut b, ACTIVATE, json!({"id": c_id}));
            selected(&mut b)
        };
        let o = send(&mut b, TABS_OPEN, json!({"workspace": b_id, "id": c_tab}));
        assert_eq!(first_reply(&o).0, "error");
        assert!(o.effects.is_empty());
        let ghost_ws = Id::new("ws").to_string();
        let o = send(
            &mut b,
            TABS_OPEN,
            json!({"workspace": ghost_ws, "id": c_tab}),
        );
        assert_eq!(first_reply(&o).0, "error");
    }

    #[test]
    fn recolouring_asks_the_host_to_redraw_and_a_shown_tab_carries_its_workspaces_colour() {
        let (_t, mut b) = backend("accent");
        let wid = id_of(&b, "Default");
        send(&mut b, COLOUR, json!({"id": wid, "colour": 0x123456}));
        let o = send(&mut b, TABS_NEW, json!({}));
        assert!(matches!(&o.effects[..], [Effect::Show { colour, .. }] if *colour == 0x123456));
    }

    #[test]
    fn groups_can_be_made_named_collapsed_and_left_through_messages() {
        let (_t, mut b) = backend("groups");
        b.startup();
        let ids = open_tabs(&mut b, 2); // [t0, ids0, ids1]
        let t0 = tab_ids(&mut b)[0].clone();
        let o = send(&mut b, GROUPS_NEW, json!({"tab": ids[0]}));
        assert!(o.effects.is_empty(), "grouping changes no page");
        let section = tabs(&mut b);
        let g = section["groups"][0]["id"].as_str().unwrap().to_string();
        assert_eq!(section["groups"][0]["name"], "Group");
        assert_eq!(
            tab_ids(&mut b),
            vec![t0.clone(), ids[1].clone(), ids[0].clone()],
            "ungrouped tabs first, the group's tab after"
        );
        assert_eq!(tabs(&mut b)["tabs"][2]["group"], g);
        send(&mut b, GROUPS_RENAME, json!({"id": g, "name": "Reading"}));
        send(&mut b, GROUPS_COLLAPSE, json!({"id": g, "collapsed": true}));
        let section = tabs(&mut b);
        assert_eq!(
            (
                section["groups"][0]["name"].as_str(),
                section["groups"][0]["collapsed"].as_bool()
            ),
            (Some("Reading"), Some(true))
        );
        // add another tab to it (the group keeps its tabs in their existing relative order), then take both out
        send(&mut b, TABS_GROUP, json!({"id": ids[1], "group": g}));
        assert_eq!(
            tab_ids(&mut b),
            vec![t0.clone(), ids[1].clone(), ids[0].clone()]
        );
        send(&mut b, TABS_GROUP, json!({"id": ids[0], "group": null}));
        send(&mut b, TABS_GROUP, json!({"id": ids[1], "group": null}));
        assert!(
            tabs(&mut b)["groups"].as_array().unwrap().is_empty(),
            "an emptied group is gone"
        );
        // bad input is a user error, and nothing changes
        let o = send(&mut b, GROUPS_RENAME, json!({"id": g, "name": "x"}));
        assert_eq!(first_reply(&o).0, "error", "the group no longer exists");
        let o = send(
            &mut b,
            GROUPS_NEW,
            json!({"tab": Id::new("tab").to_string()}),
        );
        assert_eq!(first_reply(&o).0, "error");
        let o = send(
            &mut b,
            TABS_GROUP,
            json!({"id": ids[0], "group": Id::new("grp").to_string()}),
        );
        assert_eq!(first_reply(&o).0, "error");
    }

    #[test]
    fn group_commands_group_and_ungroup_the_selected_tab_and_new_tabs_join_its_group() {
        let (_t, mut b) = backend("groupcmd");
        b.startup();
        let first = selected(&mut b);
        b.run_command("tab.group.new", Source::Page);
        assert_eq!(tabs(&mut b)["groups"].as_array().unwrap().len(), 1);
        // a new tab opened from a grouped tab stays in that group, so the group's tabs stay together
        b.run_command("tab.new", Source::Page);
        let second = selected(&mut b);
        let section = tabs(&mut b);
        assert_eq!(section["tabs"][1]["group"], section["groups"][0]["id"]);
        assert_eq!(section["tabs"][0]["group"], section["groups"][0]["id"]);
        b.run_command("tab.ungroup", Source::Page);
        assert_eq!(
            tabs(&mut b)["tabs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["id"] == second.as_str())
                .unwrap()["group"],
            Value::Null
        );
        assert_eq!(
            tabs(&mut b)["groups"].as_array().unwrap().len(),
            1,
            "the first tab is still in it"
        );
        b.run_command("tab.select.1", Source::Page);
        assert_eq!(selected(&mut b), tab_ids(&mut b)[0]);
        let _ = first;
    }

    #[test]
    fn tab_order_follows_the_sidebar_so_ctrl_tab_does_too() {
        let (_t, mut b) = backend("groupstep");
        b.startup();
        open_tabs(&mut b, 2); // [t0, t1, t2], t2 selected
        let before = tab_ids(&mut b);
        send(&mut b, GROUPS_NEW, json!({"tab": before[0]})); // t0 leaves the front: [t1, t2 | G: t0]
        let visual = tab_ids(&mut b);
        assert_eq!(
            visual,
            vec![before[1].clone(), before[2].clone(), before[0].clone()]
        );
        send(&mut b, TABS_SELECT, json!({"id": visual[0]}));
        let mut walked = vec![selected(&mut b)];
        for _ in 0..2 {
            b.run_command("tab.next", Source::Page);
            walked.push(selected(&mut b));
        }
        assert_eq!(
            walked, visual,
            "Ctrl+Tab visits the tabs in the order the sidebar lists them"
        );
    }

    #[test]
    fn group_and_expand_messages_refuse_bad_payloads() {
        let (_t, mut b) = backend("m14refused");
        for (name, payload) in [
            (WORKSPACES_EXPAND, r#"{"id":"ws-1"}"#),
            (WORKSPACES_EXPAND, r#"{"id":"ws-1","expanded":"yes"}"#),
            (TABS_OPEN, r#"{"workspace":"ws-1"}"#),
            (TABS_OPEN, r#"{"workspace":"../x","id":"tab-1"}"#),
            (TABS_GROUP, r#"{"id":"tab-1"}"#),
            (TABS_GROUP, r#"{"id":"tab-1","group":5}"#),
            (GROUPS_NEW, r#"{"tab":"tab-1","extra":1}"#),
            (GROUPS_RENAME, r#"{"id":"grp-1"}"#),
            (GROUPS_COLLAPSE, r#"{"id":"grp-1","collapsed":"no"}"#),
        ] {
            assert!(
                matches!(b.handle(name, payload), Err(Refused::BadPayload { .. })),
                "{name} {payload}"
            );
        }
    }
}
