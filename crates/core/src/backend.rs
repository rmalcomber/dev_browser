//! The backend: everything the shell may ask the app to do, decided here without any CEF. The host feeds it one message at a time and
//! executes what comes back: replies to send to the shell, and **effects** (show a workspace, delete a workspace's views and data) that
//! only the host can carry out.
//!
//! Messages (M1.2): `ping`, `workspaces.list`, `workspaces.create`, `workspaces.rename`, `workspaces.colour`, `workspaces.delete`,
//! `workspaces.activate`. Every workspace message is answered with the full `workspaces` state (the list and the active id), so the shell
//! never has to patch its own copy. Invariants kept here:
//! - there is always at least one workspace, and exactly one is active;
//! - a workspace id from the shell is parsed (`Id::parse`) before it is used for anything, and must name an existing workspace;
//! - unknown fields, wrong types and oversized payloads are refused, not guessed at.

use crate::data::{DataDir, DataError};
use crate::id::Id;
use crate::ipc::{Refused, Reply, MAX_PAYLOAD_BYTES};
use crate::{settings, workspace};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub const PING: &str = "ping";
pub const LIST: &str = "workspaces.list";
pub const CREATE: &str = "workspaces.create";
pub const RENAME: &str = "workspaces.rename";
pub const COLOUR: &str = "workspaces.colour";
pub const DELETE: &str = "workspaces.delete";
pub const ACTIVATE: &str = "workspaces.activate";

/// Colour and name of the workspace made on first run.
const FIRST_NAME: &str = "Default";
const FIRST_COLOUR: u32 = 0x2C5FA8;

/// Something only the host can do, in the order to do it.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Effect {
    /// Show this workspace's page (creating its profile context and view on first use) and hide the others.
    Activate(Id),
    /// Close this workspace's views, release its profile, then remove its folders (`DataDir::remove_workspace_folders`). The record is
    /// already gone.
    Delete(Id),
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

pub struct Backend {
    data: DataDir,
    global: Connection,
    active: Id,
    orphans_removed: Vec<Id>,
}

