//! Derived views of the status history.
//!
//! The database stores one `StatusChange` per transition. That is the right
//! thing to store, but it is the wrong thing to show someone who needs to
//! brief an outage: they want to know when a node went down, how long it was
//! down, and when it came back. This module turns the raw transitions into
//! three such views, all as pure functions over data the caller has already
//! loaded:
//!
//! - [`periods`]: the timeline of a node as a list of states, each with when it
//!   began and ended. The periods during which the monitor was not running are
//!   marked as such rather than silently counted as whatever state came before.
//! - [`outages`]: every confirmed outage of a node. An outage starts at the
//!   first failed check, is confirmed when the node reaches Offline, and ends
//!   at the first successful check. A Degraded blip that recovers before
//!   confirmation is not an outage.
//! - [`monitoring_gaps`]: the spans during which no monitoring engine was
//!   running, derived from the engine's start and heartbeat records.
//!
//! Every function takes status changes in ascending order of `changed_at`.

use crate::models::{NodeStatus, StatusChange};
use chrono::{DateTime, Duration, Utc};

/// How a timestamp is written everywhere the history is shown or exported:
/// UTC, to the second, with an explicit `Z`.
pub fn format_utc(time: DateTime<Utc>) -> String {
    time.format("%Y-%m-%d %H:%M:%SZ").to_string()
}

/// Formats a duration in milliseconds for display: `5s`, `2m 30s`, `1h 30m`, `2d 1h`.
pub fn format_duration(duration_ms: i64) -> String {
    let seconds = duration_ms / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        format!("{}d {}h", days, hours % 24)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes % 60)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds % 60)
    } else {
        format!("{}s", seconds)
    }
}

// ---------------------------------------------------------------------------
// Monitoring gaps
// ---------------------------------------------------------------------------

/// One run of the monitoring engine: when it started and when it last
/// reported itself alive. A run that ended cleanly has its final heartbeat
/// at the moment it stopped; a run that crashed has one a few seconds older.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineRun {
    pub id: i64,
    pub started_at: DateTime<Utc>,
    pub last_alive_at: DateTime<Utc>,
}

/// A span during which no monitoring engine was running. Nothing that
/// happened to a node inside a gap was observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitoringGap {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

impl MonitoringGap {
    pub fn duration_ms(&self) -> i64 {
        (self.to - self.from).num_milliseconds()
    }
}

/// The gaps between engine runs, oldest first.
///
/// A gap runs from one run's last heartbeat to the next run's start. When
/// `monitoring_active` is false the span from the latest heartbeat to `now`
/// is a gap as well. The time before the first run ever is not a gap: the
/// history simply starts there.
pub fn monitoring_gaps(
    runs: &[EngineRun],
    now: DateTime<Utc>,
    monitoring_active: bool,
) -> Vec<MonitoringGap> {
    let mut runs: Vec<&EngineRun> = runs.iter().collect();
    runs.sort_by_key(|run| run.started_at);

    let mut gaps = Vec::new();
    for pair in runs.windows(2) {
        let (previous, next) = (pair[0], pair[1]);
        if next.started_at > previous.last_alive_at {
            gaps.push(MonitoringGap {
                from: previous.last_alive_at,
                to: next.started_at,
            });
        }
    }

    if !monitoring_active {
        if let Some(last) = runs.last() {
            if now > last.last_alive_at {
                gaps.push(MonitoringGap {
                    from: last.last_alive_at,
                    to: now,
                });
            }
        }
    }

    gaps
}

// ---------------------------------------------------------------------------
// Periods
// ---------------------------------------------------------------------------

/// What a node was doing during a [`Period`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodKind {
    Status(NodeStatus),
    /// The monitor was not running, so the node's state is unknown.
    Unmonitored,
}

/// A stretch of time during which a node stayed in one state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub kind: PeriodKind,
    /// None when the history does not record when this state began.
    pub started_at: Option<DateTime<Utc>>,
    /// None while the period is still going on.
    pub ended_at: Option<DateTime<Utc>>,
}

impl Period {
    /// How long the period lasted, or has lasted so far. None when its start
    /// is unknown.
    pub fn duration_ms(&self, now: DateTime<Utc>) -> Option<i64> {
        self.started_at
            .map(|started| (self.ended_at.unwrap_or(now) - started).num_milliseconds())
    }

    pub fn is_ongoing(&self) -> bool {
        self.ended_at.is_none()
    }
}

