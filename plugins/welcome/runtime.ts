// Welcome plugin runtime.
//
// The ADE speaks newline-delimited JSON over stdio: it writes one
// `{ id, method, params }` request per line and reads one
// `{ id, result }` or `{ id, error }` response per line. A one-shot
// invocation sends a single request and closes stdin (EOF), so this reads
// everything available, answers each request, and exits.

type Request = { id: number; method: string; params?: Record<string, unknown> };
type Response = { id: number; result?: unknown; error?: string };

function handle(req: Request): Response {
  const params = req.params ?? {};
  switch (req.method) {
    case "greet":
      return {
        id: req.id,
        result: { message: `Hello from the Welcome plugin (project: ${params.project ?? "none"}).` },
      };
    case "on_run_finished":
      return { id: req.id, result: { summary: "A run finished — the Welcome plugin saw the event." } };
    case "note":
      return { id: req.id, result: { noted: String(params.text ?? "") } };
    default:
      return { id: req.id, error: `unknown method: ${req.method}` };
  }
}

const input = await new Response(Bun.stdin.stream()).text();
for (const line of input.split("\n")) {
  const trimmed = line.trim();
  if (!trimmed) continue;
  let req: Request;
  try {
    req = JSON.parse(trimmed) as Request;
  } catch {
    continue;
  }
  process.stdout.write(JSON.stringify(handle(req)) + "\n");
}
