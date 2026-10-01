//! Tests for the derived history views: periods, outages, monitoring gaps,
//! and the outage log text.

use chrono::{DateTime, Duration, TimeZone, Utc};
use net_monitor::history::{
    format_duration, format_utc, monitoring_gaps, outages, periods, EngineRun, MonitoringGap,
    OutageLog, OutageLogEntry, Period, PeriodKind,
};
use net_monitor::models::{NodeStatus, StatusChange};

fn t(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 1, hour, minute, 0).unwrap()
}

fn change(
    from: NodeStatus,
    to: NodeStatus,
    at: DateTime<Utc>,
    duration: Option<Duration>,
    last_success_at: Option<DateTime<Utc>>,
) -> StatusChange {
    StatusChange {
        id: None,
        node_id: 7,
        from_status: from,
        to_status: to,
        changed_at: at,
        duration_ms: duration.map(|d| d.num_milliseconds()),
        last_success_at,
    }
}

/// Up from 08:00, first failure 10:00, confirmed 10:00:30, back 11:12.
fn one_outage() -> Vec<StatusChange> {
    vec![
        change(
            NodeStatus::Online,
            NodeStatus::Degraded,
            t(10, 0),
            Some(Duration::hours(2)),
            Some(t(9, 59)),
        ),
        change(
            NodeStatus::Degraded,
            NodeStatus::Offline,
            t(10, 0) + Duration::seconds(30),
            Some(Duration::seconds(30)),
            Some(t(9, 59)),
        ),
        change(
            NodeStatus::Offline,
            NodeStatus::Online,
            t(11, 12),
            Some(Duration::minutes(71) + Duration::seconds(30)),
            Some(t(11, 12)),
        ),
    ]
}

// ========== Formatting ==========

#[test]
fn test_format_utc_is_explicit_about_the_zone() {
    assert_eq!(format_utc(t(10, 5)), "2026-10-01 10:05:00Z");
}

#[test]
fn test_format_duration() {
    assert_eq!(format_duration(0), "0s");
    assert_eq!(format_duration(90_000), "1m 30s");
    assert_eq!(format_duration(5_400_000), "1h 30m");
    assert_eq!(format_duration(90_000_000), "1d 1h");
}

// ========== Periods ==========

#[test]
fn test_periods_pair_each_state_with_its_own_duration() {
    let now = t(12, 0);
    let periods = periods(&one_outage(), NodeStatus::Online, None, &[]);

    assert_eq!(
        periods,
        vec![
            Period {
                kind: PeriodKind::Status(NodeStatus::Online),
                started_at: Some(t(8, 0)),
                ended_at: Some(t(10, 0)),
            },
            Period {
                kind: PeriodKind::Status(NodeStatus::Degraded),
                started_at: Some(t(10, 0)),
                ended_at: Some(t(10, 0) + Duration::seconds(30)),
            },
            Period {
                kind: PeriodKind::Status(NodeStatus::Offline),
                started_at: Some(t(10, 0) + Duration::seconds(30)),
                ended_at: Some(t(11, 12)),
            },
            Period {
                kind: PeriodKind::Status(NodeStatus::Online),
                started_at: Some(t(11, 12)),
                ended_at: None,
            },
        ]
    );
    // The Down row says how long the node was down, not how long it was up before.
    assert_eq!(
        periods[2].duration_ms(now),
        Some((Duration::minutes(71) + Duration::seconds(30)).num_milliseconds())
    );
    assert_eq!(
        periods[3].duration_ms(now),
        Some(Duration::minutes(48).num_milliseconds())
    );
    assert!(periods[3].is_ongoing());
}

#[test]
fn test_periods_without_changes_use_the_current_status_and_first_check() {
    let periods = periods(&[], NodeStatus::Online, Some(t(9, 0)), &[]);
    assert_eq!(
        periods,
        vec![Period {
            kind: PeriodKind::Status(NodeStatus::Online),
            started_at: Some(t(9, 0)),
            ended_at: None,
        }]
    );
}