/// The timeline of one node as periods, oldest first.
///
/// `changes` are the node's status changes in ascending order. `current_status`
/// is only used when there are no changes at all. `first_checked_at` bounds
/// the start of the earliest period when the changes themselves do not. Every
/// period that overlaps a monitoring gap is split around it, with an
/// `Unmonitored` period in between.
pub fn periods(
    changes: &[StatusChange],
    current_status: NodeStatus,
    first_checked_at: Option<DateTime<Utc>>,
    gaps: &[MonitoringGap],
) -> Vec<Period> {
    let mut raw = Vec::with_capacity(changes.len() + 1);

    match changes.first() {
        None => raw.push(Period {
            kind: PeriodKind::Status(current_status),
            started_at: first_checked_at,
            ended_at: None,
        }),
        Some(first) => {
            let started_at = first
                .duration_ms
                .map(|ms| first.changed_at - Duration::milliseconds(ms))
                .or(first_checked_at);
            raw.push(Period {
                kind: PeriodKind::Status(first.from_status),
                started_at,
                ended_at: Some(first.changed_at),
            });
            for (index, change) in changes.iter().enumerate() {
                raw.push(Period {
                    kind: PeriodKind::Status(change.to_status),
                    started_at: Some(change.changed_at),
                    ended_at: changes.get(index + 1).map(|next| next.changed_at),
                });
            }
        }
    }

    let mut sorted_gaps: Vec<MonitoringGap> = gaps.to_vec();
    sorted_gaps.sort_by_key(|gap| gap.from);

    let mut result = Vec::with_capacity(raw.len());
    for period in raw {
        split_around_gaps(period, &sorted_gaps, &mut result);
    }
    result
}

/// Pushes `period` onto `out`, cut into pieces wherever a gap overlaps it.
fn split_around_gaps(period: Period, gaps: &[MonitoringGap], out: &mut Vec<Period>) {
    let mut start = period.started_at;
    let end = period.ended_at;

    for gap in gaps {
        // The part of the gap that lies inside what is left of the period.
        let gap_from = start.map_or(gap.from, |s| gap.from.max(s));
        let gap_to = end.map_or(gap.to, |e| gap.to.min(e));
        if gap_from >= gap_to {
            continue;
        }

        if start.is_none_or(|s| s < gap_from) {
            out.push(Period {
                kind: period.kind,
                started_at: start,
                ended_at: Some(gap_from),
            });
        }
        out.push(Period {
            kind: PeriodKind::Unmonitored,
            started_at: Some(gap_from),
            ended_at: Some(gap_to),
        });
        start = Some(gap_to);
    }

    let remaining_is_empty = matches!((start, end), (Some(s), Some(e)) if s >= e);
    if !remaining_is_empty {
        out.push(Period {
            kind: period.kind,
            started_at: start,
            ended_at: end,
        });
    }
}

// ---------------------------------------------------------------------------
// Outages
// ---------------------------------------------------------------------------

/// One confirmed outage of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outage {
    pub node_id: i64,
    /// The first failed check: when the node left Online.
    pub started_at: DateTime<Utc>,
    /// When the node reached Offline. Same as `started_at` when the node went
    /// straight there (`max_check_attempts` of 1).
    pub confirmed_at: DateTime<Utc>,
    /// The first successful check afterwards. None while the outage goes on.
    pub ended_at: Option<DateTime<Utc>>,
    /// The last check that succeeded before the outage. The node really went
    /// down somewhere between this and `started_at`.
    pub last_success_at: Option<DateTime<Utc>>,
}

impl Outage {
    /// Time from the first failed check to recovery, or to `now` while ongoing.
    pub fn duration_ms(&self, now: DateTime<Utc>) -> i64 {
        (self.ended_at.unwrap_or(now) - self.started_at).num_milliseconds()
    }

    pub fn is_ongoing(&self) -> bool {
        self.ended_at.is_none()
    }

    /// Whether any part of the outage falls at or after `since`.
    pub fn overlaps_window(&self, since: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        self.ended_at.unwrap_or(now) >= since
    }
}

/// A failing run that has not been confirmed as an outage yet.
struct PendingOutage {
    started_at: DateTime<Utc>,
    confirmed_at: Option<DateTime<Utc>>,
    last_success_at: Option<DateTime<Utc>>,
}

