//! The agent skill: a Markdown instruction file that teaches a coding agent how
//! to drive the ADE from inside its worktree. Print it with `asylum control
//! skill` and drop it into the agent's rules/skills directory - it is not a
//! service, just instructions, exactly like herdr's approach.

/// The skill document. `{url}` and `{token}` are illustrative; at runtime the
/// app sets `ASYLUM_CONTROL_URL`, `ASYLUM_TASK_ID`, `ASYLUM_RUN_ID`, and (when
/// configured) `ASYLUM_CONTROL_TOKEN` in the agent's environment.
pub const SKILL: &str = r#"# Asylum control skill

You may be running inside an Asylum worktree as one of several agents racing the
same task. If so, you can orchestrate the fleet - spawn a helper agent, read what
a sibling is doing, run the project's checks, report your own state, and wait on
another run - through a small local HTTP API.

## Am I inside Asylum?

Only if `ASYLUM_RUN_ID` is set. If it is **not** set, you are not in an
Asylum-managed pane: do not attempt any of the below, and say so if asked.

The environment gives you:

- `ASYLUM_CONTROL_URL` - base URL, e.g. `http://127.0.0.1:8788`
- `ASYLUM_TASK_ID` - the task every sibling shares
- `ASYLUM_RUN_ID` - your own run
- `ASYLUM_CONTROL_TOKEN` - send as `Authorization: Bearer <token>` if set

Prefer the `asylum` CLI, which reads these for you:

```sh
asylum control status              # your run + siblings, with live activity
asylum control read <run-id>       # a sibling's recent transcript
asylum control spawn <agent> "<prompt>"   # queue another agent on this task
asylum control activity <state>    # report yourself: working|blocked|done
asylum control check               # run this project's checks in your worktree
asylum wait run <run-id> --status succeeded   # block until a sibling finishes
asylum wait run <run-id> --activity blocked   # block until a sibling needs input
```

## Etiquette

- Report `blocked` before you stop to ask the user something, and `done` when
  you finish, so the board and your teammates can see your state at a glance.
- Spawn a helper only when parallel work genuinely helps (e.g. write tests in a
  sibling while you implement). Helpers cost a worktree each.
- Reading a sibling is for coordination, not copying: cite what you learned.

## Raw API

If you cannot use the CLI, call the API directly (JSON in, JSON out):

- `GET  {url}/control/runs?task={task}` - siblings + activity
- `GET  {url}/control/runs/{id}` - one run + a transcript tail
- `POST {url}/control/runs/{id}/activity` `{"activity":"blocked"}`
- `POST {url}/control/runs/{id}/check` - queue a checks pass
- `POST {url}/control/tasks/{task}/spawn` `{"agent":"codex","prompt":"..."}`
- `GET  {url}/control/events?since={cursor}` - follow the fleet without polling
"#;
