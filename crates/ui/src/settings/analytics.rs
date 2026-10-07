//! Settings → Analytics: how the user works with their agents — a year of
//! daily activity, prompts per week by agent, the hour-of-day profile, and
//! per-agent totals. The engine returns raw prompts (timestamp + agent) for
//! this device's chats; every aggregate is computed here, in local time.
//!
//! Color follows the data's job: magnitude (heatmap, hours, agent bars) is
//! the single Clyra accent hue; agent identity (the stacked weekly chart)
//! uses a fixed categorical order validated for both modes — each agent owns
//! its slot, so a filter or ranking never repaints it.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Datelike, Duration as Days, Local, NaiveDate, TimeZone, Timelike};
use clyra_proto::{HarnessId, UsageAnalytics};
use gpui::{
    AnyElement, Context, Entity, Hsla, SharedString, Task, Window, div, prelude::*, px, relative,
    rgb,
};

use super::widgets;
use crate::popover;
use crate::state::AppState;
use crate::theme::{Appearance, Theme, ink};

/// Weeks on the activity grid and the weekly chart (a year + this week).
const WEEKS: usize = 53;
const CELL: f32 = 11.0;
const CELL_GAP: f32 = 3.0;
const DAY_LABEL_W: f32 = 30.0;
const WEEKLY_CHART_H: f32 = 112.0;
const HOUR_CHART_H: f32 = 116.0;
/// Refetch when the page is shown again after this long.
const STALE_AFTER: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, PartialEq, Eq)]
enum HourRange {
    All,
    Month,
    Week,
}

impl HourRange {
    const ORDER: [HourRange; 3] = [HourRange::All, HourRange::Month, HourRange::Week];

