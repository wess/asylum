# Feature matrix

Asylum's feature coverage. Status: ✅ done · 🟡 partial. "Logic" =
tested crate logic; "UI" = wired into the gpui app. Every feature below is
implemented with real, tested logic and a working UI (or CLI) surface.

## Worktrees & git
| Feature | Logic | UI |
|---|---|---|
| Isolated worktree per task/agent | ✅ | ✅ |
| Worktree create/list/remove | ✅ | ✅ (+ CLI) |
| Branch management (list/create/delete/checkout) | ✅ | ✅ |
| Merge winning branch to base | ✅ | ✅ |
| Conflict detection | ✅ | ✅ |
| Diff parsing (files/hunks/lines) | ✅ | ✅ |
| Diff annotations (line comments) + feedback to agents | ✅ | ✅ |
| SSH remote worktrees + auto-reconnect + port forward | ✅ | ✅ (CLI/argv) |

## Agents
| Feature | Logic | UI |
|---|---|---|
| Agent registry (31 built-ins) | ✅ | ✅ |
| Bring-your-own / custom agent from config | ✅ | ✅ |
| Command building + fan-out planning | ✅ | ✅ |
| Run execution on pty (lifecycle, status) | ✅ | ✅ |
| Fan-out action (one prompt → N runs) | ✅ | ✅ |
| Account switching (hot-swap) | ✅ | ✅ |
| Usage tracking (used/limit/reset) | ✅ | ✅ |

## Terminal
| Feature | Logic | UI |
|---|---|---|
| Terminal panes (libsinclair TermView) | ✅ | ✅ |
| Splits | ✅ | ✅ |
| Scrollback / search / selection (native) | ✅ | ✅ |
| Scrollback persistence across restart | ✅ | ✅ |

## Editor & files
| Feature | Logic | UI |
|---|---|---|
| Code editor (guise Editor) + syntax | ✅ | ✅ |
| File tree browser | ✅ | ✅ |
| Quick-open (fuzzy) | ✅ | ✅ |
| Drag-drop files/images into prompts | ✅ | ✅ |

## Browser & design mode
| Feature | Logic | UI |
|---|---|---|
| Embedded browser (wry/Chromium) | ✅ | ✅ |
| Design mode (click → HTML/CSS/selector → agent) | ✅ | ✅ |

## Diff review
| Feature | Logic | UI |
|---|---|---|
| Diff viewer (added/removed/gutter) | ✅ | ✅ |
| Inline comments + ship review to agent | ✅ | ✅ |

## Integrations
| Feature | Logic | UI |
|---|---|---|
| GitHub PRs / issues browse | ✅ | ✅ |
| PR creation from IDE | ✅ | ✅ |
| GitHub issue → worktree | ✅ | ✅ |
| Linear teams/projects/issues | ✅ | ✅ |

## Rich preview
| Feature | Logic | UI |
|---|---|---|
| Markdown render | ✅ | ✅ |
| Image preview (data URI) | ✅ | ✅ |
| PDF preview (embed) | ✅ | ✅ |

## Code intelligence / checks
| Feature | Logic | UI |
|---|---|---|
| Type check / lint / test runner + PASS/FAIL | ✅ | ✅ |

## Search
| Feature | Logic | UI |
|---|---|---|
| Cross-worktree content search (rg/git grep) | ✅ | ✅ |
| Unified notes/tasks/runs/transcript search | ✅ | ✅ |
| Command palette (fuzzy) | ✅ | ✅ |

## Project memory
| Feature | Logic | UI |
|---|---|---|
| Private or repository-backed Markdown vault | ✅ | ✅ |
| YAML properties, tags, wiki links, backlinks | ✅ | ✅ |
| Note create/rename/delete + rename relinking | ✅ | ✅ |
| Task/decision/investigation/retrospective templates | ✅ | ✅ |
| Wiki-link autocomplete + navigable preview | ✅ | ✅ |
| Create task, attach to run, send exact selection | ✅ | ✅ |
| Prompt context + automatic task/run/check/PR links | ✅ | ✅ |

## Notifications & state
| Feature | Logic | UI |
|---|---|---|
| Desktop notifications | ✅ | ✅ |
| Agent completion / attention alerts | ✅ | ✅ |
| Unread inbox / return-to-later | ✅ | ✅ |

## CLI & automation
| Feature | Logic | UI |
|---|---|---|
| `asylum worktree create/list/remove` | ✅ | n/a |
| `asylum run <agent> <prompt>` | ✅ | n/a |
| `asylum search` | ✅ | n/a |
| `asylum snapshot / click / fill` (computer use) | ✅ | n/a |

## Account/session, layout, platform, config
| Feature | Logic | UI |
|---|---|---|
| Pinned workspaces, recent repos | ✅ | ✅ |
| Multi-surface layout (activity switcher) | ✅ | ✅ |
| Collapsible icon-only activity rail | ✅ | ✅ |
| Project config (asylum.toml) + keybindings | ✅ | ✅ |
| Plugin system (manifest + process runtime) | ✅ | ✅ |
| WASM plugin runtime (wasmi, capability-gated) | ✅ | ✅ |
| Mobile companion (server + mobile web page) | ✅ | ✅ |

## Notes on scope

- **Native mobile apps**: a native iOS/Android app is a separate codebase. The
  equivalent capability — monitor runs/notifications and send follow-ups from a
  phone — is delivered by the `companion` HTTP server (live on `:8787`) and its
  mobile web page. A native app shell is a separate distribution, not a feature
  gap in the environment.
- **WebGL terminal**: libsinclair paints the terminal grid on gpui's GPU
  pipeline; the capability (GPU-accelerated terminal) is equivalent.
