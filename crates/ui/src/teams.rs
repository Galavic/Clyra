//! Equipos / Dots — agentes persistentes al estilo de los dots de ChatGPT.
//!
//! Un dot es una responsabilidad continua con objetivo, reglas y memoria que
//! vive en el motor (igual que una automatización: registro persistente,
//! pausar/reanudar, revisiones programadas) y delega trabajo real abriendo
//! sesiones con el harness elegido. Nada aquí es simulado: crear, editar,
//! pausar, programar revisiones y delegar operan sobre
//! [`clyra_proto::Automation`] vía RPC. La conversación persiste en su propia
//! sesión; las revisiones generan tareas separadas que se abren al revisarlas.
//!
//! El briefing que el dot ejecuta se compone por secciones estables
//! (`# Objetivo`, `# Reglas`, `# Memoria`, `# Tarea actual`) para que el
//! editor las lea y escriba sin pelearse con el texto libre.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use clyra_engine::registry::HarnessDescriptor;
use clyra_proto::{
    Automation, AutomationCadence, AutomationDraft, AutomationRunAck, AutomationSchedule,
    AutomationTrigger, AutomationsSnapshot, ChatIndicator, HarnessId, Model, SandboxLevel, Space,
};
use clyra_rpc::methods;
use gpui::{
    AnyElement, Context, Entity, EventEmitter, SharedString, Subscription, Task, Window, div,
    prelude::*, px,
};

use crate::composer::{Composer, ComposerEvent, ComposerInput};
use crate::icons;
use crate::pickers::{harness_brand_icon, harness_name, normalize_model_rows, offered_harnesses};
use crate::popover::Loadable;
use crate::state::AppState;
use crate::theme::Theme;

/// El Shell salta a la sesión que el dot abrió para revisar el trabajo.
#[derive(Debug, Clone)]
pub enum TeamsEvent {
    OpenChat(String),
    WorkspaceCommand {
        command: crate::composer::WorkspaceCommand,
        chat_id: Option<String>,
    },
}

impl EventEmitter<TeamsEvent> for TeamsPage {}

// ---- briefing por secciones ----

const GOAL_MARK: &str = "# Objetivo";
const RULES_MARK: &str = "# Reglas";
const MEMORY_MARK: &str = "# Memoria";
const TASK_MARK: &str = "# Tarea actual";

/// Compone el briefing que el dot ejecuta a partir de sus secciones.
fn compose_prompt(goal: &str, rules: &str, memory: &str, task: &str) -> String {
    format!(
        "{GOAL_MARK}\n{goal}\n\n{RULES_MARK}\n{rules}\n\n{MEMORY_MARK}\n{memory}\n\n{TASK_MARK}\n{task}\n",
        goal = goal.trim(),
        rules = rules.trim(),
        memory = memory.trim(),
        task = task.trim(),
    )
}

/// Lee las secciones de un briefing. Tolera prompts sin marcas (p. ej.
/// creados en Automations): todo el texto pasa a ser el objetivo.
fn parse_prompt(prompt: &str) -> (String, String, String, String) {
    let mut goal = String::new();
    let mut rules = String::new();
    let mut memory = String::new();
    let mut task = String::new();
    let mut current: Option<&mut String> = None;
    let mut seen_mark = false;
    for line in prompt.lines() {
        let trimmed = line.trim();
        if trimmed == GOAL_MARK {
            current = Some(&mut goal);
            seen_mark = true;
        } else if trimmed == RULES_MARK {
            current = Some(&mut rules);
            seen_mark = true;
        } else if trimmed == MEMORY_MARK {
            current = Some(&mut memory);
            seen_mark = true;
        } else if trimmed == TASK_MARK {
            current = Some(&mut task);
            seen_mark = true;
        } else if let Some(slot) = current.as_mut() {
            if !slot.is_empty() {
                slot.push('\n');
            }
            slot.push_str(line);
        }
    }
    if !seen_mark {
        goal = prompt.trim().to_string();
    }
    (
        goal.trim().to_string(),
        rules.trim().to_string(),
        memory.trim().to_string(),
        task.trim().to_string(),
    )
}

/// Color estable por dot (identidad visual sin campo extra que persistir).
const DOT_COLORS: [u32; 19] = [
    0xff5a5a, 0xffc20e, 0xb07350, 0x3f95f5, 0x1ed8c0, 0x9966ff, 0x8e8e8e, 0x8e8e8e, 0x3f95f5,
    0xee7433, 0x161618, 0x5ccb77, 0x8a5a3a, 0x8844ee, 0xee7433, 0xe4e4e1, 0x5ccb77, 0x161618,
    0x2f6bf0,
];
const DOT_ASSETS: [&[u8]; 19] = [
    include_bytes!("../assets/dots/dot-0.svg"),
    include_bytes!("../assets/dots/dot-1.svg"),
    include_bytes!("../assets/dots/dot-2.svg"),
    include_bytes!("../assets/dots/dot-3.svg"),
    include_bytes!("../assets/dots/dot-4.svg"),
    include_bytes!("../assets/dots/dot-5.svg"),
    include_bytes!("../assets/dots/dot-6.svg"),
    include_bytes!("../assets/dots/dot-7.svg"),
    include_bytes!("../assets/dots/dot-8.svg"),
    include_bytes!("../assets/dots/dot-9.svg"),
    include_bytes!("../assets/dots/dot-10.svg"),
    include_bytes!("../assets/dots/dot-11.svg"),
    include_bytes!("../assets/dots/dot-12.svg"),
    include_bytes!("../assets/dots/dot-13.svg"),
    include_bytes!("../assets/dots/dot-14.svg"),
    include_bytes!("../assets/dots/dot-15.svg"),
    include_bytes!("../assets/dots/dot-16.svg"),
    include_bytes!("../assets/dots/dot-17.svg"),
    include_bytes!("../assets/dots/dot-18.svg"),
];
fn dot_index(id: &str) -> usize {
    id.bytes().map(|b| b as usize).sum::<usize>() % DOT_COLORS.len()
}
pub(crate) fn dot_color(id: &str) -> u32 {
    DOT_COLORS[dot_index(id)]
}
fn dot_avatar(
    index: usize,
    size: f32,
    pose: crate::dot_animation::Pose,
    view: gpui::EntityId,
    cx: &mut gpui::App,
) -> gpui::Div {
    if let Some(frame) = crate::dot_animation::frame(index, pose, view, cx) {
        return div()
            .flex_none()
            .size(px(size))
            .child(gpui::img(frame).size_full());
    }
    static IMAGES: std::sync::OnceLock<Vec<std::sync::Arc<gpui::Image>>> =
        std::sync::OnceLock::new();
    let images = IMAGES.get_or_init(|| {
        DOT_ASSETS
            .iter()
            .map(|bytes| {
                std::sync::Arc::new(gpui::Image::from_bytes(
                    gpui::ImageFormat::Svg,
                    bytes.to_vec(),
                ))
            })
            .collect()
    });
    div()
        .flex_none()
        .size(px(size))
        .child(gpui::img(images[index].clone()).size_full())
}
pub(crate) fn dot_div<V: 'static>(hex: u32, size: f32, cx: &mut Context<V>) -> gpui::Div {
    dot_avatar(
        DOT_COLORS
            .iter()
            .position(|color| *color == hex)
            .unwrap_or(9),
        size,
        crate::dot_animation::Pose::Idle,
        cx.entity_id(),
        cx,
    )
}
fn avatar_for(id: &str, size: f32, cx: &mut Context<TeamsPage>) -> gpui::Div {
    animated_avatar_for(id, size, crate::dot_animation::Pose::Idle, cx)
}
fn animated_avatar_for(
    id: &str,
    size: f32,
    pose: crate::dot_animation::Pose,
    cx: &mut Context<TeamsPage>,
) -> gpui::Div {
    dot_avatar(dot_index(id), size, pose, cx.entity_id(), cx)
}

/// The durable transcript retains the briefing for the agent; the bubble
/// shows exactly the message the person sent, including any later markers.
pub(crate) fn conversation_message(raw: &str) -> &str {
    if raw.starts_with("<!-- clyra-dot-context -->\n") {
        if let Some((_, message)) = raw.split_once("\n\n<!-- clyra-dot-message -->\n") {
            return message;
        }
    }
    raw
}

fn sandbox_rows() -> [(SandboxLevel, &'static str, &'static str); 3] {
    [
        (
            SandboxLevel::ReadOnly,
            "Read only",
            "Reviews and suggests without changing files",
        ),
        (
            SandboxLevel::WorkspaceWrite,
            "Can edit",
            "Works in the project and verifies changes with tests",
        ),
        (
            SandboxLevel::DangerFullAccess,
            "Full access",
            "Unrestricted access within the project",
        ),
    ]
}

