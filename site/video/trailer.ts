import { mkdir } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { synthesize } from "./speech";

type Scene = {
  title: string;
  kicker: string;
  line: string;
  plate: string;
  screen?: string;
};

const site = dirname(import.meta.dir);
const plates = join(import.meta.dir, "plate");
const outputdir = join(site, "public", "videos");
const work = join(site, ".video", "trailer");
const output = join(outputdir, "trailer.mp4");
const captions = join(outputdir, "trailer.vtt");

const scenes: Scene[] = [
  { kicker: "THE AGENT DEVELOPMENT ENVIRONMENT", title: "ASYLUM", line: "One task should never be limited to one answer.", plate: "08.png" },
  { kicker: "ONE TASK · MULTIPLE RUNS", title: "WORK IN PARALLEL", line: "Asylum gives every agent an isolated place to work.", plate: "01.png", screen: "video/screen/notes.png" },
  { kicker: "EVERY ATTEMPT · ONE VIEW", title: "SEE THE WHOLE FLEET", line: "Follow progress, inspect context, and keep every attempt visible.", plate: "04.png", screen: "video/screen/integrations.png" },
  { kicker: "DIFFS · CHECKS · FEEDBACK", title: "COMPARE THE EVIDENCE", line: "Review what changed. Verify the result. Send precise feedback.", plate: "05.png", screen: "video/screen/diff.png" },
  { kicker: "THE STRONGEST RESULT", title: "CHOOSE WITH CONFIDENCE", line: "When the evidence is clear, merge the winner.", plate: "06.png", screen: "video/screen/settings.png" },
  { kicker: "BUILD · VERIFY · CONVERGE", title: "ASYLUM", line: "Parallel work. One confident decision.", plate: "08.png" },
];

const run = async (args: string[], label: string) => {
  const child = Bun.spawn(args, { stdout: "ignore", stderr: "pipe" });
  const error = await new Response(child.stderr).text();
  const code = await child.exited;
  if (code !== 0) throw new Error(`${label}: ${error}`);
};

const capture = async (args: string[]) => {
  const child = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
  const value = await new Response(child.stdout).text();
  const error = await new Response(child.stderr).text();
  if (await child.exited !== 0) throw new Error(error);
  return value.trim();
};

const duration = async (path: string) => Number(await capture([
  "ffprobe", "-v", "error", "-show_entries", "format=duration",
  "-of", "default=nw=1:nk=1", path,
]));

const xml = (value: string) => value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");