/// Every confirmed outage in `changes` (ascending), oldest first. An outage
/// that has not ended is returned with `ended_at` None.
pub fn outages(changes: &[StatusChange]) -> Vec<Outage> {
    let mut result = Vec::new();
    let mut pending: Option<PendingOutage> = None;
    let node_id = changes.first().map(|c| c.node_id).unwrap_or_default();

    for change in changes {
        match change.to_status {
            NodeStatus::Online => {
                if let Some(p) = pending.take() {
                    if let Some(confirmed_at) = p.confirmed_at {
                        result.push(Outage {
                            node_id,
                            started_at: p.started_at,
                            confirmed_at,
                            ended_at: Some(change.changed_at),
                            last_success_at: p.last_success_at,
                        });
                    }
                }
            }
            NodeStatus::Degraded => {
                if pending.is_none() {
                    pending = Some(PendingOutage {
                        started_at: change.changed_at,
                        confirmed_at: None,
                        last_success_at: change.last_success_at,
                    });
                }
            }
            NodeStatus::Offline => match pending.as_mut() {
                Some(p) => {
                    if p.confirmed_at.is_none() {
                        p.confirmed_at = Some(change.changed_at);
                    }
                }
                None => {
                    pending = Some(PendingOutage {
                        started_at: change.changed_at,
                        confirmed_at: Some(change.changed_at),
                        last_success_at: change.last_success_at,
                    });
                }
            },
        }
    }

    if let Some(p) = pending {
        if let Some(confirmed_at) = p.confirmed_at {
            result.push(Outage {
                node_id,
                started_at: p.started_at,
                confirmed_at,
                ended_at: None,
                last_success_at: p.last_success_at,
            });
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Outage log
// ---------------------------------------------------------------------------

/// An outage with the name of the node it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutageLogEntry {
    pub node_name: String,
    pub outage: Outage,
}

/// Every outage across all nodes that touched a time window, newest first,
/// plus the monitoring gaps inside that window. This is the turnover brief.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutageLog {
    pub since: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
    pub entries: Vec<OutageLogEntry>,
    pub gaps: Vec<MonitoringGap>,
}

impl OutageLog {
    /// Builds the log for the window `since..=generated_at`. Outages outside
    /// the window are dropped; gaps are clipped to it.
    pub fn new(
        since: DateTime<Utc>,
        generated_at: DateTime<Utc>,
        mut entries: Vec<OutageLogEntry>,
        gaps: &[MonitoringGap],
    ) -> Self {
        entries.retain(|entry| entry.outage.overlaps_window(since, generated_at));
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.outage.started_at));

        let mut gaps: Vec<MonitoringGap> = gaps
            .iter()
            .filter(|gap| gap.to > since)
            .map(|gap| MonitoringGap {
                from: gap.from.max(since),
                to: gap.to.min(generated_at),
            })
            .filter(|gap| gap.from < gap.to)
            .collect();
        gaps.sort_by_key(|gap| std::cmp::Reverse(gap.from));

        Self {
            since,
            generated_at,
            entries,
            gaps,
        }
    }

    pub fn ongoing_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.outage.is_ongoing())
            .count()
    }

    /// Total time spent in outages inside the window, over all nodes.
    pub fn total_outage_ms(&self) -> i64 {
        self.entries
            .iter()
            .map(|entry| {
                let outage = entry.outage;
                let from = outage.started_at.max(self.since);
                let to = outage.ended_at.unwrap_or(self.generated_at);
                (to - from).num_milliseconds().max(0)
            })
            .sum()
    }

    /// The log as plain text, ready to paste into a turnover report.
    pub fn to_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "OUTAGE LOG  {}  to  {}",
            format_utc(self.since),
            format_utc(self.generated_at)
        ));
        lines.push(format!(
            "Window: {}.  {} outage(s), {} ongoing, {} total down time.",
            format_duration((self.generated_at - self.since).num_milliseconds()),
            self.entries.len(),
            self.ongoing_count(),
            format_duration(self.total_outage_ms())
        ));
        lines.push(
            "All times UTC. DOWN is the first failed check; the node was still up at LAST UP."
                .to_string(),
        );
        lines.push(String::new());

        if self.entries.is_empty() {
            lines.push("No outages.".to_string());
        } else {
            let name_width = self
                .entries
                .iter()
                .map(|entry| entry.node_name.chars().count())
                .max()
                .unwrap_or(4)
                .max(4);
            lines.push(format!(
                "{:<name_width$}  {:<20}  {:<20}  {:<20}  {:<20}  DURATION",
                "NODE", "LAST UP", "DOWN", "CONFIRMED", "RESTORED",
            ));
            for entry in &self.entries {
                let outage = entry.outage;
                lines.push(format!(
                    "{:<name_width$}  {:<20}  {:<20}  {:<20}  {:<20}  {}",
                    entry.node_name,
                    outage.last_success_at.map(format_utc).unwrap_or_default(),
                    format_utc(outage.started_at),
                    format_utc(outage.confirmed_at),
                    outage
                        .ended_at
                        .map(format_utc)
                        .unwrap_or_else(|| "ongoing".to_string()),
                    format_duration(outage.duration_ms(self.generated_at)),
                ));
            }
        }

        if !self.gaps.is_empty() {
            lines.push(String::new());
            lines.push("Not monitored (nothing observed during these spans):".to_string());
            for gap in &self.gaps {
                lines.push(format!(
                    "  {}  to  {}  ({})",
                    format_utc(gap.from),
                    format_utc(gap.to),
                    format_duration(gap.duration_ms())
                ));
            }
        }

        lines.push(String::new());
        lines.join("\n")
    }
}