fn cadence_rows() -> [(AutomationCadence, &'static str, &'static str); 4] {
    [
        (
            AutomationCadence::Hourly,
            "Hourly",
            "Reviews its responsibility every hour",
        ),
        (
            AutomationCadence::Daily,
            "Daily",
            "One review each day at the selected time",
        ),
        (
            AutomationCadence::Weekdays,
            "Weekdays",
            "Monday through Friday at the selected time",
        ),
        (
            AutomationCadence::Weekly,
            "Weekly",
            "One day each week at the selected time",
        ),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CenterTab {
    Chat,
    Activity,
    Scheduled,
    Dot,
}

/// Editor del dot: valores planos + entidades de entrada, se ensambla al guardar.
struct DotEditor {
    /// Vacío para un dot nuevo (el motor asigna id al crear).
    id: String,
    name: Entity<ComposerInput>,
    goal: Entity<ComposerInput>,
    rules: Entity<ComposerInput>,
    memory: Entity<ComposerInput>,
    space_id: String,
    harness: Option<HarnessId>,
    model: Option<String>,
    advanced: bool,
    projects_open: bool,
    enabled: bool,
    sandbox: SandboxLevel,
    cadence: AutomationCadence,
    hour: u8,
    minute: u8,
    /// Gatillo no programado (p. ej. pull-requests): se conserva tal cual.
    keep_trigger: Option<AutomationTrigger>,
    saving: bool,
}

pub struct TeamsPage {
    state: Entity<AppState>,
    snapshot: Loadable<AutomationsSnapshot>,
    /// Proyectos donde un dot puede trabajar (solo este dispositivo).
    spaces: Vec<Space>,
    harnesses: Vec<HarnessDescriptor>,
    models: HashMap<HarnessId, Loadable<Vec<Model>>>,
    selected: Option<String>,
    tab: CenterTab,
    composer: Entity<Composer>,
    dot_state: Entity<AppState>,
    preparing: Option<String>,
    editor: Option<DotEditor>,
    /// Live views over the durable conversations stored by the engine.
    conversations: HashMap<String, Entity<crate::transcript::Transcript>>,
    /// Última sesión abierta por dot en esta ejecución (para "Ver sesión").
    last_chat: HashMap<String, String>,
    delete_confirm: Option<String>,
    notice: Option<SharedString>,
    error: Option<SharedString>,
    load_task: Option<Task<()>>,
    action_task: Option<Task<()>>,
    catalog_task: Option<Task<()>>,
    _refresh_task: Option<Task<()>>,
    /// Delegación en vuelo (para el estado "Working…" del chat).
    delegating: bool,
    _observe: Subscription,
    _dot_observe: Subscription,
    _task_sub: Subscription,
}

impl TeamsPage {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let dot_state = cx.new(|_| AppState::new());
        if let Some(engine) = state.read(cx).engine().cloned() {
            dot_state.update(cx, |state, cx| state.attach_engine(engine, cx));
        }
        let composer = cx.new(|cx| Composer::new(dot_state.clone(), cx));
        let dot_observe = cx.observe(&dot_state, |_, _, cx| cx.notify());
        let task_sub = cx.subscribe(
            &composer,
            |page: &mut Self, _, event: &ComposerEvent, cx| {
                if let ComposerEvent::WorkspaceCommand(command) = event {
                    cx.emit(TeamsEvent::WorkspaceCommand {
                        command: *command,
                        chat_id: page.dot_state.read(cx).selected_chat.clone(),
                    });
                }
                if matches!(
                    event,
                    ComposerEvent::Sent { .. } | ComposerEvent::Queued { .. }
                ) {
                    page.notice = None;
                    cx.notify();
                }
            },
        );
        let observe = cx.observe(&state, |page: &mut Self, _, cx| {
            page.sync_dot_engine(cx);
            cx.notify();
        });
        let mut page = Self {
            state,
            snapshot: Loadable::Idle,
            spaces: Vec::new(),
            harnesses: Vec::new(),
            models: HashMap::new(),
            selected: None,
            tab: CenterTab::Chat,
            composer,
            dot_state,
            preparing: None,
            editor: None,
            conversations: HashMap::new(),
            last_chat: HashMap::new(),
            delete_confirm: None,
            notice: None,
            error: None,
            load_task: None,
            action_task: None,
            catalog_task: None,
            _refresh_task: None,
            delegating: false,
            _observe: observe,
            _dot_observe: dot_observe,
            _task_sub: task_sub,
        };
        page.reload(cx);
        page.load_catalog(None, cx);
        page._refresh_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(20))
                    .await;
                if this.update(cx, |page, cx| page.reload(cx)).is_err() {
                    break;
                }
            }
        }));
        page
    }

    fn automations(&self) -> &[Automation] {
        match &self.snapshot {
            Loadable::Ready(snapshot) => &snapshot.automations,
            _ => &[],
        }
    }

    fn selected_automation(&self) -> Option<Automation> {
        let id = self.selected.as_deref()?;
        self.automations().iter().find(|a| a.id == id).cloned()
    }

    /// Estado vivo de la última sesión del dot, si sigue existiendo.
    fn live_session(
        &self,
        dot_id: &str,
        now: DateTime<Utc>,
        cx: &gpui::App,
    ) -> Option<(String, bool)> {
        let auto = self.automations().iter().find(|a| a.id == dot_id)?;
        let state = self.state.read(cx);
        let candidates: Vec<_> = auto
            .conversation_chat_id
            .iter()
            .chain(auto.activity_chat_ids.iter().rev())
            .filter_map(|id| state.chats.iter().find(|c| c.id == *id))
            .collect();
        let chat = candidates
            .iter()
            .find(|c| state.display_status_for(c, now) == ChatIndicator::Working)
            .copied()
            .or_else(|| candidates.first().copied())?;
        let working = state.display_status_for(chat, now) == ChatIndicator::Working;
        let title = chat.title.clone().unwrap_or_else(|| "Session".into());
        Some((title, working))
    }

    fn space_name(&self, space_id: &str) -> String {
        self.spaces
            .iter()
            .find(|s| s.id == space_id)
            .map(|s| s.display_name().to_string())
            .unwrap_or_else(|| "Project".into())
    }

    // ---- carga ----

    /// Relee el snapshot del motor. Público para el Shell: al volver a la
    /// página se recargan carreras que hayan ocurrido fuera.
    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            self.snapshot = Loadable::Error("Engine not connected".into());
            return;
        };
        if matches!(self.snapshot, Loadable::Idle | Loadable::Error(_)) {
            self.snapshot = Loadable::Loading;
        }
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let result = engine
                .client()
                .call(methods::LIST_AUTOMATIONS, serde_json::json!({}))
                .await;
            this.update(cx, |page, cx| {
                page.snapshot = match result {
                    Ok(value) => match serde_json::from_value::<AutomationsSnapshot>(value) {
                        Ok(snapshot) => Loadable::Ready(snapshot),
                        Err(err) => Loadable::Error(err.to_string()),
                    },
                    Err(err) => Loadable::Error(err.to_string()),
                };
                let local_id = page.state.read(cx).local_device_id.clone();
                page.spaces = page
                    .state
                    .read(cx)
                    .spaces
                    .iter()
                    .filter(|space| match (&local_id, &space.device_id) {
                        (Some(local), device) => local == device,
                        (None, _) => true,
                    })
                    .cloned()
                    .collect();
                // Selección estable: primer dot si la actual ya no existe.
                let ids: HashSet<String> =
                    page.automations().iter().map(|a| a.id.clone()).collect();
                if page.editor.as_ref().is_none_or(|e| !e.id.is_empty())
                    && page.selected.as_deref().is_none_or(|id| !ids.contains(id))
                {
                    page.selected = page.automations().first().map(|a| a.id.clone());
                }
                let links: Vec<_> = page
                    .automations()
                    .iter()
                    .filter_map(|a| {
                        a.conversation_chat_id
                            .clone()
                            .map(|chat| (a.id.clone(), chat))
                    })
                    .collect();
                for (dot, chat) in links {
                    if page.selected.as_deref() == Some(dot.as_str()) {
                        page.connect_conversation(&dot, &chat, cx);
                    }
                }
                page.prepare_conversation(cx);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn load_catalog(&mut self, _harness: Option<HarnessId>, cx: &mut Context<Self>) {
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            return;
        };
        self.catalog_task = Some(cx.spawn(async move |this, cx| {
            let harnesses = engine
                .client()
                .call(methods::LIST_HARNESSES, serde_json::json!({}))
                .await
                .ok()
                .and_then(|value| serde_json::from_value::<Vec<HarnessDescriptor>>(value).ok())
                .unwrap_or_default();
            this.update(cx, |page, cx| {
                page.harnesses = harnesses;
                cx.notify();
            })
            .ok();
        }));
    }

    fn load_dot_models(&mut self, harness: HarnessId, cx: &mut Context<Self>) {
        if matches!(
            self.models.get(&harness),
            Some(Loadable::Loading | Loadable::Ready(_))
        ) {
            return;
        }
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            return;
        };
        self.models.insert(harness, Loadable::Loading);
        cx.spawn(async move |this, cx| {
            let result = engine
                .client()
                .call(
                    methods::LIST_MODELS,
                    serde_json::json!({ "harness": harness }),
                )
                .await;
            this.update(cx, |page, cx| {
                let loaded = match result {
                    Ok(value) => match serde_json::from_value::<Vec<Model>>(value) {
                        Ok(models) => Loadable::Ready(normalize_model_rows(harness, models)),
                        Err(err) => Loadable::Error(err.to_string()),
                    },
                    Err(err) => Loadable::Error(err.to_string()),
                };
                page.models.insert(harness, loaded);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn select_dot_harness(&mut self, harness: Option<HarnessId>, cx: &mut Context<Self>) {
        if let Some(editor) = self.editor.as_mut() {
            if editor.harness != harness {
                editor.harness = harness;
                editor.model = None;
            }
        }
        if let Some(id) = harness {
            self.load_dot_models(id, cx);
        }
        cx.notify();
    }

    fn rpc(
        &mut self,
        method: &'static str,
        params: serde_json::Value,
        after: impl FnOnce(&mut Self, Result<serde_json::Value, String>, &mut Context<Self>)
        + Send
        + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            self.error = Some("Engine not connected".into());
            cx.notify();
            return;
        };
        self.error = None;
        self.action_task = Some(cx.spawn(async move |this, cx| {
            let result = engine
                .client()
                .call(method, params)
                .await
                .map_err(|err| err.to_string());
            this.update(cx, |page, cx| after(page, result, cx)).ok();
        }));
    }

    // ---- editor ----

    fn open_editor(&mut self, automation: Option<&Automation>, cx: &mut Context<Self>) {
        let (
            id,
            name,
            goal,
            rules,
            memory,
            space_id,
            harness,
            sandbox,
            cadence,
            hour,
            minute,
            keep,
        ) = match automation {
            Some(a) => {
                let (goal, rules, memory, _) = parse_prompt(&a.prompt);
                let (cadence, hour, minute, keep) = match &a.trigger {
                    AutomationTrigger::Schedule(s) => (s.every, s.hour, s.minute, None),
                    pr => (AutomationCadence::Daily, 9, 0, Some(pr.clone())),
                };
                (
                    a.id.clone(),
                    a.name.clone(),
                    goal,
                    rules,
                    memory,
                    a.space_id.clone(),
                    a.harness,
                    a.sandbox,
                    cadence,
                    hour,
                    minute,
                    keep,
                )
            }
            None => (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                self.spaces
                    .first()
                    .map(|s| s.id.clone())
                    .unwrap_or_default(),
                None,
                SandboxLevel::WorkspaceWrite,
                AutomationCadence::Daily,
                9,
                0,
                None,
            ),
        };
        let name_i = cx.new(|cx| ComposerInput::new("Dot name", cx));
        name_i.update(cx, |i, cx| i.set_text(name, cx));
        let goal_i =
            cx.new(|cx| ComposerInput::new("What would you like your dot to help with?", cx));
        goal_i.update(cx, |i, cx| i.set_text(goal, cx));
        let rules_i = cx.new(|cx| ComposerInput::new("Instructions and working style…", cx));
        rules_i.update(cx, |i, cx| i.set_text(rules, cx));
        let memory_i = cx.new(|cx| ComposerInput::new("Context and preferences…", cx));
        memory_i.update(cx, |i, cx| i.set_text(memory, cx));
        self.editor = Some(DotEditor {
            id,
            name: name_i,
            goal: goal_i,
            rules: rules_i,
            memory: memory_i,
            space_id,
            harness,
            model: automation.and_then(|a| a.model.clone()),
            advanced: false,
            projects_open: false,
            enabled: automation.is_some_and(|a| a.enabled),
            sandbox,
            cadence,
            hour,
            minute,
            keep_trigger: keep,
            saving: false,
        });
        if let Some(id) = harness {
            self.load_dot_models(id, cx);
        }
        self.tab = CenterTab::Dot;
        self.notice = None;
        self.error = None;
        cx.notify();
    }

    fn save_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        if editor.saving {
            return;
        }
        let name = editor.name.read(cx).text().trim().to_string();
        let goal = editor.goal.read(cx).text().trim().to_string();
        let rules = editor.rules.read(cx).text().trim().to_string();
        let memory = editor.memory.read(cx).text().trim().to_string();
        if name.is_empty() || goal.is_empty() {
            self.error = Some("Name and responsibility are required".into());
            cx.notify();
            return;
        }
        if editor.space_id.is_empty() {
            self.error = Some("Choose the project your dot will work in".into());
            cx.notify();
            return;
        }
        // La tarea actual se conserva al editar (delegar la reemplaza).
        let existing_task = self
            .selected_automation()
            .map(|a| parse_prompt(&a.prompt).3)
            .unwrap_or_default();
        let prompt = compose_prompt(&goal, &rules, &memory, &existing_task);
        let trigger = match &editor.keep_trigger {
            Some(t) => t.clone(),
            None => AutomationTrigger::Schedule(AutomationSchedule {
                every: editor.cadence,
                hour: editor.hour,
                minute: editor.minute,
                weekday: self
                    .selected_automation()
                    .and_then(|a| a.trigger.schedule().map(|s| s.weekday))
                    .unwrap_or(1),
            }),
        };
        let draft = AutomationDraft {
            id: editor.id.clone(),
            name,
            space_id: editor.space_id.clone(),
            trigger,
            prompt,
            harness: editor.harness,
            model: editor.model.clone(),
            sandbox: editor.sandbox,
            enabled: editor.enabled,
        };
        if let Some(editor) = self.editor.as_mut() {
            editor.saving = true;
        }
        self.rpc(
            methods::UPSERT_AUTOMATION,
            serde_json::json!({ "draft": draft }),
            move |page, result, cx| {
                match result {
                    Ok(value) => {
                        if let Ok(saved) = serde_json::from_value::<Automation>(value) {
                            let dot = saved.id;
                            page.selected = Some(dot.clone());
                            page.last_chat.remove(&dot);
                            page.preparing = Some(dot.clone());
                            page.tab = CenterTab::Chat;
                            page.notice = Some("Dot saved".into());
                            page.editor = None;
                            // Apply model and access edits to the durable chat before
                            // enabling its ordinary composer again.
                            page.rpc(
                                methods::RUN_AUTOMATION_NOW,
                                serde_json::json!({"automationId":dot,"prepareOnly":true}),
                                move |page, result, cx| {
                                    page.preparing = None;
                                    if let Err(error) = result {
                                        page.error = Some(error.into());
                                    }
                                    page.reload(cx);
                                    cx.notify();
                                },
                                cx,
                            );
                            cx.notify();
                            return;
                        }
                        page.tab = CenterTab::Chat;
                        page.notice = Some("Dot saved".into());
                        page.editor = None;
                    }
                    Err(err) => {
                        page.error = Some(err.into());
                        if let Some(editor) = page.editor.as_mut() {
                            editor.saving = false;
                        }
                    }
                }
                page.reload(cx);
                cx.notify();
            },
            cx,
        );
        cx.notify();
    }

    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let Some(auto) = self.selected_automation() else {
            return;
        };
        if self.delete_confirm.as_deref() != Some(auto.id.as_str()) {
            self.delete_confirm = Some(auto.id.clone());
            cx.notify();
            return;
        }
        self.delete_confirm = None;
        self.rpc(
            methods::DELETE_AUTOMATION,
            serde_json::json!({ "automationId": auto.id }),
            |page, result, cx| {
                match result {
                    Ok(_) => {
                        page.notice = Some("Dot deleted".into());
                        page.editor = None;
                    }
                    Err(err) => page.error = Some(err.into()),
                }
                page.reload(cx);
                cx.notify();
            },
            cx,
        );
    }

    fn set_enabled(&mut self, id: &str, enabled: bool, cx: &mut Context<Self>) {
        self.rpc(
            methods::SET_AUTOMATION_ENABLED,
            serde_json::json!({ "automationId": id, "enabled": enabled }),
            move |page, result, cx| {
                match result {
                    Ok(_) => {
                        page.notice = Some(if enabled {
                            "Scheduled reviews enabled".into()
                        } else {
                            "Scheduled reviews paused".into()
                        })
                    }
                    Err(err) => page.error = Some(err.into()),
                }
                page.reload(cx);
                cx.notify();
            },
            cx,
        );
    }

    // ---- delegar ----

    fn sync_dot_engine(&mut self, cx: &mut Context<Self>) {
        if let Some(engine) = self.state.read(cx).engine().cloned() {
            let connected = self
                .dot_state
                .read(cx)
                .engine()
                .is_some_and(|other| other.same_connection(&engine));
            if !connected {
                self.dot_state
                    .update(cx, |state, cx| state.attach_engine(engine, cx));
            }
        }
    }

    fn prepare_conversation(&mut self, cx: &mut Context<Self>) {
        let Some(auto) = self.selected_automation() else {
            return;
        };
        if self.preparing.is_some()
            || auto.conversation_chat_id.is_some()
            || self.last_chat.contains_key(&auto.id)
        {
            return;
        }
        self.preparing = Some(auto.id.clone());
        let dot = auto.id.clone();
        self.rpc(
            methods::RUN_AUTOMATION_NOW,
            serde_json::json!({"automationId": dot, "prepareOnly":true}),
            move |page, result, cx| {
                page.preparing = None;
                match result.and_then(|v| {
                    serde_json::from_value::<AutomationRunAck>(v).map_err(|e| e.to_string())
                }) {
                    Ok(ack) => {
                        page.connect_conversation(&dot, &ack.chat_id, cx);
                        page.reload(cx);
                    }
                    Err(error) => page.error = Some(error.into()),
                }
                cx.notify();
            },
            cx,
        );
    }

    fn connect_conversation(&mut self, dot_id: &str, chat_id: &str, cx: &mut Context<Self>) {
        if self.selected.as_deref() != Some(dot_id) {
            return;
        }
        self.sync_dot_engine(cx);
        self.dot_state.update(cx, |state, cx| {
            state.select_chat(Some(chat_id.to_string()), cx)
        });
        if let Some(auto) = self.selected_automation() {
            self.dot_state.update(cx, |state, cx| {
                state.select_space(Some(auto.space_id.clone()), cx)
            });
            self.composer.update(cx, |composer, cx| {
                composer.set_dot_briefing(Some(auto.prompt), cx)
            });
        }
        let stale: Vec<_> = self
            .conversations
            .keys()
            .filter(|id| id.as_str() != chat_id)
            .cloned()
            .collect();
        for id in stale {
            self.conversations.remove(&id);
            self.state
                .update(cx, |state, _| state.unwatch_subagent_doc(&id));
        }
        self.last_chat
            .insert(dot_id.to_string(), chat_id.to_string());
        let id = chat_id.to_string();
        self.state
            .update(cx, |state, cx| state.watch_subagent_doc(id.clone(), cx));
        if self.conversations.contains_key(chat_id) {
            return;
        }
        let state = self.state.clone();
        let transcript = cx.new(|cx| {
            crate::transcript::Transcript::for_dot(state, id.clone(), dot_color(dot_id), cx)
        });
        self.conversations.insert(id, transcript);
    }

    /// Ejecuta el briefing actual sin tarea nueva (revisión del objetivo).
    fn run_briefing(&mut self, cx: &mut Context<Self>) {
        if self.delegating {
            return;
        }
        let Some(auto) = self.selected_automation() else {
            return;
        };
        let dot_id = auto.id.clone();
        self.notice = Some("Starting review…".into());
        self.delegating = true;
        self.rpc(
            methods::RUN_AUTOMATION_NOW,
            serde_json::json!({ "automationId": dot_id }),
            move |page, run, cx| {
                match run.and_then(|v| {
                    serde_json::from_value::<AutomationRunAck>(v).map_err(|e| e.to_string())
                }) {
                    Ok(ack) => {
                        page.delegating = false;
                        page.last_chat.insert(dot_id.clone(), ack.chat_id.clone());
                        page.notice = Some("Session started".into());
                        page.reload(cx);
                    }
                    Err(err) => {
                        page.delegating = false;
                        page.error = Some(err.into());
                        page.reload(cx);
                    }
                }
                cx.notify();
            },
            cx,
        );
        cx.notify();
    }
}

fn field_label(theme: &Theme, label: &str) -> gpui::Div {
    div()
        .text_size(crate::typography::ui_rems(12.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.text_faint)
        .child(SharedString::from(label.to_string()))
}

fn edit_field(theme: &Theme, label: &str, input: Entity<ComposerInput>, min_h: f32) -> gpui::Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .text_left()
        .child(field_label(theme, label))
        .child(
            div()
                .w_full()
                .min_h(px(min_h))
                .rounded(px(10.0))
                .border_1()
                .border_color(theme.border_strong)
                .bg(crate::theme::ink(0.04))
                .px(px(10.0))
                .py(px(8.0))
                .child(input),
        )
}

fn card(theme: &Theme, title: &str, body: &str) -> gpui::Div {
    div()
        .mt(px(12.0))
        .w_full()
        .rounded(px(12.0))
        .border_1()
        .border_color(theme.border)
        .p(px(18.0))
        .flex()
        .flex_col()
        .gap(px(6.0))
        .text_left()
        .child(
            div()
                .text_size(crate::typography::ui_rems(12.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text_faint)
                .child(SharedString::from(title.to_string())),
        )
        .child(
            div()
                .text_size(crate::typography::ui_rems(12.5))
                .text_color(theme.text_muted)
                .child(SharedString::from(body.to_string())),
        )
}

fn primary_button(theme: &Theme, id: &'static str, label: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(14.0))
        .py(px(7.0))
        .rounded(px(9.0))
        .bg(theme.text)
        .text_color(theme.surface)
        .text_size(crate::typography::ui_rems(12.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(|el| el.opacity(0.8))
        .child(SharedString::from(label.to_string()))
}

fn ghost_button(theme: &Theme, id: &'static str, label: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(14.0))
        .py(px(7.0))
        .rounded(px(9.0))
        .border_1()
        .border_color(theme.border_strong)
        .text_color(theme.text)
        .text_size(crate::typography::ui_rems(12.5))
        .cursor_pointer()
        .hover(|el| el.bg(theme.glass_hover()))
        .child(SharedString::from(label.to_string()))
}

impl Render for TeamsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        crate::dot_animation::set_view_active(cx.entity_id(), _window.is_window_active(), cx);
        let theme = Theme::of(cx).clone();
        let now = Utc::now();
        let automations: Vec<Automation> = self.automations().to_vec();
        let selected = self.selected_automation();
        let selected_id = self.selected.clone();
        let tab = self.tab;
        let offered = offered_harnesses(&self.harnesses);

        // ---- izquierda: dots ----
        let mut left = div()
            .flex_none()
            .w(px(228.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .child(
                div()
                    .px(px(12.0))
                    .pt(px(14.0))
                    .pb(px(8.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(crate::typography::ui_rems(13.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(SharedString::from("Dots")),
                    )
                    .child(
                        div()
                            .id("dots-new")
                            .size(px(28.0))
                            .hover(|el| el.bg(theme.glass_hover()))
                            .rounded(px(6.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(theme.text_muted)
                            .cursor_pointer()
                            .child(crate::icons::icon(icons::PLUS).size(px(13.0)))
                            .on_click(cx.listener(|page: &mut Self, _, _, cx| {
                                page.selected = None;
                                page.delete_confirm = None;
                                page.open_editor(None, cx);
                            })),
                    ),
            );

        if automations.is_empty() {
            left = left.child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .text_size(crate::typography::ui_rems(12.5))
                    .text_color(theme.text_muted)
                    .child(SharedString::from("Your dots will appear here.")),
            );
        }
        for (rix, auto) in automations.iter().enumerate() {
            let id = auto.id.clone();
            let live_working = self
                .live_session(&id, now, cx)
                .map(|(_, working)| working)
                .unwrap_or(false);
            let is_selected = selected_id.as_deref() == Some(id.as_str());
            let status = if live_working {
                "Working"
            } else if auto.enabled {
                "Active"
            } else {
                "Available"
            };
            let harness = auto.harness.map(harness_name).unwrap_or("Default harness");
            let last = auto
                .last_run_at
                .map(|at| crate::automations::time_ago(at, now));
            let mut meta = format!("{} · {}", auto.project_label, harness);
            if let Some(last) = last {
                meta.push_str(&format!(" · {last}"));
            }
            left = left.child(
                div()
                    .id(("dots-row", rix))
                    .mx(px(8.0))
                    .px(px(10.0))
                    .py(px(8.0))
                    .rounded(px(10.0))
                    .cursor_pointer()
                    .when(is_selected, |el| el.bg(theme.glass_hover()))
                    .hover(|s| s.bg(theme.glass_hover()))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(7.0))
                            .child(animated_avatar_for(
                                &id,
                                30.0,
                                crate::dot_animation::Pose::for_status(None, live_working),
                                cx,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(crate::typography::ui_rems(13.0))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(SharedString::from(auto.name.clone())),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(crate::typography::ui_rems(11.0))
                                    .text_color(if live_working {
                                        theme.accent
                                    } else if auto.enabled {
                                        theme.text_faint
                                    } else {
                                        theme.warning
                                    })
                                    .child(SharedString::from(status.to_string())),
                            ),
                    )
                    .child(
                        div()
                            .pl(px(37.0))
                            .truncate()
                            .text_size(crate::typography::ui_rems(11.5))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(meta)),
                    )
                    .on_click(cx.listener(move |page: &mut Self, _, _, cx| {
                        page.selected = Some(id.clone());
                        page.tab = CenterTab::Chat;
                        page.delete_confirm = None;
                        page.editor = None;
                        page.notice = None;
                        if let Some(auto) = page.selected_automation() {
                            if let Some(chat) = auto.conversation_chat_id {
                                page.connect_conversation(&auto.id, &chat, cx);
                            }
                        }
                        page.prepare_conversation(cx);
                        cx.notify();
                    })),
            );
        }

        // ---- centro ----
        let tabs: AnyElement = div()
            .flex_none()
            .flex()
            .flex_row()
            .gap(px(6.0))
            .child(self.tab_button("dots-tab-chat", "Chat", tab == CenterTab::Chat, &theme, cx))
            .child(self.tab_button(
                "dots-tab-activity",
                "Activity",
                tab == CenterTab::Activity,
                &theme,
                cx,
            ))
            .child(self.tab_button(
                "dots-tab-scheduled",
                "Scheduled",
                tab == CenterTab::Scheduled,
                &theme,
                cx,
            ))
            .child(self.tab_button("dots-tab-dot", "Profile", tab == CenterTab::Dot, &theme, cx))
            .into_any_element();

        let center_body: AnyElement = match (&self.snapshot, tab) {
            (Loadable::Loading | Loadable::Idle, _) => div()
                .flex_1()
                .p(px(32.0))
                .text_color(theme.text_muted)
                .child("Loading your dots…")
                .into_any_element(),
            (Loadable::Error(error), _) => div()
                .flex_1()
                .p(px(32.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .text_color(theme.danger)
                        .child(SharedString::from(error.clone())),
                )
                .child(
                    ghost_button(&theme, "dots-retry", "Retry")
                        .on_click(cx.listener(|page, _, _, cx| page.reload(cx))),
                )
                .into_any_element(),
            (_, CenterTab::Chat) => self.render_chat(&theme, selected.clone(), now, cx),
            (_, CenterTab::Activity) => self.render_activity(&theme, selected.clone(), now, cx),
            (_, CenterTab::Scheduled) => self.render_scheduled(&theme, selected.clone(), cx),
            (_, CenterTab::Dot) => self.render_dot(&theme, selected.clone(), offered, cx),
        };

        let center: AnyElement = div()
            .id("dots-center-scroll")
            .flex_1()
            .min_w_0()
            .h_full()
            .when(tab != CenterTab::Chat, |el| el.overflow_y_scroll())
            .flex()
            .flex_col()
            .items_center()
            .when(tab != CenterTab::Chat, |el| {
                el.child(
                    div()
                        .flex_none()
                        .w_full()
                        .px(px(24.0))
                        .pt(px(12.0))
                        .pb(px(12.0))
                        .border_b_1()
                        .border_color(theme.border)
                        .child(tabs),
                )
            })
            .child(center_body)
            .into_any_element();

        div()
            .id("teams-page-host")
            .size_full()
            .flex()
            .flex_row()
            .bg(if theme.appearance.is_dark() {
                gpui::rgb(0x101010).into()
            } else {
                theme.surface
            })
            .when(tab != CenterTab::Chat, |el| el.child(left))
            .child(center)
            .into_any_element()
    }
}

impl TeamsPage {
    fn tab_button(
        &self,
        id: &'static str,
        label: &str,
        active: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let target = match id {
            "dots-tab-activity" => CenterTab::Activity,
            "dots-tab-dot" => CenterTab::Dot,
            "dots-tab-scheduled" => CenterTab::Scheduled,
            _ => CenterTab::Chat,
        };
        div()
            .id(id)
            .px(px(12.0))
            .py(px(6.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .text_size(crate::typography::ui_rems(12.5))
            .when(active, |el| {
                el.bg(theme.glass_hover()).text_color(theme.text)
            })
            .when(!active, |el| el.text_color(theme.text_muted))
            .child(SharedString::from(label.to_string()))
            .on_click(cx.listener(move |page: &mut Self, _, _, cx| {
                page.tab = target;
                cx.notify();
            }))
    }

    fn render_chat(
        &mut self,
        theme: &Theme,
        selected: Option<Automation>,
        now: DateTime<Utc>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(auto) = selected else {
            return div()
                .flex_1()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(18.0))
                .child(dot_div(0xE88764, 72.0, cx))
                .child(
                    div()
                        .text_size(crate::typography::ui_rems(26.0))
                        .text_color(theme.text)
                        .child("Meet your dot"),
                )
                .child(
                    div()
                        .max_w(px(360.0))
                        .text_center()
                        .text_color(theme.text_muted)
                        .child(
                            "Give it a responsibility, share context, and follow its work here.",
                        ),
                )
                .child(
                    primary_button(theme, "dots-create-welcome", "Create a dot")
                        .on_click(cx.listener(|page, _, _, cx| page.open_editor(None, cx))),
                )
                .into_any_element();
        };
        let chat_id = auto.conversation_chat_id.clone().or_else(|| {
            self.last_chat
                .get(&auto.id)
                .filter(|id| self.conversations.contains_key(*id))
                .cloned()
        });
        let conversation = chat_id
            .as_ref()
            .and_then(|id| self.conversations.get(id))
            .cloned();
        let expected_chat = chat_id.clone();
        let working =
            self.delegating || self.live_session(&auto.id, now, cx).is_some_and(|(_, w)| w);
        let conversation_status = chat_id.as_ref().and_then(|id| {
            let state = self.state.read(cx);
            state
                .chats
                .iter()
                .find(|chat| &chat.id == id)
                .map(|chat| state.display_status_for(chat, now))
        });
        let status_label = match conversation_status {
            Some(ChatIndicator::AwaitingInput) => "Needs your input",
            Some(ChatIndicator::Errored) => "Review the session error",
            _ if working => "Working…",
            _ => "Conversation",
        };
        let active_workers = {
            let state = self.state.read(cx);
            state
                .chats
                .iter()
                .filter(|chat| {
                    chat.parent_chat_id.as_ref().is_some_and(|parent| {
                        auto.conversation_chat_id.as_ref() == Some(parent)
                            || auto.activity_chat_ids.contains(parent)
                    })
                })
                .filter(|chat| {
                    matches!(
                        state.display_status_for(chat, now),
                        ChatIndicator::Working | ChatIndicator::AwaitingInput
                    )
                })
                .count()
        };
        let activity_label = if active_workers == 0 {
            "Activity".to_string()
        } else {
            format!("Activity · {active_workers}")
        };
        let mut body = div().flex_1().min_h_0().w_full().flex().flex_col();
        if let Some(transcript) = conversation {
            body = body.child(transcript);
        } else {
            let goal = parse_prompt(&auto.prompt).0;
            body = body
                .justify_center()
                .items_center()
                .gap(px(16.0))
                .child(avatar_for(&auto.id, 64.0, cx))
                .child(
                    div()
                        .text_size(crate::typography::ui_rems(24.0))
                        .text_color(theme.text)
                        .child(SharedString::from(format!("Hi, I’m {}", auto.name))),
                )
                .child(
                    div()
                        .max_w(px(420.0))
                        .text_center()
                        .text_color(theme.text_muted)
                        .child(SharedString::from(goal)),
                )
                .child(
                    div()
                        .text_size(crate::typography::ui_rems(12.0))
                        .text_color(theme.text_faint)
                        .child("Share an idea or tell me where to start."),
                );
        }
        let mut header =
            div()
                .w_full()
                .px(px(28.0))
                .py(px(14.0))
                .flex()
                .items_center()
                .child(div().flex_1())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(6.0))
                        .child(animated_avatar_for(
                            &auto.id,
                            64.0,
                            crate::dot_animation::Pose::for_status(conversation_status, working),
                            cx,
                        ))
                        .child(
                            div()
                                .rounded_full()
                                .px(px(12.0))
                                .py(px(3.0))
                                .bg(theme.glass_hover())
                                .text_color(theme.text)
                                .child(SharedString::from(auto.name.clone())),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            ghost_button(theme, "dots-show-activity", &activity_label).on_click(
                                cx.listener(|page, _, _, cx| {
                                    page.tab = CenterTab::Activity;
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(ghost_button(theme, "dots-show-profile", "···").on_click(
                            cx.listener(|page, _, _, cx| {
                                page.tab = CenterTab::Dot;
                                cx.notify();
                            }),
                        )),
                );
        if matches!(
            conversation_status,
            Some(ChatIndicator::AwaitingInput | ChatIndicator::Errored)
        ) {
            if let Some(id) = chat_id {
                header = header.child(
                    ghost_button(theme, "dots-review-conversation", status_label).on_click(
                        cx.listener(move |_, _, _, cx| cx.emit(TeamsEvent::OpenChat(id.clone()))),
                    ),
                );
            }
        }
        let col = div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .child(header)
            .child(body);
        let mut composer = div()
            .flex_none()
            .w_full()
            .max_w(px(960.0))
            .px(px(24.0))
            .pb(px(20.0))
            .pt(px(12.0))
            .flex()
            .flex_col()
            .gap(px(8.0));
        if let Some(error) = self.error.clone() {
            composer = composer.child(div().text_color(theme.danger).child(error));
        }
        if let Some(notice) = self.notice.clone() {
            composer = composer.child(div().text_color(theme.text_muted).child(notice));
        }
        let ready = expected_chat.is_some()
            && self
                .dot_state
                .read(cx)
                .selected_chat_row()
                .is_some_and(|chat| Some(&chat.id) == expected_chat.as_ref())
            && self.preparing.is_none();
        composer = composer.child(if ready {
            div()
                .w_full()
                .child(self.composer.clone())
                .into_any_element()
        } else {
            div()
                .w_full()
                .text_color(theme.text_muted)
                .child("Preparing the conversation…")
                .into_any_element()
        });
        col.child(div().w_full().flex().justify_center().child(composer))
            .into_any_element()
    }

    fn render_activity(
        &mut self,
        theme: &Theme,
        selected: Option<Automation>,
        now: DateTime<Utc>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut col = div()
            .w_full()
            .max_w(px(760.0))
            .p(px(28.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(crate::typography::ui_rems(22.0))
                    .text_color(theme.text)
                    .child("Activity"),
            );
        let Some(auto) = selected else {
            return col
                .child("Select a dot to see its work.")
                .into_any_element();
        };
        let state = self.state.read(cx);
        let mut active = Vec::new();
        let mut past = Vec::new();
        let mut task_ids = auto.activity_chat_ids.clone();
        task_ids.extend(
            state
                .chats
                .iter()
                .filter(|chat| {
                    chat.parent_chat_id.as_ref().is_some_and(|parent| {
                        auto.conversation_chat_id.as_ref() == Some(parent)
                            || auto.activity_chat_ids.contains(parent)
                    })
                })
                .map(|chat| chat.id.clone()),
        );
        task_ids.sort();
        task_ids.dedup();
        for id in task_ids.iter().rev() {
            if let Some(chat) = state.chats.iter().find(|c| &c.id == id) {
                let status = state.display_status_for(chat, now);
                let working = matches!(
                    status,
                    ChatIndicator::Working | ChatIndicator::AwaitingInput
                );
                let detail = match status {
                    ChatIndicator::Working => "Working now",
                    ChatIndicator::AwaitingInput => "Needs your input · open task",
                    ChatIndicator::Errored => "Review error · open task",
                    _ => "Open task and review result",
                };
                let item = (
                    id.clone(),
                    chat.title.clone().unwrap_or_else(|| auto.name.clone()),
                    working,
                    detail,
                );
                if working {
                    active.push(item);
                } else {
                    past.push(item);
                }
            }
        }
        if active.is_empty() && past.is_empty() {
            col = col.child(
                div()
                    .py(px(32.0))
                    .text_color(theme.text_muted)
                    .child("No tasks yet. Start a review to see progress and results here."),
            );
        }
        for (label, items) in [("In progress", active), ("Past activity", past)] {
            if items.is_empty() {
                continue;
            }
            col = col.child(field_label(theme, label));
            for (id, title, _working, detail) in items {
                col = col.child(
                    div()
                        .id(SharedString::from(format!("dot-activity-{id}")))
                        .w_full()
                        .rounded(px(14.0))
                        .p(px(14.0))
                        .hover(|el| el.bg(theme.glass_hover()))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(14.0))
                        .child(
                            div()
                                .size(px(42.0))
                                .rounded(px(12.0))
                                .bg(theme.glass_hover())
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(crate::icons::icon(icons::LIST).size(px(18.0))),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .truncate()
                                        .text_color(theme.text)
                                        .child(SharedString::from(title)),
                                )
                                .child(
                                    div()
                                        .text_size(crate::typography::ui_rems(12.0))
                                        .text_color(theme.text_muted)
                                        .child(detail),
                                ),
                        )
                        .child(div().text_color(theme.text_faint).child("↗"))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(TeamsEvent::OpenChat(id.clone()))
                        })),
                );
            }
        }
        col = col.child(
            primary_button(
                theme,
                "dots-run-now",
                if self.delegating {
                    "Starting…"
                } else {
                    "Start a review"
                },
            )
            .on_click(cx.listener(|page, _, _, cx| page.run_briefing(cx))),
        );
        if let Some(error) = self.error.clone() {
            col = col.child(div().text_color(theme.danger).child(error));
        }
        col.into_any_element()
    }

    fn render_scheduled(
        &mut self,
        theme: &Theme,
        selected: Option<Automation>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut col = div()
            .w_full()
            .max_w(px(760.0))
            .p(px(28.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(crate::typography::ui_rems(22.0))
                    .text_color(theme.text)
                    .child("Scheduled"),
            );
        let Some(auto) = selected else {
            return col
                .child("Select a dot to see its reviews.")
                .into_any_element();
        };
        let next = auto
            .next_run_at
            .filter(|_| auto.enabled)
            .map(|at| {
                at.with_timezone(&chrono::Local)
                    .format("%d/%m · %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| {
                if auto.enabled {
                    "Waiting for the event".into()
                } else {
                    "Reviews paused".into()
                }
            });
        col = col.child(card(theme, "Review its responsibility", &crate::automations::schedule_label(&auto)))
            .child(div().text_color(theme.text_muted).child(SharedString::from(format!("Next review: {next}"))))
            .child(div().text_size(crate::typography::ui_rems(12.0)).text_color(theme.text_faint).child("Reviews run while this device’s engine is running. Pausing reviews does not stop tasks already in progress."));
        let id = auto.id.clone();
        let enabled = auto.enabled;
        col = col.child(
            div()
                .flex()
                .gap(px(10.0))
                .child(
                    primary_button(
                        theme,
                        "dots-schedule-toggle",
                        if enabled {
                            "Pause reviews"
                        } else {
                            "Enable reviews"
                        },
                    )
                    .on_click(
                        cx.listener(move |page, _, _, cx| page.set_enabled(&id, !enabled, cx)),
                    ),
                )
                .child(
                    ghost_button(theme, "dots-schedule-edit", "Edit schedule").on_click(
                        cx.listener(move |page, _, _, cx| page.open_editor(Some(&auto), cx)),
                    ),
                ),
        );
        if let Some(error) = self.error.clone() {
            col = col.child(div().text_color(theme.danger).child(error));
        }
        col.into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_dot(
        &mut self,
        theme: &Theme,
        selected: Option<Automation>,
        offered: Vec<HarnessDescriptor>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.editor.is_none() {
            if let Some(auto) = selected.as_ref() {
                let (goal, rules, memory, _) = parse_prompt(&auto.prompt);
                let mut profile =
                    div()
                        .w_full()
                        .max_w(px(760.0))
                        .p(px(32.0))
                        .flex()
                        .flex_col()
                        .gap(px(14.0))
                        .child(avatar_for(&auto.id, 72.0, cx))
                        .child(
                            div()
                                .text_size(crate::typography::ui_rems(26.0))
                                .text_color(theme.text)
                                .child(SharedString::from(auto.name.clone())),
                        )
                        .child(div().text_color(theme.text_muted).child(SharedString::from(
                            format!(
                                "{} · {} · {}",
                                self.space_name(&auto.space_id),
                                auto.harness
                                    .map(harness_name)
                                    .unwrap_or("Automatic harness"),
                                auto.model.as_deref().unwrap_or("Default model")
                            ),
                        )))
                        .child(card(theme, "Responsibility", &goal));
                if !rules.is_empty() {
                    profile = profile.child(card(theme, "Working style", &rules));
                }
                if !memory.is_empty() {
                    profile = profile.child(card(theme, "Context and preferences", &memory));
                }
                let edit = auto.clone();
                return profile
                    .child(
                        ghost_button(theme, "dots-edit-profile", "Edit profile").on_click(
                            cx.listener(move |page, _, _, cx| page.open_editor(Some(&edit), cx)),
                        ),
                    )
                    .into_any_element();
            }
        }
        // A new dot opens its setup; existing dots show their profile first.
        if self.editor.is_none() {
            let auto = selected.clone();
            self.open_editor(auto.as_ref(), cx);
        }
        let Some(editor) = self.editor.as_ref() else {
            return div().into_any_element();
        };
        let is_new = editor.id.is_empty();
        let title = if is_new {
            "New dot".to_string()
        } else {
            format!(
                "Dot · {}",
                selected
                    .as_ref()
                    .map(|a| a.name.clone())
                    .unwrap_or_default()
            )
        };
        let name_i = editor.name.clone();
        let goal_i = editor.goal.clone();
        let rules_i = editor.rules.clone();
        let memory_i = editor.memory.clone();
        let space_id = editor.space_id.clone();
        let harness = editor.harness;
        let model = editor.model.clone();
        let advanced = editor.advanced;
        let projects_open = editor.projects_open;
        let sandbox = editor.sandbox;
        let cadence = editor.cadence;
        let hour = editor.hour;
        let minute = editor.minute;
        let has_pr_trigger = editor.keep_trigger.is_some();
        let saving = editor.saving;
        let dot_enabled = editor.enabled;
        let dot_id = selected.as_ref().map(|a| a.id.clone()).unwrap_or_default();
        let delete_armed = self.delete_confirm.as_deref() == Some(dot_id.as_str()) && !is_new;

        let mut col = div()
            .w_full()
            .max_w(px(680.0))
            .px(px(24.0))
            .pt(px(32.0))
            .pb(px(48.0))
            .flex()
            .flex_col()
            .gap(px(12.0));
        col = col.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(16.0))
                .child(avatar_for(
                    if is_new { "new-dot" } else { &dot_id },
                    72.0,
                    cx,
                ))
                .child(
                    div()
                        .text_size(crate::typography::ui_rems(24.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .child(SharedString::from(title)),
                )
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(2.0))
                        .rounded_full()
                        .bg(crate::theme::ink(0.07))
                        .text_size(crate::typography::ui_rems(11.0))
                        .text_color(if dot_enabled {
                            theme.text_muted
                        } else {
                            theme.warning
                        })
                        .child(SharedString::from(if is_new {
                            "Draft".to_string()
                        } else if dot_enabled {
                            "Reviews enabled".to_string()
                        } else {
                            "Conversation only".to_string()
                        })),
                ),
        );
        col = col.child(div().text_color(theme.text_muted).child(if is_new {
            "Give it a name and tell it what you need help with."
        } else {
            "Personalize your dot and how it works with you."
        }));
        let mut left = div().w_full().flex().flex_col().gap(px(12.0));
        let mut right = div().w_full().flex().flex_col().gap(px(12.0));
        left = left.child(edit_field(theme, "Name", name_i, 40.0));
        left = left.child(edit_field(theme, "What should it help with?", goal_i, 88.0));

        // Agente.
        right = right.child(field_label(theme, "Harness"));
        {
            let mut list = div().w_full().flex().flex_col().gap(px(4.0));
            list = list.child(self.pick_row(
                "dots-harness-auto",
                "Automatic",
                "Use the default harness",
                harness.is_none(),
                theme,
                cx,
                move |page: &mut Self, cx| {
                    page.select_dot_harness(None, cx);
                },
            ));
            for (hix, descriptor) in offered.into_iter().enumerate() {
                let id = descriptor.id;
                let (mark, tint) = harness_brand_icon(id);
                let label = harness_name(id).to_string();
                let active = harness == Some(id);
                list = list.child(
                    div()
                        .id(("dots-harness", hix))
                        .w_full()
                        .px(px(10.0))
                        .py(px(7.0))
                        .rounded(px(8.0))
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .when(active, |el| el.bg(theme.glass_hover()))
                        .hover(|s| s.bg(theme.glass_hover()))
                        .child(
                            crate::icons::icon(mark)
                                .size(px(14.0))
                                .text_color(tint.unwrap_or(theme.text_muted)),
                        )
                        .child(
                            div()
                                .text_size(crate::typography::ui_rems(12.5))
                                .text_color(theme.text)
                                .child(SharedString::from(label)),
                        )
                        .when(active, |el| {
                            el.child(
                                crate::icons::icon(icons::CHECK)
                                    .size(px(13.0))
                                    .text_color(theme.text_muted),
                            )
                        })
                        .on_click(cx.listener(move |page: &mut Self, _, _, cx| {
                            page.select_dot_harness(Some(id), cx);
                        })),
                );
            }
            right = right.child(list);
        }

        right = right.child(field_label(theme, "Model"));
        if let Some(id) = harness {
            right = right.child(self.pick_row(
                "dots-model-default",
                "Harness default",
                "",
                model.is_none(),
                theme,
                cx,
                |page, cx| {
                    if let Some(e) = page.editor.as_mut() {
                        e.model = None;
                    }
                    cx.notify();
                },
            ));
            match self.models.get(&id).cloned() {
                Some(Loadable::Ready(models)) => {
                    let mut rows = div()
                        .id("dots-model-list")
                        .max_h(px(180.0))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap(px(4.0));
                    if models.is_empty() {
                        rows = rows.child(div().text_color(theme.text_muted).child("This harness has not published any models. You can use its default model."));
                    }
                    // Preserve a saved model even if it is absent from the latest catalog.
                    if let Some(saved) = model
                        .as_ref()
                        .filter(|saved| !models.iter().any(|m| &m.id == *saved))
                    {
                        rows = rows.child(
                            div()
                                .text_color(theme.text_muted)
                                .child(SharedString::from(format!("Current: {saved}"))),
                        );
                    }
                    for item in models {
                        let active = model.as_deref() == Some(item.id.as_str());
                        let picked = item.id.clone();
                        rows = rows.child(self.pick_row(
                            "dots-model",
                            &item.label,
                            item.description.as_deref().unwrap_or(""),
                            active,
                            theme,
                            cx,
                            move |page, cx| {
                                if let Some(e) = page.editor.as_mut() {
                                    e.model = Some(picked.clone());
                                }
                                cx.notify();
                            },
                        ));
                    }
                    right = right.child(rows);
                }
                Some(Loadable::Error(error)) => {
                    right = right.child(
                        div()
                            .text_color(theme.danger)
                            .child(SharedString::from(error)),
                    );
                    right = right.child(
                        ghost_button(theme, "dots-model-retry", "Retry loading models").on_click(
                            cx.listener(move |page, _, _, cx| page.load_dot_models(id, cx)),
                        ),
                    );
                }
                _ => {
                    right = right.child(div().text_color(theme.text_muted).child("Loading models…"))
                }
            }
        } else {
            right = right.child(
                div()
                    .text_color(theme.text_muted)
                    .child("Choose a harness to select one of its models."),
            );
        }

        // Proyecto.
        left = left.child(field_label(theme, "Project"));
        left = left.child(
            ghost_button(
                theme,
                "dots-project-picker",
                &format!(
                    "{}  ·  {}",
                    self.space_name(&space_id),
                    if projects_open { "Close" } else { "Change" }
                ),
            )
            .on_click(cx.listener(|page, _, _, cx| {
                if let Some(e) = page.editor.as_mut() {
                    e.projects_open = !e.projects_open;
                }
                cx.notify();
            })),
        );
        if projects_open {
            {
                let mut list = div().w_full().flex().flex_col().gap(px(4.0));
                if self.spaces.is_empty() {
                    list = list.child(
                        div()
                            .text_size(crate::typography::ui_rems(12.5))
                            .text_color(theme.text_muted)
                            .child(SharedString::from("No projects on this device.")),
                    );
                }
                for (six, space) in self.spaces.clone().into_iter().enumerate() {
                    let sid = space.id.clone();
                    let label = space.display_name().to_string();
                    let active = space_id == sid;
                    list = list.child(
                        div()
                            .id(("dots-space", six))
                            .w_full()
                            .px(px(10.0))
                            .py(px(7.0))
                            .rounded(px(8.0))
                            .cursor_pointer()
                            .truncate()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .when(active, |el| el.bg(theme.glass_hover()))
                            .hover(|s| s.bg(theme.glass_hover()))
                            .child(
                                crate::icons::icon(icons::FOLDER)
                                    .size(px(14.0))
                                    .text_color(theme.text_muted),
                            )
                            .child(
                                div()
                                    .text_size(crate::typography::ui_rems(12.5))
                                    .text_color(theme.text)
                                    .child(SharedString::from(label)),
                            )
                            .on_click(cx.listener(move |page: &mut Self, _, _, cx| {
                                if let Some(e) = page.editor.as_mut() {
                                    e.space_id = sid.clone();
                                    e.projects_open = false;
                                    cx.notify();
                                }
                            })),
                    );
                }
                left = left.child(list);
            }
        }
        col = col.child(left).child(right);
        col = col.child(
            ghost_button(
                theme,
                "dots-advanced",
                if advanced {
                    "Hide advanced options"
                } else {
                    "Advanced options"
                },
            )
            .on_click(cx.listener(|page, _, _, cx| {
                if let Some(e) = page.editor.as_mut() {
                    e.advanced = !e.advanced;
                }
                cx.notify();
            })),
        );
        if advanced {
            let mut left = div().w_full().flex().flex_col().gap(px(12.0));
            let mut right = div().w_full().flex().flex_col().gap(px(12.0));
            // Acceso.
            right = right.child(field_label(theme, "Dot permissions"));
            {
                let mut list = div().w_full().flex().flex_col().gap(px(4.0));
                for (level, title, hint) in sandbox_rows() {
                    let active = sandbox == level;
                    list = list.child(self.pick_row(
                        "dots-sandbox",
                        title,
                        hint,
                        active,
                        theme,
                        cx,
                        move |page: &mut Self, cx| {
                            if let Some(e) = page.editor.as_mut() {
                                e.sandbox = level;
                                cx.notify();
                            }
                        },
                    ));
                }
                right = right.child(list);
            }

            // Revisiones programadas.
            left = left.child(field_label(theme, "Scheduled reviews"));
            left = left.child(self.pick_row(
                "dots-schedule-enabled",
                "Review on its own",
                "Enable recurring reviews alongside the conversation",
                dot_enabled,
                theme,
                cx,
                |page, cx| {
                    if let Some(e) = page.editor.as_mut() {
                        e.enabled = !e.enabled;
                    }
                    cx.notify();
                },
            ));
            if dot_enabled {
                if has_pr_trigger {
                    left = left.child(
                        div()
                            .text_size(crate::typography::ui_rems(12.5))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(
                                "This dot responds to pull requests. This trigger is preserved when saving.",
                            )),
                    );
                } else {
                    let mut list = div().w_full().flex().flex_col().gap(px(4.0));
                    for (every, title, hint) in cadence_rows() {
                        let active = cadence == every;
                        list = list.child(self.pick_row(
                            "dots-cadence",
                            title,
                            hint,
                            active,
                            theme,
                            cx,
                            move |page: &mut Self, cx| {
                                if let Some(e) = page.editor.as_mut() {
                                    e.cadence = every;
                                    cx.notify();
                                }
                            },
                        ));
                    }
                    left = left.child(list);
                    left = left.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(crate::typography::ui_rems(12.5))
                                    .text_color(theme.text_muted)
                                    .child(SharedString::from(format!(
                                        "{hour:02}:{minute:02} UTC"
                                    ))),
                            )
                            .child(step_button(
                                theme,
                                "dots-hour-down",
                                "−",
                                cx,
                                |page, cx| {
                                    if let Some(e) = page.editor.as_mut() {
                                        e.hour = e.hour.checked_sub(1).unwrap_or(23);
                                        cx.notify();
                                    }
                                },
                            ))
                            .child(step_button(theme, "dots-hour-up", "+", cx, |page, cx| {
                                if let Some(e) = page.editor.as_mut() {
                                    e.hour = (e.hour + 1) % 24;
                                    cx.notify();
                                }
                            }))
                            .child(
                                div()
                                    .text_size(crate::typography::ui_rems(12.5))
                                    .text_color(theme.text_faint)
                                    .child(SharedString::from("hour")),
                            )
                            .child(step_button(
                                theme,
                                "dots-min-down",
                                "−",
                                cx,
                                |page, cx| {
                                    if let Some(e) = page.editor.as_mut() {
                                        e.minute = e.minute.checked_sub(1).unwrap_or(59);
                                        cx.notify();
                                    }
                                },
                            ))
                            .child(step_button(theme, "dots-min-up", "+", cx, |page, cx| {
                                if let Some(e) = page.editor.as_mut() {
                                    e.minute = (e.minute + 1) % 60;
                                    cx.notify();
                                }
                            }))
                            .child(
                                div()
                                    .text_size(crate::typography::ui_rems(12.5))
                                    .text_color(theme.text_faint)
                                    .child(SharedString::from("min")),
                            ),
                    );
                }
            }

            right = right.child(edit_field(
                theme,
                "Instructions and working style",
                rules_i,
                64.0,
            ));
            right = right.child(edit_field(theme, "Context and preferences", memory_i, 64.0));

            col = col.child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(20.0))
                    .child(left)
                    .child(right),
            );
        }

        col = col.child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(8.0))
                .child(if saving {
                    ghost_button(theme, "dots-saving", "Saving…")
                } else {
                    primary_button(
                        theme,
                        "dots-save",
                        if is_new { "Create dot" } else { "Save changes" },
                    )
                    .on_click(cx.listener(|page: &mut Self, _, _, cx| {
                        page.save_editor(cx);
                    }))
                })
                .child(
                    ghost_button(theme, "dots-cancel-edit", "Cancel").on_click(cx.listener(
                        |page, _, _, cx| {
                            page.editor = None;
                            page.tab = CenterTab::Chat;
                            page.error = None;
                            cx.notify();
                        },
                    )),
                )
                .when(!is_new, |el| {
                    el.child(
                        ghost_button(
                            theme,
                            "dots-delete",
                            if delete_armed {
                                "Confirm deletion"
                            } else {
                                "Delete"
                            },
                        )
                        .on_click(cx.listener(
                            |page: &mut Self, _, _, cx| {
                                page.delete_selected(cx);
                            },
                        )),
                    )
                }),
        );
        if let Some(notice) = self.notice.clone() {
            col = col.child(
                div()
                    .text_size(crate::typography::ui_rems(12.0))
                    .text_color(theme.text_muted)
                    .child(notice),
            );
        }
        if let Some(error) = self.error.clone() {
            col = col.child(
                div()
                    .text_size(crate::typography::ui_rems(12.0))
                    .text_color(theme.danger)
                    .child(error),
            );
        }
        col.into_any_element()
    }

    fn pick_row(
        &self,
        id: &'static str,
        title: &str,
        hint: &str,
        active: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
        on_pick: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(format!("{id}-{title}")))
            .w_full()
            .px(px(10.0))
            .py(px(7.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .when(active, |el| el.bg(theme.glass_hover()))
            .hover(|s| s.bg(theme.glass_hover()))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(crate::typography::ui_rems(12.5))
                            .text_color(theme.text)
                            .child(SharedString::from(title.to_string())),
                    )
                    .child(
                        div()
                            .text_size(crate::typography::ui_rems(11.5))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(hint.to_string())),
                    ),
            )
            .when(active, |el| {
                el.child(
                    crate::icons::icon(icons::CHECK)
                        .size(px(13.0))
                        .text_color(theme.text_muted),
                )
            })
            .on_click(cx.listener(move |page: &mut Self, _, _, cx| {
                on_pick(page, cx);
            }))
    }
}

