//! The launcher (M2): `Ctrl+P` search over what the user may see right now.
//!
//! **Scope is decided here, before anything is read.** The launcher page sends a *request* (`launcher.scope`); core resolves it to the
//! workspaces it is allowed to search (unknown ids are dropped, nothing outside the existing workspaces can be named), holds the result,
//! and a search carries only the query. Only the databases of those workspaces are opened, so results from any other workspace do not
//! exist to be filtered later. Closing the launcher resets the scope in core and no result list is kept anywhere (S3, S4).
//!
//! A result's id is opaque text; `launcher.run` resolves it again against the *current* scope and refuses one that is outside it (S2).

use super::*;
use crate::{bookmark, commands, entry, history, search};

pub const LAUNCHER_OPEN: &str = "launcher.open";
pub const LAUNCHER_CLOSE: &str = "launcher.close";
pub const LAUNCHER_SEARCH: &str = "launcher.search";
pub const LAUNCHER_SCOPE: &str = "launcher.scope";
pub const LAUNCHER_RUN: &str = "launcher.run";
pub const LAUNCHER_DRAG: &str = "launcher.drag";

pub const MAX_QUERY_CHARS: usize = 256;
pub const MAX_RESULTS: usize = 30;
const MAX_DRAG: i32 = 4000;
const CMD_OPEN: &str = "launcher.open";

/// What the launcher asked to search. Resolved by `Backend::scope_ids`; never used as it is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ScopeRequest {
    #[default]
    Contextual,
    Selected(Vec<Id>),
    All,
}

