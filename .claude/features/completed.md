# Shipped

What the app does today, roughly in the order it arrived. Release-by-release detail is on the GitHub Releases page; from v1.4.11 on the notes are generated from the commit history.

## Monitoring
- HTTP/HTTPS checks against an expected status code, self-signed certificates accepted
- ICMP ping with count and timeout, hostnames accepted, no root needed on Linux/macOS
- TCP port checks with a timeout
- Soft/hard state model: Degraded after one failure, Offline after `max_check_attempts`, faster `retry_interval` while Degraded
- Per-node monitoring interval, editable while monitoring runs
- Failed checks report no latency
- Checks run concurrently as tokio tasks; a slow node never holds up the rest

## History
- Status change log for every transition, stamped with the check's own time, the duration of the previous state, and the last successful check
- History view per node as a timeline: each state with when it began, when it ended, and how long it lasted, all in UTC
- Outage log across all nodes for the last 8h / 12h / 24h / 7d: last good check, first failure, confirmation, restoration, duration; exportable as text for a turnover brief
- Monitoring gaps: each engine run is recorded with a heartbeat, and spans with no engine running show as "Not monitored" rather than as uptime
- One transaction per check, so the node's status and its history can never disagree; a failed write is logged and the transition recorded on the next check
- A node's first check sets its status without recording a transition
- Last status change time survives restarts

## TUI
- Node table with name, target, type, status, latency, uptime/downtime, last check
- Add / edit / delete with confirmation; forms stay open when a save fails
- Reorder mode with persisted `display_order`
- Import & Skip Conflicts / Clear & Import All, with the file validated before anything is deleted
- Export to JSON
- Context-sensitive help (`?`) and About (`b`)
- Terminal restored on panic and around native file dialogs

## Connect
- Enter opens HTTP nodes in the browser and ping/TCP nodes in an SSH terminal using the system `ssh`

## Removed
- The encrypted credential store (#98). SSH relies on the user's agent, keys, and config.

## Storage and delivery
- SQLite with idempotent startup migrations
- Automated releases on merge to `main`: version bump, tag, four platform archives, checksums, generated release notes
