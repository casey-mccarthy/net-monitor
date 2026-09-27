# Planned

Ideas that have come up, roughly ordered by how likely they are to happen. Nothing here is committed to; open an issue with the `enhancement` label to argue for one.

## Near term
- **Notifications**: desktop or email alerts on Offline/Online transitions, with a digest mode to avoid alert storms.
- **Response time history**: store the latency of every check (today only transitions are stored) and show a sparkline per node.
- **Per-node thresholds**: warning/critical latency levels alongside the up/down state.
- **History pruning**: the history tables only grow on transitions, but flapping nodes can still fill them; add a retention window.

## Medium term
- **Certificate expiry check** for HTTPS nodes.
- **DNS resolution check** as a monitor type.
- **Bulk operations**: multi-select for delete and interval edits.
- **Custom command checks**: run a script and treat exit 0 as Online.

## Long term
- Read-only web dashboard or JSON API.
- Distributed agents reporting to one database.

## Not planned
- Multi-user accounts, roles, and audit logs. It is a local tool.
- Storing SSH credentials. The system `ssh` and its agent already do this better.
