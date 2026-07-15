# Chapter 11: Agent Orchestration and the Control Surface

This is the deep chapter. So far *you* have orchestrated the fleet — composing
tasks, fanning out, reviewing, merging. The **control surface** lets a *running
agent* do the same from inside its own worktree: spawn a helper agent, read what
a sibling is doing, run the project's checks, report its own semantic state, and
wait on another run. This is how a lead agent can grow a small team around a task.

By the end you will understand the environment variables that mark an agent as
"inside Asylum," the skill document that teaches an agent the API, the CLI and
raw HTTP interfaces, the queue/drain model that keeps writes safe, and a worked
example of a lead agent spawning and waiting on a test-writer sibling.

## The idea

Asylum already races many agents at one task, each in isolation. The control
surface adds a small **local HTTP/JSON server** that a running agent can call to
coordinate that fleet — without breaking isolation. Reads answer directly from
the shared store. Writes that need real git/pty side-effects (spawning a run,
running checks) are **queued** and performed by the desktop app, not by the
server thread. That design keeps the surface safe and its routing a pure function
over the store.

The control server is configured under the `control` key in `settings.json`,
localhost-only by default (it can spawn runs, so you keep it off the network):

```jsonc
{
  "control": {
    "enabled": true,
    "bind": "127.0.0.1:8788",
    "token": ""
  }
}
```

## Am I inside Asylum?

When the app launches an agent, it injects environment variables into that
agent's process:

- `ASYLUM_CONTROL_URL` — the control server base URL, e.g.
  `http://127.0.0.1:8788`.
- `ASYLUM_TASK_ID` — the task every sibling of this fan-out shares.
- `ASYLUM_RUN_ID` — this agent's own run id.
- `ASYLUM_CONTROL_TOKEN` — sent as `Authorization: Bearer <token>` when a token
  is configured.

**The presence of `ASYLUM_RUN_ID` is the test.** An agent is "inside Asylum" only
if `ASYLUM_RUN_ID` is set. If it is not set, the agent is not in a managed pane
and must not attempt any orchestration. This single check keeps an agent from
misbehaving when run standalone.

## The skill

An agent learns this API not from a service call but from a **skill document** —
a Markdown instruction file you drop into the agent's rules/skills directory.
Print it with:

```sh
asylum control skill
```

The skill explains the "am I inside Asylum?" check, lists the environment
variables, shows the CLI commands, gives etiquette, and documents the raw API.
Because it is just instructions, it works with any agent that reads a skills or
rules directory — there is no runtime dependency to install. The skill also lays
out **etiquette**: report `blocked` before you stop to ask the user something and
`done` when you finish; spawn a helper only when parallel work genuinely helps
(each helper costs a worktree); and read a sibling for coordination, not copying.

## The CLI interface

An agent should prefer the `asylum control` CLI, which reads the injected
environment for it. The commands:

```sh
asylum control status              # your run + siblings, with live activity
asylum control read <run-id>       # a sibling's recent transcript tail
asylum control spawn <agent> "<prompt>"   # queue another agent on this task
asylum control activity <state>    # report yourself: working|blocked|done|idle
asylum control check               # run this project's checks in your worktree
asylum wait run <run-id> --status succeeded   # block until a sibling finishes
asylum wait run <run-id> --activity blocked   # block until a sibling needs input
```

`asylum control status` prints the task, marks your own run with `*`, and lists
every sibling with its agent, lifecycle status, and live activity — the same
"who needs me" picture the board shows, but in text an agent can read.

## Reporting your own state

The activity classifier ([Chapter 4](04-the-fleet-in-depth.md)) guesses an
agent's state from its output, but the agent knows its own state best. Reporting
it explicitly makes the board, your phone, and sibling agents accurate:

```sh
asylum control activity working   # I'm implementing
asylum control activity blocked   # I've stopped to ask the user something
asylum control activity done      # I'm finished; ready for review
```

Report `blocked` *before* you pause for input and `done` when you finish. Each
report updates the run's activity and emits a `run_activity`
[event](12-the-mobile-companion-and-events.md).

## The queue/drain model

Two kinds of control operations exist, and they are handled very differently.

**Reads** — `status`, `read`, listing checks, the event stream — answer directly
from the SQLite store the server shares with the app. They are immediate.

**Writes with side-effects** — `spawn` (create a new run, which needs a branch, a
worktree, and a launched pty) and `check` (run commands in a worktree) — are
**not** performed by the server. They are recorded as a *control request* row and
returned immediately as "queued." The desktop app polls for pending control
requests, performs the git/pty work, and marks each processed. This is the same
pattern the mobile companion uses for follow-ups.

Two consequences matter in practice:

1. A `spawn` or `check` is *asynchronous*. `asylum control spawn ...` returns as
   soon as the request is queued, not when the new run is live. To act on the
   result, follow up with `asylum control status` or `asylum wait`.
2. The app must be running to drain the queue. The control server can accept
   requests, but only the app turns them into worktrees and processes.