#[derive(Debug, Default)]
pub struct LauncherState {
    pub open: bool,
    pub scope: ScopeRequest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SearchIn {
    pub(super) query: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScopeIn {
    pub(super) kind: String,
    #[serde(default)]
    pub(super) workspaces: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DragIn {
    pub(super) dx: i32,
    pub(super) dy: i32,
}

#[derive(Serialize)]
struct WorkspaceRef {
    id: String,
    name: String,
    colour: u32,
}

#[derive(Serialize)]
struct ResultOut {
    id: String,
    kind: &'static str,
    title: String,
    subtitle: String,
    /// The workspace a result belongs to (shown with its colour); none for a command.
    workspace: Option<WorkspaceRef>,
    score: u32,
    /// A short label shown as a badge: the environment of an entry (`prod` is drawn in red).
    tag: Option<&'static str>,
}

#[derive(Serialize)]
struct ScopeOut {
    kind: &'static str,
    /// The workspaces being searched, so the launcher can say so on screen.
    pub(super) workspaces: Vec<String>,
}

#[derive(Serialize)]
struct ResultsOut {
    scope: ScopeOut,
    /// Every workspace's name and colour (the sidebar already shows these), so the scope can be changed.
    all: Vec<WorkspaceRef>,
    results: Vec<ResultOut>,
}

impl Backend {
    fn workspace_ref(w: &workspace::Workspace) -> WorkspaceRef {
        WorkspaceRef {
            id: w.id.to_string(),
            name: w.name.clone(),
            colour: w.colour,
        }
    }

    /// The workspaces the launcher may search right now: the request, narrowed to workspaces that exist. A request that names nothing
    /// valid falls back to the contextual scope (the active workspace) rather than to a wider one.
    pub(super) fn scope_ids(&self) -> Vec<Id> {
        let all = workspace::list(&self.global).unwrap_or_default();
        let existing = |id: &Id| all.iter().any(|w| &w.id == id);
        match &self.launcher.scope {
            ScopeRequest::All => all.iter().map(|w| w.id.clone()).collect(),
            ScopeRequest::Selected(ids) => {
                let ok: Vec<Id> = ids.iter().filter(|i| existing(i)).cloned().collect();
                if ok.is_empty() {
                    vec![self.active.clone()]
                } else {
                    ok
                }
            }
            ScopeRequest::Contextual => vec![self.active.clone()],
        }
    }

    fn scope_kind(&self) -> &'static str {
        match self.launcher.scope {
            ScopeRequest::Contextual => "contextual",
            ScopeRequest::Selected(_) => "selected",
            ScopeRequest::All => "all",
        }
    }

    /// Open the launcher (or keep it open) and tell it what it is searching.
    pub(super) fn launcher_open(&mut self) -> Outcome {
        self.launcher.open = true;
        let mut o = Outcome {
            replies: vec![],
            effects: vec![Effect::ShowLauncher],
        };
        o.replies.push(self.launcher_results(""));
        o
    }

    /// Close the launcher: the scope goes back to the context's default and nothing is kept (S4).
    pub(super) fn launcher_close(&mut self) -> Outcome {
        self.launcher = LauncherState::default();
        Outcome {
            replies: vec![],
            effects: vec![Effect::HideLauncher],
        }
    }

    pub(super) fn launcher_set_scope(
        &mut self,
        kind: &str,
        ids: &[String],
    ) -> Result<Outcome, Failure> {
        self.launcher.scope = match kind {
            "contextual" => ScopeRequest::Contextual,
            "all" => ScopeRequest::All,
            "selected" => {
                // ids that do not parse are ignored; ids that parse but do not exist are narrowed away in `scope_ids`
                ScopeRequest::Selected(ids.iter().filter_map(|i| Id::parse(i).ok()).collect())
            }
            _ => return Err("unknown search scope".into()),
        };
        Ok(Outcome {
            replies: vec![self.launcher_results("")],
            effects: vec![],
        })
    }

    pub(super) fn launcher_search(&mut self, query: &str) -> Result<Outcome, Failure> {
        if query.chars().count() > MAX_QUERY_CHARS {
            return Err("that search is too long".into());
        }
        Ok(Outcome {
            replies: vec![self.launcher_results(query)],
            effects: vec![],
        })
    }

    fn launcher_results(&mut self, query: &str) -> Reply {
        let query = query.trim();
        let ids = self.scope_ids();
        let all = workspace::list(&self.global).unwrap_or_default();
        let mut results: Vec<ResultOut> = Vec::new();
        for w in all.iter().filter(|w| ids.contains(&w.id)) {
            // only the databases of workspaces in scope are opened
            let tabs = self.with_db(&w.id, tab::list).unwrap_or_default();
            let mut known: HashSet<String> = HashSet::new();
            for t in &tabs {
                known.insert(t.url.split('#').next().unwrap_or("").to_string());
            }
            // bookmarks first (so a history row for the same page is not listed twice)
            for b in self
                .with_db(&w.id, |c| Ok(bookmark::all(c)?))
                .unwrap_or_default()
            {
                let title = if b.title.trim().is_empty() {
                    b.url.clone()
                } else {
                    b.title.clone()
                };
                let s = [
                    search::score(query, &title).map(|s| s * 2),
                    search::score(query, &b.url),
                ]
                .into_iter()
                .flatten()
                .max();
                if let Some(s) = s {
                    known.insert(b.url.split('#').next().unwrap_or("").to_string());
                    results.push(ResultOut {
                        id: format!("bm/{}/{}", w.id, b.id),
                        kind: "bookmark",
                        title,
                        subtitle: b.url,
                        workspace: Some(Self::workspace_ref(w)),
                        score: s + 8,
                        tag: None,
                    });
                }
            }
            // saved entries (Home Screen links), listed like bookmarks; a page that is an entry is not also shown from history
            for e in self
                .with_home(&w.id, |c| Ok(entry::all(c)?))
                .unwrap_or_default()
            {
                let s = [
                    search::score(query, &e.title).map(|s| s * 2),
                    search::score(query, &e.url),
                ]
                .into_iter()
                .flatten()
                .max();
                if let Some(s) = s {
                    known.insert(e.url.split('#').next().unwrap_or("").to_string());
                    results.push(ResultOut {
                        id: format!("entry/{}/{}", w.id, e.id),
                        kind: "entry",
                        title: e.title,
                        subtitle: e.url,
                        workspace: Some(Self::workspace_ref(w)),
                        score: s + 10,
                        tag: e.env.map(entry::Env::as_str),
                    });
                }
            }
            // history is only searched, never listed: an empty query shows no history (S3: no recent list)
            if !query.is_empty() {
                let mut found: Vec<ResultOut> = Vec::new();
                for v in self
                    .with_db(&w.id, |c| Ok(history::all(c)?))
                    .unwrap_or_default()
                {
                    if known.contains(&v.url) {
                        continue;
                    }
                    let title = if v.title.trim().is_empty() {
                        v.url.clone()
                    } else {
                        v.title.clone()
                    };
                    let s = [
                        search::score(query, &title).map(|s| s * 2),
                        search::score(query, &v.url),
                    ]
                    .into_iter()
                    .flatten()
                    .max();
                    if let Some(s) = s {
                        found.push(ResultOut {
                            id: format!("hist/{}/{}", w.id, v.id),
                            kind: "history",
                            title,
                            subtitle: v.url,
                            workspace: Some(Self::workspace_ref(w)),
                            // pages visited often rank a little higher
                            score: s + (v.visits.clamp(0, 10) as u32),
                            tag: None,
                        });
                    }
                }
                found.sort_by_key(|r| std::cmp::Reverse(r.score));
                found.truncate(MAX_RESULTS);
                results.extend(found);
            }
            for t in tabs {
                let title = if t.title.trim().is_empty() {
                    t.url.clone()
                } else {
                    t.title.clone()
                };
                let s = [
                    search::score(query, &title).map(|s| s * 2),
                    search::score(query, &t.url),
                ]
                .into_iter()
                .flatten()
                .max();
                if let Some(s) = s {
                    results.push(ResultOut {
                        id: format!("tab/{}/{}", w.id, t.id),
                        kind: "tab",
                        title,
                        subtitle: t.url,
                        workspace: Some(Self::workspace_ref(w)),
                        // an open tab is more relevant than a command with the same match
                        score: s + 5,
                        tag: None,
                    });
                }
            }
        }
        for c in commands::COMMANDS.iter().filter(|c| c.id != CMD_OPEN) {
            // a command is searched by its label, with its id as a second chance ("tab.new")
            let s = [
                search::score(query, c.label).map(|s| s * 2),
                search::score(query, c.id),
            ]
            .into_iter()
            .flatten()
            .max();
            if let Some(s) = s {
                results.push(ResultOut {
                    id: format!("cmd/{}", c.id),
                    kind: "command",
                    title: c.label.to_string(),
                    subtitle: c
                        .chords
                        .first()
                        .map(|ch| ch.to_string())
                        .unwrap_or_default(),
                    workspace: None,
                    score: s,
                    tag: None,
                });
            }
        }
        // How often and how recently each result was opened from its workspace nudges the order (never what matches, never what is listed).
        let now = entry::clock();
        let mut counts: HashMap<Id, HashMap<String, (i64, i64)>> = HashMap::new();
        for w in ids.iter().chain(std::iter::once(&self.active.clone())) {
            if !counts.contains_key(w) {
                let u = self
                    .with_home(w, |c| Ok(entry::usage(c)?))
                    .unwrap_or_default();
                counts.insert(w.clone(), u);
            }
        }
        for r in results.iter_mut() {
            let key = match r.id.split('/').collect::<Vec<_>>().as_slice() {
                [kind @ ("bm" | "hist" | "entry"), ws, n] => {
                    Id::parse(ws).ok().map(|w| (w, format!("{kind}:{n}")))
                }
                ["cmd", cmd] => Some((self.active.clone(), format!("cmd:{cmd}"))),
                _ => None,
            };
            if let Some((w, key)) = key {
                if let Some((count, last)) = counts.get(&w).and_then(|u| u.get(&key)) {
                    r.score += search::usage_boost(*count, now - last);
                }
            }
        }
        // best first; the sort is stable, so ties stay in tab order, then command order
        results.sort_by_key(|r| std::cmp::Reverse(r.score));
        results.truncate(MAX_RESULTS);
        let out = ResultsOut {
            scope: ScopeOut {
                kind: self.scope_kind(),
                workspaces: ids.iter().map(|i| i.to_string()).collect(),
            },
            all: all.iter().map(Self::workspace_ref).collect(),
            results,
        };
        Reply {
            name: "launcher.results",
            payload: serde_json::to_string(&out).expect("results serialise"),
        }
    }

    /// Run a result. Its id is checked against the scope as it is *now* (S2): a tab of a workspace outside it is refused.
    pub(super) fn launcher_run(&mut self, result_id: &str) -> Result<Outcome, Failure> {
        let mut close = self.launcher_close_after(result_id)?;
        if let Some(rest) = result_id.strip_prefix("tab/") {
            let (ws, tab_id) = rest.split_once('/').ok_or("that result is not valid")?;
            let ws = Id::parse(ws).map_err(|_| "that result is not valid")?;
            let tab_id = Id::parse(tab_id).map_err(|_| "that result is not valid")?;
            let o = self.run(
                Action::OpenTab {
                    workspace: ws,
                    id: tab_id,
                },
                Source::Shell,
            )?;
            close.replies.extend(o.replies);
            close.effects.extend(o.effects);
            close.effects.push(Effect::FocusPage);
            Ok(close)
        } else if let Some((kind, rest)) = result_id
            .strip_prefix("bm/")
            .map(|r| ("bm", r))
            .or_else(|| result_id.strip_prefix("hist/").map(|r| ("hist", r)))
            .or_else(|| result_id.strip_prefix("entry/").map(|r| ("entry", r)))
        {
            // the address comes from the database of that workspace, never from the page that sent the id
            let (ws, n) = rest.split_once('/').ok_or("that result is not valid")?;
            let ws = Id::parse(ws).map_err(|_| "that result is not valid")?;
            let n: i64 = n.parse().map_err(|_| "that result is not valid")?;
            let url = self
                .with_db(&ws, |c| {
                    Ok(match kind {
                        "bm" => bookmark::get(c, n)?.map(|b| b.url),
                        "entry" => None,
                        _ => history::get(c, n)?.map(|v| v.url),
                    })
                })?
                .or_else(|| {
                    if kind == "entry" {
                        self.with_home(&ws, |c| Ok(entry::get(c, n)?.map(|e| e.url)))
                            .ok()
                            .flatten()
                    } else {
                        None
                    }
                })
                .ok_or("that page is no longer saved")?;
            let _ = self.with_home(&ws, |c| Ok(entry::record_use(c, &format!("{kind}:{n}"))?));
            // opened as a new tab in that workspace, so it loads with that workspace's own login (S10)
            let o = self.run(Action::OpenIn { workspace: ws, url }, Source::Shell)?;
            close.replies.extend(o.replies);
            close.effects.extend(o.effects);
            Ok(close)
        } else if let Some(cmd) = result_id.strip_prefix("cmd/") {
            let cmd = commands::find(cmd)
                .filter(|c| c.id != CMD_OPEN)
                .ok_or("that result is not valid")?;
            let active = self.active.clone();
            let _ = self.with_home(&active, |c| {
                Ok(entry::record_use(c, &format!("cmd:{}", cmd.id))?)
            });
            let o = self.command(cmd.id, Source::Shell)?;
            close.replies.extend(o.replies);
            close.effects.extend(o.effects);
            Ok(close)
        } else {
            Err("that result is not valid".into())
        }
    }

    /// Validate a result id against the scope, then close the launcher (which resets the scope). Returns the close outcome.
    fn launcher_close_after(&mut self, result_id: &str) -> Result<Outcome, Failure> {
        if let Some(rest) = ["tab/", "bm/", "hist/", "entry/"]
            .iter()
            .find_map(|p| result_id.strip_prefix(p))
        {
            let ws = rest.split_once('/').map(|(w, _)| w).unwrap_or("");
            let ws = Id::parse(ws).map_err(|_| "that result is not valid")?;
            if !self.scope_ids().contains(&ws) {
                return Err("that result is outside the search scope".into());
            }
        }
        Ok(self.launcher_close())
    }

    pub(super) fn launcher_drag(dx: i32, dy: i32) -> Outcome {
        Outcome {
            replies: vec![],
            effects: vec![Effect::MoveLauncher {
                dx: dx.clamp(-MAX_DRAG, MAX_DRAG),
                dy: dy.clamp(-MAX_DRAG, MAX_DRAG),
            }],
        }
    }

    /// The saved top-left of the launcher, if the user has moved it.
    pub fn launcher_pos(&self) -> Option<(i32, i32)> {
        let text = settings::get(&self.global, "launcher_pos").ok().flatten()?;
        let (x, y) = text.split_once(',')?;
        Some((x.parse().ok()?, y.parse().ok()?))
    }

    pub fn set_launcher_pos(&self, x: i32, y: i32) {
        if let Err(e) = settings::set(&self.global, "launcher_pos", &format!("{x},{y}")) {
            eprintln!("[core] could not save the launcher position: {e}");
        }
    }
}
