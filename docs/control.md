# Agent control surface, semantic states, and events

Asylum races several agents at one task, each isolated in its own worktree. Two
capabilities make that fleet legible and self-organising: **semantic states**
(which agent needs you right now) and the **control surface** (an agent
orchestrating the fleet from inside its worktree). Both ride an append-only
**event log** so a phone or an agent can follow along without polling.

## Semantic states

A run's lifecycle `status` — `queued → running → succeeded/failed/cancelled` —
tells you the process is alive. It does not tell you that agent #3 of five is
**blocked waiting for your input** while the others still churn. So each run also
carries a live *activity*, classified from its terminal output on every snapshot:

| Activity  | Meaning                                             |
|-----------|-----------------------------------------------------|
| `working` | Actively thinking, editing, or running a command.   |
| `blocked` | Stopped at a prompt, waiting for you to answer.     |
| `done`    | Printed a completion marker; awaiting review.       |
| `idle`    | Initialised but producing no recognisable signal.   |

Detection lives in `agent::activity` as a pure function: it strips ANSI escapes,
scans the recent lines, and matches against generic rules plus per-agent
additions. A live input prompt (`blocked`) wins over the others because it is the
most actionable. The Tasks board shows an activity chip on each running card, and
the same token is exposed over the mobile and control APIs.

## The control surface

When the app launches an agent it injects four environment variables:

- `ASYLUM_CONTROL_URL` — base URL of the local control server (default
  `http://127.0.0.1:8788`).
- `ASYLUM_TASK_ID` — the task every sibling shares.
- `ASYLUM_RUN_ID` — the agent's own run. **Its presence is how a skill knows it
  is inside an Asylum-managed worktree.**
- `ASYLUM_CONTROL_TOKEN` — sent as `Authorization: Bearer <token>`. Always
  present: the app provisions a per-session token when none is configured.

An agent learns the API from a Markdown skill (`asylum control skill`) dropped
into its rules/skills directory. Through the `asylum` CLI it can:

```sh
asylum control status                    # its run + siblings, with live activity
asylum control read <run-id>             # a sibling's recent transcript
asylum control spawn <agent> "<prompt>"  # queue another agent on this task
asylum control activity <state>          # report itself: working|blocked|done
asylum control check                     # run this project's checks in its worktree
asylum wait run <id> --status succeeded  # block until a sibling finishes
asylum wait run <id> --activity blocked  # block until a sibling needs input
```

### How writes stay safe

Reads answer straight from the store. Writes that need git/pty effects — spawning
a run, running checks — are **queued** as `store::ControlRequest` rows and drained
by the desktop app on a timer, the same contract as mobile follow-ups. That keeps
the control router a pure function over the store (fully tested without sockets)
and keeps worktree creation and process spawning on the app's side.

### Raw API

All endpoints are under `/control`:

| Method + path                       | Effect                                 |
|-------------------------------------|----------------------------------------|
| `GET  /control/health`              | liveness                               |
| `GET  /control/runs?task=<id>`      | sibling runs of a task, with activity  |
| `GET  /control/runs/<id>`           | one run + a tail of its transcript     |
| `GET  /control/runs/<id>/checks`    | that run's verification results        |
| `POST /control/runs/<id>/activity`  | self-report semantic state             |
| `POST /control/runs/<id>/check`     | queue a checks pass in the worktree    |
| `POST /control/tasks/<id>/spawn`    | queue a helper run (`agent` + `prompt`)|
| `GET  /control/events?since=<id>`   | replay the event log from a cursor     |

## Events

Every meaningful transition appends an `Event` to the store: `run_started`,
`run_activity`, `run_finished`, `run_failed`, `run_spawned`. Both servers replay
it from a cursor — `GET /api/events?since=<id>` (companion) and
`GET /control/events?since=<id>` (control) — so a phone follows the fleet live and
an agent can react to a sibling without hammering every table. The log is trimmed
to a bounded tail (`Db::prune_events`).

## Fan-out layouts

A *layout* is a named fan-out preset in `settings.json`: race a set of agents in
one pick instead of ticking boxes each time.

```jsonc
"layouts": [
  { "name": "duel",  "description": "Two frontier agents, head to head.",
    "agents": ["claude-code", "codex"] },
  { "name": "swarm", "description": "A wide net; three at a time.",
    "agents": ["claude-code", "codex", "opencode", "gemini", "aider"],
    "concurrency": 3 }
]
```

`concurrency` caps how many of the preset's runs are live at once (`0` defers to
the global `max_parallel_runs`). Inspect them with `asylum layout list` and
`asylum layout show <name>`. Omit the `layouts` key to keep the built-ins.

## Configuration

```jsonc
"control": {
  "enabled": true,          // off = agents cannot orchestrate the ADE
  "bind": "127.0.0.1:8788", // loopback only — a non-loopback bind is refused
  "token": ""               // empty = a per-session token is generated
}
```

Because the control surface can spawn runs and read transcripts, localhost is
not treated as an authentication boundary: the server always runs authenticated.
The `token` value is a **signing key** — leaving it empty makes the app provision
a strong per-session key at startup, kept in memory only and never written back
to `settings.json`.

For each run, the app mints a **scoped** token from that key, bound to the run's
task, and injects it as `ASYLUM_CONTROL_TOKEN`. The server verifies the signature
and confines the caller to its own task: an agent can list its siblings, read
their runs, report activity, queue checks, and spawn helpers *on its own task*,
but a request for another task is refused (`403`). Tokens carry an expiry and are
invalidated when the session key rotates (each app start). The bind is
loopback-only; a non-loopback bind is refused at startup and the refusal is
surfaced in the Inbox.