impl Backend {
    /// Open the data folder's global database, make sure there is a workspace and an active one, and sweep orphaned workspace folders
    /// (left by a crash or a locked folder). Safe to call on every start.
    pub fn open(data: DataDir) -> Result<Backend, DataError> {
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
        Ok(Backend {
            data,
            global,
            active,
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
    /// and after CEF has shut down. Chromium keeps a deleted workspace's profile open for the rest of the run, so the host sweeps again at
    /// exit and a deleted workspace's data is gone when the app closes, not only at the next start.
    pub fn sweep_orphans(&self) -> Result<Vec<Id>, DataError> {
        self.data.remove_orphans(&self.global)
    }

    /// Orphaned workspace folders removed by `open`, for the log.
    pub fn orphans_removed(&self) -> &[Id] {
        &self.orphans_removed
    }

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

    fn parse<T: for<'de> Deserialize<'de>>(message: &str, payload: &str) -> Result<T, Refused> {
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

    fn state_outcome(&self, effects: Vec<Effect>) -> Outcome {
        Outcome {
            replies: vec![self.state_reply()],
            effects,
        }
    }

    fn user_error(request: &str, message: impl std::fmt::Display) -> Outcome {
        Outcome {
            replies: vec![Self::error_reply(request, message)],
            effects: vec![],
        }
    }

    fn exists(&self, id: &Id) -> bool {
        workspace::get(&self.global, id).is_ok()
    }

    fn set_active(&mut self, id: &Id) -> Result<(), DataError> {
        self.data.open_workspace(id)?;
        settings::set(&self.global, settings::ACTIVE_WORKSPACE, id.as_str())?;
        self.active = id.clone();
        Ok(())
    }

    /// Decide what to do with one message from the shell.
    pub fn handle(&mut self, name: &str, payload: &str) -> Result<Outcome, Refused> {
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(Refused::PayloadTooLarge(payload.len()));
        }
        match name {
            PING => Ok(Outcome {
                replies: vec![Reply {
                    name: "pong",
                    payload: payload.to_string(),
                }],
                effects: vec![],
            }),
            LIST => {
                if !payload.is_empty() && payload != "{}" {
                    return Err(Refused::BadPayload {
                        message: name.into(),
                        why: "takes no payload".into(),
                    });
                }
                Ok(self.state_outcome(vec![]))
            }
            CREATE => {
                let r: CreateIn = Self::parse(name, payload)?;
                match self
                    .data
                    .create_workspace(&mut self.global, &r.name, r.colour)
                {
                    Ok(ws) => match self.set_active(&ws.id) {
                        Ok(()) => Ok(self.state_outcome(vec![Effect::Activate(ws.id)])),
                        Err(e) => Ok(Self::user_error(name, e)),
                    },
                    Err(e) => Ok(Self::user_error(name, e)),
                }
            }
            RENAME => {
                let r: RenameIn = Self::parse(name, payload)?;
                let id = Self::parse_id(name, &r.id)?;
                match workspace::rename(&self.global, &id, &r.name) {
                    Ok(_) => Ok(self.state_outcome(vec![])),
                    Err(e) => Ok(Self::user_error(name, e)),
                }
            }
            COLOUR => {
                let r: ColourIn = Self::parse(name, payload)?;
                let id = Self::parse_id(name, &r.id)?;
                match workspace::set_colour(&self.global, &id, r.colour) {
                    Ok(_) => Ok(self.state_outcome(vec![])),
                    Err(e) => Ok(Self::user_error(name, e)),
                }
            }
            ACTIVATE => {
                let r: IdIn = Self::parse(name, payload)?;
                let id = Self::parse_id(name, &r.id)?;
                if !self.exists(&id) {
                    return Ok(Self::user_error(name, "there is no such workspace"));
                }
                match self.set_active(&id) {
                    Ok(()) => Ok(self.state_outcome(vec![Effect::Activate(id)])),
                    Err(e) => Ok(Self::user_error(name, e)),
                }
            }
            DELETE => {
                let r: IdIn = Self::parse(name, payload)?;
                let id = Self::parse_id(name, &r.id)?;
                let all = match workspace::list(&self.global) {
                    Ok(a) => a,
                    Err(e) => return Ok(Self::user_error(name, e)),
                };
                let Some(index) = all.iter().position(|w| w.id == id) else {
                    return Ok(Self::user_error(name, "there is no such workspace"));
                };
                if all.len() == 1 {
                    return Ok(Self::user_error(
                        name,
                        "you cannot delete your only workspace",
                    ));
                }
                if let Err(e) = workspace::delete_record(&mut self.global, &id) {
                    return Ok(Self::user_error(name, e));
                }
                let mut effects = vec![Effect::Delete(id.clone())];
                if self.active == id {
                    // the workspace that took its place in the list, or the one before it if it was last
                    let next = all
                        .get(index + 1)
                        .or_else(|| all.get(index.wrapping_sub(1)))
                        .expect("there was more than one workspace");
                    let next = next.id.clone();
                    if let Err(e) = self.set_active(&next) {
                        return Ok(Self::user_error(name, e));
                    }
                    effects.push(Effect::Activate(next));
                }
                Ok(self.state_outcome(effects))
            }
            other => Err(Refused::UnknownMessage(other.chars().take(64).collect())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use serde_json::{json, Value};

    fn backend(tag: &str) -> (TempDir, Backend) {
        let t = TempDir::new(tag);
        let b = Backend::open(DataDir::open(t.path()).unwrap()).unwrap();
        (t, b)
    }

    fn state(b: &Backend) -> Value {
        serde_json::from_str(&b.state_reply().payload).unwrap()
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

    #[test]
    fn a_fresh_data_folder_gets_one_active_default_workspace() {
        let (_t, b) = backend("fresh");
        assert_eq!(names(&b), vec!["Default"]);
        assert_eq!(state(&b)["active"], id_of(&b, "Default"));
        assert!(b.data().workspace_db_path(b.active()).is_file());
        assert!(b.data().chromium_profile(b.active()).is_dir());
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
    fn create_makes_the_workspace_active_and_asks_the_host_to_show_it() {
        let (_t, mut b) = backend("create");
        let o = send(
            &mut b,
            CREATE,
            json!({"name": " Work ", "colour": 0x00AA00}),
        );
        let work = id_of(&b, "Work");
        assert_eq!(o.effects, vec![Effect::Activate(Id::parse(&work).unwrap())]);
        let (name, s) = first_reply(&o);
        assert_eq!(name, "workspaces");
        assert_eq!(s["active"], work);
        assert_eq!(names(&b), vec!["Default", "Work"]);
        assert!(b
            .data()
            .chromium_profile(&Id::parse(&work).unwrap())
            .is_dir());
    }

    #[test]
    fn rename_and_colour_change_the_record_only() {
        let (_t, mut b) = backend("rename");
        let id = id_of(&b, "Default");
        let o = send(&mut b, RENAME, json!({"id": id, "name": "Home"}));
        assert!(o.effects.is_empty());
        assert_eq!(names(&b), vec!["Home"]);
        let o = send(&mut b, COLOUR, json!({"id": id, "colour": 255}));
        assert!(o.effects.is_empty());
        assert_eq!(state(&b)["workspaces"][0]["colour"], 255);
    }

    #[test]
    fn bad_names_and_colours_are_user_errors_that_change_nothing() {
        let (_t, mut b) = backend("baduser");
        let id = id_of(&b, "Default");
        for (msg, p) in [
            (CREATE, json!({"name": "", "colour": 1})),
            (CREATE, json!({"name": "x", "colour": 0x1000000u32})),
            (RENAME, json!({"id": id, "name": "  "})),
            (COLOUR, json!({"id": id, "colour": 0x1000000u32})),
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
    fn activate_switches_and_survives_a_restart() {
        let t = TempDir::new("activate");
        let second;
        {
            let mut b = Backend::open(DataDir::open(t.path()).unwrap()).unwrap();
            send(&mut b, CREATE, json!({"name": "Two", "colour": 2}));
            let first = id_of(&b, "Default");
            second = id_of(&b, "Two");
            let o = send(&mut b, ACTIVATE, json!({"id": first}));
            assert_eq!(
                o.effects,
                vec![Effect::Activate(Id::parse(&first).unwrap())]
            );
            send(&mut b, ACTIVATE, json!({"id": second}));
        }
        let b = Backend::open(DataDir::open(t.path()).unwrap()).unwrap();
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
        let b = Backend::open(data).unwrap();
        assert_eq!(state(&b)["active"], id_of(&b, "Default"));
    }

    #[test]
    fn deleting_a_workspace_removes_the_record_and_tells_the_host_to_remove_the_data() {
        let (_t, mut b) = backend("delete");
        send(&mut b, CREATE, json!({"name": "Two", "colour": 2}));
        let first = id_of(&b, "Default");
        let second = id_of(&b, "Two");
        // the active one is "Two"; delete the other (not active)
        let o = send(&mut b, DELETE, json!({"id": first}));
        assert_eq!(o.effects, vec![Effect::Delete(Id::parse(&first).unwrap())]);
        assert_eq!(names(&b), vec!["Two"]);
        assert_eq!(
            state(&b)["active"],
            second,
            "the active workspace did not change"
        );
    }

    #[test]
    fn deleting_the_active_workspace_activates_a_neighbour() {
        let (_t, mut b) = backend("deleteactive");
        send(&mut b, CREATE, json!({"name": "B", "colour": 2}));
        send(&mut b, CREATE, json!({"name": "C", "colour": 3}));
        let (a, bb, c) = (id_of(&b, "Default"), id_of(&b, "B"), id_of(&b, "C"));
        // C is active and last: deleting it moves to the one before
        let o = send(&mut b, DELETE, json!({"id": c}));
        assert_eq!(
            o.effects,
            vec![
                Effect::Delete(Id::parse(&c).unwrap()),
                Effect::Activate(Id::parse(&bb).unwrap())
            ]
        );
        assert_eq!(state(&b)["active"], bb);
        // B is active and last again; activate the first and delete it: the next one takes over
        send(&mut b, ACTIVATE, json!({"id": a}));
        let o = send(&mut b, DELETE, json!({"id": a}));
        assert_eq!(
            o.effects,
            vec![
                Effect::Delete(Id::parse(&a).unwrap()),
                Effect::Activate(Id::parse(&bb).unwrap())
            ]
        );
        assert_eq!(names(&b), vec!["B"]);
    }

    #[test]
    fn the_only_workspace_cannot_be_deleted() {
        let (_t, mut b) = backend("onlyone");
        let id = id_of(&b, "Default");
        let o = send(&mut b, DELETE, json!({"id": id}));
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

    #[test]
    fn protocol_violations_are_refused() {
        let (_t, mut b) = backend("refused");
        assert_eq!(
            b.handle("delete-everything", ""),
            Err(Refused::UnknownMessage("delete-everything".into()))
        );
        assert!(matches!(
            b.handle(CREATE, "not json"),
            Err(Refused::BadPayload { .. })
        ));
        assert!(
            matches!(
                b.handle(CREATE, r#"{"name":"x"}"#),
                Err(Refused::BadPayload { .. })
            ),
            "a missing field"
        );
        assert!(
            matches!(
                b.handle(CREATE, r#"{"name":"x","colour":1,"extra":true}"#),
                Err(Refused::BadPayload { .. })
            ),
            "an unknown field"
        );
        assert!(
            matches!(
                b.handle(CREATE, r#"{"name":5,"colour":1}"#),
                Err(Refused::BadPayload { .. })
            ),
            "a wrong type"
        );
        assert!(
            matches!(
                b.handle(DELETE, r#"{"id":"../../Windows"}"#),
                Err(Refused::BadPayload { .. })
            ),
            "a path trick in an id"
        );
        assert!(matches!(
            b.handle(LIST, r#"{"x":1}"#),
            Err(Refused::BadPayload { .. })
        ));
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

    #[test]
    fn a_deleted_workspaces_folders_are_removed_by_the_sweep() {
        let (_t, mut b) = backend("sweep");
        send(&mut b, CREATE, json!({"name": "Two", "colour": 2}));
        let two = Id::parse(&id_of(&b, "Two")).unwrap();
        let first = id_of(&b, "Default");
        let two_id = id_of(&b, "Two");
        send(&mut b, DELETE, json!({"id": two_id}));
        // the host has not removed the folders (Chromium still holds them): they are still on disk, with no record
        assert!(b.data().workspace_dir(&two).is_dir() && b.data().chromium_profile(&two).is_dir());
        assert_eq!(b.sweep_orphans().unwrap(), vec![two.clone()]);
        assert!(
            !b.data().workspace_dir(&two).exists() && !b.data().chromium_profile(&two).exists()
        );
        assert!(
            b.data().workspace_dir(&Id::parse(&first).unwrap()).is_dir(),
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
        let b = Backend::open(data).unwrap();
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
}
