# Episode 13 — Mobile Companion and Events

**Duration:** ~5 min · **Level:** Advanced
**You'll learn:** the companion server and its settings, the append-only event stream, and how to follow the fleet — including the blocked signal — from your phone.
**Prerequisites:** [Episode 03](03-your-first-fanout.md) — you have runs to follow.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | A long-running fan-out on the board. | Land on a busy board. | "A big fan-out can run longer than your attention span at the desk. The mobile companion lets you keep an eye on the fleet from your phone, and the event stream lets your phone — and your agents — follow along without hammering the database." |
| 0:26 | Settings `companion` block. | Show the config. | "The companion is a small HTTP server the app runs on a background thread, over the same store the desktop uses. Configure it under the companion key — enabled, a bind address, and a token." |
| 0:52 | Security overlay: localhost vs 0.0.0.0. | Show both binds. | "Here's the rule of thumb. Localhost with no token for solo desktop use. To reach it from your phone on the LAN, bind zero-dot-zero-dot-zero-dot-zero — but only with a token set. Never expose it to the network without a token. The token is the gate." |
| 1:24 | Edit bind to `0.0.0.0:8787` + a token. | Set both; save. | "Set a token, bind to all interfaces on port eight-seven-eight-seven, and save. Live reload applies it." |
| 1:50 | Phone opening the bound address. | Open the URL on a phone. | "Open that address in your phone's browser and you get a self-contained mobile status page — it polls the API and shows your projects, tasks, runs, and inbox at a glance." |
| 2:18 | Phone showing runs with activity chips. | Point at a blocked chip on the phone. | "And this is the payoff: the runs list includes each run's live activity. So the phone shows which agent is blocked and needs you — exactly like the board. You can be away from your machine and still know a run is waiting on your answer." |
| 2:48 | Phone: type and send a follow-up. | Send a follow-up message. | "You can nudge a run from your phone too. Send a follow-up — a message the app queues and delivers into a live run, and also posts to your inbox. A live, stdin-capable agent gets it immediately; a finished run starts a fresh attempt with it." |
| 3:20 | Diagram: append-only event log. | Show rows appended. | "Under all of this is the event stream. Polling every table for 'what changed' is wasteful, so every meaningful transition appends one row to an append-only log. It's never mutated after insert, so a cursor is a stable position." |
| 3:48 | Event kinds list. | Reveal the five kinds. | "The kinds you'll see: run_started, run_activity when a run's semantic state changes, run_finished, run_failed, and run_spawned. Each carries an id — the cursor — a kind, the task and run it concerns, an optional payload, and a timestamp." |
| 4:14 | Terminal. | `curl .../api/events?since=0`. | "Follow it from a cursor. Call with since equals zero to read from the start; the response gives you a batch and a cursor — the id of the last event. Pass that cursor back as since on the next call to get only what's new." |
| 4:36 | Second curl with the returned cursor. | Show the incremental page. | "That's the whole protocol. A page is capped at two hundred events; lower it with a limit parameter." |
| 4:52 | Overlay: `/api/events` and `/control/events`. | Show both endpoints. | "And the same stream is served on both servers — the companion for your phone, and control for your agents. So a lead agent can tail the fleet's events the same way your phone does. Next: extending the ADE itself with plugins." |

## B-roll / capture notes
- Use a real phone on the same LAN as the desktop; bind `0.0.0.0:8787` with a token and blur the token on screen.
- The best shot is a **blocked** run visible on the phone — line this up with an agent that pauses for input, or self-report blocked from a run.
- Capture the two-call event cursor flow in a terminal: `since=0`, note the returned `cursor`, then `since=<cursor>` after fanning out a fresh task to show new `run_started` / `run_activity` events.
- Send a follow-up from the phone and cut to the run receiving it.

## Recap card (end screen)
- The companion is a localhost/LAN HTTP server over the store; secure any non-localhost bind with a token.
- Its API exposes projects, tasks, runs (with live activity), notifications, and follow-ups.
- The append-only event log (`run_started`, `run_activity`, `run_finished`, `run_failed`, `run_spawned`) is replayed from a cursor.
- The same stream is served at `/api/events` (companion) and `/control/events` (control).

## Next
- [Episode 14 — Plugins](14-plugins.md)

Go deeper: [book chapter 12](../book/12-the-mobile-companion-and-events.md).
