//! Automations — the list surface for prompts that start a run on their own.
//!
//! Mirrors the reference client's shape: one card per automation (trigger on
//! top, name, then project · last run · model), a kill switch on the right,
//! and a "Run now" that does not wait for the schedule. The engine owns the
//! records and the firing ([`crate::engine::automations`]); this page only
//! reads the snapshot, mutates it, and renders.
//!
//! The editor is a single modal: an automation is a name, a project, a
//! trigger, a prompt, a provider and an access level. Everything else about a
//! run (tools, worktree, permissions) is the session's business once the run
//! starts.

use chrono::{DateTime, Utc};
use clyra_engine::registry::HarnessDescriptor;
use clyra_proto::{
    Automation, AutomationCadence, AutomationDraft, AutomationRunAck, AutomationSchedule,
    AutomationTrigger, AutomationsSnapshot, HarnessId, Model, PullRequestEvent, PullRequestTrigger,
    SandboxLevel, Space,
};
use clyra_rpc::methods;
use gpui::{
    AnyElement, Context, Entity, SharedString, Subscription, Task, Window, div, prelude::*, px,
};

use crate::composer::ComposerInput;
use crate::popover::{self, Loadable};
use crate::settings::widgets;
use crate::state::AppState;
use crate::theme::Theme;

/// Human names for the ISO weekday a weekly schedule fires on.
const WEEKDAYS: [(&str, u8); 7] = [
    ("Monday", 1),
    ("Tuesday", 2),
    ("Wednesday", 3),
    ("Thursday", 4),
    ("Friday", 5),
    ("Saturday", 6),
    ("Sunday", 7),
];

pub struct AutomationsPage {
    state: Entity<AppState>,
    scroll: widgets::PageScroll,
    snapshot: Loadable<AutomationsSnapshot>,
    /// Projects an automation can target, read from the workspace rows.
    spaces: Vec<Space>,
    load_task: Option<Task<()>>,
    action_task: Option<Task<()>>,
    catalog_task: Option<Task<()>>,
    error: Option<SharedString>,
    /// A one-line confirmation ("Started a session", "Deleted"). Cleared by
    /// the next mutation — a notice that can go stale is worse than none.
    notice: Option<SharedString>,
    editor: Option<Editor>,
    delete_confirm: Option<String>,
    /// Row under the pointer — "Run now" is revealed on hover, never a
    /// permanent third control on every card.
    hovered: Option<usize>,
    harnesses: Vec<HarnessDescriptor>,
    models: Vec<Model>,
    project_menu_state: popover::Popup<()>,
    trigger_menu_state: popover::Popup<()>,
    weekday_menu_state: popover::Popup<()>,
    harness_menu_state: popover::Popup<()>,
    model_menu_state: popover::Popup<()>,
    sandbox_menu_state: popover::Popup<()>,
    /// Shared by the six dropdowns; see [`menu_card`].
    menu_scroll: gpui::ScrollHandle,
    _observe: Subscription,
}

/// The half-open form of a record while it is being edited. Fields are plain
/// values so a stepper or a menu row is a one-line mutation; the draft is
/// assembled once, on save.
struct Editor {
    /// Empty for a new automation.
    id: String,
    name: Entity<ComposerInput>,
    prompt: Entity<ComposerInput>,
    space_id: String,
    pull_request: bool,
    cadence: AutomationCadence,
    hour: u8,
    minute: u8,
    weekday: u8,
    opened: bool,
    updated: bool,
    harness: Option<HarnessId>,
    model: Option<String>,
    sandbox: SandboxLevel,
    enabled: bool,
    saving: bool,
}

