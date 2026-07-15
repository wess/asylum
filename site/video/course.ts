import { mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { synthesize } from "./speech";

type Shot = { screen: string; action: string; narration: string };
type Episode = { id: string; title: string; level: string; sources: string[] };

const site = dirname(import.meta.dir);
const repo = dirname(site);
const output = join(site, "public", "videos");
const work = join(site, ".video", "course");

const episodes: Episode[] = [
  { id: "01", title: "What Asylum is", level: "Beginner", sources: ["00-welcome.md"] },
  { id: "02", title: "Install and first launch", level: "Beginner", sources: ["01-install-and-first-launch.md"] },
  { id: "03", title: "Projects, tasks, agents, and worktrees", level: "Beginner", sources: ["02-open-a-project-and-pick-agents.md"] },
  { id: "04", title: "Run your first fan-out", level: "Beginner", sources: ["03-your-first-fanout.md"] },
  { id: "05", title: "Agent activity and terminals", level: "Intermediate", sources: ["04-reading-semantic-states.md", "10-terminal-editor-preview-browser.md"] },
  { id: "06", title: "Diffs, checks, and annotations", level: "Intermediate", sources: ["05-review-diffs-checks-annotations.md"] },
  { id: "07", title: "Select and merge the winner", level: "Intermediate", sources: ["06-merge-the-winner.md"] },
  { id: "08", title: "Notes, context, and reusable workflows", level: "Intermediate", sources: ["08-notes-and-knowledge.md", "07-layouts-and-presets.md"] },
  { id: "09", title: "CLI, integrations, companion, and plugins", level: "Advanced", sources: ["11-the-cli-tour.md", "09-integrations.md", "13-mobile-companion-and-events.md", "14-plugins.md"] },
  { id: "10", title: "Expert orchestration and security", level: "Expert", sources: ["12-agent-control-surface.md", "15-expert-workflows.md"] },
];

const requested = new Set((Bun.env.EPISODES ?? "").split(",").filter(Boolean));

const run = async (args: string[], label: string) => {
  const child = Bun.spawn(args, { stdout: "ignore", stderr: "pipe" });
  const error = await new Response(child.stderr).text();
  const code = await child.exited;
  if (code !== 0) throw new Error(`${label}: ${error}`);
};

const capture = async (args: string[]) => {
  const child = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
  const result = await new Response(child.stdout).text();
  const error = await new Response(child.stderr).text();
  const code = await child.exited;
  if (code !== 0) throw new Error(error);
  return result.trim();
};

const mediaDuration = async (path: string) =>
  Number(await capture(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1", path]));

const clean = (value: string) =>
  value
    .trim()
    .replace(/^"|"$/g, "")
    .replace(/\*\*/g, "")
    .replace(/`/g, "")
    .replace(/<[^>]+>/g, "")
    .replace(/\s+/g, " ");

const shots = async (sources: string[]) => {
  const result: Shot[] = [];
  for (const source of sources) {
    const text = await Bun.file(join(repo, "docs", "videos", source)).text();
    for (const line of text.split("\n")) {
      if (!/^\|\s*\d+:\d+\s*\|/.test(line)) continue;
      const cells = line.split("|").slice(1, -1).map(clean);
      if (cells.length < 4 || !cells[3]) continue;
      result.push({ screen: cells[1], action: cells[2], narration: cells[3] });
    }
  }
  return result;
};

const xml = (value: string) =>
  value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");

const wrap = (value: string, width: number, lines = 3) => {
  const words = clean(value).split(" ");
  const result: string[] = [];
  let row = "";
  for (const word of words) {
    if (`${row} ${word}`.trim().length > width && row) {
      result.push(row);
      row = word;
    } else row = `${row} ${word}`.trim();
  }
  if (row) result.push(row);
  if (result.length > lines) {
    result[lines - 1] = `${result.slice(lines - 1).join(" ").slice(0, width - 1)}…`;
  }
  return result.slice(0, lines);
};

const textblock = (lines: string[], y: number, size: number, color: string, weight: number) =>
  lines
    .map((line, index) => `<text x="220" y="${y + index * size * 1.25}" fill="${color}" font-family="Arial" font-size="${size}" font-weight="${weight}">${xml(line)}</text>`)
    .join("");

const stamp = (seconds: number) => {
  const value = Math.round(seconds * 1000);
  const hours = Math.floor(value / 3_600_000);
  const minutes = Math.floor((value % 3_600_000) / 60_000);
  const secs = Math.floor((value % 60_000) / 1000);
  const millis = value % 1000;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
};

await mkdir(output, { recursive: true });
await mkdir(join(output, "transcripts"), { recursive: true });
await mkdir(work, { recursive: true });

for (const episode of episodes) {
  if (requested.size > 0 && !requested.has(episode.id)) continue;
  const episodework = join(work, episode.id);
  await mkdir(episodework, { recursive: true });
  const sequence = await shots(episode.sources);
  const clips: string[] = [];
  const captions = ["WEBVTT", ""];
  const transcript = [`# ${episode.title}`, "", `${episode.level} · Episode ${Number(episode.id)}`, ""];
  let cursor = 0;

  for (const [index, shot] of sequence.entries()) {
    const scene = String(index + 1).padStart(3, "0");
    const speech = join(episodework, `${scene}.wav`);
    const svg = join(episodework, `${scene}.svg`);
    const frame = join(episodework, `${scene}.png`);
    const clip = join(episodework, `${scene}.mp4`);
    await synthesize({ text: shot.narration, voice: "af_heart", speed: 1.08, output: speech });
    const speechlength = await mediaDuration(speech);
    const spokenlength = speechlength;
    const cliplength = Math.max(spokenlength + 0.65, 3.4);
    const heading = wrap(shot.screen, 38, 3);
    const detail = wrap(shot.action, 68, 2);
    const accent = index % 4 === 1 ? "#ffb347" : index % 4 === 2 ? "#80f0c0" : index % 4 === 3 ? "#ff5a6f" : "#33d6ff";
    await Bun.write(
      svg,
      `<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080"><rect width="1920" height="1080" fill="#05070b"/><g opacity=".14" stroke="#33d6ff" stroke-width="1">${Array.from({ length: 17 }, (_, i) => `<path d="M${i * 120} 0V1080"/>`).join("")}${Array.from({ length: 10 }, (_, i) => `<path d="M0 ${i * 120}H1920"/>`).join("")}</g><rect x="0" y="0" width="14" height="1080" fill="${accent}"/><text x="220" y="150" fill="${accent}" font-family="Arial" font-size="28" font-weight="700" letter-spacing="5">ASYLUM · EPISODE ${episode.id} · ${xml(episode.level.toUpperCase())}</text>${textblock(heading, 355, 72, "#f4fbff", 700)}<rect x="220" y="${610 + Math.max(0, heading.length - 1) * 84}" width="520" height="3" fill="${accent}"/>${textblock(detail, 705 + Math.max(0, heading.length - 1) * 84, 30, "#8da0af", 400)}<text x="1700" y="940" text-anchor="end" fill="#3c4d5b" font-family="Arial" font-size="150" font-weight="700">${String(index + 1).padStart(2, "0")}</text><circle cx="1700" cy="160" r="42" fill="none" stroke="${accent}" stroke-width="3"/><circle cx="1700" cy="160" r="10" fill="${accent}"/></svg>`,
    );
    await run(["rsvg-convert", "-o", frame, svg], `episode ${episode.id} frame ${scene}`);
    await run(
      [
        "ffmpeg", "-y", "-loop", "1", "-framerate", "30", "-i", frame, "-i", speech,
        "-t", String(cliplength), "-vf", `fade=t=in:st=0:d=0.35,fade=t=out:st=${Math.max(0, cliplength - 0.45)}:d=0.45`,
        "-af", `adelay=200|200,apad=pad_dur=${cliplength}`,
        "-c:v", "libx264", "-preset", "veryfast", "-crf", "21", "-pix_fmt", "yuv420p",
        "-c:a", "aac", "-b:a", "160k", "-ar", "48000", "-shortest", clip,
      ],
      `episode ${episode.id} scene ${scene}`,
    );
    clips.push(clip);
    captions.push(String(index + 1), `${stamp(cursor + 0.2)} --> ${stamp(cursor + 0.2 + spokenlength)}`, shot.narration, "");
    transcript.push(`## ${index + 1}. ${shot.screen}`, "", shot.narration, "");
    cursor += cliplength;
  }

  const list = join(episodework, "concat.txt");
  await Bun.write(list, clips.map((path) => `file '${path.replaceAll("'", "'\\''")}'`).join("\n"));
  const joined = join(episodework, "joined.mp4");
  await run(["ffmpeg", "-y", "-f", "concat", "-safe", "0", "-i", list, "-c", "copy", joined], `episode ${episode.id} assembly`);
  const video = join(output, `episode${episode.id}.mp4`);
  await run(
    [
      "ffmpeg", "-y", "-i", joined, "-f", "lavfi", "-i", `sine=frequency=58:sample_rate=48000:duration=${cursor}`,
      "-filter_complex", "[1:a]volume=.005,tremolo=f=.18:d=.55[bed];[0:a][bed]amix=inputs=2:duration=first,loudnorm=I=-16:LRA=7:TP=-1.5[a]",
      "-map", "0:v", "-map", "[a]", "-c:v", "copy", "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart", video,
    ],
    `episode ${episode.id} mix`,
  );
  await Bun.write(join(output, `episode${episode.id}.vtt`), captions.join("\n"));
  await Bun.write(join(output, "transcripts", `episode${episode.id}.md`), transcript.join("\n"));
  console.log(`episode ${episode.id}: ${sequence.length} scenes, ${cursor.toFixed(1)} seconds`);
}