fn step_button(
    theme: &Theme,
    id: &'static str,
    label: &str,
    cx: &mut Context<TeamsPage>,
    on_step: impl Fn(&mut TeamsPage, &mut Context<TeamsPage>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(px(24.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(theme.border_strong)
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme.text)
        .text_size(crate::typography::ui_rems(13.0))
        .cursor_pointer()
        .child(SharedString::from(label.to_string()))
        .on_click(cx.listener(move |page: &mut TeamsPage, _, _, cx| {
            on_step(page, cx);
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_bubbles_hide_context_and_preserve_the_exact_message() {
        let message = "Revisa esto\n\n<!-- clyra-dot-message -->\nY esto también";
        let raw = format!(
            "<!-- clyra-dot-context -->\n# Objetivo\nAyudar\n\n<!-- clyra-dot-message -->\n{message}"
        );
        assert_eq!(conversation_message(&raw), message);
        assert_eq!(
            conversation_message("# Mensaje\nTexto normal"),
            "# Mensaje\nTexto normal"
        );
    }

    #[test]
    fn prompt_sections_round_trip() {
        let composed = compose_prompt("vigilar la API", "sin prisa", "nada aún", "revisa /saludo");
        let (goal, rules, memory, task) = parse_prompt(&composed);
        assert_eq!(goal, "vigilar la API");
        assert_eq!(rules, "sin prisa");
        assert_eq!(memory, "nada aún");
        assert_eq!(task, "revisa /saludo");
    }

    #[test]
    fn prompt_without_marks_becomes_goal() {
        let (goal, rules, memory, task) = parse_prompt("haz la migración");
        assert_eq!(goal, "haz la migración");
        assert!(rules.is_empty() && memory.is_empty() && task.is_empty());
    }

    #[test]
    fn dot_color_is_stable() {
        assert_eq!(dot_color("abc"), dot_color("abc"));
    }
}
