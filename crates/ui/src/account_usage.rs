//! Provider account quota as its own section at the bottom of the sidebar
//! column: the account the composer is about to run, its tightest rate-limit
//! window as a meter plus percent, the countdown to that window's reset, and a
//! refresh that forces the engine's usage probe.
//!
//! The numbers are the CLI plan windows the providers themselves report
//! (`engine/src/agent_accounts.rs` — Claude's five-hour/seven-day buckets,
//! Codex's primary/secondary windows, Cursor's current-period spend), NOT
//! per-turn token accounting. The engine caches probes for 60s, so the section
//! forces one on its first paint and on the refresh button and rides the cache
//! in between: the minute tick re-reads the snapshot (keeping the countdown
//! honest) and only re-probes when the last one failed. Nothing starts until
//! the section is actually painted, so a route that never mounts the sidebar
//! spends no probe and no timer.
use std::time::Duration;

use chrono::{DateTime, Utc};
use gpui::{
    AnimationExt, AnyElement, App, Context, Entity, Render, SharedString, Task, Window, div,
    prelude::*, px,
};

use clyra_proto::{AgentAccount, AgentAccountsSnapshot, AgentUsageWindow, HarnessId};
use clyra_rpc::methods;

use crate::pickers::Pickers;
use crate::popover::Loadable;
use crate::settings::accounts::{UsageLevel, usage_color, usage_level};
use crate::state::AppState;
use crate::theme::Theme;

/// Background tick. Matches the engine's usage cache TTL, so the countdown
/// text stays honest to the minute without a repaint driver of its own.
const POLL: Duration = Duration::from_secs(60);
const REFRESH_KEY: &str = "account-usage-refresh";
/// The meter flexes inside the sidebar column: its ceiling, and the floor it
/// keeps when the column is dragged narrow (the account label gives up space
/// first).
const BAR_W: f32 = 44.0;
const BAR_MIN_W: f32 = 24.0;
const BAR_H: f32 = 5.0;
/// The metered row and its section gutters. The entrance animation drives the
/// section's height between 0 and this, so the number is the layout truth
/// rather than a magic constant in two places.
const ROW_H: f32 = 20.0;
const SECTION_PAD_V: f32 = 7.0;
const SECTION_H: f32 = ROW_H + 2.0 * SECTION_PAD_V;
/// Tiny non-zero usage still paints — a 0.4% window must not read as an
/// empty track (the settings page's rule).
const BAR_FLOOR: f32 = 0.015;

// ---------------------------------------------------------------------------
// Pure: which account, which window, how to count down
// ---------------------------------------------------------------------------

/// The account whose quota the strip reports: the live login for the provider
/// the composer will run (the chat's harness, else the remembered default),
/// then that provider's first account when nothing is marked active (the CLI
/// login isn't detected yet), then any active account. The chat's own provider
/// always wins — another provider's quota would answer the wrong question.
pub fn active_account<'a>(
    snapshot: &'a AgentAccountsSnapshot,
    harness: Option<HarnessId>,
) -> Option<&'a AgentAccount> {
    if let Some(harness) = harness {
        let of_provider = |active: bool| {
            snapshot
                .accounts
                .iter()
                .find(|a| a.harness == harness && a.active == active)
        };
        if let Some(account) = of_provider(true).or_else(|| of_provider(false)) {
            return Some(account);
        }
    }
    snapshot.accounts.iter().find(|a| a.active)
}

/// The window worth the single meter: the most spent one. Providers report
/// several (a session bucket and a week bucket); the tightest is the one that
/// runs out first, and ties keep the provider's own order so the strip never
/// flips between two equally-spent windows.
pub fn tightest_window(account: &AgentAccount) -> Option<&AgentUsageWindow> {
    let mut best: Option<&AgentUsageWindow> = None;
    for window in &account.usage_windows {
        if best.map_or(true, |b| window.used_fraction > b.used_fraction) {
            best = Some(window);
        }
    }
    best
}

/// Who the meter belongs to, in the settings page's order (email → display
/// name → plan badge), so the two surfaces never disagree about which login
/// is which.
pub fn account_label(account: &AgentAccount) -> String {
    account
        .email
        .clone()
        .filter(|email| !email.trim().is_empty())
        .or_else(|| {
            account
                .display_name
                .clone()
                .filter(|name| !name.trim().is_empty())
        })
        .or_else(|| account.plan_label.clone())
        .unwrap_or_else(|| "Account".into())
}