#[test]
fn test_periods_first_state_start_is_unknown_without_a_duration() {
    let changes = vec![change(
        NodeStatus::Online,
        NodeStatus::Offline,
        t(10, 0),
        None,
        None,
    )];
    let periods = periods(&changes, NodeStatus::Offline, None, &[]);
    assert_eq!(periods[0].started_at, None);
    assert_eq!(periods[0].duration_ms(t(12, 0)), None);
    assert_eq!(periods[0].ended_at, Some(t(10, 0)));
}

#[test]
fn test_periods_are_split_around_monitoring_gaps() {
    let changes = vec![change(
        NodeStatus::Online,
        NodeStatus::Offline,
        t(10, 0),
        Some(Duration::hours(4)),
        Some(t(5, 0)),
    )];
    let gaps = [
        MonitoringGap {
            from: t(7, 0),
            to: t(8, 0),
        },
        MonitoringGap {
            from: t(11, 0),
            to: t(11, 30),
        },
    ];
    let periods = periods(&changes, NodeStatus::Offline, None, &gaps);

    type Row = (PeriodKind, Option<DateTime<Utc>>, Option<DateTime<Utc>>);
    let kinds: Vec<Row> = periods
        .iter()
        .map(|p| (p.kind, p.started_at, p.ended_at))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (
                PeriodKind::Status(NodeStatus::Online),
                Some(t(6, 0)),
                Some(t(7, 0))
            ),
            (PeriodKind::Unmonitored, Some(t(7, 0)), Some(t(8, 0))),
            (
                PeriodKind::Status(NodeStatus::Online),
                Some(t(8, 0)),
                Some(t(10, 0))
            ),
            (
                PeriodKind::Status(NodeStatus::Offline),
                Some(t(10, 0)),
                Some(t(11, 0))
            ),
            (PeriodKind::Unmonitored, Some(t(11, 0)), Some(t(11, 30))),
            (
                PeriodKind::Status(NodeStatus::Offline),
                Some(t(11, 30)),
                None
            ),
        ]
    );
}

#[test]
fn test_periods_ignore_gaps_outside_the_timeline() {
    let changes = one_outage();
    let gaps = [
        MonitoringGap {
            from: t(1, 0),
            to: t(2, 0),
        },
        MonitoringGap {
            from: t(7, 0),
            to: t(8, 0),
        },
    ];
    let periods = periods(&changes, NodeStatus::Online, None, &gaps);
    assert!(
        periods.iter().all(|p| p.kind != PeriodKind::Unmonitored),
        "{:?}",
        periods
    );
    assert_eq!(periods.len(), 4);
}

// ========== Outages ==========

#[test]
fn test_outages_span_from_first_failure_to_recovery() {
    let found = outages(&one_outage());
    assert_eq!(found.len(), 1);
    let outage = found[0];
    assert_eq!(outage.node_id, 7);
    assert_eq!(outage.started_at, t(10, 0));
    assert_eq!(outage.confirmed_at, t(10, 0) + Duration::seconds(30));
    assert_eq!(outage.ended_at, Some(t(11, 12)));
    assert_eq!(outage.last_success_at, Some(t(9, 59)));
    assert_eq!(
        outage.duration_ms(t(23, 0)),
        Duration::minutes(72).num_milliseconds()
    );
    assert!(!outage.is_ongoing());
}

#[test]
fn test_degraded_blip_is_not_an_outage() {
    let changes = vec![
        change(
            NodeStatus::Online,
            NodeStatus::Degraded,
            t(10, 0),
            Some(Duration::hours(1)),
            Some(t(9, 59)),
        ),
        change(
            NodeStatus::Degraded,
            NodeStatus::Online,
            t(10, 0) + Duration::seconds(15),
            Some(Duration::seconds(15)),
            Some(t(10, 0) + Duration::seconds(15)),
        ),
    ];
    assert!(outages(&changes).is_empty());
}

