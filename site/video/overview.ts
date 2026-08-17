import { copyFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { synthesize } from "./speech";

type Scene = {
  id: string;
  visual: string;
  chapter: string;
  heading: string;
  narration: string;
};

const site = dirname(import.meta.dir);
const repo = dirname(site);
const output = join(site, "public", "videos");
const work = join(site, ".video", "overview");
const screens = join(site, "video", "screen");

const run = async (args: string[], label: string) => {
  const child = Bun.spawn(args, { stdout: "ignore", stderr: "pipe" });
  const error = await new Response(child.stderr).text();
  if (await child.exited !== 0) throw new Error(`${label}: ${error}`);
};

const capture = async (args: string[]) => {
  const child = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
  const result = await new Response(child.stdout).text();
  const error = await new Response(child.stderr).text();
  if (await child.exited !== 0) throw new Error(error);
  return result.trim();
};

const mediaDuration = async (path: string) =>
  Number(
    await capture([
      "ffprobe",
      "-v",
      "error",
      "-show_entries",
      "format=duration",
      "-of",
      "default=noprint_wrappers=1:nokey=1",
      path,
    ]),
  );

const clean = (value: string) =>
  value
    .trim()
    .replace(/^"|"$/g, "")
    .replace(/\*\*/g, "")
    .replace(/`/g, "")
    .replace(/<[^>]+>/g, "")
    .replace(/\s+/g, " ");

const xml = (value: string) =>
  value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");

const scenes = async () => {
  const text = await Bun.file(join(repo, "docs", "videos", "overview.md")).text();
  const result: Scene[] = [];
  for (const line of text.split("\n")) {
    if (!/^\|\s*\d{2}\s*\|/.test(line)) continue;
    const cells = line.split("|").slice(1, -1).map(clean);
    if (cells.length !== 5) continue;
    result.push({
      id: cells[0],
      visual: cells[1],
      chapter: cells[2],
      heading: cells[3],
      narration: cells[4],
    });
  }
  return result;
};

const wrap = (value: string, width: number, limit = 4) => {
  const words = clean(value).split(" ");
  const result: string[] = [];
  let row = "";
  for (const word of words) {
    if (`${row} ${word}`.trim().length > width && row) {
      result.push(row);
      row = word;
    } else {
      row = `${row} ${word}`.trim();
    }
  }
  if (row) result.push(row);
  return result.slice(0, limit);
};

const text = (
  lines: string[],
  x: number,
  y: number,
  size: number,
  color: string,
  weight = 600,
  spacing = 1.15,
) =>
  lines
    .map(
      (line, index) =>
        `<text x="${x}" y="${y + index * size * spacing}" fill="${color}" font-family="Arial, sans-serif" font-size="${size}" font-weight="${weight}">${xml(line)}</text>`,
    )
    .join("");

const stamp = (seconds: number) => {
  const value = Math.round(seconds * 1000);
  const hours = Math.floor(value / 3_600_000);
  const minutes = Math.floor((value % 3_600_000) / 60_000);
  const secs = Math.floor((value % 60_000) / 1000);
  const millis = value % 1000;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
};

const captionChunks = (value: string) => {
  const fits = (left: string, right: string) => {
    const joined = `${left} ${right}`.trim();
    return joined.split(/\s+/).length <= 12 && joined.length <= 72;
  };
  const sentences = value.match(/[^.!?]+[.!?]+|[^.!?]+$/g) ?? [value];
  const raw: string[] = [];
  for (const sentence of sentences) {
    const words = clean(sentence).split(" ");
    while (words.length > 0) {
      const chunk: string[] = [];
      while (words.length > 0 && (chunk.length === 0 || fits(chunk.join(" "), words[0]))) {
        chunk.push(words.shift()!);
      }
      raw.push(chunk.join(" "));
    }
  }
  const result: string[] = [];
  for (let index = 0; index < raw.length; index += 1) {
    const chunk = raw[index];
    const count = chunk.split(/\s+/).length;
    const next = raw[index + 1];
    const nextCount = next?.split(/\s+/).length ?? 0;
    if (count < 5 && next && count + nextCount <= 12 && fits(chunk, next)) {
      raw[index + 1] = `${chunk} ${next}`;
    } else if (count < 5 && result.length > 0) {
      const previous = result[result.length - 1];
      if (fits(previous, chunk)) {
        result[result.length - 1] = `${previous} ${chunk}`;
      } else {
        result.push(chunk);
      }
    } else {
      result.push(chunk);
    }
  }
  return result;
};

const captionDurations = (chunks: string[], total: number) => {
  const minimum = 1;
  const weights = chunks.map((chunk) => chunk.length);
  if (total < chunks.length * minimum) {
    const sum = weights.reduce((value, weight) => value + weight, 0);
    return weights.map((weight) => total * (weight / sum));
  }

  const durations = Array(chunks.length).fill(0) as number[];
  const active = new Set(chunks.map((_, index) => index));
  let remaining = total;
  while (active.size > 0) {
    const activeWeight = [...active].reduce((value, index) => value + weights[index], 0);
    const tooShort = [...active].filter(
      (index) => remaining * (weights[index] / activeWeight) < minimum,
    );
    if (tooShort.length === 0) {
      for (const index of active) {
        durations[index] = remaining * (weights[index] / activeWeight);
      }
      break;
    }
    for (const index of tooShort) {
      durations[index] = minimum;
      remaining -= minimum;
      active.delete(index);
    }
  }
  return durations;
};

const captionText = (value: string) => wrap(value, 42, 2).join("\n");

const accents = ["#33d6ff", "#80f0c0", "#ffb347", "#ff7085"];

const background = (accent: string) => `
  <rect width="1920" height="1080" fill="#05070b"/>
  <radialGradient id="glow"><stop stop-color="${accent}" stop-opacity=".18"/><stop offset="1" stop-color="${accent}" stop-opacity="0"/></radialGradient>
  <circle cx="1550" cy="260" r="650" fill="url(#glow)"/>
  <g opacity=".09" stroke="#9db2c3" stroke-width="1">
    ${Array.from({ length: 17 }, (_, index) => `<path d="M${index * 120} 0V1080"/>`).join("")}
    ${Array.from({ length: 10 }, (_, index) => `<path d="M0 ${index * 120}H1920"/>`).join("")}
  </g>
  <rect x="0" y="0" width="12" height="1080" fill="${accent}"/>
`;

const card = (x: number, y: number, width: number, height: number, label: string, accent: string) => `
  <rect x="${x}" y="${y}" width="${width}" height="${height}" rx="24" fill="#101722" stroke="#253445" stroke-width="2"/>
  <circle cx="${x + 38}" cy="${y + 42}" r="10" fill="${accent}"/>
  <text x="${x + 64}" y="${y + 52}" fill="#f4fbff" font-family="Arial, sans-serif" font-size="30" font-weight="700">${xml(label)}</text>
`;

const diagram = (scene: Scene, index: number) => {
  const accent = accents[index % accents.length];
  const header = `
    <text x="138" y="105" fill="${accent}" font-family="Arial, sans-serif" font-size="24" font-weight="700" letter-spacing="4">ASYLUM · START HERE · ${xml(scene.chapter.toUpperCase())}</text>
    ${text(wrap(scene.heading, 29, 3), 138, 235, 68, "#f4fbff", 750, 1.08)}
  `;
  let art = "";

  if (scene.visual === "title") {
    art = `
      <path d="M1260 455h390M1260 455v250M1455 455v250M1650 455v250" stroke="#34485b" stroke-width="5"/>
      ${card(1170, 390, 180, 170, "ASK", accents[0])}
      ${card(1365, 575, 180, 170, "CHECK", accents[1])}
      ${card(1560, 390, 180, 170, "KEEP", accents[2])}
      <text x="138" y="725" fill="#91a4b4" font-family="Arial, sans-serif" font-size="30">One safe workflow from first launch to a deliberate merge.</text>
      <rect x="138" y="800" width="525" height="72" rx="36" fill="#0d2937" stroke="#1c8ab5"/>
      <text x="400" y="847" text-anchor="middle" fill="#b9ecff" font-family="Arial, sans-serif" font-size="24" font-weight="700">CAPTIONS ON · FULL TRANSCRIPT</text>
    `;
  } else if (scene.visual === "eli5") {
    art = `
      <path d="M1060 340H1740" stroke="#304355" stroke-width="8" stroke-linecap="round"/>
      ${card(1040, 380, 210, 250, "TABLE 1", accents[0])}
      ${card(1285, 380, 210, 250, "TABLE 2", accents[1])}
      ${card(1530, 380, 210, 250, "TABLE 3", accents[2])}
      <g fill="none" stroke-width="12" stroke-linecap="round">
        <path d="M1090 550l54-56 54 56" stroke="${accents[0]}"/>
        <path d="M1335 550l54-82 54 82" stroke="${accents[1]}"/>
        <path d="M1580 550l54-64 54 64" stroke="${accents[2]}"/>
      </g>
      ${text(["same blocks", "same instruction", "separate space"], 1090, 715, 27, "#91a4b4", 500, 1.55)}
    `;
  } else if (scene.visual === "nouns") {
    art = `
      ${card(1030, 310, 720, 150, "PROJECT", accents[0])}
      ${card(1100, 500, 650, 150, "TASK", accents[1])}
      ${card(1170, 690, 580, 150, "RUN", accents[2])}
      <text x="1072" y="420" fill="#91a4b4" font-family="Arial" font-size="24">your code repository</text>
      <text x="1142" y="610" fill="#91a4b4" font-family="Arial" font-size="24">one outcome, in ordinary language</text>
      <text x="1212" y="800" fill="#91a4b4" font-family="Arial" font-size="24">one agent's isolated attempt</text>
    `;
  } else if (scene.visual === "loop") {
    const labels = ["ASK", "RACE", "COMPARE", "KEEP"];
    art = labels
      .map((label, position) => {
        const angle = -Math.PI / 2 + position * (Math.PI / 2);
        const x = 1410 + Math.cos(angle) * 260;
        const y = 570 + Math.sin(angle) * 260;
        return `${card(x - 100, y - 65, 200, 130, label, accents[position])}`;
      })
      .join("") + `<circle cx="1410" cy="570" r="260" fill="none" stroke="#354a5e" stroke-width="5" stroke-dasharray="15 14"/><text x="1410" y="585" text-anchor="middle" fill="#f4fbff" font-family="Arial" font-size="42" font-weight="700">REPEAT</text>`;
  } else if (scene.visual === "audience") {
    art = `
      <path d="M1040 635H1760" stroke="#304355" stroke-width="12" stroke-linecap="round"/>
      <path d="M1040 635H1390" stroke="${accents[0]}" stroke-width="12" stroke-linecap="round"/>
      <circle cx="1080" cy="635" r="32" fill="${accents[0]}"/><circle cx="1390" cy="635" r="32" fill="${accents[1]}"/><circle cx="1720" cy="635" r="32" fill="${accents[2]}"/>
      ${text(["ONE AGENT", "VISIBLE DETAILS", "DEEP AUTOMATION"], 980, 735, 24, "#d7e4ed", 700, 1.4)}
      <text x="1040" y="880" fill="#91a4b4" font-family="Arial" font-size="26">The workflow stays the same. You choose how deep to go.</text>
    `;
  } else if (scene.visual === "layouts") {
    art = `
      ${card(1010, 350, 220, 300, "DUEL", accents[0])}
      ${card(1260, 350, 220, 300, "TRIAD", accents[1])}
      ${card(1510, 350, 220, 300, "SWARM", accents[2])}
      <g fill="#f4fbff">${[1080, 1160].map((x) => `<circle cx="${x}" cy="535" r="21"/>`).join("")}${[1310, 1370, 1430].map((x) => `<circle cx="${x}" cy="535" r="17"/>`).join("")}${[1555, 1600, 1645, 1690].map((x) => `<circle cx="${x}" cy="515" r="15"/>`).join("")}</g>
      <text x="1010" y="740" fill="#91a4b4" font-family="Arial" font-size="26">Fan out when comparison is worth the time and cost.</text>
    `;
  } else if (scene.visual === "worktrees") {
    art = `
      ${card(1250, 280, 300, 120, "MAIN", accents[0])}
      <path d="M1400 400v90M1400 490H1110v80M1400 490v80M1400 490h290v80" fill="none" stroke="#40576b" stroke-width="6"/>
      ${card(980, 570, 260, 190, "RUN A", accents[0])}
      ${card(1270, 570, 260, 190, "RUN B", accents[1])}
      ${card(1560, 570, 260, 190, "RUN C", accents[2])}
      <text x="1400" y="850" text-anchor="middle" fill="#91a4b4" font-family="Arial" font-size="27">one history · separate folders · changes never collide</text>
    `;
  } else if (scene.visual === "merge") {
    art = `
      ${card(980, 330, 250, 130, "STAGE", accents[0])}
      ${card(1280, 330, 250, 130, "PREFLIGHT", accents[1])}
      ${card(1580, 330, 250, 130, "CONFIRM", accents[2])}
      <path d="M1230 395h50M1530 395h50" stroke="#526a7d" stroke-width="8"/>
      <path d="M1405 520v175" stroke="#526a7d" stroke-width="8"/>
      <rect x="1210" y="695" width="390" height="120" rx="60" fill="#173e31" stroke="#3aa873" stroke-width="3"/>
      <text x="1405" y="770" text-anchor="middle" fill="#b8f6d8" font-family="Arial" font-size="34" font-weight="700">YOUR DECISION</text>
    `;
  } else if (scene.visual === "safety") {
    art = `
      <path d="M1400 250l330 115v235c0 160-138 278-330 355-192-77-330-195-330-355V365z" fill="#0d1d28" stroke="${accent}" stroke-width="7"/>
      <path d="M1250 575l105 105 215-240" fill="none" stroke="#80f0c0" stroke-width="28" stroke-linecap="round" stroke-linejoin="round"/>
      ${text(["SMALL SCOPE", "VISIBLE COST", "EXPLICIT TRUST"], 1130, 830, 25, "#d7e4ed", 700, 1.45)}
    `;
  } else if (scene.visual === "finish") {
    const labels = ["OPEN", "ASK", "RUN", "REVIEW", "KEEP"];
    art = `<path d="M985 610H1770" stroke="#354a5e" stroke-width="10"/>` + labels.map((label, position) => {
      const x = 1030 + position * 175;
      return `<circle cx="${x}" cy="610" r="48" fill="#101722" stroke="${accents[position % accents.length]}" stroke-width="5"/><text x="${x}" y="705" text-anchor="middle" fill="#dce9f2" font-family="Arial" font-size="24" font-weight="700">${label}</text><text x="${x}" y="621" text-anchor="middle" fill="#f4fbff" font-family="Arial" font-size="28" font-weight="700">${position + 1}</text>`;
    }).join("") + `<text x="1380" y="850" text-anchor="middle" fill="#91a4b4" font-family="Arial" font-size="28">Separate tables. Visible evidence. One deliberate winner.</text>`;
  }

  return `<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080">${background(accent)}${header}${art}<text x="1780" y="980" text-anchor="end" fill="#344554" font-family="Arial" font-size="118" font-weight="700">${scene.id}</text></svg>`;
};

const screenshot = (visual: string) => {
  if (visual === "onboarding") return join(screens, "onboardingdemo.png");
  if (visual === "diff") return join(screens, "diffdemo.png");
  if (visual === "notes") return join(screens, "notesdemo.png");
  if (visual === "integrations") return join(screens, "integrationsdemo.png");
  return join(screens, "tasksdemo.png");
};

const screenOverlay = (scene: Scene, index: number) => {
  const accent = accents[index % accents.length];
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080">
    <defs><linearGradient id="shade" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#03060a" stop-opacity=".94"/><stop offset="1" stop-color="#03060a" stop-opacity="0"/></linearGradient></defs>
    <rect width="1920" height="270" fill="url(#shade)"/>
    <rect x="60" y="62" width="8" height="140" rx="4" fill="${accent}"/>
    <text x="98" y="105" fill="${accent}" font-family="Arial" font-size="21" font-weight="700" letter-spacing="3">${xml(scene.chapter.toUpperCase())}</text>
    ${text(wrap(scene.heading, 42, 2), 98, 171, 44, "#f4fbff", 750, 1.05)}
  </svg>`;
};

await mkdir(output, { recursive: true });
await mkdir(join(output, "transcripts"), { recursive: true });
await mkdir(work, { recursive: true });

const sequence = await scenes();
if (sequence.length === 0) throw new Error("overview script contains no scenes");
const reuseMedia = Bun.env.REUSE_MEDIA === "1";

const clips: string[] = [];
const captions = ["WEBVTT", "Kind: captions", "Language: en", ""];
const transcript = [
  "# Asylum: Start Here",
  "",
  "An inclusive first-launch tour from the basic mental model through safe review and merge.",
  "",
];
const chapterStarts: { title: string; start: number }[] = [];
let cursor = 0;
let previousChapter = "";
let captionIndex = 0;

for (const [index, scene] of sequence.entries()) {
  const speech = join(work, `${scene.id}.wav`);
  const svg = join(work, `${scene.id}.svg`);
  const frame = join(work, `${scene.id}.png`);
  const clip = join(work, `${scene.id}.mp4`);
  const overlaySvg = join(work, `${scene.id}-overlay.svg`);
  const overlay = join(work, `${scene.id}-overlay.png`);

  console.log(`[${scene.id}/${sequence.length}] ${scene.heading}`);
  if (!reuseMedia || !(await Bun.file(speech).exists())) {
    await synthesize({ text: scene.narration, voice: "af_heart", speed: 1.02, output: speech });
  }
  const spoken = await mediaDuration(speech);
  const length = Math.max(spoken + 1.05, 4.5);
  const isScreen = ["onboarding", "tasks", "diff", "notes", "integrations"].includes(scene.visual);

  if (reuseMedia && (await Bun.file(clip).exists())) {
    // Caption-only passes still use the exact speech and clip durations that
    // produced the master, avoiding a lossy re-encode while timing is refined.
  } else if (isScreen) {
    await Bun.write(overlaySvg, screenOverlay(scene, index));
    await run(["rsvg-convert", "-o", overlay, overlaySvg], `overlay ${scene.id}`);
    await run(
      [
        "ffmpeg",
        "-y",
        "-loop",
        "1",
        "-framerate",
        "30",
        "-i",
        screenshot(scene.visual),
        "-loop",
        "1",
        "-framerate",
        "30",
        "-i",
        overlay,
        "-i",
        speech,
        "-t",
        String(length),
        "-filter_complex",
        `[0:v]scale=2000:1125,crop=1920:1080:x='(iw-ow)/2+10*sin(t/8)':y='(ih-oh)/2+6*cos(t/7)'[base];[1:v]format=rgba,fade=t=out:st=2.8:d=0.55:alpha=1[title];[base][title]overlay,fade=t=in:st=0:d=0.28,fade=t=out:st=${Math.max(0, length - 0.38)}:d=0.38[v]`,
        "-map",
        "[v]",
        "-map",
        "2:a",
        "-af",
        `adelay=260|260,apad=pad_dur=${length}`,
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-crf",
        "20",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-b:a",
        "160k",
        "-ar",
        "48000",
        "-shortest",
        clip,
      ],
      `screen scene ${scene.id}`,
    );
  } else {
    await Bun.write(svg, diagram(scene, index));
    await run(["rsvg-convert", "-o", frame, svg], `frame ${scene.id}`);
    await run(
      [
        "ffmpeg",
        "-y",
        "-loop",
        "1",
        "-framerate",
        "30",
        "-i",
        frame,
        "-i",
        speech,
        "-t",
        String(length),
        "-vf",
        `scale=1950:1097,crop=1920:1080:x='(iw-ow)/2+4*sin(t/9)':y='(ih-oh)/2',fade=t=in:st=0:d=0.28,fade=t=out:st=${Math.max(0, length - 0.38)}:d=0.38`,
        "-af",
        `adelay=260|260,apad=pad_dur=${length}`,
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-crf",
        "20",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-b:a",
        "160k",
        "-ar",
        "48000",
        "-shortest",
        clip,
      ],
      `diagram scene ${scene.id}`,
    );
  }

  if (scene.chapter !== previousChapter) {
    chapterStarts.push({ title: scene.chapter, start: cursor });
    previousChapter = scene.chapter;
  }
  const chunks = captionChunks(scene.narration);
  const durations = captionDurations(chunks, spoken);
  let captionCursor = cursor + 0.26;
  for (const [chunkIndex, chunk] of chunks.entries()) {
    const end =
      chunkIndex === chunks.length - 1
        ? cursor + 0.26 + spoken
        : captionCursor + durations[chunkIndex];
    captionIndex += 1;
    captions.push(
      String(captionIndex),
      `${stamp(captionCursor)} --> ${stamp(end)}`,
      captionText(chunk),
      "",
    );
    captionCursor = end;
  }
  transcript.push(`## ${scene.chapter}: ${scene.heading}`, "", scene.narration, "");
  clips.push(clip);
  cursor += length;
}

const list = join(work, "concat.txt");
await Bun.write(list, clips.map((path) => `file '${path.replaceAll("'", "'\\''")}'`).join("\n"));
const joined = join(work, "joined.mp4");
await run(["ffmpeg", "-y", "-f", "concat", "-safe", "0", "-i", list, "-c", "copy", joined], "assemble overview");

const video = join(output, "overview.mp4");
await run(
  [
    "ffmpeg",
    "-y",
    "-i",
    joined,
    "-f",
    "lavfi",
    "-i",
    `sine=frequency=55:sample_rate=48000:duration=${cursor}`,
    "-filter_complex",
    "[1:a]volume=0.004,tremolo=f=0.16:d=0.5[bed];[0:a][bed]amix=inputs=2:duration=first,loudnorm=I=-16:LRA=7:TP=-1.5[a]",
    "-map",
    "0:v",
    "-map",
    "[a]",
    "-c:v",
    "copy",
    "-c:a",
    "aac",
    "-b:a",
    "192k",
    "-movflags",
    "+faststart",
    video,
  ],
  "mix overview",
);

const chapters = ["WEBVTT", "Kind: chapters", "Language: en", ""];
for (const [index, chapter] of chapterStarts.entries()) {
  const end = chapterStarts[index + 1]?.start ?? cursor;
  chapters.push(String(index + 1), `${stamp(chapter.start)} --> ${stamp(end)}`, chapter.title, "");
}

await Bun.write(join(output, "overview.vtt"), captions.join("\n"));
await Bun.write(join(output, "overviewchapters.vtt"), chapters.join("\n"));
await Bun.write(join(output, "transcripts", "overview.md"), transcript.join("\n"));
await copyFile(join(work, "01.png"), join(output, "overview.png"));
console.log(`overview: ${sequence.length} scenes, ${cursor.toFixed(1)} seconds`);
