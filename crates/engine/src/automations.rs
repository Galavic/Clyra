//! Host-local automations: a prompt that starts a run on its own, in one
//! project, on a schedule or on pull-request activity.
//!
//! Stored outside the synced workspace registry, exactly like
//! [`crate::project_actions`]: the engine that owns the project is the only
//! authority that can fire the run, because the agent CLI lives there. The UI
//! reaches another device's automations the same way it reaches another
//! device's project Actions — `targetDeviceId` relay forwarding.
//!
//! Two trigger families, deliberately small:
//!
//! - **Schedule** — hourly / daily / weekdays / weekly, computed in UTC by
//!   [`next_fire_after`].
//! - **Pull request** — GitHub activity in the project's checkout, read
//!   through the `gh` CLI. State-based, not stream-based: the trigger records
//!   the PR numbers it has already fired on, so a poll (or a restart) can
//!   never fire the same PR twice.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration as StdDuration, Instant};

use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use clyra_doc::SessionCommandPayload;
use clyra_proto::{
    Automation, AutomationCadence, AutomationDraft, AutomationRunAck, AutomationSchedule,
    AutomationTrigger, AutomationsSnapshot, ChatConfig, HarnessId, PullRequestEvent, RunRequest,
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::EngineError;
use crate::doc_host::DocHost;
use crate::source_control::{ChangeRequestResolver, PullRequestActivity};
use crate::workspace_host::WorkspaceHost;

pub const MAX_AUTOMATIONS: usize = 200;
pub const MAX_AUTOMATION_NAME_CHARS: usize = 120;
pub const MAX_AUTOMATION_PROMPT_BYTES: usize = 64 * 1024;
pub const MAX_AUTOMATION_ID_BYTES: usize = 96;
/// How many PR numbers an automation remembers. Deep enough that a week of
/// activity on a busy repo cannot re-fire an old PR; bounded so the store
/// file cannot grow without limit.
pub const MAX_SEEN_PULL_REQUESTS: usize = 200;

const STORE_FILE: &str = "automations.json";
const STORE_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Schedule math (pure)
// ---------------------------------------------------------------------------

/// The first fire strictly after `from` for `schedule`, in UTC.
///
/// Pure and total: a malformed field (hour 99, weekday 0) is clamped rather
/// than rejected, so a hand-edited store file cannot wedge the scheduler.
/// Weekday-only cadences skip non-matching days instead of firing late.
pub fn next_fire_after(
    schedule: &AutomationSchedule,
    from: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let minute = schedule.minute.min(59);
    match schedule.every {
        // On the hour, next hour strictly after `from`.
        AutomationCadence::Hourly => {
            let base = from.with_minute(0)?.with_second(0)?.with_nanosecond(0)?;
            Some(base + Duration::hours(1))
        }
        AutomationCadence::Daily => {
            at_next_occurrence(from, minute, schedule.hour.min(23), |_| true)
        }
        AutomationCadence::Weekdays => {
            at_next_occurrence(from, minute, schedule.hour.min(23), |day| day <= 5)
        }
        AutomationCadence::Weekly => {
            // ISO weekday 1..=7; anything else falls back to Monday.
            let target = match schedule.weekday {
                1..=7 => schedule.weekday,
                _ => 1,
            };
            at_next_occurrence(from, minute, schedule.hour.min(23), |day| day == target)
        }
    }
}

/// The next `hour:minute` on a day matching `matches`, strictly after `from`.
/// Walks at most seven days ahead — a `matches` that never passes would
/// otherwise spin forever.
fn at_next_occurrence<F>(
    from: DateTime<Utc>,
    minute: u8,
    hour: u8,
    matches: F,
) -> Option<DateTime<Utc>>
where
    F: Fn(u8) -> bool,
{
    let base = from
        .date_naive()
        .and_hms_opt(hour.into(), minute.into(), 0)?
        .and_utc();
    for day in 0..=7 {
        let Some(candidate) = base.checked_add_signed(Duration::days(day)) else {
            return None;
        };
        if candidate <= from {
            continue;
        }
        if matches(u8::try_from(candidate.weekday().number_from_monday()).unwrap_or(0)) {
            return Some(candidate);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutomationsFile {
    version: u32,
    #[serde(default)]
    automations: BTreeMap<String, Automation>,
    #[serde(default)]
    task_reports: BTreeMap<String, String>,
}

struct AutomationsStoreInner {
    path: PathBuf,
    state: Mutex<AutomationsFile>,
}

/// Account-scoped storage for automations.
#[derive(Clone)]
pub struct AutomationsStore {
    inner: Arc<AutomationsStoreInner>,
}

impl AutomationsStore {
    pub fn open(profile_store_root: &Path) -> Result<Self, EngineError> {
        std::fs::create_dir_all(profile_store_root)?;
        let path = profile_store_root.join(STORE_FILE);
        let state = match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<AutomationsFile>(&bytes) {
                Ok(file) if file.version == STORE_VERSION => file,
                Ok(file) => {
                    tracing::warn!(
                        path = %path.display(),
                        version = file.version,
                        "unsupported automations store version; starting empty"
                    );
                    empty_file()
                }
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %err,
                        "invalid automations store; starting empty"
                    );
                    empty_file()
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => empty_file(),
            Err(err) => return Err(err.into()),
        };
        Ok(Self {
            inner: Arc::new(AutomationsStoreInner {
                path,
                state: Mutex::new(state),
            }),
        })
    }

    /// Every stored automation, oldest first — a list surface, not a map.
    pub fn list(&self) -> Result<Vec<Automation>, EngineError> {
        let state = lock(&self.inner.state);
        let mut automations: Vec<Automation> = state.automations.values().cloned().collect();
        automations.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        Ok(automations)
    }

    pub fn snapshot(&self) -> Result<AutomationsSnapshot, EngineError> {
        Ok(AutomationsSnapshot {
            automations: self.list()?,
            warnings: Vec::new(),
        })
    }

    pub fn get(&self, id: &str) -> Result<Option<Automation>, EngineError> {
        Ok(lock(&self.inner.state).automations.get(id).cloned())
    }

    /// Create or replace. `project_label`/`project_path` are supplied by the
    /// caller (it resolved the space) and pinned onto the record so the list
    /// can render without a second lookup, and so a renamed project does not
    /// silently retarget an automation.
    pub fn upsert(
        &self,
        draft: AutomationDraft,
        project_label: String,
        project_path: String,
        now: DateTime<Utc>,
    ) -> Result<Automation, EngineError> {
        let draft = normalize_draft(draft)?;
        let mut state = lock(&self.inner.state);
        let mut next = state.clone();

        let id = if draft.id.is_empty() {
            if next.automations.len() >= MAX_AUTOMATIONS {
                return Err(EngineError::Other(format!(
                    "At most {MAX_AUTOMATIONS} automations"
                )));
            }
            new_id()
        } else {
            if !next.automations.contains_key(&draft.id) {
                return Err(EngineError::Other("Automation not found".into()));
            }
            draft.id.clone()
        };

        // Editing preserves the run bookkeeping: a rename must not forget when
        // the automation last fired, nor re-fire on PRs it already saw.
        let previous = next.automations.get(&id).cloned();
        let next_run_at = match draft.trigger.schedule() {
            Some(schedule) => {
                // A schedule edit re-arms from now; an untouched schedule keeps
                // the pending fire it already had.
                let rearm = previous
                    .as_ref()
                    .is_none_or(|p| p.trigger.schedule() != Some(schedule));
                if rearm {
                    next_fire_after(schedule, now)
                } else {
                    previous.as_ref().and_then(|p| p.next_run_at)
                }
            }
            None => None,
        };
        let conversation_chat_id = previous
            .as_ref()
            .filter(|p| p.project_path == project_path && p.harness == draft.harness)
            .and_then(|p| p.conversation_chat_id.clone());
        let automation = Automation {
            id,
            name: draft.name,
            space_id: draft.space_id,
            project_label,
            project_path,
            trigger: draft.trigger,
            prompt: draft.prompt,
            harness: draft.harness,
            model: draft.model,
            sandbox: draft.sandbox,
            enabled: draft.enabled,
            created_at: previous.as_ref().map_or(now, |p| p.created_at),
            updated_at: now,
            last_run_at: previous.as_ref().and_then(|p| p.last_run_at),
            conversation_chat_id,
            activity_chat_ids: previous
                .as_ref()
                .map(|p| p.activity_chat_ids.clone())
                .unwrap_or_default(),
            seen_pull_requests: previous
                .as_ref()
                .map(|p| p.seen_pull_requests.clone())
                .unwrap_or_default(),
            next_run_at,
        };
        next.automations
            .insert(automation.id.clone(), automation.clone());
        persist(&self.inner.path, &next)?;
        *state = next;
        Ok(automation)
    }

    pub fn delete(&self, id: &str) -> Result<bool, EngineError> {
        let mut state = lock(&self.inner.state);
        if !state.automations.contains_key(id) {
            return Ok(false);
        }
        let mut next = state.clone();
        next.automations.remove(id);
        persist(&self.inner.path, &next)?;
        *state = next;
        Ok(true)
    }

    /// The kill switch. Returns the automation as it now stands, so the caller
    /// can re-arm (or clear) its schedule in the same breath.
    pub fn set_enabled(
        &self,
        id: &str,
        enabled: bool,
        now: DateTime<Utc>,
    ) -> Result<Option<Automation>, EngineError> {
        let mut state = lock(&self.inner.state);
        let Some(current) = state.automations.get(id).cloned() else {
            return Ok(None);
        };
        if current.enabled == enabled {
            return Ok(Some(current));
        }
        let mut next = state.clone();
        let Some(entry) = next.automations.get_mut(id) else {
            return Ok(None);
        };
        entry.enabled = enabled;
        entry.updated_at = now;
        // Re-enabling an hourly schedule that has been off for a week must
        // not fire seven times on the next tick: arm the next slot from now.
        entry.next_run_at = match (enabled, entry.trigger.schedule()) {
            (true, Some(schedule)) => next_fire_after(schedule, now),
            _ => None,
        };
        let updated = entry.clone();
        persist(&self.inner.path, &next)?;
        *state = next;
        Ok(Some(updated))
    }

    /// Stamp a started run and advance the schedule. Pure bookkeeping, called
    /// after the run is queued (never before, so a failed fire does not skip
    /// a slot).
    pub fn note_run_started(
        &self,
        id: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<Automation>, EngineError> {
        self.mutate(id, now, |entry| {
            entry.last_run_at = Some(now);
            entry.next_run_at = entry
                .trigger
                .schedule()
                .and_then(|s| next_fire_after(s, now));
        })
    }

    /// Record the PR numbers a pull-request trigger has now seen, so the next
    /// poll fires only on genuinely new activity.
    pub fn note_pull_requests(
        &self,
        id: &str,
        seen: &[u64],
        now: DateTime<Utc>,
    ) -> Result<Option<Automation>, EngineError> {
        self.mutate(id, now, |entry| {
            let mut merged = entry.seen_pull_requests.clone();
            merged.extend_from_slice(seen);
            merged.sort_unstable();
            merged.dedup();
            if merged.len() > MAX_SEEN_PULL_REQUESTS {
                // Keep the newest: a PR number only grows, so the tail is the
                // recent history.
                merged.drain(..merged.len() - MAX_SEEN_PULL_REQUESTS);
            }
            entry.seen_pull_requests = merged;
        })
    }

    fn mutate<F>(
        &self,
        id: &str,
        now: DateTime<Utc>,
        apply: F,
    ) -> Result<Option<Automation>, EngineError>
    where
        F: FnOnce(&mut Automation),
    {
        let mut state = lock(&self.inner.state);
        if !state.automations.contains_key(id) {
            return Ok(None);
        }
        let mut next = state.clone();
        let Some(entry) = next.automations.get_mut(id) else {
            return Ok(None);
        };
        apply(entry);
        entry.updated_at = now;
        let updated = entry.clone();
        persist(&self.inner.path, &next)?;
        *state = next;
        Ok(Some(updated))
    }

    /// Enabled automations whose schedule is due, in fire order. The caller
    /// drains them and stamps each one it starts.
    pub fn due(&self, now: DateTime<Utc>) -> Result<Vec<Automation>, EngineError> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|a| a.enabled)
            .filter(|a| a.next_run_at.is_some_and(|at| at <= now))
            .collect())
    }

    /// Enabled pull-request automations — the ones that need a poll.
    pub fn pull_request_triggers(&self) -> Result<Vec<Automation>, EngineError> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|a| a.enabled)
            .filter(|a| matches!(a.trigger, AutomationTrigger::PullRequest(_)))
            .collect())
    }
}