const stamp = (seconds: number) => {
  const millis = Math.round(seconds * 1000);
  const hours = Math.floor(millis / 3_600_000);
  const minutes = Math.floor((millis % 3_600_000) / 60_000);
  const secs = Math.floor((millis % 60_000) / 1000);
  const ms = millis % 1000;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}.${String(ms).padStart(3, "0")}`;
};

await mkdir(outputdir, { recursive: true });
await mkdir(work, { recursive: true });

const clips: string[] = [];
const cues = ["WEBVTT", ""];
let cursor = 0;

for (const [index, scene] of scenes.entries()) {
  const id = String(index + 1).padStart(2, "0");
  const speech = join(work, `${id}.wav`);
  const title = join(work, `${id}.svg`);
  const titlepng = join(work, `${id}.png`);
  const clip = join(work, `${id}.mp4`);
  await synthesize({ text: scene.line, voice: "am_onyx", speed: 0.92, output: speech });
  const spoken = await duration(speech);
  const length = Math.max(spoken + 0.8, index === scenes.length - 1 ? 5.4 : 4.5);
  const accent = index === 4 ? "#ff8a2a" : "#57fff0";
  const titletext = xml(scene.title);
  const kicker = xml(scene.kicker);
  await Bun.write(title, `<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080">
    <defs><filter id="glow"><feGaussianBlur stdDeviation="9" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>
    <path d="M104 752H520" stroke="${accent}" stroke-width="7"/><path d="M1400 752H1816" stroke="${accent}" stroke-width="7"/>
    <text x="960" y="706" text-anchor="middle" fill="#020609" stroke="#020609" stroke-width="24" paint-order="stroke" font-family="DIN Condensed" font-size="116" font-weight="700" letter-spacing="5">${titletext}</text>
    <text x="960" y="706" text-anchor="middle" fill="#f5ffff" stroke="${accent}" stroke-width="2" paint-order="stroke" filter="url(#glow)" font-family="DIN Condensed" font-size="116" font-weight="700" letter-spacing="5">${titletext}</text>
    <text x="960" y="794" text-anchor="middle" fill="${accent}" stroke="#020609" stroke-width="8" paint-order="stroke" font-family="Impact" font-size="30" letter-spacing="8">${kicker}</text>
    <rect x="780" y="838" width="360" height="3" fill="${accent}"/><rect x="938" y="828" width="44" height="23" fill="${accent}" transform="skewX(-24)"/>
  </svg>`);
  await run(["rsvg-convert", "-o", titlepng, title], `title ${id}`);
  const zoom = index % 2 === 0 ? "min(zoom+0.00055,1.10)" : "if(eq(on,1),1.08,max(zoom-0.00035,1.01))";
  const flash = index === 0 ? "" : ",fade=t=in:st=0:d=0.10:color=white";
  const base = `[0:v]scale=2300:-2,crop=2200:1238,zoompan=z='${zoom}':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=${Math.ceil(length * 30)}:s=1920x1080:fps=30,eq=contrast=1.08:saturation=0.88:brightness=-0.10,noise=alls=2:allf=t,vignette=PI/4${flash}[base]`;
  const words = `[1:v]format=rgba,fade=t=in:st=0.18:d=0.45:alpha=1,fade=t=out:st=${Math.max(0, length - 0.45)}:d=0.4:alpha=1[words]`;
  const screen = scene.screen
    ? `;[2:v]scale=1180:-2,pad=iw+6:ih+6:3:3:color=0x57fff0,format=rgba,fade=t=in:st=0.25:d=0.55:alpha=1,fade=t=out:st=${Math.max(0, length - 0.5)}:d=0.45:alpha=1[card];[base][card]overlay=(W-w)/2:(H-h)/2-70[stage];[stage][words]overlay=0:0,fade=t=out:st=${Math.max(0, length - 0.2)}:d=0.2[video]`
    : `;[base][words]overlay=0:0,fade=t=out:st=${Math.max(0, length - 0.2)}:d=0.2[video]`;
  const filter = `${base};${words}${screen}`;
  const screeninput = scene.screen ? ["-loop", "1", "-i", join(site, scene.screen)] : [];
  const speechindex = scene.screen ? "3:a" : "2:a";
  await run([
    "ffmpeg", "-y", "-loop", "1", "-i", join(plates, scene.plate), "-loop", "1", "-i", titlepng,
    ...screeninput, "-i", speech,
    "-filter_complex", filter, "-map", "[video]", "-map", speechindex, "-t", String(length),
    "-af", `highpass=f=60,lowpass=f=10500,bass=g=2:f=110,acompressor=threshold=.12:ratio=3:attack=10:release=130,aecho=.8:.18:42:.05,adelay=180|180,apad=pad_dur=${length}`,
    "-c:v", "libx264", "-preset", "medium", "-crf", "18", "-pix_fmt", "yuv420p",
    "-c:a", "aac", "-ar", "48000", "-ac", "2", "-b:a", "192k", "-shortest", clip,
  ], `scene ${id}`);
  clips.push(clip);
  cues.push(String(index + 1), `${stamp(cursor + .18)} --> ${stamp(cursor + .18 + spoken)}`, scene.line, "");
  cursor += length;
}

const list = join(work, "concat.txt");
const joined = join(work, "joined.mp4");
await Bun.write(list, clips.map((path) => `file '${path}'`).join("\n"));
await run(["ffmpeg", "-y", "-f", "concat", "-safe", "0", "-i", list, "-c:v", "libx264", "-preset", "fast", "-crf", "18", "-c:a", "aac", "-ar", "48000", "-ac", "2", joined], "assembly");

const endfade = Math.max(0, cursor - 4);
await run([
  "ffmpeg", "-y", "-i", joined,
  "-f", "lavfi", "-i", `sine=frequency=42:sample_rate=48000:duration=${cursor}`,
  "-f", "lavfi", "-i", `sine=frequency=84:sample_rate=48000:duration=${cursor}`,
  "-f", "lavfi", "-i", `sine=frequency=252:sample_rate=48000:duration=${cursor}`,
  "-f", "lavfi", "-i", `anoisesrc=color=pink:amplitude=.24:sample_rate=48000:duration=${cursor}`,
  "-f", "lavfi", "-i", `aevalsrc=sin(2*PI*48*t)*exp(-15*(t-floor(t/2.25)*2.25)):s=48000:d=${cursor}`,
  "-filter_complex",
  `[1:a]volume=.20,lowpass=f=135,tremolo=f=2:d=.66[bass];[2:a]volume=.12,lowpass=f=1250,tremolo=f=4:d=.78[pulse];[3:a]volume=.075,highpass=f=160,tremolo=f=8:d=.84[seq];[4:a]highpass=f=420,lowpass=f=8500,tremolo=f=4:d=.91,volume=.14[perc];[5:a]lowpass=f=175,volume=.34[hit];[bass][pulse][seq][perc][hit]amix=inputs=5:normalize=0,acompressor=threshold=.15:ratio=3:attack=12:release=210,afade=t=in:st=0:d=1.0,afade=t=out:st=${endfade}:d=4,volume=.90[music];[0:a]volume=1.14,asplit=2[voice][key];[music][key]sidechaincompress=threshold=.025:ratio=5:attack=12:release=320[ducked];[voice][ducked]amix=inputs=2:duration=first:weights='1 1.52',loudnorm=I=-14.5:LRA=9:TP=-1.2,alimiter=limit=.94[a]`,
  "-map", "0:v", "-map", "[a]", "-c:v", "copy", "-c:a", "aac", "-ar", "48000", "-ac", "2", "-b:a", "256k", "-movflags", "+faststart", output,
], "score");

await Bun.write(captions, cues.join("\n"));
console.log(`created ${basename(output)} (${cursor.toFixed(1)} seconds)`);
