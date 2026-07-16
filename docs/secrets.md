# Secrets proxy & keep

Let agents make outbound API calls **without ever seeing the credentials**.
Asylum holds the keys in an encrypted, scoped **keep** and injects them for you;
an agent only references a service by name.

## How it works

You define named **upstreams** — a base URL plus which secret to inject — and
store the secret values in the keep. An agent calls a local endpoint by name;
Asylum resolves the secret (scoped to the agent's project), attaches it
server-side, and forwards *only* to that upstream's host.

```
agent:  asylum call openai POST /v1/chat/completions --data @body.json
        └─ hits  http://127.0.0.1:8789/openai/v1/chat/completions
              with a signed per-run token (names the run's project)
proxy:  → https://api.openai.com/v1/chat/completions
              with Authorization: Bearer <real key from the keep>   ← injected here
```

## The keep

Secret values live in `~/.config/asylum/keep.enc` — **AES-256-GCM**, encrypted
under a key derived from a passphrase (PBKDF2-HMAC-SHA256). Unlocking decrypts it
into memory; from then on the values live only in Asylum's heap and are never
written back in the clear.

The keep is **scoped**:

- **Global** — available to every project.
- **Project** — available only to that project, and it **overrides** a global
  secret of the same name for that project.

Resolution for an agent in project *P* = *P*'s keep overlaid on the global keep.

Manage it from the shell (values are read from stdin or `--value`, never the
argv of a logged command):

```sh
export ASYLUM_KEEP_PASSPHRASE="…"          # unlocks the keep
asylum keep set openai --value 'sk-…'      # global
asylum keep set stripe --project 7 --value 'sk_live_…'   # project 7 only
asylum keep list                            # global names
asylum keep list --project 7
asylum keep rm openai
```

## Security model

- **The agent can't read the key.** Values live only in the encrypted keep and,
  once unlocked, in Asylum's memory — never in `settings.json`, never in a file
  the agent could read in the clear, never in the agent's environment. The
  passphrase is read from `ASYLUM_KEEP_PASSPHRASE` and immediately scrubbed from
  Asylum's environment, so it isn't in `/proc/<pid>/environ` either.
- **Scope can't be forged.** Each run gets a token signed with the session key
  that names its project; the proxy trusts the extracted project, not anything
  the agent sets. A project can't reach another project's secrets.
- **No exfiltration.** The destination host is fixed by the upstream config, so a
  request can never redirect the secret elsewhere. An unknown upstream is a 404;
  a locked keep is a 503.
- **Never reflected.** The proxy injects the secret only into the outgoing
  request; it never returns it. A supplied header is overridden.
- **Transcript redaction.** As a safety net, any known secret value that appears
  in terminal output (e.g. an upstream that echoes it) is masked before the
  transcript is stored.

Caveat: if you point an upstream at a service that *reflects* your credential
back in its response, the agent receives that response and the masking is
defeated — only configure upstreams you trust.

## Configuration

```jsonc
"proxy": {
  "enabled": true,          // off until you define upstreams + unlock the keep
  "bind": "127.0.0.1:8789"  // loopback only — a non-loopback bind is refused
},
"upstreams": [
  {
    "name": "openai",                       // agents address /openai/...
    "base_url": "https://api.openai.com",   // the only host the secret reaches
    "secret": "openai",                     // the keep entry to inject
    "project": 0,                           // 0 = global; a project id scopes it
    "header": "Authorization",              // default: Authorization
    "format": "Bearer {secret}"             // default: Bearer {secret}
  }
]
```

Start Asylum with `ASYLUM_KEEP_PASSPHRASE` set to unlock the keep. (An in-app
unlock prompt and a Global/Project scope editor are planned in the Settings
surface.)

## For agents

```sh
asylum call                      # list the upstreams you may use
asylum call --skill              # print the usage skill
asylum call openai POST /v1/chat/completions --data '{"model":"gpt-4o-mini",...}'
```

The response body prints to stdout. Under the hood this hits
`$ASYLUM_PROXY_URL/<upstream>/<path>` with `Authorization: Bearer
$ASYLUM_PROXY_TOKEN`; `asylum call` handles that for you.