// ---------------------------------------------------------------------------
// Service: the scheduler and the fire path
// ---------------------------------------------------------------------------

/// How often due schedules are drained. A minute of slack on an hourly cadence
/// is invisible; a minute of slack on "every hour" is the difference between
/// firing at :00 and firing when the tab happened to wake up.
const TICK: StdDuration = StdDuration::from_secs(10);
/// How often pull-request automations poll `gh`. Deliberately slow: the
/// cheapest possible way to be wrong about a PR is to ask GitHub too often
/// and get rate-limited into never learning about any.
const PR_POLL: StdDuration = StdDuration::from_secs(180);

struct AutomationsInner {
    message_lock: tokio::sync::Mutex<()>,
    store: AutomationsStore,
    workspace: WorkspaceHost,
    doc_host: DocHost,
    resolver: ChangeRequestResolver,
    device_id: String,
    default_harness: HarnessId,
    cancel: CancellationToken,
    supervisor: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// One poll per project path per cycle, not one per automation: five
    /// automations on one repo cost one `gh` call.
    last_pr_poll: Mutex<HashMap<String, Instant>>,
}

/// Owns the automations store and fires the due ones. The loop is the whole
/// product: an automation is a record until something starts its run.
#[derive(Clone)]
pub struct Automations {
    inner: Arc<AutomationsInner>,
}