#[test]
fn test_ongoing_outage_has_no_end() {
    let changes = one_outage()[..2].to_vec();
    let found = outages(&changes);
    assert_eq!(found.len(), 1);
    assert!(found[0].is_ongoing());
    assert_eq!(found[0].started_at, t(10, 0));
    assert_eq!(
        found[0].duration_ms(t(12, 0)),
        Duration::hours(2).num_milliseconds()
    );
}

#[test]
fn test_unconfirmed_degraded_at_end_is_not_an_outage_yet() {
    let changes = one_outage()[..1].to_vec();
    assert!(outages(&changes).is_empty());
}

#[test]
fn test_outage_straight_to_offline_is_confirmed_at_its_start() {
    let changes = vec![
        change(
            NodeStatus::Online,
            NodeStatus::Offline,
            t(10, 0),
            Some(Duration::hours(1)),
            Some(t(9, 55)),
        ),
        change(
            NodeStatus::Offline,
            NodeStatus::Online,
            t(10, 30),
            Some(Duration::minutes(30)),
            Some(t(10, 30)),
        ),
    ];
    let found = outages(&changes);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].started_at, t(10, 0));
    assert_eq!(found[0].confirmed_at, t(10, 0));
    assert_eq!(found[0].last_success_at, Some(t(9, 55)));
}

#[test]
fn test_legacy_placeholder_recovery_is_ignored() {
    // Older versions recorded Offline → Online when a new node's first check
    // succeeded. That is not a recovery from anything.
    let changes = vec![change(
        NodeStatus::Offline,
        NodeStatus::Online,
        t(10, 0),
        None,
        None,
    )];
    assert!(outages(&changes).is_empty());
}

#[test]
fn test_several_outages_in_order() {
    let mut changes = one_outage();
    changes.extend(vec![
        change(
            NodeStatus::Online,
            NodeStatus::Degraded,
            t(14, 0),
            Some(Duration::minutes(168)),
            Some(t(13, 59)),
        ),
        change(
            NodeStatus::Degraded,
            NodeStatus::Offline,
            t(14, 1),
            Some(Duration::minutes(1)),
            Some(t(13, 59)),
        ),
        change(
            NodeStatus::Offline,
            NodeStatus::Online,
            t(14, 20),
            Some(Duration::minutes(19)),
            Some(t(14, 20)),
        ),
    ]);
    let found = outages(&changes);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].started_at, t(10, 0));
    assert_eq!(found[1].started_at, t(14, 0));
    assert_eq!(found[1].ended_at, Some(t(14, 20)));
}

// ========== Monitoring gaps ==========

fn run(id: i64, started: DateTime<Utc>, alive: DateTime<Utc>) -> EngineRun {
    EngineRun {
        id,
        started_at: started,
        last_alive_at: alive,
    }
}

#[test]
fn test_gaps_lie_between_runs() {
    let runs = vec![
        run(1, t(1, 0), t(3, 0)),
        run(2, t(5, 0), t(9, 0)),
        run(3, t(9, 0), t(10, 0)),
    ];
    let gaps = monitoring_gaps(&runs, t(10, 0), true);
    assert_eq!(
        gaps,
        vec![MonitoringGap {
            from: t(3, 0),
            to: t(5, 0),
        }]
    );
}

#[test]
fn test_gaps_are_found_whatever_order_runs_arrive_in() {
    let runs = vec![run(2, t(5, 0), t(9, 0)), run(1, t(1, 0), t(3, 0))];
    let gaps = monitoring_gaps(&runs, t(9, 0), true);
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].from, t(3, 0));
}

#[test]
fn test_stopped_monitoring_is_a_gap_up_to_now() {
    let runs = vec![run(1, t(1, 0), t(3, 0))];
    let gaps = monitoring_gaps(&runs, t(4, 0), false);
    assert_eq!(
        gaps,
        vec![MonitoringGap {
            from: t(3, 0),
            to: t(4, 0),
        }]
    );
    assert_eq!(gaps[0].duration_ms(), Duration::hours(1).num_milliseconds());
}

#[test]
fn test_running_monitoring_has_no_tail_gap_and_no_runs_means_no_gaps() {
    let runs = vec![run(1, t(1, 0), t(3, 0))];
    assert!(monitoring_gaps(&runs, t(4, 0), true).is_empty());
    assert!(monitoring_gaps(&[], t(4, 0), false).is_empty());
}