`asylum control activity`, by contrast, is a direct store write (it has no
git/pty effect), so it applies immediately.

## The raw API

If an agent cannot use the CLI, it can call the HTTP API directly — JSON in, JSON
out, under `/control`. All endpoints except `/control/health` require the bearer
token when one is configured.

| Method + path                        | Effect                                    |
|--------------------------------------|-------------------------------------------|
| `GET  /control/health`               | liveness (`{"ok":true}`)                  |
| `GET  /control/runs?task=<id>`       | sibling runs of a task, with activity     |
| `GET  /control/runs/<id>`            | one run + a tail of its transcript        |
| `GET  /control/runs/<id>/checks`     | that run's verification results           |
| `POST /control/runs/<id>/activity`   | self-report semantic state                |
| `POST /control/runs/<id>/check`      | queue a checks pass in the worktree       |
| `POST /control/tasks/<id>/spawn`     | queue a helper run (agent + prompt)       |
| `GET  /control/events?since=<id>`    | replay the event log from a cursor        |

A raw request, using the injected variables, looks like this:

```sh
curl -s "$ASYLUM_CONTROL_URL/control/runs?task=$ASYLUM_TASK_ID" \
  -H "Authorization: Bearer $ASYLUM_CONTROL_TOKEN"

curl -s -X POST "$ASYLUM_CONTROL_URL/control/runs/$ASYLUM_RUN_ID/activity" \
  -H "Authorization: Bearer $ASYLUM_CONTROL_TOKEN" \
  -d '{"activity":"blocked"}'

curl -s -X POST "$ASYLUM_CONTROL_URL/control/tasks/$ASYLUM_TASK_ID/spawn" \
  -H "Authorization: Bearer $ASYLUM_CONTROL_TOKEN" \
  -d '{"agent":"codex","prompt":"write tests for the parser"}'
```

A run read returns the run summary plus `output_tail` (the last ~40 transcript
lines), `exit_code`, and `error`. A `spawn` or `check` returns the queued
request id.

## Worked example: a lead agent grows a team

Imagine `claude-code` is the lead on a task: *implement a JSON config parser with
tests.* Splitting the work across a sibling genuinely helps — one agent writes
the parser while another writes tests against the intended interface. From inside
its worktree the lead agent does this:

```sh
# 1. Confirm I'm inside Asylum.
test -n "$ASYLUM_RUN_ID" || { echo "not in a managed run"; exit 0; }

# 2. Tell the board I'm working.
asylum control activity working

# 3. Spawn a sibling to write tests against the interface I'm about to build.
asylum control spawn codex \
  "Write unit tests for a parse_config(path) -> Config function: valid file,
   missing file, and malformed JSON. Do not implement parse_config."

# 4. See what runs exist now and find the sibling's id.
asylum control status
#   task 7 (you are run 12)
#   * run 12   claude-code    running    working
#     run 15   codex          running    working

# 5. Implement the parser in my own worktree... (the agent does its work)

# 6. Wait for the test-writer sibling to finish before I reconcile.
asylum wait run 15 --status succeeded --timeout 300

# 7. Read what it produced, to align my implementation with its tests.
asylum control read 15

# 8. Run my worktree's checks, then report done.
asylum control check
asylum control activity done
```

Each spawned helper runs in its *own* worktree on its *own* branch — the lead
agent's isolation is never violated. The lead coordinates through the control
surface: it reads the sibling's transcript to align interfaces (citing what it
learned, not copying blindly), waits on it explicitly rather than polling, and
reports its own state so you — and any other sibling — can see the whole team's
progress on the board and on your phone.

## Try it

You need the app running with `control.enabled` true, and an agent whose
skills/rules directory you can edit.

1. `asylum control skill` and drop the output into your agent's rules directory.
2. Fan a task out to one agent. From that agent's terminal, run
   `asylum control status` and confirm it sees its own run.
3. Have the agent (or you, in its pane) run
   `asylum control spawn codex "add a test"`, then `asylum control status` again
   and watch the new run appear after the app drains the queue.
4. `asylum wait run <new-id> --status succeeded` and observe it return when the
   sibling finishes.

## Recap

- The control surface lets a running agent orchestrate the fleet from inside its
  worktree via a local HTTP/JSON server.
- `ASYLUM_RUN_ID` present = inside Asylum; the app also injects
  `ASYLUM_CONTROL_URL`, `ASYLUM_TASK_ID`, and `ASYLUM_CONTROL_TOKEN`.
- Agents learn the API from the skill (`asylum control skill`), a Markdown
  instruction file — not a service.
- Reads answer from the store; writes (spawn, check) are queued and drained by
  the app, so they are asynchronous.
- A lead agent can spawn helpers, read siblings, report state, and wait — each
  helper isolated in its own worktree.

## Next

[Chapter 12: The Mobile Companion and Events](12-the-mobile-companion-and-events.md)
follows the same event stream from your phone.