impl Automations {
    pub fn start(
        store: AutomationsStore,
        workspace: WorkspaceHost,
        doc_host: DocHost,
        device_id: &str,
        default_harness: HarnessId,
    ) -> Self {
        let automations = Self {
            inner: Arc::new(AutomationsInner {
                message_lock: tokio::sync::Mutex::new(()),
                store,
                workspace,
                doc_host,
                resolver: ChangeRequestResolver::new(),
                device_id: device_id.to_string(),
                default_harness,
                cancel: CancellationToken::new(),
                supervisor: Mutex::new(None),
                last_pr_poll: Mutex::new(HashMap::new()),
            }),
        };
        let task = tokio::spawn(automation_task(Arc::downgrade(&automations.inner)));
        *lock(&automations.inner.supervisor) = Some(task);
        automations
    }

    pub fn store(&self) -> &AutomationsStore {
        &self.inner.store
    }

    pub async fn shutdown(&self) {
        self.inner.cancel.cancel();
        let task = lock(&self.inner.supervisor).take();
        if let Some(task) = task {
            let _ = task.await;
        }
    }

    /// The list surface: stored automations plus the warnings a user can act
    /// on (a project path that no longer exists).
    pub fn snapshot(&self) -> Result<AutomationsSnapshot, EngineError> {
        let automations = self.inner.store.list()?;
        let mut warnings = Vec::new();
        for automation in &automations {
            if !std::path::Path::new(&automation.project_path).is_dir() {
                warnings.push(format!(
                    "{}: {} is no longer on this device",
                    automation.name, automation.project_path
                ));
            }
        }
        Ok(AutomationsSnapshot {
            automations,
            warnings,
        })
    }

    /// One cycle: drain due schedules, then poll pull-request triggers whose
    /// project is off the poll interval. Never propagates an error — a failing
    /// automation must not take the loop down with it.
    pub async fn tick(&self, now: DateTime<Utc>) {
        if let Err(err) = self.report_worker_results().await {
            tracing::warn!(error = %err, "dot worker report failed");
        }
        if let Err(err) = self.fire_due_schedules(now).await {
            tracing::warn!(error = %err, "automation schedule drain failed");
        }
        if let Err(err) = self.poll_pull_requests(now).await {
            tracing::warn!(error = %err, "automation pull-request poll failed");
        }
    }

