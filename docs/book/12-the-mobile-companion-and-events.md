# Chapter 12: The Mobile Companion and Events

A fan-out can run for a while. The **mobile companion** lets you keep an eye on
the fleet from your phone, and the **event stream** lets both your phone and your
agents follow what is happening without hammering the database. This chapter
covers the companion server, its settings, the append-only event log, and how to
follow the fleet in real time.

## The companion server

The companion is a small, dependency-light HTTP server the app runs on a
background thread. It serves a mobile web page and a JSON API over the same
SQLite store the desktop uses, so anything you see in the app is available to your
phone: projects, tasks, runs, notifications, and the event stream. It also
accepts a **follow-up** — a message you send from your phone that the app
delivers into a live run.

Configure it under `companion` in `settings.json`:

```jsonc
{
  "companion": {
    "enabled": true,
    "bind": "127.0.0.1:8787",
    "token": ""
  }
}
```

- **enabled** — whether the server runs at all.
- **bind** — the address. The default binds localhost-only. To reach it from a
  phone on your LAN, bind `0.0.0.0:8787` — but only do that *with a token set*.
- **token** — a bearer token. Empty means localhost-only with no auth; a
  non-empty token is required as `Authorization: Bearer <token>` and is what
  makes a non-localhost bind safe.

The rule of thumb: **localhost + no token** for solo desktop use, or
**`0.0.0.0` + a token** to reach it from your phone. Never expose it to the LAN
without a token — the token is the gate.

Open the bound address in a browser and you get a self-contained mobile status
page that polls the API and shows your inbox and projects at a glance.

## The companion API

The endpoints are small and predictable:

| Method + path                          | Returns / does                      |
|----------------------------------------|-------------------------------------|
| `GET  /`                               | the mobile status web page          |
| `GET  /api/health`                     | liveness                            |
| `GET  /api/projects`                   | projects (id, name, pinned)         |
| `GET  /api/projects/<id>/tasks`        | a project's tasks                   |
| `GET  /api/tasks/<id>/runs`            | a task's runs, **with activity**    |
| `GET  /api/notifications`              | unread count + notifications        |
| `GET  /api/events?since=<cursor>`      | the event log from a cursor         |
| `POST /api/tasks/<id>/followup`        | queue a follow-up into a live run   |

Note that `/api/tasks/<id>/runs` includes each run's live **activity** — so the
phone shows which agent is `blocked` and needs you, exactly like the board. That
is the whole point of surfacing activity beyond the desktop: you can be away from
your machine and still know a run is waiting on your answer.

### Sending a follow-up

A follow-up is how you nudge a run from your phone. POST a message to a task and
the app both queues it for delivery — draining the queue and sending it to an
active run — and posts it as a `followup` notification so it shows up in your
inbox:

```sh
curl -s -X POST "http://<host>:8787/api/tasks/7/followup" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"message":"use the v2 endpoint, not v1"}'
```

A live, stdin-capable agent receives the message immediately; a finished run
starts a fresh attempt with it (the retry/continuation model from
[Chapter 4](04-the-fleet-in-depth.md)).

## The event stream

Polling every table to see "what changed" is wasteful. Instead, every meaningful
state change appends a row to an **append-only event log**, and clients replay it
from a cursor. The log is never mutated after insert, so a cursor is a stable,
monotonic position.

The event kinds you will see include:

- `run_started` — a run's agent launched.
- `run_activity` — a run's semantic activity changed (working/blocked/done/idle).
- `run_finished` — a run finished.
- `run_failed` — a run failed.
- `run_spawned` — a run was spawned (for example by an agent via the control
  surface).

Each event carries an `id` (the cursor), a `kind`, the `task` and `run` it
concerns, an optional `data` payload, and a timestamp.

### Following without polling

The stream is exposed on **both** servers with the same cursor semantics:

- Companion: `GET /api/events?since=<cursor>`
- Control: `GET /control/events?since=<cursor>`

The protocol is a simple long-poll-friendly loop:

1. Call with `since=0` to read from the beginning (or start from "now" if you
   only care about future events).
2. The response returns a batch of events and a **`cursor`** — the id of the last
   event returned.
3. Pass that `cursor` back as `since` on the next call to get only what happened
   after it.

```sh
# First page from the start.
curl -s "http://<host>:8787/api/events?since=0"
# -> {"cursor": 128, "items": [ ... ]}

# Next page: only events after 128.
curl -s "http://<host>:8787/api/events?since=128"
```

A page is capped (default and maximum 200 events) and you can lower it with
`?limit=`. Because the same log feeds the control API, an **agent** can follow the
fleet the same way a phone does — a lead agent tailing `/control/events` sees its
siblings start, change activity, and finish, and can react without polling each
run individually. The log is periodically trimmed so a long session stays
bounded, so treat old cursors as best-effort and keep your cursor current.

## Try it

1. Set a `companion.token` and bind `0.0.0.0:8787`, then open the address from
   your phone on the same network and watch your projects and inbox load.
2. From a shell, `GET /api/events?since=0`, note the `cursor`, fan a task out,
   then `GET /api/events?since=<cursor>` and see the `run_started` and
   `run_activity` events for the new runs.
3. POST a follow-up to a running task and watch the agent receive it.

## Recap

- The companion is a localhost/LAN HTTP server over the store; secure a
  non-localhost bind with a token.
- Its API exposes projects, tasks, runs (with live activity), notifications, and
  follow-ups.
- The append-only event log (`run_started`, `run_activity`, `run_finished`,
  `run_failed`, `run_spawned`) is replayed from a cursor.
- The same stream is served by the companion (`/api/events`) and the control
  (`/control/events`) APIs, so phones and agents both follow the fleet without
  polling.

## Next

[Chapter 13: Plugins](13-plugins.md) extends the ADE itself.
