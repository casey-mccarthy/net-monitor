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
- Status change log for every transition, with the duration of the previous state
- History view per node with uptime/downtime and the most recent change reachable
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