// ========== Outage log ==========

fn entry(name: &str, changes: &[StatusChange]) -> Vec<OutageLogEntry> {
    outages(changes)
        .into_iter()
        .map(|outage| OutageLogEntry {
            node_name: name.to_string(),
            outage,
        })
        .collect()
}

#[test]
fn test_outage_log_keeps_only_outages_touching_the_window_newest_first() {
    let mut entries = entry("core-router", &one_outage());
    let old = vec![
        change(
            NodeStatus::Online,
            NodeStatus::Offline,
            t(1, 0),
            None,
            Some(t(0, 59)),
        ),
        change(
            NodeStatus::Offline,
            NodeStatus::Online,
            t(2, 0),
            Some(Duration::hours(1)),
            Some(t(2, 0)),
        ),
    ];
    entries.extend(entry("old-switch", &old));
    let ongoing = one_outage()[..2].to_vec();
    entries.extend(entry("file-server", &ongoing));

    let log = OutageLog::new(t(4, 0), t(12, 0), entries, &[]);

    let names: Vec<&str> = log.entries.iter().map(|e| e.node_name.as_str()).collect();
    assert_eq!(names, vec!["core-router", "file-server"]);
    assert_eq!(log.ongoing_count(), 1);
    assert_eq!(
        log.total_outage_ms(),
        (Duration::minutes(72) + Duration::hours(2)).num_milliseconds()
    );
}

#[test]
fn test_outage_log_clips_gaps_to_the_window() {
    let gaps = [
        MonitoringGap {
            from: t(1, 0),
            to: t(2, 0),
        },
        MonitoringGap {
            from: t(3, 0),
            to: t(5, 0),
        },
        MonitoringGap {
            from: t(11, 0),
            to: t(11, 30),
        },
    ];
    let log = OutageLog::new(t(4, 0), t(12, 0), Vec::new(), &gaps);
    assert_eq!(
        log.gaps,
        vec![
            MonitoringGap {
                from: t(11, 0),
                to: t(11, 30),
            },
            MonitoringGap {
                from: t(4, 0),
                to: t(5, 0),
            },
        ]
    );
}

#[test]
fn test_outage_log_text_reads_as_a_brief() {
    let entries = entry("core-router", &one_outage());
    let gaps = [MonitoringGap {
        from: t(6, 0),
        to: t(6, 30),
    }];
    let log = OutageLog::new(t(4, 0), t(12, 0), entries, &gaps);
    let text = log.to_text();

    assert!(text.starts_with("OUTAGE LOG  2026-10-01 04:00:00Z  to  2026-10-01 12:00:00Z"));
    assert!(text.contains("1 outage(s), 0 ongoing, 1h 12m total down time"));
    assert!(text.contains("NODE         LAST UP               DOWN                  CONFIRMED             RESTORED              DURATION"));
    assert!(text.contains(
        "core-router  2026-10-01 09:59:00Z  2026-10-01 10:00:00Z  2026-10-01 10:00:30Z  2026-10-01 11:12:00Z  1h 12m"
    ));
    assert!(text.contains("Not monitored"));
    assert!(text.contains("  2026-10-01 06:00:00Z  to  2026-10-01 06:30:00Z  (30m 0s)"));
}

#[test]
fn test_outage_log_text_with_nothing_to_report() {
    let log = OutageLog::new(t(4, 0), t(12, 0), Vec::new(), &[]);
    let text = log.to_text();
    assert!(text.contains("No outages."));
    assert!(!text.contains("Not monitored"));
}

#[test]
fn test_outage_log_text_marks_ongoing_outages() {
    let entries = entry("file-server", &one_outage()[..2]);
    let log = OutageLog::new(t(4, 0), t(12, 0), entries, &[]);
    let text = log.to_text();
    assert!(text.contains("ongoing               2h 0m"), "{}", text);
    assert!(text.contains("1 outage(s), 1 ongoing"));
}