    async fn report_worker_results(&self) -> Result<(), EngineError> {
        let _guard = self.inner.message_lock.lock().await;
        let Some(sessions) = self.inner.doc_host.sessions() else {
            return Ok(());
        };
        let roots: Vec<String> = self
            .inner
            .store
            .list()?
            .into_iter()
            .flat_map(|a| {
                a.conversation_chat_id
                    .into_iter()
                    .chain(a.activity_chat_ids)
            })
            .collect();
        if roots.is_empty() {
            return Ok(());
        }
        for child in self.inner.workspace.read_chats()? {
            let Some(parent) = child.parent_chat_id.as_ref().filter(|p| roots.contains(p)) else {
                continue;
            };
            if child.archived || child.device_id != self.inner.device_id {
                continue;
            }
            let Some(session) = sessions.session_status(&child.id) else {
                continue;
            };
            if session.status == clyra_proto::SessionStatus::Working {
                continue;
            }
            let entries = self.inner.doc_host.open(&child.id)?.doc().read_entries()?;
            let worker = entries.iter().any(|e| e.parts.iter().any(|p| matches!(p,
                clyra_doc::MessagePart::Text { text, .. } if text.starts_with("<!-- clyra-dot-worker -->"))));
            if !worker {
                continue;
            }
            let result = entries.iter().rev().find(|e| {
                e.role == clyra_doc::MessageRole::Assistant
                    && e.status == Some(clyra_doc::MessageStatus::Complete)
            });
            let (turn, text) = match session.status {
                clyra_proto::SessionStatus::Idle => {
                    if sessions.turn_in_flight(&child.id) {
                        continue;
                    }
                    let Some(turn) = session.last_completed_turn.clone() else {
                        continue;
                    };
                    let Some(result) = result else { continue };
                    let text = result
                        .parts
                        .iter()
                        .filter_map(|p| match p {
                            clyra_doc::MessagePart::Text { text, .. } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    (turn, text)
                }
                clyra_proto::SessionStatus::AwaitingInput => {
                    let input = entries
                        .iter()
                        .rev()
                        .flat_map(|e| e.parts.iter())
                        .find_map(|p| match p {
                            clyra_doc::MessagePart::Input {
                                request_id,
                                resolved: false,
                                ..
                            } => Some(request_id),
                            _ => None,
                        });
                    let Some(request) = input else { continue };
                    (format!("input-{request}"), "This worker needs a human response. Ask the user to open its session from Activity and answer the pending request. Do not approve permissions on the user's behalf; this is not a completed task.".into())
                }
                clyra_proto::SessionStatus::Errored => {
                    let error = entries
                        .iter()
                        .rev()
                        .flat_map(|e| e.parts.iter())
                        .find_map(|p| match p {
                            clyra_doc::MessagePart::Error { id, message } => Some((id, message)),
                            _ => None,
                        });
                    let Some((id, error)) = error else { continue };
                    (
                        format!("error-{id}"),
                        format!(
                            "Worker failed: {error}. Review its session and report or recover the failed task; do not treat it as successful."
                        ),
                    )
                }
                _ => continue,
            };
            if lock(&self.inner.store.inner.state)
                .task_reports
                .get(&child.id)
                == Some(&turn)
            {
                continue;
            }
            let text = if text.trim().is_empty() {
                "Worker completed without a textual summary. Read its session and inspect its changes before reporting the outcome.".to_string()
            } else {
                text.chars().take(16_000).collect::<String>()
            };
            self.inner.doc_host.queue_message_with_id(parent, &format!("dot-result-{}-{turn}", child.id), &format!(
                "<!-- clyra-dot-result -->\nAgent update from {} (session {}). Review this update with the other tasks; continue coordinating.\n{}",
                child.title.as_deref().unwrap_or("Agent"), child.id, text), Vec::new(), true)?;
            let mut state = lock(&self.inner.store.inner.state);
            let mut next = state.clone();
            next.task_reports.insert(child.id, turn.clone());
            persist(&self.inner.store.inner.path, &next)?;
            *state = next;
        }
        Ok(())
    }

    async fn fire_due_schedules(&self, now: DateTime<Utc>) -> Result<(), EngineError> {
        for automation in self.inner.store.due(now)? {
            if let Err(err) = self
                .fire(&automation, None, now)
                .await
                .map_err(|err| EngineError::Other(format!("{}: {err}", automation.name)))
            {
                tracing::warn!(automation = %automation.id, error = %err, "scheduled automation did not start");
            }
        }
        Ok(())
    }

    async fn poll_pull_requests(&self, now: DateTime<Utc>) -> Result<(), EngineError> {
        for automation in self.inner.store.pull_request_triggers()? {
            let AutomationTrigger::PullRequest(trigger) = &automation.trigger else {
                continue;
            };
            if !self.pr_poll_due(&automation.project_path) {
                continue;
            }
            let Some(pulls) = self.open_pull_requests(&automation).await else {
                continue;
            };
            let numbers: Vec<u64> = pulls.iter().map(|pr| pr.number).collect();
            let fresh = new_pull_requests(trigger, &automation.seen_pull_requests, &numbers);
            if fresh.is_empty() {
                continue;
            }
            let context = Some(pull_request_context(&automation, &pulls, &fresh, now));
            if let Err(err) = self
                .fire(&automation, context, now)
                .await
                .map_err(|err| EngineError::Other(format!("{}: {err}", automation.name)))
            {
                tracing::warn!(automation = %automation.id, error = %err, "pull-request automation did not start");
                continue;
            }
            // Recorded only after a successful fire: a failed start must be
            // retried on the next poll, not silently written off as seen.
            self.inner
                .store
                .note_pull_requests(&automation.id, &numbers, now)?;
        }
        Ok(())
    }

    fn pr_poll_due(&self, project_path: &str) -> bool {
        let mut last = lock(&self.inner.last_pr_poll);
        match last.get(project_path) {
            Some(at) if at.elapsed() < PR_POLL => false,
            _ => {
                last.insert(project_path.to_string(), Instant::now());
                true
            }
        }
    }

    /// The checkout's open pull requests, or `None` when they cannot be read
    /// (no `gh`, not a GitHub remote, signed out). A trigger that cannot
    /// poll must stay quiet, not fire on nothing.
    async fn open_pull_requests(
        &self,
        automation: &Automation,
    ) -> Option<Vec<PullRequestActivity>> {
        let cwd = std::path::PathBuf::from(&automation.project_path);
        let source = self.inner.resolver.inspect_checkout(&cwd).await.ok()?;
        let pulls = self
            .inner
            .resolver
            .list_github_pull_requests(&source)
            .await
            .ok()?;
        // The event the user asked for: `Updated` only reacts to movement on a
        // PR the automation has already seen, so a fresh PR that nobody asked
        // about stays quiet unless `Opened` is also selected.
        let AutomationTrigger::PullRequest(trigger) = &automation.trigger else {
            return None;
        };
        if !trigger.events.contains(&PullRequestEvent::Updated) {
            return Some(pulls);
        }
        let seen = &automation.seen_pull_requests;
        Some(
            pulls
                .into_iter()
                .filter(|pr| {
                    !seen.contains(&pr.number) || trigger.events.contains(&PullRequestEvent::Opened)
                })
                .collect(),
        )
    }

    /// Start the automation's run: mint the session, name it, queue the
    /// prompt. The run itself is the host's ordinary command drain, so it
    /// shows up in the sidebar, persists, and survives a restart exactly like
    /// a turn the user sent.
    pub async fn fire(
        &self,
        automation: &Automation,
        context: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<String, EngineError> {
        let chat_id = new_id();
        let message_id = new_id();
        let harness = automation.harness.unwrap_or(self.inner.default_harness);
        let prompt = match &context {
            Some(context) => format!("{}\n\n{context}", automation.prompt),
            None => automation.prompt.clone(),
        };
        let prompt = format!("{prompt}\n\n{DOT_COORDINATION}");
        let config = ChatConfig {
            harness,
            model: automation.model.clone(),
            reasoning: None,
            model_options: dot_model_options(),
            sandbox: automation.sandbox,
        };
        let cwd = automation.project_path.clone();
        // The row first: a session that appears named and configured from its
        // first frame, instead of an empty chat the run fills in later.
        self.inner.workspace.create_chat(
            &chat_id,
            Some(&automation.space_id),
            Some(&self.inner.device_id),
            Some(config.clone()),
            Some(cwd.clone()),
        )?;
        let _ = self.inner.workspace.rename_chat(&chat_id, &automation.name);
        self.inner.doc_host.queue_command(
            &chat_id,
            SessionCommandPayload::Run {
                request: RunRequest {
                    prompt,
                    harness: Some(harness),
                    model: automation.model.clone(),
                    reasoning: None,
                    model_options: dot_model_options(),
                    cwd,
                    sandbox: automation.sandbox,
                    auto_approve: false,
                    resume: None,
                    attachments: Vec::new(),
                    worktree: None,
                },
                message_id,
            },
        )?;
        self.inner.store.note_run_started(&automation.id, now)?;
        self.inner.store.mutate(&automation.id, now, |entry| {
            entry.activity_chat_ids.push(chat_id.clone());
            if entry.activity_chat_ids.len() > 100 {
                entry.activity_chat_ids.remove(0);
            }
        })?;
        Ok(chat_id)
    }

    /// Messages continue the same durable session without modifying the
    /// responsibility or replacing the scheduled briefing with a one-off task.
    pub async fn prepare(
        &self,
        id: &str,
        now: DateTime<Utc>,
    ) -> Result<AutomationRunAck, EngineError> {
        self.conversation(id, "", now).await
    }

    pub async fn message(
        &self,
        id: &str,
        message: &str,
        now: DateTime<Utc>,
    ) -> Result<AutomationRunAck, EngineError> {
        if message.trim().is_empty() {
            return Err(EngineError::Other("Message must not be empty".into()));
        }
        self.conversation(id, message, now).await
    }

    async fn conversation(
        &self,
        id: &str,
        message: &str,
        now: DateTime<Utc>,
    ) -> Result<AutomationRunAck, EngineError> {
        let _guard = self.inner.message_lock.lock().await;
        let message = message.trim();
        if message.len() > MAX_AUTOMATION_PROMPT_BYTES {
            return Err(EngineError::Other(
                "Message must contain 1–65536 bytes".into(),
            ));
        }
        let automation = self
            .inner
            .store
            .get(id)?
            .ok_or_else(|| EngineError::Other("Automation not found".into()))?;
        let existing = automation
            .conversation_chat_id
            .as_ref()
            .filter(|id| self.inner.workspace.chat(id).ok().flatten().is_some());
        let chat_id = existing.cloned().unwrap_or_else(new_id);
        let harness = automation.harness.unwrap_or(self.inner.default_harness);
        if existing.is_none() {
            self.inner.workspace.create_chat(
                &chat_id,
                Some(&automation.space_id),
                Some(&self.inner.device_id),
                Some(ChatConfig {
                    harness,
                    model: automation.model.clone(),
                    reasoning: None,
                    model_options: dot_model_options(),
                    sandbox: automation.sandbox,
                }),
                Some(automation.project_path.clone()),
            )?;
            self.inner
                .workspace
                .rename_chat(&chat_id, &automation.name)?;
            self.inner.store.mutate(id, now, |entry| {
                entry.conversation_chat_id = Some(chat_id.clone())
            })?;
        }
        self.inner.workspace.set_chat_config(
            &chat_id,
            &ChatConfig {
                harness,
                model: automation.model,
                reasoning: None,
                model_options: dot_model_options(),
                sandbox: automation.sandbox,
            },
        )?;
        if message.is_empty() {
            return Ok(AutomationRunAck { chat_id });
        }
        self.inner.doc_host.queue_message(
            &chat_id,
            &format!(
                "<!-- clyra-dot-context -->\n{}\n\n<!-- clyra-dot-message -->\n{message}",
                format!("{}\n\n{}", automation.prompt, DOT_COORDINATION)
            ),
            Vec::new(),
        )?;
        Ok(AutomationRunAck { chat_id })
    }

    /// "Run now" from the list: fires a disabled automation too (the toggle
    /// governs the schedule, not the user's explicit press) and still stamps
    /// the run, so the schedule does not immediately double-fire.
    pub async fn run_now(
        &self,
        id: &str,
        now: DateTime<Utc>,
    ) -> Result<AutomationRunAck, EngineError> {
        let Some(automation) = self.inner.store.get(id)? else {
            return Err(EngineError::Other("Automation not found".into()));
        };
        let chat_id = self
            .fire(&automation, Some(MANUAL_CONTEXT.to_string()), now)
            .await?;
        Ok(AutomationRunAck { chat_id })
    }
}

pub const DOT_COORDINATION: &str = "You are the user's dot coordinator. Use clyra MCP delegate_task automatically when a request contains independent tasks that benefit from parallel work. Give each worker a clear scope, context and acceptance criteria; assign disjoint files because workers share the project checkout. Use at most four concurrent workers. Keep simple tasks in this conversation. Track workers with list_chats/get_chat/read_chat and send_message. Worker final results are queued back here automatically and reactivate you. Review their work and synthesize one answer for the user. Do not claim a worker exists until the tool succeeds. AwaitingInput is not completion: direct the user to its session for permissions. Inherit permissions and do not bypass approval controls.";

fn dot_model_options() -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::from_iter([("_clyra_dot".into(), serde_json::Value::Bool(true))])
}

const MANUAL_CONTEXT: &str = "Run manually from the Automations list.";

/// The block appended to the prompt for an event trigger: which pull requests,
/// and what about them is new. The agent gets the facts, not a URL to guess at.
fn pull_request_context(
    automation: &Automation,
    pulls: &[PullRequestActivity],
    fresh: &[u64],
    _now: DateTime<Utc>,
) -> String {
    let mut lines = vec![format!(
        "Open pull requests in {}:",
        automation.project_label
    )];
    for pr in pulls {
        let marker = if fresh.contains(&pr.number) {
            "NEW"
        } else {
            "existing"
        };
        lines.push(format!(
            "- #{number} ({marker}): {title} — {url}",
            number = pr.number,
            title = pr.title,
            url = pr.url
        ));
    }
    lines.join("\n")
}

/// Weak handles so dropping the service tears the loop down; the token ends it
/// eagerly on shutdown. Same supervisor shape as `SpacesSync`.
async fn automation_task(weak: Weak<AutomationsInner>) {
    let mut tick = tokio::time::interval(TICK);
    // Delay, not catch-up: a laptop that slept for an hour must not fire every
    // hourly slot it slept through on wake.
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tick.tick().await; // consume tokio's immediate first tick
    let Some(inner) = weak.upgrade() else { return };
    let cancel = inner.cancel.clone();
    let automations = Automations { inner };
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tick.tick() => {
                if weak.upgrade().is_none() {
                    break;
                }
                automations.tick(Utc::now()).await;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn empty_file() -> AutomationsFile {
    AutomationsFile {
        version: STORE_VERSION,
        automations: BTreeMap::new(),
        task_reports: BTreeMap::new(),
    }
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// Validate and bound a draft. Everything that can wedge the scheduler is
/// rejected here rather than at fire time.
fn normalize_draft(mut draft: AutomationDraft) -> Result<AutomationDraft, EngineError> {
    draft.name = draft.name.trim().to_string();
    if draft.name.is_empty() {
        return Err(EngineError::Other("An automation needs a name".into()));
    }
    if draft.name.chars().count() > MAX_AUTOMATION_NAME_CHARS {
        return Err(EngineError::Other(format!(
            "Name is longer than {MAX_AUTOMATION_NAME_CHARS} characters"
        )));
    }
    draft.prompt = draft.prompt.trim().to_string();
    if draft.prompt.is_empty() {
        return Err(EngineError::Other("An automation needs a prompt".into()));
    }
    if draft.prompt.len() > MAX_AUTOMATION_PROMPT_BYTES {
        return Err(EngineError::Other("Prompt is too long".into()));
    }
    if draft.space_id.trim().is_empty() {
        return Err(EngineError::Other("An automation needs a project".into()));
    }
    if !draft.id.is_empty() && draft.id.len() > MAX_AUTOMATION_ID_BYTES {
        return Err(EngineError::Other("Automation id is too long".into()));
    }
    if let AutomationTrigger::PullRequest(trigger) = &mut draft.trigger
        && trigger.events.is_empty()
    {
        return Err(EngineError::Other(
            "Choose at least one pull-request event".into(),
        ));
    }
    draft.model = draft.model.clone().filter(|m| !m.trim().is_empty());
    Ok(draft)
}

/// The prompt an event trigger runs with, or `None` when nothing is new.
/// Pure: the caller passes the PRs the poll found and the numbers already
/// recorded on the automation.
pub fn new_pull_requests(
    trigger: &clyra_proto::PullRequestTrigger,
    seen: &[u64],
    numbers: &[u64],
) -> Vec<u64> {
    let _ = trigger;
    numbers
        .iter()
        .copied()
        .filter(|number| !seen.contains(number))
        .collect()
}

fn persist(path: &Path, file: &AutomationsFile) -> Result<(), EngineError> {
    let mut bytes = serde_json::to_vec_pretty(file)
        .map_err(|err| EngineError::Other(format!("serialize automations: {err}")))?;
    bytes.push(b'\n');
    let parent = path
        .parent()
        .ok_or_else(|| EngineError::Other("Automations store has no parent".into()))?;
    std::fs::create_dir_all(parent)?;
    // A per-write temp name: two automations saved in the same tick must not
    // race on one another's scratch file.
    let temp_path = parent.join(format!(".{STORE_FILE}.tmp-{}", Uuid::new_v4()));
    let result = (|| -> Result<(), EngineError> {
        let mut temp = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)?;
        temp.write_all(&bytes)?;
        temp.sync_all()?;
        std::fs::rename(&temp_path, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use clyra_proto::SandboxLevel;

    fn schedule(every: AutomationCadence, hour: u8, minute: u8, weekday: u8) -> AutomationSchedule {
        AutomationSchedule {
            every,
            hour,
            minute,
            weekday,
        }
    }

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn hourly_lands_on_the_next_hour_not_this_one() {
        let spec = schedule(AutomationCadence::Hourly, 0, 0, 0);
        assert_eq!(
            next_fire_after(&spec, at(2026, 9, 26, 12, 30)),
            Some(at(2026, 9, 26, 13, 0))
        );
        // Exactly on the hour: still the NEXT one, never a double fire.
        assert_eq!(
            next_fire_after(&spec, at(2026, 9, 26, 12, 0)),
            Some(at(2026, 9, 26, 13, 0))
        );
    }

    #[test]
    fn daily_waits_for_tomorrows_clock_when_todays_has_passed() {
        let spec = schedule(AutomationCadence::Daily, 9, 30, 0);
        assert_eq!(
            next_fire_after(&spec, at(2026, 9, 26, 8, 0)),
            Some(at(2026, 9, 26, 9, 30))
        );
        assert_eq!(
            next_fire_after(&spec, at(2026, 9, 26, 10, 0)),
            Some(at(2026, 9, 27, 9, 30))
        );
    }

    #[test]
    fn weekdays_skip_the_weekend() {
        let spec = schedule(AutomationCadence::Weekdays, 9, 0, 0);
        // 2026-09-26 is a Saturday: the next weekday fire is Monday.
        assert_eq!(
            next_fire_after(&spec, at(2026, 9, 26, 12, 0)),
            Some(at(2026, 9, 28, 9, 0))
        );
        // Friday after the hour → Monday.
        assert_eq!(
            next_fire_after(&spec, at(2026, 9, 25, 10, 0)),
            Some(at(2026, 9, 28, 9, 0))
        );
    }

    #[test]
    fn weekly_pins_the_named_iso_weekday() {
        let spec = schedule(AutomationCadence::Weekly, 10, 0, 1); // Monday
        assert_eq!(
            next_fire_after(&spec, at(2026, 9, 26, 12, 0)),
            Some(at(2026, 9, 28, 10, 0))
        );
        // Out-of-range weekday falls back to Monday instead of never firing.
        let broken = schedule(AutomationCadence::Weekly, 10, 0, 0);
        assert_eq!(
            next_fire_after(&broken, at(2026, 9, 26, 12, 0)),
            Some(at(2026, 9, 28, 10, 0))
        );
    }

    #[test]
    fn out_of_range_fields_are_clamped_not_rejected() {
        let spec = schedule(AutomationCadence::Daily, 99, 99, 0);
        assert_eq!(
            next_fire_after(&spec, at(2026, 9, 26, 0, 0)),
            Some(at(2026, 9, 26, 23, 59))
        );
    }

    fn draft(name: &str) -> AutomationDraft {
        AutomationDraft {
            id: String::new(),
            name: name.into(),
            space_id: "space-1".into(),
            trigger: AutomationTrigger::Schedule(schedule(AutomationCadence::Daily, 9, 0, 0)),
            prompt: "Review the diff".into(),
            harness: None,
            model: None,
            sandbox: SandboxLevel::WorkspaceWrite,
            enabled: true,
        }
    }

    fn store() -> (tempfile::TempDir, AutomationsStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = AutomationsStore::open(dir.path()).unwrap();
        (dir, store)
    }

    #[test]
    fn a_created_schedule_arms_its_first_fire() {
        let (_dir, store) = store();
        let now = at(2026, 9, 26, 12, 0);
        let created = store
            .upsert(draft("Nightly"), "Repo".into(), "/repo".into(), now)
            .unwrap();
        assert_eq!(created.next_run_at, Some(at(2026, 9, 27, 9, 0)));
        // Due-ness is what the scheduler polls on.
        assert!(store.due(now).unwrap().is_empty());
        assert_eq!(store.due(at(2026, 9, 27, 9, 0)).unwrap().len(), 1);
    }

    #[test]
    fn conversation_and_activity_survive_edits_and_restart_without_rearming() {
        let (dir, store) = store();
        let now = at(2026, 9, 26, 12, 0);
        let created = store
            .upsert(draft("Dot"), "Repo".into(), "/repo".into(), now)
            .unwrap();
        store
            .mutate(&created.id, now, |a| {
                a.conversation_chat_id = Some("conversation".into());
                a.activity_chat_ids = vec!["task-1".into(), "task-2".into()];
            })
            .unwrap();
        let mut edit = draft("Renamed");
        edit.id = created.id.clone();
        store
            .upsert(edit, "Repo".into(), "/repo".into(), now)
            .unwrap();
        let reopened = AutomationsStore::open(dir.path())
            .unwrap()
            .get(&created.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            reopened.conversation_chat_id.as_deref(),
            Some("conversation")
        );
        assert_eq!(reopened.activity_chat_ids, ["task-1", "task-2"]);
        assert_eq!(reopened.next_run_at, created.next_run_at);
        assert_eq!(reopened.last_run_at, None);
    }

    #[test]
    fn editing_preserves_run_bookkeeping_and_rearms_only_on_a_schedule_change() {
        let (_dir, store) = store();
        let now = at(2026, 9, 26, 12, 0);
        let mut created = store
            .upsert(draft("Nightly"), "Repo".into(), "/repo".into(), now)
            .unwrap();
        store
            .note_run_started(&created.id, at(2026, 9, 27, 9, 0))
            .unwrap();

        // A rename keeps the pending fire and the last-run stamp.
        let mut rename = draft("Nightly review");
        rename.id = created.id.clone();
        let renamed = store
            .upsert(
                rename,
                "Repo".into(),
                "/repo".into(),
                at(2026, 9, 27, 10, 0),
            )
            .unwrap();
        assert_eq!(renamed.last_run_at, Some(at(2026, 9, 27, 9, 0)));
        assert_eq!(renamed.next_run_at, Some(at(2026, 9, 28, 9, 0)));
        assert_eq!(renamed.created_at, created.created_at);

        // A schedule change re-arms from the edit.
        let mut reschedule = draft("Nightly review");
        reschedule.id = created.id.clone();
        reschedule.trigger =
            AutomationTrigger::Schedule(schedule(AutomationCadence::Hourly, 0, 0, 0));
        let moved = store
            .upsert(
                reschedule,
                "Repo".into(),
                "/repo".into(),
                at(2026, 9, 27, 10, 15),
            )
            .unwrap();
        assert_eq!(moved.next_run_at, Some(at(2026, 9, 27, 11, 0)));
    }

    #[test]
    fn disabling_is_a_kill_switch_that_does_not_lose_the_slot() {
        let (_dir, store) = store();
        let now = at(2026, 9, 26, 12, 0);
        let created = store
            .upsert(draft("Nightly"), "Repo".into(), "/repo".into(), now)
            .unwrap();
        let off = store.set_enabled(&created.id, false, now).unwrap().unwrap();
        assert!(!off.enabled);
        assert_eq!(off.next_run_at, None);
        assert!(store.due(at(2027, 1, 1, 0, 0)).unwrap().is_empty());

        // Re-enabling arms from NOW — a week disabled must not fire 168 times.
        let later = at(2026, 10, 3, 12, 0);
        let on = store
            .set_enabled(&created.id, true, later)
            .unwrap()
            .unwrap();
        assert_eq!(on.next_run_at, Some(at(2026, 10, 4, 9, 0)));
    }

    #[test]
    fn seen_pull_requests_are_deduped_and_bounded() {
        let (_dir, store) = store();
        let now = at(2026, 9, 26, 12, 0);
        let mut pr = draft("PR review");
        pr.trigger = AutomationTrigger::PullRequest(clyra_proto::PullRequestTrigger {
            repo: String::new(),
            events: vec![clyra_proto::PullRequestEvent::Opened],
        });
        let created = store
            .upsert(pr, "Repo".into(), "/repo".into(), now)
            .unwrap();
        store
            .note_pull_requests(&created.id, &[7, 7, 3], now)
            .unwrap();
        let after = store.get(&created.id).unwrap().unwrap();
        assert_eq!(after.seen_pull_requests, vec![3, 7]);
        assert!(
            after.next_run_at.is_none(),
            "event triggers never arm a slot"
        );

        let many: Vec<u64> = (1..=(MAX_SEEN_PULL_REQUESTS as u64 + 20)).collect();
        store.note_pull_requests(&created.id, &many, now).unwrap();
        let capped = store.get(&created.id).unwrap().unwrap();
        assert_eq!(capped.seen_pull_requests.len(), MAX_SEEN_PULL_REQUESTS);
        assert_eq!(
            *capped.seen_pull_requests.last().unwrap(),
            MAX_SEEN_PULL_REQUESTS as u64 + 20
        );
    }

    #[test]
    fn only_genuinely_new_pull_requests_fire() {
        let trigger = clyra_proto::PullRequestTrigger {
            repo: String::new(),
            events: vec![clyra_proto::PullRequestEvent::Opened],
        };
        assert_eq!(new_pull_requests(&trigger, &[3], &[3, 4, 5]), vec![4, 5]);
        assert!(new_pull_requests(&trigger, &[3, 4], &[3, 4]).is_empty());
    }

    #[test]
    fn drafts_are_validated_before_they_can_wedge_the_scheduler() {
        let (_dir, store) = store();
        let now = at(2026, 9, 26, 12, 0);
        let mut blank = draft("  ");
        assert!(
            store
                .upsert(blank.clone(), "R".into(), "/r".into(), now)
                .is_err()
        );
        blank.name = "Fine".into();
        blank.prompt = "   ".into();
        assert!(
            store
                .upsert(blank.clone(), "R".into(), "/r".into(), now)
                .is_err()
        );
        blank.prompt = "Do it".into();
        blank.space_id = String::new();
        assert!(
            store
                .upsert(blank.clone(), "R".into(), "/r".into(), now)
                .is_err()
        );
        blank.space_id = "space-1".into();
        blank.trigger = AutomationTrigger::PullRequest(clyra_proto::PullRequestTrigger {
            repo: String::new(),
            events: Vec::new(),
        });
        assert!(
            store.upsert(blank, "R".into(), "/r".into(), now).is_err(),
            "a pull-request trigger with no events would poll forever and never fire"
        );
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn the_store_round_trips_and_survives_a_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = AutomationsStore::open(dir.path()).unwrap();
        let now = at(2026, 9, 26, 12, 0);
        let created = store
            .upsert(draft("Nightly"), "Repo".into(), "/repo".into(), now)
            .unwrap();
        drop(store);

        let reopened = AutomationsStore::open(dir.path()).unwrap();
        assert_eq!(reopened.list().unwrap(), vec![created.clone()]);
        assert!(reopened.delete(&created.id).unwrap());
        assert!(reopened.list().unwrap().is_empty());
        assert!(
            !reopened.delete(&created.id).unwrap(),
            "delete is idempotent"
        );

        std::fs::write(dir.path().join(STORE_FILE), b"{ not json").unwrap();
        let recovered = AutomationsStore::open(dir.path()).unwrap();
        assert!(recovered.list().unwrap().is_empty());
    }

    #[test]
    fn edits_to_an_unknown_automation_fail_loudly() {
        let (_dir, store) = store();
        let now = at(2026, 9, 26, 12, 0);
        let mut orphan = draft("Ghost");
        orphan.id = "does-not-exist".into();
        assert!(store.upsert(orphan, "R".into(), "/r".into(), now).is_err());
        assert!(
            store
                .set_enabled("does-not-exist", true, now)
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .note_run_started("does-not-exist", now)
                .unwrap()
                .is_none()
        );
    }
}