    fn label(self) -> &'static str {
        match self {
            HourRange::All => "All time",
            HourRange::Month => "Last 30 days",
            HourRange::Week => "Last 7 days",
        }
    }

    fn days(self) -> Option<i64> {
        match self {
            HourRange::All => None,
            HourRange::Month => Some(30),
            HourRange::Week => Some(7),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AgentMetric {
    Sessions,
    Prompts,
}

enum Load {
    Loading,
    Ready(Arc<Stats>),
    Failed(SharedString),
}

/// Everything the charts draw, computed once per fetch.
struct Stats {
    total_prompts: usize,
    /// First day (a Sunday) of the activity grid.
    grid_start: NaiveDate,
    today: NaiveDate,
    per_day: HashMap<NaiveDate, u32>,
    max_day: u32,
    current_streak: u32,
    longest_streak: u32,
    busiest: Option<(NaiveDate, u32)>,
    /// Per grid week: prompts per agent.
    weekly: Vec<HashMap<HarnessId, u32>>,
    /// Prompt timestamps as (local date, hour) for the hour chart's ranges.
    stamps: Vec<(NaiveDate, u32)>,
    prompts_by_agent: HashMap<HarnessId, u32>,
    sessions_by_agent: Vec<(HarnessId, u32)>,
}

impl Stats {
    fn compute(data: &UsageAnalytics) -> Self {
        let today = Local::now().date_naive();
        let week_start = today - Days::days(today.weekday().num_days_from_sunday() as i64);
        let grid_start = week_start - Days::weeks(WEEKS as i64 - 1);
        let mut per_day: HashMap<NaiveDate, u32> = HashMap::new();
        let mut weekly: Vec<HashMap<HarnessId, u32>> = vec![HashMap::new(); WEEKS];
        let mut stamps = Vec::with_capacity(data.prompts.len());
        let mut prompts_by_agent: HashMap<HarnessId, u32> = HashMap::new();
        for prompt in &data.prompts {
            let Some(at) = Local.timestamp_millis_opt(prompt.at).single() else {
                continue;
            };
            let date = at.date_naive();
            *per_day.entry(date).or_default() += 1;
            *prompts_by_agent.entry(prompt.harness).or_default() += 1;
            stamps.push((date, at.hour()));
            let offset = (date - grid_start).num_days();
            if (0..(WEEKS as i64 * 7)).contains(&offset) {
                *weekly[(offset / 7) as usize]
                    .entry(prompt.harness)
                    .or_default() += 1;
            }
        }
        let max_day = per_day.values().copied().max().unwrap_or(0);
        let busiest = per_day
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(date, count)| (*date, *count));
        // Current streak: consecutive active days ending today — or
        // yesterday, so a streak isn't "broken" before today's first prompt.
        let active = |date: &NaiveDate| per_day.get(date).is_some_and(|n| *n > 0);
        let mut cursor = if active(&today) {
            today
        } else {
            today - Days::days(1)
        };
        let mut current_streak = 0;
        while active(&cursor) {
            current_streak += 1;
            cursor -= Days::days(1);
        }
        let mut days: Vec<NaiveDate> = per_day.keys().copied().collect();
        days.sort();
        let (mut longest_streak, mut run, mut prev): (u32, u32, Option<NaiveDate>) = (0, 0, None);
        for day in days {
            run = match prev {
                Some(p) if day - p == Days::days(1) => run + 1,
                _ => 1,
            };
            longest_streak = longest_streak.max(run);
            prev = Some(day);
        }
        let mut sessions_by_agent: Vec<(HarnessId, u32)> =
            data.sessions.iter().map(|s| (s.harness, s.count)).collect();
        sessions_by_agent.sort_by(|a, b| b.1.cmp(&a.1));
        Self {
            total_prompts: data.prompts.len(),
            grid_start,
            today,
            per_day,
            max_day,
            current_streak,
            longest_streak,
            busiest,
            weekly,
            stamps,
            prompts_by_agent,
            sessions_by_agent,
        }
    }

    fn hours(&self, range: HourRange) -> [u32; 24] {
        let since = range.days().map(|days| self.today - Days::days(days - 1));
        let mut hours = [0u32; 24];
        for (date, hour) in &self.stamps {
            if since.is_none_or(|since| *date >= since) {
                hours[*hour as usize] += 1;
            }
        }
        hours
    }
}

pub struct AnalyticsPage {
    state: Entity<AppState>,
    scroll: widgets::PageScroll,
    load: Load,
    fetched_at: Option<Instant>,
    task: Option<Task<()>>,
    hour_range: HourRange,
    agent_metric: AgentMetric,
}

impl AnalyticsPage {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            state,
            scroll: widgets::PageScroll::default(),
            load: Load::Loading,
            fetched_at: None,
            task: None,
            hour_range: HourRange::All,
            agent_metric: AgentMetric::Sessions,
        };
        page.fetch(cx);
        page
    }

    fn fetch(&mut self, cx: &mut Context<Self>) {
        if self.task.is_some() {
            return;
        }
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            return; // engine still booting — the next render retries
        };
        self.fetched_at = Some(Instant::now());
        self.task = Some(cx.spawn(async move |this, cx| {
            let reply = crate::attachments::call_with_timeout(
                &engine,
                cx.background_executor(),
                clyra_rpc::methods::GET_USAGE_ANALYTICS,
                serde_json::json!({}),
                Duration::from_secs(60),
            )
            .await;
            let load = match reply.and_then(|value| {
                serde_json::from_value::<UsageAnalytics>(value).map_err(|e| e.to_string())
            }) {
                Ok(data) => Load::Ready(Arc::new(Stats::compute(&data))),
                Err(err) => Load::Failed(err.into()),
            };
            this.update(cx, |page, cx| {
                page.task = None;
                // Keep the charts on a failed refresh; only a first load
                // shows the error.
                if matches!(load, Load::Ready(_)) || !matches!(page.load, Load::Ready(_)) {
                    page.load = load;
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn on_scroll_hovered(&mut self, hovered: &bool, _: &mut Window, cx: &mut Context<Self>) {
        if self.scroll.set_list_hovered(*hovered) {
            cx.notify();
        }
    }
}

impl popover::ScrollRailHost for AnalyticsPage {
    fn rail_bar(&mut self) -> &mut popover::MenuScrollbarState {
        self.scroll.rail_bar()
    }

    fn rail_scroll(&self) -> Option<gpui::ScrollHandle> {
        self.scroll.rail_scroll()
    }
}

// ---------------------------------------------------------------------------
// Formatting + palette (pure)
// ---------------------------------------------------------------------------

fn fmt_count(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (ix, ch) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn plural(n: u32, one: &str, many: &str) -> String {
    format!("{} {}", fmt_count(n), if n == 1 { one } else { many })
}

fn hour_label(hour: usize) -> String {
    match hour {
        0 => "12 AM".into(),
        12 => "12 PM".into(),
        h if h < 12 => format!("{h} AM"),
        h => format!("{} PM", h - 12),
    }
}

fn agent_name(harness: HarnessId) -> &'static str {
    crate::pickers::harness_name(harness)
}

/// Fixed categorical slot per agent (identity never follows rank). Agents
/// past the validated seven share "Other".
fn agent_slot(harness: HarnessId) -> Option<usize> {
    match harness {
        HarnessId::Codex => Some(0),
        HarnessId::ClaudeCode => Some(1),
        HarnessId::Cursor => Some(2),
        HarnessId::Grok => Some(3),
        HarnessId::Pi => Some(4),
        HarnessId::Devin => Some(5),
        HarnessId::Opencode => Some(6),
        HarnessId::Hermes | HarnessId::Antigravity | HarnessId::Mock => None,
    }
}

/// The categorical order validated (dataviz six checks) against the card
/// plane in each mode: blue, orange, aqua, yellow, magenta, green, violet.
fn slot_color(slot: Option<usize>, appearance: Appearance) -> Hsla {
    const DARK: [u32; 7] = [
        0x3987e5, 0xd95926, 0x199e70, 0xc98500, 0xd55181, 0x008300, 0x9085e9,
    ];
    const LIGHT: [u32; 7] = [
        0x2a78d6, 0xeb6834, 0x1baf7a, 0xeda100, 0xe87ba4, 0x008300, 0x4a3aa7,
    ];
    match (slot, appearance) {
        (Some(slot), Appearance::Dark) => rgb(DARK[slot]).into(),
        (Some(slot), Appearance::Light) => rgb(LIGHT[slot]).into(),
        (None, Appearance::Dark) => rgb(0x737373).into(),
        (None, Appearance::Light) => rgb(0xa3a3a3).into(),
    }
}

/// Sequential ramp on the accent hue: empty, then four magnitude steps.
fn heat_color(theme: &Theme, level: usize) -> Hsla {
    match level {
        0 => ink(0.06),
        1 => theme.accent.opacity(0.28),
        2 => theme.accent.opacity(0.5),
        3 => theme.accent.opacity(0.75),
        _ => theme.accent,
    }
}

fn heat_level(count: u32, max: u32) -> usize {
    if count == 0 || max == 0 {
        0
    } else {
        ((count as f32 / max as f32 * 4.0).ceil() as usize).clamp(1, 4)
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

struct ChartTip(SharedString);

impl Render for ChartTip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        div()
            .px(px(8.0))
            .py(px(5.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.surface_raised)
            .shadow_md()
            .text_size(px(11.5))
            .text_color(theme.text)
            .child(self.0.clone())
    }
}

/// A stateful mark carrying a hover tooltip.
fn tipped(id: String, tip: String, mark: gpui::Div) -> gpui::Stateful<gpui::Div> {
    let tip: SharedString = tip.into();
    mark.id(SharedString::from(id)).tooltip(move |_, cx| {
        let tip = tip.clone();
        cx.new(|_| ChartTip(tip)).into()
    })
}

fn section(theme: &Theme, title: &str, subtitle: String, control: Option<AnyElement>) -> gpui::Div {
    widgets::section_card(theme).p(px(18.0)).child(
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(12.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(crate::typography::ui_rems(14.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(SharedString::from(title.to_string())),
                    )
                    .child(
                        div()
                            .text_size(crate::typography::ui_rems(12.0))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(subtitle)),
                    ),
            )
            .children(control),
    )
}

/// `‹ label ›` stepper for a chart's view option.
fn stepper(
    theme: &Theme,
    id: &'static str,
    label: &str,
    cx: &mut Context<AnalyticsPage>,
    step: impl Fn(&mut AnalyticsPage, i32) + Clone + 'static,
) -> AnyElement {
    let arrow = |dir: i32, icon: &'static str, cx: &mut Context<AnalyticsPage>| {
        let step = step.clone();
        div()
            .id(SharedString::from(format!("{id}-{dir}")))
            .size(px(22.0))
            .rounded(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(ink(0.07)))
            .on_click(cx.listener(move |page, _, _, cx| {
                step(page, dir);
                cx.notify();
            }))
            .child(
                crate::icons::icon(icon)
                    .size(px(14.0))
                    .text_color(theme.text_muted),
            )
    };
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .child(arrow(-1, crate::icons::ALT_ARROW_LEFT, cx))
        .child(
            div()
                .min_w(px(84.0))
                .flex()
                .justify_center()
                .text_size(crate::typography::ui_rems(12.0))
                .text_color(theme.text_muted)
                .child(SharedString::from(label.to_string())),
        )
        .child(arrow(1, crate::icons::ALT_ARROW_RIGHT, cx))
        .into_any_element()
}

fn cycle<T: Copy + PartialEq>(order: &[T], current: T, dir: i32) -> T {
    let ix = order.iter().position(|v| *v == current).unwrap_or(0) as i32;
    let len = order.len() as i32;
    order[((ix + dir).rem_euclid(len)) as usize]
}

/// Month labels over week columns: a label wherever a column's first day
/// starts a new month (skipping the grid's very first, partial column).
fn month_labels(grid_start: NaiveDate, step: f32) -> gpui::Div {
    let mut row = div().flex().flex_row().h(px(16.0));
    for col in 0..WEEKS {
        let first = grid_start + Days::weeks(col as i64);
        let prev = first - Days::weeks(1);
        let label =
            (col > 0 && first.month() != prev.month()).then(|| first.format("%b").to_string());
        row = row.child(
            div()
                .flex_none()
                .w(px(step))
                .children(label.map(|label| div().whitespace_nowrap().child(label))),
        );
    }
    row
}

fn render_activity(stats: &Stats, theme: &Theme) -> gpui::Div {
    let step = CELL + CELL_GAP;
    let mut columns = div().flex().flex_row().gap(px(CELL_GAP));
    for col in 0..WEEKS {
        let mut column = div().flex_none().flex().flex_col().gap(px(CELL_GAP));
        for row in 0..7 {
            let date = stats.grid_start + Days::days((col * 7 + row) as i64);
            let cell = div().size(px(CELL)).rounded(px(2.5));
            if date > stats.today {
                column = column.child(cell);
                continue;
            }
            let count = stats.per_day.get(&date).copied().unwrap_or(0);
            let tip = format!(
                "{} · {}",
                date.format("%a, %b %-d, %Y"),
                plural(count, "prompt", "prompts")
            );
            column = column.child(tipped(
                format!("heat-{col}-{row}"),
                tip,
                cell.bg(heat_color(theme, heat_level(count, stats.max_day))),
            ));
        }
        columns = columns.child(column);
    }
    let day_labels = div()
        .flex_none()
        .w(px(DAY_LABEL_W))
        .flex()
        .flex_col()
        .gap(px(CELL_GAP))
        .children((0..7).map(|row| {
            div().h(px(CELL)).flex().items_center().children(match row {
                1 => Some("Mon"),
                3 => Some("Wed"),
                5 => Some("Fri"),
                _ => None,
            })
        }));
    // Right-anchored: a narrow window clips the OLDEST weeks, never today.
    let grid = div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_row()
        .justify_end()
        .child(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(month_labels(stats.grid_start, step))
                .child(columns),
        );
    let summary = {
        let mut parts = vec![format!("{}-day streak", stats.current_streak)];
        parts.push(format!(
            "Longest {}",
            plural(stats.longest_streak, "day", "days")
        ));
        if let Some((date, count)) = stats.busiest {
            parts.push(format!(
                "Busiest {} ({})",
                date.format("%b %-d"),
                plural(count, "prompt", "prompts")
            ));
        }
        parts.join(" · ")
    };
    let legend = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(3.0))
        .child(div().mr(px(3.0)).child("Less"))
        .children((0..5).map(|level| {
            div()
                .size(px(CELL))
                .rounded(px(2.5))
                .bg(heat_color(theme, level))
        }))
        .child(div().ml(px(3.0)).child("More"));
    div()
        .mt(px(16.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .text_size(crate::typography::ui_rems(11.0))
        .text_color(theme.text_muted)
        .child(
            div()
                .flex()
                .flex_row()
                .items_end()
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(div().h(px(16.0)))
                        .child(day_labels),
                )
                .child(grid),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .child(div().min_w_0().truncate().child(summary))
                .child(legend),
        )
}

/// Agents present in the weekly data, in slot order ("Other" last).
fn weekly_agents(stats: &Stats) -> Vec<Option<usize>> {
    let mut slots: Vec<Option<usize>> = stats
        .weekly
        .iter()
        .flat_map(|week| week.keys().map(|harness| agent_slot(*harness)))
        .collect();
    slots.sort_by_key(|slot| slot.unwrap_or(usize::MAX));
    slots.dedup();
    slots
}

fn slot_name(slot: Option<usize>) -> &'static str {
    const ORDER: [HarnessId; 7] = [
        HarnessId::Codex,
        HarnessId::ClaudeCode,
        HarnessId::Cursor,
        HarnessId::Grok,
        HarnessId::Pi,
        HarnessId::Devin,
        HarnessId::Opencode,
    ];
    slot.map_or("Other", |slot| agent_name(ORDER[slot]))
}

fn render_weekly(stats: &Stats, theme: &Theme) -> gpui::Div {
    let appearance = theme.appearance;
    let slots = weekly_agents(stats);
    let per_slot = |week: &HashMap<HarnessId, u32>| {
        let mut out: Vec<(Option<usize>, u32)> = Vec::new();
        for (harness, count) in week {
            let slot = agent_slot(*harness);
            match out.iter_mut().find(|(s, _)| *s == slot) {
                Some((_, n)) => *n += count,
                None => out.push((slot, *count)),
            }
        }
        out.sort_by_key(|(slot, _)| slot.unwrap_or(usize::MAX));
        out
    };
    let max_week = stats
        .weekly
        .iter()
        .map(|week| week.values().sum::<u32>())
        .max()
        .unwrap_or(0)
        .max(1);
    let mut bars = div()
        .h(px(WEEKLY_CHART_H))
        .flex()
        .flex_row()
        .items_end()
        .gap(px(2.0));
    for (col, week) in stats.weekly.iter().enumerate() {
        let segments = per_slot(week);
        let total: u32 = segments.iter().map(|(_, n)| n).sum();
        let first = stats.grid_start + Days::weeks(col as i64);
        let mut tip = format!(
            "Week of {} · {}",
            first.format("%b %-d"),
            plural(total, "prompt", "prompts")
        );
        for (slot, count) in &segments {
            tip.push_str(&format!("\n{} {}", slot_name(*slot), fmt_count(*count)));
        }
        let height = WEEKLY_CHART_H * total as f32 / max_week as f32;
        // Stack bottom-up in slot order, 1px surface gap between segments,
        // rounded data-end on top.
        let mut stack = div()
            .w_full()
            .h(px(height))
            .flex()
            .flex_col_reverse()
            .gap(px(1.0))
            .rounded_t(px(2.0))
            .overflow_hidden();
        for (slot, count) in &segments {
            stack = stack.child(
                div()
                    .w_full()
                    // Heights split in proportion to each agent's count.
                    .flex_grow(*count as f32)
                    .flex_basis(px(0.0))
                    .bg(slot_color(*slot, appearance)),
            );
        }
        bars = bars.child(tipped(
            format!("week-{col}"),
            tip,
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .justify_end()
                .child(if total == 0 {
                    div().w_full().h(px(2.0)).rounded(px(1.0)).bg(ink(0.06))
                } else {
                    stack
                }),
        ));
    }
    // Month ticks under the bars, on the same flex columns.
    let mut ticks = div().flex().flex_row().gap(px(2.0)).h(px(16.0));
    for col in 0..WEEKS {
        let first = stats.grid_start + Days::weeks(col as i64);
        let prev = first - Days::weeks(1);
        let label =
            (col > 0 && first.month() != prev.month()).then(|| first.format("%b").to_string());
        ticks = ticks.child(
            div()
                .flex_1()
                .min_w_0()
                .children(label.map(|label| div().whitespace_nowrap().child(label))),
        );
    }
    let legend = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap_x(px(14.0))
        .gap_y(px(4.0))
        .children(slots.into_iter().map(|slot| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .size(px(8.0))
                        .rounded(px(2.0))
                        .bg(slot_color(slot, appearance)),
                )
                .child(slot_name(slot))
        }));
    div()
        .mt(px(16.0))
        .flex()
        .flex_col()
        .gap(px(6.0))
        .text_size(crate::typography::ui_rems(11.0))
        .text_color(theme.text_muted)
        .child(bars)
        .child(ticks)
        .child(div().mt(px(4.0)).child(legend))
}

fn render_hours(hours: &[u32; 24], theme: &Theme) -> gpui::Div {
    let max = hours.iter().copied().max().unwrap_or(0).max(1);
    let peak = hours
        .iter()
        .enumerate()
        .max_by_key(|(_, n)| **n)
        .filter(|(_, n)| **n > 0)
        .map(|(hour, _)| hour);
    let mut bars = div()
        .h(px(HOUR_CHART_H))
        .flex()
        .flex_row()
        .items_end()
        .gap(px(4.0));
    for (hour, count) in hours.iter().enumerate() {
        let is_peak = peak == Some(hour);
        let height = (HOUR_CHART_H * *count as f32 / max as f32).max(3.0);
        bars = bars.child(tipped(
            format!("hour-{hour}"),
            format!(
                "{} · {}",
                hour_label(hour),
                plural(*count, "prompt", "prompts")
            ),
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .justify_end()
                .child(
                    div()
                        .w_full()
                        .h(px(height))
                        .rounded_t(px(3.0))
                        .bg(if *count == 0 {
                            ink(0.06)
                        } else if is_peak {
                            theme.accent
                        } else {
                            theme.accent.opacity(0.5)
                        }),
                ),
        ));
    }
    let ticks = div()
        .flex()
        .flex_row()
        .gap(px(4.0))
        .h(px(16.0))
        .children((0..24).map(|hour| {
            div().flex_1().min_w_0().children((hour % 6 == 0).then(|| {
                div()
                    .whitespace_nowrap()
                    .child(SharedString::from(hour_label(hour)))
            }))
        }));
    div()
        .mt(px(16.0))
        .flex()
        .flex_col()
        .gap(px(6.0))
        .text_size(crate::typography::ui_rems(11.0))
        .text_color(theme.text_muted)
        .child(bars)
        .child(ticks)
}

fn render_agents(rows: &[(HarnessId, u32)], theme: &Theme) -> gpui::Div {
    let max = rows.iter().map(|(_, n)| *n).max().unwrap_or(0).max(1);
    div()
        .mt(px(14.0))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .children(rows.iter().map(|(harness, count)| {
            let (icon, tint) = crate::pickers::harness_brand_icon(*harness);
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .text_size(crate::typography::ui_rems(12.5))
                .child(
                    div()
                        .flex_none()
                        .w(px(150.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            crate::icons::icon(icon)
                                .size(px(15.0))
                                .text_color(tint.unwrap_or(theme.text)),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_color(theme.text)
                                .child(agent_name(*harness)),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(6.0))
                        .rounded_full()
                        .bg(ink(0.07))
                        .child(
                            div()
                                .h_full()
                                .w(relative(*count as f32 / max as f32))
                                .rounded_full()
                                .bg(theme.accent),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(56.0))
                        .flex()
                        .justify_end()
                        .text_color(theme.text_muted)
                        .child(SharedString::from(fmt_count(*count))),
                )
        }))
}

impl Render for AnalyticsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        // Showing the page again refreshes stale numbers (and retries a
        // fetch the booting engine could not take yet).
        if self.fetched_at.is_none_or(|at| at.elapsed() > STALE_AFTER) {
            self.fetch(cx);
        }
        let body: AnyElement = match &self.load {
            Load::Loading => div()
                .mt(px(24.0))
                .text_size(crate::typography::ui_rems(13.0))
                .text_color(theme.text_muted)
                .child("Loading analytics…")
                .into_any_element(),
            Load::Failed(message) => {
                widgets::error_strip(&theme, format!("Couldn't load analytics: {message}"))
                    .into_any_element()
            }
            Load::Ready(stats)
                if stats.total_prompts == 0 && stats.sessions_by_agent.is_empty() =>
            {
                div()
                    .mt(px(24.0))
                    .text_size(crate::typography::ui_rems(13.0))
                    .text_color(theme.text_muted)
                    .child("No activity yet — your prompts will show up here.")
                    .into_any_element()
            }
            Load::Ready(stats) => {
                let stats = stats.clone();
                let leader = stats
                    .weekly
                    .last()
                    .and_then(|week| week.iter().max_by_key(|(_, n)| **n))
                    .map(|(harness, _)| agent_name(*harness));
                let weekly_subtitle = match leader {
                    Some(leader) => format!("Prompts per week by agent · {leader} leads this week"),
                    None => "Prompts per week by agent".to_string(),
                };
                let hours = stats.hours(self.hour_range);
                let peak = hours
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, n)| **n)
                    .filter(|(_, n)| **n > 0)
                    .map(|(hour, _)| hour);
                let hour_subtitle = match peak {
                    Some(hour) => format!(
                        "{} · Most active around {}",
                        self.hour_range.label(),
                        hour_label(hour)
                    ),
                    None => format!("{} · No prompts", self.hour_range.label()),
                };
                let hour_control = stepper(
                    &theme,
                    "analytics-hours",
                    self.hour_range.label(),
                    cx,
                    |page, dir| page.hour_range = cycle(&HourRange::ORDER, page.hour_range, dir),
                );
                let agent_rows: Vec<(HarnessId, u32)> = match self.agent_metric {
                    AgentMetric::Sessions => stats.sessions_by_agent.clone(),
                    AgentMetric::Prompts => {
                        let mut rows: Vec<(HarnessId, u32)> = stats
                            .prompts_by_agent
                            .iter()
                            .map(|(h, n)| (*h, *n))
                            .collect();
                        rows.sort_by(|a, b| b.1.cmp(&a.1));
                        rows
                    }
                };
                let metric_label = match self.agent_metric {
                    AgentMetric::Sessions => "Sessions",
                    AgentMetric::Prompts => "Prompts",
                };
                let agent_control =
                    stepper(&theme, "analytics-agents", metric_label, cx, |page, dir| {
                        page.agent_metric = cycle(
                            &[AgentMetric::Sessions, AgentMetric::Prompts],
                            page.agent_metric,
                            dir,
                        )
                    });
                let agents_subtitle = match self.agent_metric {
                    AgentMetric::Sessions => "Sessions started with each agent",
                    AgentMetric::Prompts => "Prompts sent to each agent",
                };
                div()
                    .flex()
                    .flex_col()
                    .child(
                        section(
                            &theme,
                            "Activity",
                            format!(
                                "Prompts you sent, day by day · {} in total",
                                plural(stats.total_prompts as u32, "prompt", "prompts")
                            ),
                            None,
                        )
                        .child(render_activity(&stats, &theme)),
                    )
                    .child(
                        section(&theme, "Over time", weekly_subtitle, None)
                            .child(render_weekly(&stats, &theme)),
                    )
                    .child(
                        section(&theme, "By hour", hour_subtitle, Some(hour_control))
                            .child(render_hours(&hours, &theme)),
                    )
                    .child(
                        section(
                            &theme,
                            "Agents",
                            agents_subtitle.to_string(),
                            Some(agent_control),
                        )
                        .child(render_agents(&agent_rows, &theme)),
                    )
                    .into_any_element()
            }
        };

        let scrollbar = popover::rail(self, "analytics-page-scrollbar", &theme, cx);
        div()
            .id("analytics-page-host")
            .relative()
            .size_full()
            .on_hover(cx.listener(Self::on_scroll_hovered))
            .child(
                div()
                    .id("analytics-page")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll.scroll)
                    .child(
                        widgets::page_column()
                            // A year of weeks needs more room than a form page.
                            .max_w(px(900.0))
                            .child(widgets::page_header(
                                &theme,
                                crate::shell::SettingsSection::Analytics.icon(),
                                "Analytics",
                                None,
                            ))
                            .child(
                                widgets::page_subtitle(
                                    &theme,
                                    "How you work with your agents on this device.",
                                )
                                .line_height(px(20.0)),
                            )
                            .child(body),
                    ),
            )
            .children(scrollbar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clyra_proto::UsagePrompt;

    fn at(date: NaiveDate, hour: u32) -> i64 {
        Local
            .from_local_datetime(&date.and_hms_opt(hour, 0, 0).unwrap())
            .single()
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn streaks_busiest_and_hours() {
        let today = Local::now().date_naive();
        let day = |ago: i64| today - Days::days(ago);
        let prompts = [
            (day(0), 9),
            (day(1), 9),
            (day(1), 16),
            (day(2), 16),
            (day(5), 16),
            (day(6), 10),
            (day(7), 11),
            (day(8), 12),
        ]
        .into_iter()
        .map(|(date, hour)| UsagePrompt {
            at: at(date, hour),
            harness: HarnessId::Codex,
        })
        .collect();
        let stats = Stats::compute(&UsageAnalytics {
            prompts,
            sessions: Vec::new(),
        });
        assert_eq!(stats.current_streak, 3);
        assert_eq!(stats.longest_streak, 4);
        assert_eq!(stats.busiest, Some((day(1), 2)));
        let hours = stats.hours(HourRange::All);
        assert_eq!(hours[16], 3);
        assert_eq!(stats.hours(HourRange::Week)[12], 0);
        assert_eq!(
            stats
                .weekly
                .iter()
                .map(|w| w.values().sum::<u32>())
                .sum::<u32>(),
            8
        );
    }

    #[test]
    fn counts_and_levels_format() {
        assert_eq!(fmt_count(1281), "1,281");
        assert_eq!(fmt_count(97), "97");
        assert_eq!(fmt_count(1_000_000), "1,000,000");
        assert_eq!(hour_label(0), "12 AM");
        assert_eq!(hour_label(16), "4 PM");
        assert_eq!(heat_level(0, 10), 0);
        assert_eq!(heat_level(1, 10), 1);
        assert_eq!(heat_level(10, 10), 4);
    }
}
