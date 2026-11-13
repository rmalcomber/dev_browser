//! The right-hand widget panel: which tools are pinned, and whether the panel is open and how wide.
//!
//! The panel shows the pinned tools of the **workspaces in view**, as sections coloured for their workspace. With one pane that is the
//! active workspace only (split view, M4, will show both visible workspaces). A widget can only be acted on in a workspace that is in
//! view: its id is looked up in the active workspace's own database, so another workspace's widget id is simply "no such widget".
//! Layout (which tools, their order, collapsed or not) is stored; **what was typed into a widget is not** (see `widget.rs`).

use super::*;
use crate::{tools, widget};

pub const WIDGETS_LIST: &str = "widgets.list";
pub const WIDGETS_PIN: &str = "widgets.pin";
pub const WIDGETS_UNPIN: &str = "widgets.unpin";
pub const WIDGETS_COLLAPSE: &str = "widgets.collapse";
pub const WIDGET_RUN: &str = "widget.run";
pub const PANEL_RESIZE: &str = "panel.resize";

pub const PANEL_MIN: i32 = 220;
pub const PANEL_MAX: i32 = 640;
pub const PANEL_DEFAULT: i32 = 320;

const KEY_OPEN: &str = "panel_open";
const KEY_WIDTH: &str = "panel_width";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PinIn {
    pub(super) tool: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WidgetIdIn {
    pub(super) id: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CollapseIn {
    pub(super) id: i64,
    pub(super) collapsed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WidgetRunIn {
    pub(super) id: i64,
    pub(super) input: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResizeIn {
    pub(super) dx: i32,
    pub(super) done: bool,
}

#[derive(Serialize)]
struct WidgetOut {
    id: i64,
    tool: &'static str,
    name: &'static str,
    description: &'static str,
    takes_input: bool,
    sensitive: bool,
    collapsed: bool,
}

#[derive(Serialize)]
struct SectionOut {
    workspace: WorkspaceOut,
    widgets: Vec<WidgetOut>,
}

#[derive(Serialize)]
struct PanelOut {
    open: bool,
    width: i32,
    sections: Vec<SectionOut>,
}

impl Backend {
    /// The saved panel state, read once when the backend opens.
    pub(super) fn load_panel(global: &Connection) -> (bool, i32) {
        let open = settings::get(global, KEY_OPEN).ok().flatten().as_deref() == Some("1");
        let width = settings::get(global, KEY_WIDTH)
            .ok()
            .flatten()
            .and_then(|w| w.parse::<i32>().ok())
            .map_or(PANEL_DEFAULT, |w| w.clamp(PANEL_MIN, PANEL_MAX));
        (open, width)
    }

    fn save_panel(&self) {
        let _ = settings::set(
            &self.global,
            KEY_OPEN,
            if self.panel_open { "1" } else { "0" },
        );
        let _ = settings::set(&self.global, KEY_WIDTH, &self.panel_width.to_string());
    }

    pub(super) fn panel_effect(&self, focus: bool) -> Effect {
        Effect::Panel {
            open: self.panel_open,
            width: self.panel_width,
            focus: focus && self.panel_open,
        }
    }

    /// What the panel shows: its state and the pinned tools of the workspaces in view (the active one).
    pub(super) fn widgets_reply(&mut self) -> Reply {
        let ws = self.active.clone();
        let sections = match (
            workspace::get(&self.global, &ws),
            self.with_home(&ws, |c| Ok(widget::all(c)?)),
        ) {
            (Ok(w), Ok(list)) => vec![SectionOut {
                workspace: WorkspaceOut {
                    id: w.id.to_string(),
                    name: w.name,
                    colour: w.colour,
                    position: w.position,
                },
                widgets: list
                    .into_iter()
                    .filter_map(|x| {
                        let t = tools::info(&x.tool)?;
                        Some(WidgetOut {
                            id: x.id,
                            tool: t.id,
                            name: t.name,
                            description: t.description,
                            takes_input: t.takes_input,
                            sensitive: t.sensitive,
                            collapsed: x.collapsed,
                        })
                    })
                    .collect(),
            }],
            _ => vec![],
        };
        Reply {
            name: "widgets",
            payload: serde_json::to_string(&PanelOut {
                open: self.panel_open,
                width: self.panel_width,
                sections,
            })
            .expect("widgets serialise"),
        }
    }

    pub(super) fn panel_toggle(&mut self) -> Result<Outcome, Failure> {
        self.panel_open = !self.panel_open;
        self.save_panel();
        Ok(Outcome {
            replies: vec![self.widgets_reply()],
            effects: vec![self.panel_effect(true)],
        })
    }

    pub(super) fn widgets_pin(&mut self, tool: &str) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let tool = tool.to_string();
        let pinned = self.with_home(&ws, |c| {
            widget::pin(c, &tool).map_err(tab::TabError::Message)
        })?;
        let name = tools::info(&pinned.tool).map_or("tool", |t| t.name);
        // pinning shows the panel, so the new widget is seen
        self.panel_open = true;
        self.save_panel();
        Ok(Outcome {
            replies: vec![
                self.widgets_reply(),
                Self::notice_reply(&format!("Pinned: {name}")),
            ],
            effects: vec![self.panel_effect(false)],
        })
    }

    pub(super) fn widgets_unpin(&mut self, id: i64) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let removed = self.with_home(&ws, |c| Ok(widget::unpin(c, id)?))?;
        if !removed {
            return Err("there is no such widget".into());
        }
        Ok(Outcome {
            replies: vec![self.widgets_reply()],
            effects: vec![],
        })
    }

    pub(super) fn widgets_collapse(
        &mut self,
        id: i64,
        collapsed: bool,
    ) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        if !self.with_home(&ws, |c| Ok(widget::set_collapsed(c, id, collapsed)?))? {
            return Err("there is no such widget".into());
        }
        Ok(Outcome {
            replies: vec![self.widgets_reply()],
            effects: vec![],
        })
    }

    /// Run a pinned tool on the text typed into its widget. Only a widget of the active workspace can be run. The text is not kept.
    pub(super) fn widget_run(&mut self, id: i64, input: &str) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let w = self
            .with_home(&ws, |c| Ok(widget::get(c, id)?))?
            .ok_or("there is no such widget")?;
        let payload = match tools::run(&w.tool, input, entry::clock()) {
            Ok(o) => serde_json::json!({ "id": id, "output": o, "error": null }),
            Err(message) => serde_json::json!({ "id": id, "output": null, "error": message }),
        };
        Ok(Outcome {
            replies: vec![Reply {
                name: "widget.output",
                payload: payload.to_string(),
            }],
            effects: vec![],
        })
    }

    /// The panel's edge was dragged: `dx` pixels to the right (negative is wider). Remembered when the drag ends.
    pub(super) fn panel_resize(&mut self, dx: i32, done: bool) -> Result<Outcome, Failure> {
        if !self.panel_open {
            return Ok(Outcome::default());
        }
        let dx = dx.clamp(-2000, 2000);
        self.panel_width = (self.panel_width - dx).clamp(PANEL_MIN, PANEL_MAX);
        if done {
            self.save_panel();
        }
        Ok(Outcome {
            replies: vec![],
            effects: vec![self.panel_effect(false)],
        })
    }
}