impl AutomationsPage {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&state, |_, _, cx| cx.notify());
        let mut page = Self {
            state,
            scroll: widgets::PageScroll::default(),
            snapshot: Loadable::Idle,
            spaces: Vec::new(),
            load_task: None,
            action_task: None,
            catalog_task: None,
            error: None,
            notice: None,
            editor: None,
            delete_confirm: None,
            hovered: None,
            harnesses: Vec::new(),
            models: Vec::new(),
            project_menu_state: popover::Popup::default(),
            trigger_menu_state: popover::Popup::default(),
            weekday_menu_state: popover::Popup::default(),
            harness_menu_state: popover::Popup::default(),
            model_menu_state: popover::Popup::default(),
            sandbox_menu_state: popover::Popup::default(),
            menu_scroll: gpui::ScrollHandle::default(),
            _observe: observe,
        };
        page.load(cx);
        page
    }

    /// The list plus the project rows it labels rows with. Both come from the
    /// engine: the automations are host-local, the spaces are synced. Public
    /// to the shell: arriving on the destination re-probes (a schedule may
    /// have fired since the last visit).
    pub(super) fn load(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            self.snapshot = Loadable::Error("Engine not connected".into());
            return;
        };
        if !self.snapshot.is_loading() {
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
                // Only projects this device can run in: an automation fires
                // where its CLI lives, so offering a remote project would
                // create something that can never start.
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
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Re-arm a fired schedule, kill one, or delete it. Every mutation
    /// re-lists: the engine owns the truth, including the next fire time.
    fn act(
        &mut self,
        method: &'static str,
        params: serde_json::Value,
        notice: &'static str,
        cx: &mut Context<Self>,
    ) {
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            self.error = Some("Engine not connected".into());
            return;
        };
        self.error = None;
        self.notice = Some(notice.into());
        self.action_task = Some(cx.spawn(async move |this, cx| {
            let result = engine.client().call(method, params).await;
            this.update(cx, |page, cx| {
                if let Err(err) = result {
                    page.error = Some(err.to_string().into());
                    page.notice = None;
                }
                cx.notify();
            })
            .ok();
            this.update(cx, |page, cx| page.load(cx)).ok();
        }));
        cx.notify();
    }

    fn set_enabled(&mut self, automation: &Automation, enabled: bool, cx: &mut Context<Self>) {
        self.act(
            methods::SET_AUTOMATION_ENABLED,
            serde_json::json!({ "automationId": automation.id, "enabled": enabled }),
            if enabled {
                "Automation enabled"
            } else {
                "Automation paused"
            },
            cx,
        );
    }

    fn run_now(&mut self, automation: &Automation, cx: &mut Context<Self>) {
        let id = automation.id.clone();
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            self.error = Some("Engine not connected".into());
            return;
        };
        self.error = None;
        self.notice = Some("Starting a session…".into());
        self.action_task = Some(cx.spawn(async move |this, cx| {
            let result = engine
                .client()
                .call(
                    methods::RUN_AUTOMATION_NOW,
                    serde_json::json!({ "automationId": id }),
                )
                .await;
            this.update(cx, |page, cx| {
                match result {
                    Ok(value) => {
                        let chat_id = serde_json::from_value::<AutomationRunAck>(value)
                            .map(|ack| ack.chat_id)
                            .unwrap_or_default();
                        page.notice = Some(if chat_id.is_empty() {
                            "Session started".into()
                        } else {
                            "Session started in the sidebar".into()
                        });
                    }
                    Err(err) => {
                        page.error = Some(err.to_string().into());
                        page.notice = None;
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Delete, two-step: the first press arms the button (and only it), the
    /// second removes. A confirmation dialog for one row would be heavier than
    /// the mistake it guards.
    fn delete(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.delete_confirm.as_deref() != Some(id) {
            self.delete_confirm = Some(id.to_string());
            cx.notify();
            return;
        }
        self.delete_confirm = None;
        self.act(
            methods::DELETE_AUTOMATION,
            serde_json::json!({ "automationId": id }),
            "Automation deleted",
            cx,
        );
        // There is nothing left to edit: leaving the form up would be editing a
        // row that is already gone.
        if self.editor.as_ref().map(|editor| editor.id.as_str()) == Some(id) {
            self.close_editor(cx);
        }
    }

    // ---- editor ----------------------------------------------------------

    fn open_editor(&mut self, automation: Option<&Automation>, cx: &mut Context<Self>) {
        // The editor replaces the list in the same scroller: start it at the
        // top so the name field is always the first thing you see.
        self.scroll.scroll.set_offset(gpui::Point::default());
        // The provider/model lists back the editor's two pickers.
        let harness = automation
            .and_then(|a| a.harness)
            .or_else(|| self.harnesses.first().map(|h| h.id));
        self.load_catalog(harness, cx);
        let name = cx.new(|cx| ComposerInput::new("Nightly review", cx));
        let prompt = cx.new(|cx| ComposerInput::new("What should this automation do?", cx));
        if let Some(automation) = automation {
            // Seeded rather than bound: the save reads the live text, so an
            // empty field is a validation error, not a silent blank prompt.
            name.update(cx, |input, cx| input.set_text(automation.name.clone(), cx));
            prompt.update(cx, |input, cx| {
                input.set_text(automation.prompt.clone(), cx)
            });
        }
        let schedule = automation
            .and_then(|a| a.trigger.schedule())
            .copied()
            .unwrap_or(AutomationSchedule {
                every: AutomationCadence::Weekdays,
                hour: 9,
                minute: 0,
                weekday: 1,
            });
        let (pull_request, opened, updated) = match automation.map(|a| &a.trigger) {
            Some(AutomationTrigger::PullRequest(trigger)) => (
                true,
                trigger.events.contains(&PullRequestEvent::Opened),
                trigger.events.contains(&PullRequestEvent::Updated),
            ),
            _ => (false, true, false),
        };
        self.editor = Some(Editor {
            id: automation.map(|a| a.id.clone()).unwrap_or_default(),
            name,
            prompt,
            space_id: automation
                .map(|a| a.space_id.clone())
                .or_else(|| self.spaces.first().map(|space| space.id.clone()))
                .unwrap_or_default(),
            pull_request,
            cadence: schedule.every,
            hour: schedule.hour,
            minute: schedule.minute,
            weekday: schedule.weekday.max(1),
            opened,
            updated,
            harness: automation.and_then(|a| a.harness),
            model: automation.and_then(|a| a.model.clone()),
            sandbox: automation
                .map(|a| a.sandbox)
                .unwrap_or(SandboxLevel::WorkspaceWrite),
            enabled: automation.map(|a| a.enabled).unwrap_or(true),
            saving: false,
        });
        cx.notify();
    }

    fn close_editor(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        self.delete_confirm = None;
        // The dropdowns unmount with the editor (their triggers live inside it);
        // clearing the flag is what drops them.
        self.project_menu_state.begin_close();
        self.trigger_menu_state.begin_close();
        self.weekday_menu_state.begin_close();
        self.harness_menu_state.begin_close();
        self.model_menu_state.begin_close();
        self.sandbox_menu_state.begin_close();
        // Both views share the scroller, so the list must come back where the
        // user left it, not wherever the editor ended.
        self.scroll.scroll.set_offset(gpui::Point::default());
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
        let prompt = editor.prompt.read(cx).text().trim().to_string();
        if name.is_empty() || prompt.is_empty() || editor.space_id.is_empty() {
            self.error = Some("Name, project and prompt are required".into());
            return;
        }
        let trigger = if editor.pull_request {
            AutomationTrigger::PullRequest(PullRequestTrigger {
                repo: String::new(),
                events: [
                    (PullRequestEvent::Opened, editor.opened),
                    (PullRequestEvent::Updated, editor.updated),
                ]
                .into_iter()
                .filter_map(|(event, on)| on.then_some(event))
                .collect(),
            })
        } else {
            AutomationTrigger::Schedule(AutomationSchedule {
                every: editor.cadence,
                hour: editor.hour,
                minute: editor.minute,
                weekday: editor.weekday,
            })
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
        self.error = None;
        self.act(
            methods::UPSERT_AUTOMATION,
            serde_json::json!({ "draft": draft }),
            "Automation saved",
            cx,
        );
        self.editor = None;
    }

    /// Provider list, then the model list for the picked provider.
    fn load_catalog(&mut self, harness: Option<HarnessId>, cx: &mut Context<Self>) {
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
            let target = harness.or_else(|| harnesses.first().map(|h| h.id));
            let models = match target {
                Some(id) => engine
                    .client()
                    .call(methods::LIST_MODELS, serde_json::json!({ "harness": id }))
                    .await
                    .ok()
                    .and_then(|value| serde_json::from_value::<Vec<Model>>(value).ok())
                    .unwrap_or_default(),
                None => Vec::new(),
            };
            this.update(cx, |page, cx| {
                page.harnesses = harnesses;
                page.models = models;
                cx.notify();
            })
            .ok();
        }));
    }
}

impl popover::ScrollRailHost for AutomationsPage {
    fn rail_bar(&mut self) -> &mut popover::MenuScrollbarState {
        self.scroll.rail_bar()
    }
    fn rail_scroll(&self) -> Option<gpui::ScrollHandle> {
        self.scroll.rail_scroll()
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl gpui::Render for AutomationsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let theme = Theme::of(cx).clone();
        let now = Utc::now();
        let scrollbar = popover::rail(self, "automations-page-scrollbar", &theme, cx);
        // Two states of ONE surface: the list, or the editor that takes its
        // place. A modal floating over the list would cover the very rows you
        // are configuring against and add a second thing to dismiss.
        let body: AnyElement = if self.editor.is_some() {
            self.render_editor(&theme, cx)
        } else {
            self.render_list(&theme, now, cx)
        };
        div()
            .id("automations-page-host")
            .relative()
            .size_full()
            .on_hover(cx.listener(|page: &mut Self, hovered: &bool, _, cx| {
                if page.scroll.set_list_hovered(*hovered) {
                    cx.notify();
                }
            }))
            .child(
                div()
                    .id("automations-page")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll.scroll)
                    .child(body),
            )
            .children(scrollbar)
    }
}

impl AutomationsPage {
    fn render_list(
        &mut self,
        theme: &Theme,
        now: DateTime<Utc>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (automations, warnings) = match &self.snapshot {
            Loadable::Ready(snapshot) => (snapshot.automations.clone(), snapshot.warnings.clone()),
            Loadable::Loading if self.snapshot.ready().is_none() => {
                return widgets::page_column()
                    .child(widgets::page_header(
                        theme,
                        crate::icons::CLOCK_CIRCLE,
                        "Automations",
                        None,
                    ))
                    .child(widgets::page_subtitle(theme, "Loading…"))
                    .into_any_element();
            }
            Loadable::Error(message) => {
                return widgets::page_column()
                    .child(widgets::page_header(
                        theme,
                        crate::icons::CLOCK_CIRCLE,
                        "Automations",
                        None,
                    ))
                    .child(widgets::error_strip(theme, message))
                    .into_any_element();
            }
            _ => (Vec::new(), Vec::new()),
        };
        let count = (!automations.is_empty()).then_some(automations.len());
        let mut column = widgets::page_column()
            .child(widgets::page_header(
                theme,
                crate::icons::CLOCK_CIRCLE,
                "Automations",
                count,
            ))
            .child(widgets::page_subtitle(
                theme,
                "Prompts that start a session on their own — on a schedule, or when a pull request moves.",
            ));
        if let Some(strip) = self.render_error(&theme, cx) {
            column = column.child(strip);
        }
        if let Some(notice) = self.notice.clone() {
            column = column.child(widgets::badge(theme, &notice));
        }
        for warning in warnings {
            column = column.child(widgets::warning_strip(theme, &warning));
        }
        let body: AnyElement = if automations.is_empty() {
            div()
                .mt(px(72.0))
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .text_color(theme.text_muted.opacity(0.5))
                .child(
                    crate::icons::icon(crate::icons::CLOCK_CIRCLE)
                        .size(px(28.0))
                        .text_color(theme.text_muted.opacity(0.2)),
                )
                .child(
                    div()
                        .mt(px(12.0))
                        .text_size(crate::typography::ui_rems(14.0))
                        .child(SharedString::from("No automations yet")),
                )
                .child(
                    div()
                        .mt(px(4.0))
                        .text_size(crate::typography::ui_rems(12.0))
                        .text_color(theme.text_muted.opacity(0.4))
                        .child(SharedString::from(
                            "Add one to review a project every morning, or on new pull requests.",
                        )),
                )
                .into_any_element()
        } else {
            let rows: Vec<AnyElement> = automations
                .iter()
                .enumerate()
                .map(|(ix, automation)| self.render_row(ix, automation, theme, now, cx))
                .collect();
            div()
                .mt(px(16.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .children(rows)
                .into_any_element()
        };
        column
            .child(
                // The one creation affordance, top-right of the list: a page
                // of existing automations is not a form.
                div().mt(px(4.0)).flex().justify_end().child(
                    widgets::ghost_action(theme)
                        .id("automations-add")
                        .child(crate::icons::icon(crate::icons::PLUS).size(px(14.0)))
                        .child(SharedString::from("Add automation"))
                        .on_click(
                            cx.listener(|page: &mut Self, _, _, cx| page.open_editor(None, cx)),
                        ),
                ),
            )
            .child(body)
            .into_any_element()
    }

    /// One automation card: the trigger chip on top (the cadence for a
    /// schedule, the events for a pull request), then the name, then the meta
    /// line — project · last run · provider — and the kill switch on the
    /// right. The whole row opens the editor; the toggle is its own target.
    fn render_row(
        &mut self,
        ix: usize,
        automation: &Automation,
        theme: &Theme,
        now: DateTime<Utc>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = automation.id.clone();
        let label = automation.name.clone();
        let enabled = automation.enabled;
        let pull_request = matches!(&automation.trigger, AutomationTrigger::PullRequest(_));
        let (chip, chip_icon) = if pull_request {
            (pull_request_label(automation), crate::icons::GIT_BRANCH)
        } else {
            (schedule_label(automation), crate::icons::CLOCK_CIRCLE)
        };
        let model = automation
            .model
            .clone()
            .or_else(|| {
                automation
                    .harness
                    .map(|id| harness_label(id, &self.harnesses))
            })
            .unwrap_or_else(|| "Default provider".into());
        let last_run = automation.last_run_at.map(|at| time_ago(at, now));
        let project = automation.project_label.clone();
        let toggle = widgets::toggle_switch(theme, enabled)
            .id(("automation-toggle", ix))
            .cursor_pointer()
            .on_click(cx.listener({
                // The row's own click also needs the id, so the toggle
                // handler takes its own copy.
                let id = id.clone();
                move |page: &mut Self, _, _, cx| {
                    page.set_enabled_by_id(&id, !enabled, cx);
                }
            }));
        div()
            .id(("automation-row", ix))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .rounded(px(10.0))
            .border_1()
            .border_color(if enabled {
                theme.border
            } else {
                theme.border.opacity(0.6)
            })
            .px(px(14.0))
            .py(px(12.0))
            .cursor_pointer()
            .hover(|s| s.bg(theme.surface_raised))
            .on_hover(cx.listener(move |page: &mut Self, hovered: &bool, _, cx| {
                if *hovered {
                    page.hovered = Some(ix);
                } else if page.hovered == Some(ix) {
                    page.hovered = None;
                }
                cx.notify();
            }))
            .on_click(cx.listener({
                // The toggle above took a copy; this one owns the id.
                let id = id.clone();
                move |page: &mut Self, _, _, cx| {
                    let Some(automation) = page.automation(&id).cloned() else {
                        return;
                    };
                    page.open_editor(Some(&automation), cx);
                }
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .text_size(crate::typography::ui_rems(11.0))
                            .text_color(theme.text_muted.opacity(0.7))
                            .child(
                                crate::icons::icon(chip_icon)
                                    .size(px(12.0))
                                    .text_color(theme.text_faint),
                            )
                            .child(SharedString::from(chip)),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(crate::typography::ui_rems(14.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(if enabled {
                                theme.text
                            } else {
                                theme.text_muted
                            })
                            .child(SharedString::from(label)),
                    )
                    .child(meta_line(theme, &[Some(project), last_run, Some(model)])),
            )
            .child(
                // "Run now": the escape hatch for a schedule you do not want
                // to wait for. Fires a disabled automation too — the toggle
                // governs the schedule, not an explicit press.
                widgets::ghost_action(theme)
                    .id(("automation-run-now", ix))
                    .flex_none()
                    .opacity(if self.hovered == Some(ix) { 1.0 } else { 0.0 })
                    .child(crate::icons::icon(crate::icons::REFRESH).size(px(13.0)))
                    .child(SharedString::from("Run now"))
                    .on_click(cx.listener({
                        let id = id.clone();
                        move |page: &mut Self, _, _, cx| {
                            let Some(automation) = page.automation(&id).cloned() else {
                                return;
                            };
                            page.run_now(&automation, cx);
                        }
                    })),
            )
            .child(toggle)
            .into_any_element()
    }

    fn automation(&self, id: &str) -> Option<&Automation> {
        self.snapshot
            .ready()?
            .automations
            .iter()
            .find(|automation| automation.id == id)
    }

    fn set_enabled_by_id(&mut self, id: &str, enabled: bool, cx: &mut Context<Self>) {
        let Some(automation) = self.automation(id).cloned() else {
            return;
        };
        self.set_enabled(&automation, enabled, cx);
    }
}

// ---------------------------------------------------------------------------
// Editor
// ---------------------------------------------------------------------------

impl AutomationsPage {
    fn render_editor(&mut self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = self.editor.as_ref() else {
            return div().into_any_element();
        };
        let (
            space_id,
            pull_request,
            cadence,
            hour,
            minute,
            weekday,
            opened,
            updated,
            harness,
            model,
            sandbox,
            enabled,
            saving,
            editing,
        ) = (
            editor.space_id.clone(),
            editor.pull_request,
            editor.cadence,
            editor.hour,
            editor.minute,
            editor.weekday,
            editor.opened,
            editor.updated,
            editor.harness,
            editor.model.clone(),
            editor.sandbox,
            editor.enabled,
            editor.saving,
            !editor.id.is_empty(),
        );
        let name_input = editor.name.clone();
        let prompt_input = editor.prompt.clone();
        let spaces: Vec<Space> = self.spaces.clone();
        let harnesses = self.harnesses.clone();
        let space_label = spaces
            .iter()
            .find(|space| space.id == space_id)
            .map(|space| space.name.clone().unwrap_or_else(|| space.path.clone()))
            .unwrap_or_else(|| "Choose a project".into());
        let harness_label = harness
            .map(|id| harness_label(id, &harnesses))
            .unwrap_or_else(|| "Default provider".into());
        let model_label = model.clone().unwrap_or_else(|| "Default model".into());

        // Built before the fields: each menu rides inside its own trigger, and
        // a trigger is part of the field row.
        let project_menu = self.project_menu(theme, cx);
        let cadence_menu = self.cadence_menu(theme, cx);
        let weekday_menu = self.weekday_menu(theme, cx);
        let harness_menu = self.harness_menu(theme, cx);
        let model_menu = self.model_menu(theme, cx);
        let sandbox_menu = self.sandbox_menu(theme, cx);

        let mut rows: Vec<AnyElement> = Vec::new();
        rows.push(field(
            theme,
            "Name",
            div()
                .id("automation-name")
                .h(px(32.0))
                .w_full()
                .px(px(8.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(theme.border)
                .child(name_input),
        ));
        rows.push(field(
            theme,
            "Project",
            menu_button(
                theme,
                "automation-project",
                &space_label,
                self.project_menu_state.is_open(),
                on(cx.entity(), |page, cx| {
                    if page.project_menu_state.begin_close() {
                        popover::reap_popup(cx, |page: &mut AutomationsPage| {
                            &mut page.project_menu_state
                        });
                    } else {
                        page.project_menu_state.open(());
                    }
                    cx.notify();
                }),
                project_menu,
            ),
        ));
        // Trigger: a two-way choice first, then only the fields that kind
        // needs. Showing a clock next to a pull-request trigger would be a lie.
        rows.push(field(
            theme,
            "Starts on",
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(segment(
                    theme,
                    "automation-trigger-schedule",
                    "Schedule",
                    !pull_request,
                    on(cx.entity(), |page, cx| {
                        if let Some(editor) = page.editor.as_mut() {
                            editor.pull_request = false;
                        }
                        cx.notify();
                    }),
                ))
                .child(segment(
                    theme,
                    "automation-trigger-pr",
                    "Pull request",
                    pull_request,
                    on(cx.entity(), |page, cx| {
                        if let Some(editor) = page.editor.as_mut() {
                            editor.pull_request = true;
                        }
                        let harness = page.editor.as_ref().and_then(|editor| editor.harness);
                        page.load_catalog(harness, cx);
                        cx.notify();
                    }),
                )),
        ));
        if pull_request {
            rows.push(field(
                theme,
                "Events",
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(14.0))
                    .child(check_row(theme, "Pull request opened", opened, {
                        let page = cx.entity();
                        move |_, _, cx| {
                            page.update(cx, |page: &mut AutomationsPage, cx| {
                                if let Some(editor) = page.editor.as_mut() {
                                    editor.opened = !editor.opened;
                                }
                                cx.notify();
                            })
                        }
                    }))
                    .child(check_row(theme, "Pull request updated", updated, {
                        let page = cx.entity();
                        move |_, _, cx| {
                            page.update(cx, |page: &mut AutomationsPage, cx| {
                                if let Some(editor) = page.editor.as_mut() {
                                    editor.updated = !editor.updated;
                                }
                                cx.notify();
                            })
                        }
                    })),
            ));
            rows.push(hint(
                theme,
                "Reads the open pull requests of this project's repository through the GitHub CLI.",
            ));
        } else {
            rows.push(field(
                theme,
                "Cadence",
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(menu_button(
                        theme,
                        "automation-cadence",
                        cadence_label(cadence),
                        self.trigger_menu_state.is_open(),
                        on(cx.entity(), |page, cx| {
                            if page.trigger_menu_state.begin_close() {
                                popover::reap_popup(cx, |page: &mut AutomationsPage| {
                                    &mut page.trigger_menu_state
                                });
                            } else {
                                page.trigger_menu_state.open(());
                            }
                            cx.notify();
                        }),
                        cadence_menu,
                    ))
                    .child(time_stepper(
                        theme,
                        "automation-hour",
                        &format!("{hour:02}"),
                        {
                            let page = cx.entity();
                            move |_, _, cx| {
                                page.update(cx, |page: &mut AutomationsPage, cx| {
                                    if let Some(editor) = page.editor.as_mut() {
                                        editor.hour = (editor.hour + 23) % 24;
                                    }
                                    cx.notify();
                                })
                            }
                        },
                        {
                            let page = cx.entity();
                            move |_, _, cx| {
                                page.update(cx, |page: &mut AutomationsPage, cx| {
                                    if let Some(editor) = page.editor.as_mut() {
                                        editor.hour = (editor.hour + 1) % 24;
                                    }
                                    cx.notify();
                                })
                            }
                        },
                    ))
                    .child(
                        div()
                            .text_size(crate::typography::ui_rems(12.5))
                            .text_color(theme.text_faint)
                            .child(SharedString::from(":")),
                    )
                    .child(time_stepper(
                        theme,
                        "automation-minute",
                        &format!("{minute:02}"),
                        {
                            let page = cx.entity();
                            move |_, _, cx| {
                                page.update(cx, |page: &mut AutomationsPage, cx| {
                                    if let Some(editor) = page.editor.as_mut() {
                                        editor.minute = (editor.minute + 55) % 60;
                                    }
                                    cx.notify();
                                })
                            }
                        },
                        {
                            let page = cx.entity();
                            move |_, _, cx| {
                                page.update(cx, |page: &mut AutomationsPage, cx| {
                                    if let Some(editor) = page.editor.as_mut() {
                                        editor.minute = (editor.minute + 5) % 60;
                                    }
                                    cx.notify();
                                })
                            }
                        },
                    )),
            ));
            if cadence == AutomationCadence::Weekly {
                rows.push(field(
                    theme,
                    "On",
                    menu_button(
                        theme,
                        "automation-weekday",
                        WEEKDAYS
                            .iter()
                            .find(|(_, day)| *day == weekday)
                            .map(|(name, _)| *name)
                            .unwrap_or("Monday"),
                        self.weekday_menu_state.is_open(),
                        on(cx.entity(), |page, cx| {
                            if page.weekday_menu_state.begin_close() {
                                popover::reap_popup(cx, |page: &mut AutomationsPage| {
                                    &mut page.weekday_menu_state
                                });
                            } else {
                                page.weekday_menu_state.open(());
                            }
                            cx.notify();
                        }),
                        weekday_menu,
                    ),
                ));
            }
            rows.push(hint(
                theme,
                "UTC — the engine owns the clock, so the same automation means the same instant on every device.",
            ));
        }
        rows.push(field(
            theme,
            "Provider",
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(menu_button(
                    theme,
                    "automation-harness",
                    &harness_label,
                    self.harness_menu_state.is_open(),
                    on(cx.entity(), |page, cx| {
                        if page.harness_menu_state.begin_close() {
                            popover::reap_popup(cx, |page: &mut AutomationsPage| {
                                &mut page.harness_menu_state
                            });
                        } else {
                            page.harness_menu_state.open(());
                        }
                        cx.notify();
                    }),
                    harness_menu,
                ))
                .child(menu_button(
                    theme,
                    "automation-model",
                    &model_label,
                    self.model_menu_state.is_open(),
                    on(cx.entity(), |page, cx| {
                        if page.model_menu_state.begin_close() {
                            popover::reap_popup(cx, |page: &mut AutomationsPage| {
                                &mut page.model_menu_state
                            });
                        } else {
                            page.model_menu_state.open(());
                        }
                        cx.notify();
                    }),
                    model_menu,
                )),
        ));
        rows.push(field(
            theme,
            "Access",
            menu_button(
                theme,
                "automation-sandbox",
                sandbox_label(sandbox),
                self.sandbox_menu_state.is_open(),
                on(cx.entity(), |page, cx| {
                    if page.sandbox_menu_state.begin_close() {
                        popover::reap_popup(cx, |page: &mut AutomationsPage| {
                            &mut page.sandbox_menu_state
                        });
                    } else {
                        page.sandbox_menu_state.open(());
                    }
                    cx.notify();
                }),
                sandbox_menu,
            ),
        ));
        rows.push(field(
            theme,
            "Prompt",
            div()
                .id("automation-prompt")
                .min_h(px(96.0))
                .w_full()
                .px(px(8.0))
                .py(px(6.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(theme.border)
                .child(prompt_input),
        ));
        rows.push(field(
            theme,
            "Starts on launch",
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(widgets::toggle_switch(theme, enabled))
                .child(
                    div()
                        .text_size(crate::typography::ui_rems(12.0))
                        .text_color(theme.text_muted)
                        .child(SharedString::from(if enabled {
                            "Runs unattended"
                        } else {
                            "Paused"
                        })),
                ),
        ));

        // The editor is a page, not a card: it reuses the list's own column so
        // the two read as one surface, and the header doubles as the way back
        // to the list.
        let mut column = widgets::page_column();
        if let Some(strip) = self.render_error(&theme, cx) {
            column = column.child(strip);
        }
        column
            .child(
                div()
                    .id("automation-editor-header")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .cursor_pointer()
                    .hover(|style| style.text_color(theme.text))
                    .on_click(cx.listener(|page: &mut Self, _, _, cx| page.close_editor(cx)))
                    .child(
                        crate::icons::icon(crate::icons::ALT_ARROW_LEFT)
                            .size(px(16.0))
                            .text_color(theme.text_muted),
                    )
                    .child(
                        div()
                            .text_size(crate::typography::ui_rems(22.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(SharedString::from(if editing {
                                "Edit automation"
                            } else {
                                "New automation"
                            })),
                    ),
            )
            .child(widgets::page_subtitle(
                theme,
                "A prompt that starts a session on its own.",
            ))
            .child(
                div()
                    .mt(px(18.0))
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .children(rows),
            )
            .child(
                div()
                    .mt(px(18.0))
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().flex_1())
                    .when(editing, |el| {
                        let armed = self.delete_confirm.is_some();
                        el.child(
                            widgets::ghost_action(theme)
                                .id("automation-delete")
                                .text_color(theme.danger)
                                .child(SharedString::from(if armed {
                                    "Confirm delete"
                                } else {
                                    "Delete"
                                }))
                                .on_click(cx.listener(|page: &mut Self, _, _, cx| {
                                    let id = page
                                        .editor
                                        .as_ref()
                                        .map(|editor| editor.id.clone())
                                        .unwrap_or_default();
                                    if !id.is_empty() {
                                        page.delete(&id, cx);
                                    }
                                })),
                        )
                    })
                    .child(
                        widgets::ghost_action(theme)
                            .id("automation-cancel")
                            .child(SharedString::from("Cancel"))
                            .on_click(
                                cx.listener(|page: &mut Self, _, _, cx| page.close_editor(cx)),
                            ),
                    )
                    .child(
                        widgets::ghost_action(theme)
                            .id("automation-save")
                            .opacity(if saving { 0.5 } else { 1.0 })
                            .child(SharedString::from(if saving {
                                "Saving…"
                            } else {
                                "Save automation"
                            }))
                            .on_click(
                                cx.listener(|page: &mut Self, _, _, cx| page.save_editor(cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// Inline failure, dismissible. Both views show it: the list owns the
    /// engine/action errors, the editor owns the validation ones, and the
    /// editor used to be an overlay that hid the list's copy.
    fn render_error(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let message = self.error.clone()?;
        Some(
            widgets::error_strip(theme, message)
                .id("automations-error")
                .cursor_pointer()
                .on_click(cx.listener(|page: &mut Self, _, _, cx| {
                    page.error = None;
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    /// The six dropdowns. Each builds only its own rows, and the caller mounts
    /// the result inside the matching trigger: a menu is positioned against
    /// its parent, so trigger and menu have to share a box.
    fn project_menu(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.project_menu_state.is_open() {
            return None;
        }
        let current = self.editor.as_ref().map(|editor| editor.space_id.clone());
        let spaces = self.spaces.clone();
        let rows: Vec<AnyElement> = spaces
            .iter()
            .map(|space| {
                let id = space.id.clone();
                let label = space
                    .name
                    .clone()
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| {
                        std::path::Path::new(&space.path)
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_else(|| space.path.clone())
                    });
                let active = current.as_deref() == Some(space.id.as_str());
                menu_choice(theme, &label, active, {
                    let page = cx.entity();
                    move |_, _, cx| {
                        page.update(cx, |page: &mut AutomationsPage, cx| {
                            if let Some(editor) = page.editor.as_mut() {
                                editor.space_id = id.clone();
                            }
                            page.project_menu_state.begin_close();
                            cx.notify();
                        })
                    }
                })
            })
            .collect();
        Some(popover::anchored_menu_below(
            "automation-project-menu",
            menu_card(theme, &self.menu_scroll, rows),
            self.project_menu_state.closing_since(),
        ))
    }

    fn cadence_menu(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.trigger_menu_state.is_open() {
            return None;
        }
        let current = self.editor.as_ref().map(|editor| editor.cadence);
        let rows: Vec<AnyElement> = [
            AutomationCadence::Hourly,
            AutomationCadence::Daily,
            AutomationCadence::Weekdays,
            AutomationCadence::Weekly,
        ]
        .iter()
        .map(|cadence| {
            let cadence = *cadence;
            menu_choice(theme, cadence_label(cadence), current == Some(cadence), {
                let page = cx.entity();
                move |_, _, cx| {
                    page.update(cx, |page: &mut AutomationsPage, cx| {
                        if let Some(editor) = page.editor.as_mut() {
                            editor.cadence = cadence;
                        }
                        page.trigger_menu_state.begin_close();
                        cx.notify();
                    })
                }
            })
        })
        .collect();
        Some(popover::anchored_menu_below(
            "automation-cadence-menu",
            menu_card(theme, &self.menu_scroll, rows),
            self.trigger_menu_state.closing_since(),
        ))
    }

    fn weekday_menu(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.weekday_menu_state.is_open() {
            return None;
        }
        let current = self.editor.as_ref().map(|editor| editor.weekday);
        let rows: Vec<AnyElement> = WEEKDAYS
            .iter()
            .map(|(name, day)| {
                let day = *day;
                menu_choice(theme, name, current == Some(day), {
                    let page = cx.entity();
                    move |_, _, cx| {
                        page.update(cx, |page: &mut AutomationsPage, cx| {
                            if let Some(editor) = page.editor.as_mut() {
                                editor.weekday = day;
                            }
                            page.weekday_menu_state.begin_close();
                            cx.notify();
                        })
                    }
                })
            })
            .collect();
        Some(popover::anchored_menu_below(
            "automation-weekday-menu",
            menu_card(theme, &self.menu_scroll, rows),
            self.weekday_menu_state.closing_since(),
        ))
    }

    fn harness_menu(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.harness_menu_state.is_open() {
            return None;
        }
        let current = self.editor.as_ref().and_then(|editor| editor.harness);
        let harnesses = self.harnesses.clone();
        let rows: Vec<AnyElement> = harnesses
            .iter()
            .map(|descriptor| {
                let id = descriptor.id;
                menu_choice(theme, &descriptor.name, current == Some(id), {
                    let page = cx.entity();
                    move |_, _, cx| {
                        page.update(cx, |page: &mut AutomationsPage, cx| {
                            if let Some(editor) = page.editor.as_mut() {
                                editor.harness = Some(id);
                                // The old model belongs to the old provider;
                                // carrying it over would offer a model this
                                // provider does not have.
                                editor.model = None;
                            }
                            page.harness_menu_state.begin_close();
                            cx.notify();
                            // Re-read the catalog so the model menu lists the
                            // provider that was just picked.
                            page.load_catalog(Some(id), cx);
                        })
                    }
                })
            })
            .collect();
        Some(popover::anchored_menu_below(
            "automation-harness-menu",
            menu_card(theme, &self.menu_scroll, rows),
            self.harness_menu_state.closing_since(),
        ))
    }

    fn model_menu(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.model_menu_state.is_open() {
            return None;
        }
        let current = self.editor.as_ref().and_then(|editor| editor.model.clone());
        let models = self.models.clone();
        let rows: Vec<AnyElement> = models
            .iter()
            .map(|model| {
                let id = model.id.clone();
                menu_choice(
                    theme,
                    &model.label,
                    current.as_deref() == Some(id.as_str()),
                    {
                        let page = cx.entity();
                        move |_, _, cx| {
                            page.update(cx, |page: &mut AutomationsPage, cx| {
                                if let Some(editor) = page.editor.as_mut() {
                                    editor.model = Some(id.clone());
                                }
                                page.model_menu_state.begin_close();
                                cx.notify();
                            })
                        }
                    },
                )
            })
            .collect();
        Some(popover::anchored_menu_below(
            "automation-model-menu",
            menu_card(theme, &self.menu_scroll, rows),
            self.model_menu_state.closing_since(),
        ))
    }

    fn sandbox_menu(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.sandbox_menu_state.is_open() {
            return None;
        }
        let current = self.editor.as_ref().map(|editor| editor.sandbox);
        let rows: Vec<AnyElement> = [
            (SandboxLevel::ReadOnly, "Read only"),
            (SandboxLevel::WorkspaceWrite, "Can edit"),
            (SandboxLevel::DangerFullAccess, "Full access"),
        ]
        .iter()
        .map(|(level, label)| {
            let level = *level;
            menu_choice(theme, label, current == Some(level), {
                let page = cx.entity();
                move |_, _, cx| {
                    page.update(cx, |page: &mut AutomationsPage, cx| {
                        if let Some(editor) = page.editor.as_mut() {
                            editor.sandbox = level;
                        }
                        page.sandbox_menu_state.begin_close();
                        cx.notify();
                    })
                }
            })
        })
        .collect();
        Some(popover::anchored_menu_below(
            "automation-sandbox-menu",
            menu_card(theme, &self.menu_scroll, rows),
            self.sandbox_menu_state.closing_since(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Small pieces
// ---------------------------------------------------------------------------

/// A labelled form row. The label sits above the control, not beside it: the
/// controls here are menus and editors, and a 520px card has no room for two
/// columns without the prompts getting squeezed. Generic over the control so
/// a menu button and a bordered editor box can both drop in.
fn field(theme: &Theme, label: &str, control: impl gpui::IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_size(crate::typography::ui_rems(11.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text_faint)
                .child(SharedString::from(label.to_string())),
        )
        .child(control)
        .into_any_element()
}

fn hint(theme: &Theme, text: &str) -> AnyElement {
    div()
        .text_size(crate::typography::ui_rems(11.5))
        .text_color(theme.text_faint)
        .child(SharedString::from(text.to_string()))
        .into_any_element()
}

/// The meta line under a name: `project · 8h ago · model`, with empty parts
/// dropped rather than leaving stray dots.
fn meta_line(theme: &Theme, parts: &[Option<String>]) -> AnyElement {
    let present: Vec<String> = parts
        .iter()
        .flatten()
        .filter(|p| !p.is_empty())
        .cloned()
        .collect();
    div()
        .mt(px(4.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .text_size(crate::typography::ui_rems(11.5))
        .text_color(theme.text_muted.opacity(0.6))
        .children(
            present
                .into_iter()
                .enumerate()
                .map(|(ix, part)| {
                    let dot = div()
                        .size(px(3.0))
                        .rounded_full()
                        .bg(theme.text_faint.opacity(0.6))
                        .into_any_element();
                    let text = div().child(SharedString::from(part)).into_any_element();
                    if ix == 0 {
                        text
                    } else {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(dot)
                            .child(text)
                            .into_any_element()
                    }
                })
                .collect::<Vec<AnyElement>>(),
        )
        .into_any_element()
}

/// A click handler bound to this page. The modal's controls are small helper
/// elements (`menu_button`, `stepper`, …) that take a plain `&mut App`
/// listener, so the page entity is captured here instead of at every call.
fn on<F>(
    page: Entity<AutomationsPage>,
    apply: F,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App)
where
    F: Fn(&mut AutomationsPage, &mut Context<AutomationsPage>) + 'static,
{
    move |_, _, cx| {
        page.update(cx, |page, cx| apply(page, cx));
    }
}

/// A dropdown trigger plus, when open, the menu anchored to it. The menu is a
/// child of the trigger's box on purpose: `anchored_menu_below` positions
/// itself against its parent, so a menu mounted anywhere else opens wherever
/// that parent happens to sit.
fn menu_button(
    theme: &Theme,
    id: &'static str,
    label: &str,
    open: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    menu: Option<AnyElement>,
) -> AnyElement {
    let trigger = div()
        .id(id)
        .h(px(32.0))
        .max_w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(if open { theme.accent } else { theme.border })
        .bg(if open {
            theme.accent_wash
        } else {
            theme.surface_raised
        })
        .cursor_pointer()
        .hover(|s| s.bg(theme.element_hover))
        .text_size(crate::typography::ui_rems(12.5))
        .text_color(theme.text)
        .on_click(on_click)
        .child(
            div()
                .min_w(px(0.0))
                .flex_1()
                .overflow_hidden()
                .text_ellipsis()
                .child(SharedString::from(label.to_string())),
        )
        .child(
            crate::icons::icon(crate::icons::ALT_ARROW_DOWN)
                .size(px(12.0))
                .flex_none()
                .text_color(theme.text_muted),
        );
    let mut host = div().relative().child(trigger);
    if let Some(menu) = menu {
        host = host.child(menu);
    }
    host.into_any_element()
}

fn segment(
    theme: &Theme,
    id: &'static str,
    label: &str,
    active: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(28.0))
        .px(px(12.0))
        .rounded(px(6.0))
        .bg(if active {
            theme.accent_fill()
        } else {
            theme.surface_raised
        })
        .cursor_pointer()
        .hover(|s| s.bg(theme.element_hover))
        .flex()
        .items_center()
        .text_size(crate::typography::ui_rems(12.0))
        .font_weight(if active {
            gpui::FontWeight::MEDIUM
        } else {
            gpui::FontWeight::NORMAL
        })
        .text_color(if active {
            theme.on_accent
        } else {
            theme.text_muted
        })
        .on_click(on_click)
        .child(SharedString::from(label.to_string()))
}

/// A `− value +` stepper: two square buttons around a fixed-width readout, so
/// the digits sit on the same centre line hour to hour and the colon is a
/// separator instead of a button face. `id` prefixes both buttons - the hour
/// and the minute stepper are on screen at once.
fn time_stepper(
    theme: &Theme,
    id: &'static str,
    value: &str,
    on_down: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    on_up: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .child(stepper_button(theme, down_id(id), "−", on_down))
        .child(
            div()
                .w(px(30.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .font_family(theme.font_mono.clone())
                .text_size(crate::typography::ui_rems(12.5))
                .text_color(theme.text)
                .child(SharedString::from(value.to_string())),
        )
        .child(stepper_button(theme, up_id(id), "+", on_up))
        .into_any_element()
}

/// Ids are built at runtime (the prefix is a `&'static str`, the suffix is not),
/// so the two faces of a stepper stay distinct even though their glyphs repeat.
fn down_id(prefix: &str) -> SharedString {
    SharedString::from(format!("{prefix}-down"))
}

fn up_id(prefix: &str) -> SharedString {
    SharedString::from(format!("{prefix}-up"))
}

/// The square `−`/`+` face of a [`time_stepper`].
fn stepper_button(
    theme: &Theme,
    id: SharedString,
    glyph: &str,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface_raised)
        .cursor_pointer()
        .hover(|s| s.bg(theme.element_hover))
        .text_size(crate::typography::ui_rems(13.0))
        .text_color(theme.text_muted)
        .on_click(on_click)
        .child(SharedString::from(glyph.to_string()))
}

fn check_row(
    theme: &Theme,
    label: &str,
    checked: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(SharedString::from(format!("automation-check-{label}")))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .on_click(on_click)
        .child(
            div()
                .size(px(14.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(if checked { theme.accent } else { theme.border })
                .bg(if checked {
                    theme.accent_fill()
                } else {
                    theme.surface_raised
                })
                .flex()
                .items_center()
                .justify_center()
                .child(
                    crate::icons::icon(crate::icons::CHECK)
                        .size(px(10.0))
                        .text_color(theme.on_accent),
                ),
        )
        .child(
            div()
                .text_size(crate::typography::ui_rems(12.5))
                .text_color(theme.text)
                .child(SharedString::from(label.to_string())),
        )
        .into_any_element()
}

/// The menu surface. The frosted layer only paints the blur, so the card's own
/// background, border and shadow have to come from here - the same
/// `popover_card` every other menu in the app uses. One shared scroll handle:
/// only one menu is open at a time, and a long provider list must scroll
/// instead of growing past the viewport.
fn menu_card(theme: &Theme, scroll: &gpui::ScrollHandle, rows: Vec<AnyElement>) -> AnyElement {
    popover::popover_card(&theme.for_popup())
        .w(px(240.0))
        .max_h(px(320.0))
        .id("automation-menu-scroll")
        .overflow_y_scroll()
        .track_scroll(scroll)
        .children(rows)
        .into_any_element()
}

/// One option. The selected row gets a check in a fixed-width leading slot, so
/// the labels stay on one left edge whether or not anything is selected.
fn menu_choice(
    theme: &Theme,
    label: &str,
    active: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let theme = &theme.for_popup();
    let slot: AnyElement = if active {
        crate::icons::icon(crate::icons::CHECK)
            .size(px(12.0))
            .text_color(theme.accent)
            .into_any_element()
    } else {
        div().size(px(12.0)).into_any_element()
    };
    div()
        .id(SharedString::from(format!("automation-menu-{label}")))
        .h(px(28.0))
        .px(px(8.0))
        .rounded(px(6.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .cursor_pointer()
        .hover(|s| s.bg(theme.element_hover))
        .text_size(crate::typography::ui_rems(12.5))
        .text_color(theme.text)
        .on_click(on_click)
        .child(slot)
        .child(
            div()
                .min_w(px(0.0))
                .flex_1()
                .overflow_hidden()
                .text_ellipsis()
                .child(SharedString::from(label.to_string())),
        )
        .into_any_element()
}

pub(crate) fn cadence_label(cadence: AutomationCadence) -> &'static str {
    match cadence {
        AutomationCadence::Hourly => "Every hour",
        AutomationCadence::Daily => "Every day",
        AutomationCadence::Weekdays => "Weekdays",
        AutomationCadence::Weekly => "Weekly",
    }
}

pub(crate) fn sandbox_label(level: SandboxLevel) -> &'static str {
    match level {
        SandboxLevel::ReadOnly => "Read only",
        SandboxLevel::WorkspaceWrite => "Can edit",
        SandboxLevel::DangerFullAccess => "Full access",
    }
}

fn harness_label(id: HarnessId, harnesses: &[HarnessDescriptor]) -> String {
    harnesses
        .iter()
        .find(|descriptor| descriptor.id == id)
        .map(|descriptor| descriptor.name.clone())
        .unwrap_or_else(|| format!("{id:?}"))
}

/// "Weekdays at 9:00" — the cadence plus the clock, the way the reference
/// names a schedule.
pub(crate) fn schedule_label(automation: &Automation) -> String {
    let Some(schedule) = automation.trigger.schedule() else {
        return "On demand".into();
    };
    let clock = format!("{:02}:{:02}", schedule.hour, schedule.minute);
    match schedule.every {
        AutomationCadence::Hourly => "Every hour".into(),
        AutomationCadence::Daily => format!("Every day at {clock}"),
        AutomationCadence::Weekdays => format!("Weekdays at {clock}"),
        AutomationCadence::Weekly => {
            let day = WEEKDAYS
                .iter()
                .find(|(_, day)| *day == schedule.weekday.max(1))
                .map(|(name, _)| *name)
                .unwrap_or("Monday");
            format!("{day} at {clock}")
        }
    }
}

fn pull_request_label(automation: &Automation) -> String {
    let AutomationTrigger::PullRequest(trigger) = &automation.trigger else {
        return "On demand".into();
    };
    let mut events: Vec<&str> = Vec::new();
    if trigger.events.contains(&PullRequestEvent::Opened) {
        events.push("Pull request opened");
    }
    if trigger.events.contains(&PullRequestEvent::Updated) {
        events.push("updated");
    }
    if events.is_empty() {
        "Pull request".into()
    } else {
        events.join(" · ")
    }
}

/// "just now" / "12m ago" / "8h ago" / "3d ago" — the reference's granularity
/// for a last-run stamp, and enough precision to answer "did it run?".
pub(crate) fn time_ago(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let minutes = now.signed_duration_since(at).num_minutes().max(0);
    match minutes {
        0..=1 => "just now".into(),
        2..=59 => format!("{minutes}m ago"),
        60..=1439 => format!("{}h ago", minutes / 60),
        _ => format!("{}d ago", minutes / (60 * 24)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        chrono::Utc.with_ymd_and_hms(2026, 9, 26, h, m, 0).unwrap()
    }

    fn scheduled(every: AutomationCadence, hour: u8, minute: u8, weekday: u8) -> Automation {
        Automation {
            id: "a".into(),
            name: "Nightly".into(),
            space_id: "s".into(),
            project_label: "Repo".into(),
            project_path: "/repo".into(),
            trigger: AutomationTrigger::Schedule(AutomationSchedule {
                every,
                hour,
                minute,
                weekday,
            }),
            prompt: "Review".into(),
            harness: None,
            model: None,
            sandbox: SandboxLevel::ReadOnly,
            enabled: true,
            created_at: at(0, 0),
            updated_at: at(0, 0),
            last_run_at: None,
            conversation_chat_id: None,
            activity_chat_ids: Vec::new(),
            seen_pull_requests: Vec::new(),
            next_run_at: None,
        }
    }

    #[test]
    fn schedule_chips_name_the_cadence_and_the_clock() {
        assert_eq!(
            schedule_label(&scheduled(AutomationCadence::Weekdays, 9, 0, 0)),
            "Weekdays at 09:00"
        );
        assert_eq!(
            schedule_label(&scheduled(AutomationCadence::Daily, 17, 30, 0)),
            "Every day at 17:30"
        );
        assert_eq!(
            schedule_label(&scheduled(AutomationCadence::Hourly, 9, 0, 0)),
            "Every hour"
        );
        assert_eq!(
            schedule_label(&scheduled(AutomationCadence::Weekly, 10, 0, 3)),
            "Wednesday at 10:00"
        );
    }

    #[test]
    fn pull_request_chips_list_only_the_selected_events() {
        let mut automation = scheduled(AutomationCadence::Daily, 9, 0, 0);
        automation.trigger = AutomationTrigger::PullRequest(PullRequestTrigger {
            repo: String::new(),
            events: vec![PullRequestEvent::Opened],
        });
        assert_eq!(pull_request_label(&automation), "Pull request opened");
        automation.trigger = AutomationTrigger::PullRequest(PullRequestTrigger {
            repo: String::new(),
            events: vec![PullRequestEvent::Opened, PullRequestEvent::Updated],
        });
        assert_eq!(
            pull_request_label(&automation),
            "Pull request opened · updated"
        );
    }

    #[test]
    fn last_run_ages_read_at_a_glance() {
        let now = at(12, 0);
        assert_eq!(time_ago(at(12, 0), now), "just now");
        assert_eq!(time_ago(at(11, 48), now), "12m ago");
        assert_eq!(time_ago(at(4, 0), now), "8h ago");
        assert_eq!(time_ago(at(11, 0), now), "1h ago");
        // Days take over past 24h, not at the day boundary.
        assert_eq!(time_ago(now - chrono::Duration::days(2), now), "2d ago");
        // A clock that ran backwards must not print a negative age.
        assert_eq!(time_ago(at(12, 30), now), "just now");
    }
}