/// Provider display name. Only the three providers with a usage probe ever
/// produce an account (the engine iterates exactly those), so the settings
/// page's provider table is the whole map.
pub fn provider_label(harness: HarnessId) -> &'static str {
    crate::settings::accounts::PROVIDERS
        .iter()
        .find(|(id, _, _)| *id == harness)
        .map(|(_, name, _)| *name)
        .unwrap_or("Provider")
}

/// Time left on a window, compactly: "45m", "6h 20m", "6d 12h". `None` when
/// the provider reports no reset, and "now" for an already-spent window —
/// which is a real state (providers reset lazily) and must never render as a
/// negative duration.
pub fn format_countdown(resets_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Option<String> {
    let at = resets_at?;
    let minutes = now.signed_duration_since(at).num_minutes();
    if minutes >= 0 {
        return Some("now".into());
    }
    let left = (-minutes) as u64;
    if left < 60 {
        return Some(format!("{left}m"));
    }
    if left < 60 * 24 {
        return Some(format!("{}h {:02}m", left / 60, left % 60));
    }
    Some(format!(
        "{}d {:02}h",
        left / (60 * 24),
        (left % (60 * 24)) / 60
    ))
}

/// Whole percent of a window, rounded and clamped.
pub fn percent(window: &AgentUsageWindow) -> u32 {
    (window.used_fraction.clamp(0.0, 1.0) * 100.0).round() as u32
}

/// What the section shows while it is on screen — and, held past the data's
/// death, while it plays its exit. A snapshot read at paint time cannot
/// survive the frame the quota disappears (the engine went away, the reported
/// provider stopped advertising a window), and gpui unmounts an element the
/// frame its state drops — so the reading is kept here for the length of the
/// exit, exactly like `popover::Popup`'s closing phase.
#[derive(Clone)]
struct Quota {
    account: AgentAccount,
    window: AgentUsageWindow,
}

impl Quota {
    /// Identity for the steady state: same account, same metered window, same
    /// numbers. A refresh that reports the same quota must not count as a new
    /// reading (and must not restart the entrance).
    fn same(&self, other: &Quota) -> bool {
        self.account.id == other.account.id
            && self.account.harness == other.account.harness
            && self.window.label == other.window.label
            && self.window.used_fraction == other.window.used_fraction
            && self.window.resets_at == other.window.resets_at
    }
}

/// How long the exit runs before the section's state is dropped. Derived from
/// the same spec the animation uses, so the hold can never outlast it.
fn exit_span() -> std::time::Duration {
    crate::motion::COLLAPSE
        .total()
        .mul_f32(crate::motion::speed_scale())
}

// ---------------------------------------------------------------------------
// Entity
// ---------------------------------------------------------------------------

/// The section item. Owns its snapshot, the forced load, and the minute tick.
pub struct AccountUsage {
    state: Entity<AppState>,
    /// The composer pickers own the effective harness (chat config, else the
    /// remembered default) — the same answer the model chip shows.
    pickers: Entity<Pickers>,
    snapshot: Loadable<AgentAccountsSnapshot>,
    /// The `(provider, device)` the current snapshot describes. A change
    /// re-probes: another provider's quota is a different question.
    probed: Option<(Option<HarnessId>, Option<String>)>,
    error: Option<SharedString>,
    /// A load is in flight (the refresh button dims). A plain flag, not
    /// `load_task.is_some()`: a finished task is never cleared from its slot.
    refreshing: bool,
    /// The reading currently mounted, and when the exit started (`None` while
    /// it is simply present). `gpui` keys animation state by element id, so
    /// `mount` — bumped on every fresh arrival — is what makes a section that
    /// left and came back replay its entrance instead of reappearing frozen.
    quota: Option<Quota>,
    closing_since: Option<std::time::Instant>,
    mount: usize,
    /// Nothing starts until the section is actually painted: an unpainted
    /// shell (a route that never mounts the sidebar) must not spend a
    /// provider probe or a timer.
    started: bool,
    load_task: Option<Task<()>>,
    poll_task: Option<Task<()>>,
    _observation: gpui::Subscription,
}

impl AccountUsage {
    pub fn new(state: Entity<AppState>, pickers: Entity<Pickers>, cx: &mut Context<Self>) -> Self {
        let observation = cx.observe(&state, |this: &mut Self, _, cx| this.sync(cx));
        Self {
            state,
            pickers,
            snapshot: Loadable::Idle,
            probed: None,
            error: None,
            refreshing: false,
            quota: None,
            closing_since: None,
            mount: 0,
            started: false,
            load_task: None,
            poll_task: None,
            _observation: observation,
        }
    }

    /// First paint: probe and start the tick. The frame itself paints nothing
    /// — the snapshot arrives a moment later.
    fn start(&mut self, cx: &mut Context<Self>) {
        if self.started {
            return;
        }
        self.started = true;
        self.sync(cx);
        self.poll_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let alive = this
                    .update(cx, |this, cx| {
                        if this.error.is_some() {
                            this.load(true, cx);
                        } else {
                            cx.notify();
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        }));
    }

    /// The provider/device pair the strip reports on right now. The
    /// `targetDeviceId` passthrough only matters for a REMOTE device: the
    /// connected engine answers for its own logins.
    fn target(&self, cx: &App) -> (Option<HarnessId>, Option<String>) {
        let harness = self.pickers.read(cx).effective_harness(cx);
        let state = self.state.read(cx);
        let local = state.local_device_id.clone();
        let target = match (state.effective_device_id(), local) {
            (Some(device), Some(local)) if device != local => Some(device),
            _ => None,
        };
        (harness, target)
    }

    /// Reload when the reported provider or device changed. Runs on every
    /// state notification; the comparison is the whole cost.
    fn sync(&mut self, cx: &mut Context<Self>) {
        if !self.started {
            return;
        }
        let target = self.target(cx);
        if self.probed == Some(target.clone()) {
            return;
        }
        self.probed = Some(target);
        self.load(true, cx);
    }

    fn load(&mut self, force_usage: bool, cx: &mut Context<Self>) {
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            // The engine connects after the shell does. Forget the key so the
            // next state notification (or the minute tick) tries again.
            self.error = Some("Usage offline".into());
            self.refreshing = false;
            self.probed = None;
            return;
        };
        let (_, target) = self.target(cx);
        let mut params = serde_json::Map::new();
        params.insert("forceUsage".into(), serde_json::json!(force_usage));
        if let Some(target) = target {
            params.insert("targetDeviceId".into(), serde_json::json!(target));
        }
        self.error = None;
        self.refreshing = true;
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let result = engine
                .client()
                .call(
                    methods::LIST_AGENT_ACCOUNTS,
                    serde_json::Value::Object(params),
                )
                .await;
            this.update(cx, |this, cx| {
                this.refreshing = false;
                // A failed probe never erases a good reading: a blip leaves the
                // last known numbers on screen with a dimmed refresh, instead
                // of collapsing the section and re-growing it a second later.
                match result {
                    Ok(value) => match serde_json::from_value::<AgentAccountsSnapshot>(value) {
                        Ok(snapshot) => {
                            this.error = None;
                            this.snapshot = Loadable::Ready(snapshot);
                        }
                        Err(err) => this.error = Some(err.to_string().into()),
                    },
                    Err(err) => this.error = Some(err.to_string().into()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// The reading the live data resolves to, if any: a snapshot, the reported
    /// provider's account, and a window on it. No engine, no login, or a
    /// provider with no quota view (a Cursor key with no dashboard session)
    /// resolves to nothing.
    fn resolve(&self, cx: &App) -> Option<Quota> {
        let (harness, _) = self.target(cx);
        let account = active_account(self.snapshot.ready()?, harness)?;
        let window = tightest_window(account)?;
        Some(Quota {
            account: account.clone(),
            window: window.clone(),
        })
    }

    /// The mounted reading: the live one, or the last one held for the exit.
    fn advance(&mut self, cx: &App) -> Option<Quota> {
        match self.resolve(cx) {
            Some(quota) => {
                // A fresh arrival gets a fresh animation id; a plain update
                // (new numbers, another provider's account) keeps the row.
                if self.quota.is_none() {
                    self.mount += 1;
                }
                self.closing_since = None;
                // The steady state re-resolves every frame; only a reading that
                // actually moved replaces the mounted clone.
                if self.quota.as_ref().is_none_or(|live| !live.same(&quota)) {
                    self.quota = Some(quota);
                }
            }
            // The data died with the section still mounted: start the exit and
            // keep painting the last reading until it finishes.
            None if self.quota.is_some() && self.closing_since.is_none() => {
                if crate::motion::reduced_motion(cx) {
                    // Nothing animates, so nothing needs holding.
                    self.quota = None;
                } else {
                    self.closing_since = Some(std::time::Instant::now());
                }
            }
            None => {}
        }
        if self
            .closing_since
            .is_some_and(|since| since.elapsed() >= exit_span())
        {
            self.quota = None;
            self.closing_since = None;
        }
        self.quota.clone()
    }

    /// The section for a mounted reading. `closing` plays the exit: the box
    /// collapses and fades, and the refresh goes dead so a dying section takes
    /// no clicks.
    fn content(
        &self,
        quota: &Quota,
        theme: &Theme,
        now: DateTime<Utc>,
        mount: usize,
        closing: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let account = &quota.account;
        let window = &quota.window;
        let (mark, tint) = crate::pickers::harness_brand_icon(account.harness);
        let level = usage_level(window.used_fraction);
        let fill = usage_color(level, theme);
        let label = SharedString::from(account_label(account));
        let percent_label = SharedString::from(format!("{}%", percent(window)));
        // A window with no reset still has a percent worth showing; only the
        // countdown slot goes empty.
        let countdown = format_countdown(window.resets_at, now).map(SharedString::from);
        // Every window, not just the metered one — the tightest is a policy
        // of this row, and the week bucket is what people actually plan
        // around.
        let mut lines: Vec<String> = account
            .usage_windows
            .iter()
            .map(|w| {
                let reset = format_countdown(w.resets_at, now)
                    .map(|left| format!(" · resets in {left}"))
                    .unwrap_or_default();
                format!("{} · {}% used{reset}", w.label, percent(w))
            })
            .collect();
        if let Some(plan) = account.plan_label.clone() {
            lines.push(plan);
        }
        let refresh = div()
            .id("account-usage-refresh")
            .flex_none()
            .size(px(16.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .opacity(if self.refreshing { 0.4 } else { 1.0 })
            .bg(crate::motion::hover_blend(
                REFRESH_KEY,
                gpui::transparent_black(),
                theme.element_hover,
            ))
            .on_hover(crate::motion::hover_listener(REFRESH_KEY))
            // A dying section takes no clicks.
            .when(!closing, |el| {
                el.cursor_pointer()
                    .on_click(cx.listener(|this: &mut Self, _, _, cx| this.load(true, cx)))
            })
            .child(
                crate::icons::icon(crate::icons::REFRESH)
                    .size(px(11.0))
                    .text_color(theme.text_faint),
            );
        let meter = div()
            .flex_1()
            .min_w(px(BAR_MIN_W))
            .max_w(px(BAR_W))
            .h(px(BAR_H))
            .rounded_full()
            .overflow_hidden()
            .bg(crate::theme::ink(0.07))
            .child(
                div()
                    .h_full()
                    .w(gpui::relative(
                        window.used_fraction.clamp(0.0, 1.0).max(BAR_FLOOR),
                    ))
                    .rounded_full()
                    .bg(fill),
            );
        let title = format!("{} · {label}", provider_label(account.harness));
        let item = div()
            .id("account-usage")
            .w_full()
            .flex()
            .items_center()
            .gap(px(6.0))
            .h(px(ROW_H))
            .text_size(crate::typography::ui_rems(11.0))
            .child(
                crate::icons::icon(mark)
                    .size(px(12.0))
                    .text_color(tint.unwrap_or(theme.text_muted)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(theme.text_muted)
                    .child(label.clone()),
            )
            .child(meter)
            .child(
                div()
                    .flex_none()
                    .min_w(px(28.0))
                    .text_right()
                    .font_family(theme.font_mono.clone())
                    // The percent is the number that decides whether to keep
                    // working, so it takes the meter's tone once it matters.
                    .text_color(match level {
                        UsageLevel::Normal => theme.text,
                        _ => fill,
                    })
                    .child(percent_label),
            )
            .when_some(countdown, |el, countdown| {
                el.child(
                    div()
                        .flex_none()
                        .text_color(theme.text_faint)
                        .child(countdown),
                )
            })
            .child(refresh);
        let body = div()
            .id("account-usage-section")
            .w_full()
            // The section frame: a hairline off the lists above and the
            // sidebar's own gutter, so the row reads as its own section
            // rather than another line of the list above it.
            .border_t_1()
            .border_color(theme.border)
            .px(px(Theme::SPACE_SM + 2.0))
            .pt(px(SECTION_PAD_V))
            .pb(px(SECTION_PAD_V))
            .child(item)
            .tooltip(move |_, cx| {
                cx.new(|_| UsageTooltip {
                    title: title.clone(),
                    lines: lines.clone(),
                })
                .into()
            })
            .tooltip_show_delay(Duration::from_millis(350));
        // The quota lands seconds after the window does (a provider probe), so
        // mounting it bare read as a jolt — and so did its death. The section
        // animates its HEIGHT as well as its opacity, which is what glides the
        // lists above it instead of letting them jump: the sidebar's last row
        // arriving or leaving is a layout change, so it animates like one.
        // `with_animation` snaps to the end state under reduced motion (the
        // exit then drops on the next frame, the entrance is instant).
        let box_ = div().w_full().flex_none().overflow_hidden();
        if closing {
            box_.with_animation(
                ("account-usage-out", mount),
                crate::motion::COLLAPSE.animation(),
                |el, t| {
                    let left = 1.0 - t;
                    el.opacity(left).h(px(SECTION_H * left))
                },
            )
            .child(body)
            .into_any_element()
        } else {
            box_.with_animation(
                ("account-usage-in", mount),
                crate::motion::RESIZE.animation(),
                |el, t| el.opacity(t).h(px(SECTION_H * t)),
            )
            .child(body)
            .into_any_element()
        }
    }
}

impl Render for AccountUsage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The first paint is what earns the probe and the timer.
        self.start(cx);
        let theme = Theme::of(cx).clone();
        if let Some(quota) = self.advance(cx) {
            let closing = self.closing_since.is_some();
            return self.content(&quota, &theme, Utc::now(), self.mount, closing, cx);
        }
        // Nothing mounted: the sidebar's column keeps its height (no
        // placeholder, and no orphan hairline). A failed probe with nothing to
        // fall back on is worth one quiet word.
        match self.error.clone() {
            Some(error) => div()
                .flex_none()
                .text_size(crate::typography::ui_rems(11.0))
                .text_color(theme.text_faint)
                .child(error)
                .into_any_element(),
            None => div().into_any_element(),
        }
    }
}

/// Hover card: every window the provider reported, plus the plan.
struct UsageTooltip {
    title: String,
    lines: Vec<String>,
}

impl Render for UsageTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &Theme::of(cx).for_popup();
        let card = crate::popover::popover_card(theme)
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(SharedString::from(self.title.clone())),
            )
            .children(self.lines.iter().map(|line| {
                // Break only at the tooltip's own newlines: it sizes from the
                // unwrapped text, so soft wrapping clipped the last line.
                div()
                    .text_size(px(12.0))
                    .line_height(px(18.0))
                    .whitespace_nowrap()
                    .text_color(theme.text_muted)
                    .child(SharedString::from(line.clone()))
            }));
        crate::frost::frosted(crate::popover::CARD_RADIUS, crate::frost::MENU_BLUR, card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn window(label: &str, used: f32) -> AgentUsageWindow {
        AgentUsageWindow {
            label: label.into(),
            used_fraction: used,
            resets_at: None,
        }
    }

    fn account(harness: HarnessId, active: bool, windows: Vec<AgentUsageWindow>) -> AgentAccount {
        AgentAccount {
            id: format!("{harness:?}-{active}"),
            harness,
            email: Some("dev@example.com".into()),
            plan_label: Some("Max 20×".into()),
            active,
            usage_windows: windows,
            display_name: None,
            organization: None,
            auth_kind: None,
            switchable: true,
            saved_at: None,
        }
    }

    fn snapshot(accounts: Vec<AgentAccount>) -> AgentAccountsSnapshot {
        AgentAccountsSnapshot {
            accounts,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn the_chats_provider_answers_before_any_active_account() {
        let accounts = snapshot(vec![
            account(HarnessId::Codex, true, vec![window("Primary", 0.5)]),
            account(HarnessId::ClaudeCode, false, vec![window("Week", 0.2)]),
        ]);
        let picked = active_account(&accounts, Some(HarnessId::ClaudeCode)).unwrap();
        assert_eq!(picked.harness, HarnessId::ClaudeCode);
        // Without a chat the strip still answers: the active login of any
        // provider beats nothing.
        assert_eq!(
            active_account(&accounts, None).unwrap().harness,
            HarnessId::Codex
        );
        assert!(active_account(&snapshot(Vec::new()), Some(HarnessId::Codex)).is_none());
    }

    #[test]
    fn the_active_login_wins_within_its_own_provider() {
        let accounts = snapshot(vec![
            account(HarnessId::Codex, false, vec![window("Primary", 0.1)]),
            account(HarnessId::Codex, true, vec![window("Primary", 0.9)]),
        ]);
        let picked = active_account(&accounts, Some(HarnessId::Codex)).unwrap();
        assert_eq!(percent(tightest_window(picked).unwrap()), 90);
    }

    #[test]
    fn the_meter_shows_the_tightest_window_and_keeps_order_on_ties() {
        let accounts = snapshot(vec![account(
            HarnessId::ClaudeCode,
            true,
            vec![
                window("Session", 0.09),
                window("Week", 0.4),
                window("Month", 0.4),
            ],
        )]);
        let picked = active_account(&accounts, Some(HarnessId::ClaudeCode)).unwrap();
        assert_eq!(tightest_window(picked).unwrap().label, "Week");
        // An account with no reported window has nothing to meter.
        assert!(tightest_window(&account(HarnessId::ClaudeCode, true, Vec::new())).is_none());
    }

    #[test]
    fn countdowns_read_as_time_left() {
        let now = Utc.with_ymd_and_hms(2026, 9, 26, 12, 0, 0).unwrap();
        assert_eq!(format_countdown(None, now), None);
        // An already-spent window reads as a fact, not a negative duration.
        assert_eq!(
            format_countdown(Some(now - Duration::hours(2)), now).as_deref(),
            Some("now")
        );
        assert_eq!(
            format_countdown(Some(now + Duration::minutes(45)), now).as_deref(),
            Some("45m")
        );
        assert_eq!(
            format_countdown(Some(now + Duration::hours(6)), now).as_deref(),
            Some("6h 00m")
        );
        assert_eq!(
            format_countdown(Some(now + Duration::hours(6 * 24 + 12)), now).as_deref(),
            Some("6d 12h")
        );
    }

    #[test]
    fn the_label_follows_the_settings_page_order() {
        let mut account = account(HarnessId::Codex, true, Vec::new());
        assert_eq!(account_label(&account), "dev@example.com");
        account.email = Some("   ".into());
        account.display_name = Some("Ada".into());
        assert_eq!(account_label(&account), "Ada");
        account.display_name = None;
        assert_eq!(account_label(&account), "Max 20×");
        account.plan_label = None;
        assert_eq!(account_label(&account), "Account");
        assert_eq!(provider_label(HarnessId::ClaudeCode), "Claude Code");
        assert_eq!(provider_label(HarnessId::Codex), "Codex");
    }

    #[test]
    fn percents_round_and_clamp() {
        assert_eq!(percent(&window("Session", 0.094)), 9);
        assert_eq!(percent(&window("Session", 1.4)), 100);
        assert_eq!(percent(&window("Session", -1.0)), 0);
    }

    #[test]
    fn a_refresh_reporting_the_same_quota_is_not_a_new_reading() {
        let quota = Quota {
            account: account(HarnessId::Codex, true, Vec::new()),
            window: window("Primary", 0.4),
        };
        let mut same = quota.clone();
        // A plan badge or email refresh does not move the meter.
        same.account.plan_label = Some("ChatGPT Plus".into());
        assert!(quota.same(&same));
        // A different account, window, percentage or reset does.
        let mut moved = quota.clone();
        moved.window.used_fraction = 0.5;
        assert!(!quota.same(&moved));
        let mut other_window = quota.clone();
        other_window.window.label = "Secondary".into();
        assert!(!quota.same(&other_window));
        let mut other_account = quota.clone();
        other_account.account.id = "someone-else".into();
        assert!(!quota.same(&other_account));
    }

    #[test]
    fn the_exit_hold_is_derived_from_the_spec_it_plays() {
        // The hold must never outlast the animation, or the section would sit
        // invisible for a frame too long; nor fall short of it, which would cut
        // the collapse off mid-flight.
        assert_eq!(
            exit_span(),
            crate::motion::COLLAPSE
                .total()
                .mul_f32(crate::motion::speed_scale())
        );
        assert!(!exit_span().is_zero());
    }
}
