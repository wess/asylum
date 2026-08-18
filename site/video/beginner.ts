/// Renders the beginner tour from `docs/videos/beginner.md`.
///
/// Two kinds of scene. A drawn plate, for the ideas that have no interface yet,
/// and a `shot:` scene, which is real footage of the application: frames
/// captured by the recorder in `crates/app/src/shot.rs` while it drove the
/// window offscreen. Narration is synthesized first, and the footage is fitted
/// to it — by stretching the pauses rather than the movement, so the pointer
/// keeps a human speed however long the sentence turns out to be.

import { copyFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";

import {
  captionChunks,
  captionDurations,
  captionText,
  diagram,
  overlay,
  scenesFrom,
  stamp,
  type Scene,
} from "./cards";
import { synthesize } from "./speech";

const site = dirname(import.meta.dir);
const repo = dirname(site);
const output = join(site, "public", "videos");
const work = join(site, ".video", "beginner");
const shots = join(site, ".video", "shots");

const FPS = 30;
/// A frame held no longer than one tick is part of a movement, not a pause.
const TICK = 1 / FPS + 0.0005;

const run = async (args: string[], label: string) => {
  const child = Bun.spawn(args, { stdout: "ignore", stderr: "pipe" });
  const error = await new Response(child.stderr).text();
  if ((await child.exited) !== 0) throw new Error(`${label}: ${error}`);
};

const capture = async (args: string[]) => {
  const child = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
  const result = await new Response(child.stdout).text();
  const error = await new Response(child.stderr).text();
  if ((await child.exited) !== 0) throw new Error(error);
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

type Frame = { file: string; hold: number };

/// Fit a recorded scene to the narration. Extra time goes to the frames that
/// were already pauses; movement keeps its recorded speed. When the narration is
/// shorter than the movement itself, everything compresses together, which is
/// the only case where the pointer changes pace.
const fit = (frames: Frame[], target: number): Frame[] => {
  const motion = frames.reduce((total, frame) => total + Math.min(frame.hold, TICK), 0);
  const pause = frames.reduce((total, frame) => total + Math.max(frame.hold - TICK, 0), 0);

  if (target < motion + 0.2) {
    const scale = target / Math.max(motion + pause, 0.001);
    return frames.map((frame) => ({ ...frame, hold: frame.hold * scale }));
  }
  if (pause < 0.001) {
    const padded = frames.map((frame) => ({ ...frame }));
    padded[padded.length - 1].hold += target - motion;
    return padded;
  }
  const scale = (target - motion) / pause;
  return frames.map((frame) => ({
    ...frame,
    hold: Math.min(frame.hold, TICK) + Math.max(frame.hold - TICK, 0) * scale,
  }));
};

/// The concat demuxer reads the last entry's duration from the following line,
/// so the final image is listed twice.
const concatList = (dir: string, frames: Frame[]) => {
  const lines: string[] = [];
  for (const frame of frames) {
    lines.push(`file '${join(dir, frame.file).replaceAll("'", "'\\''")}'`);
    lines.push(`duration ${frame.hold.toFixed(4)}`);
  }
  const last = frames[frames.length - 1];
  lines.push(`file '${join(dir, last.file).replaceAll("'", "'\\''")}'`);
  return lines.join("\n");
};

await mkdir(output, { recursive: true });
await mkdir(join(output, "transcripts"), { recursive: true });
await mkdir(work, { recursive: true });

const sequence = await scenesFrom(join(repo, "docs", "videos", "beginner.md"));
if (sequence.length === 0) throw new Error("beginner script contains no scenes");
const reuse = Bun.env.REUSE_MEDIA === "1";
const voice = Bun.env.TTS_VOICE ?? "male";
const speed = Number(Bun.env.TTS_SPEED ?? "0.96");

const clips: string[] = [];
const captions = ["WEBVTT", "Kind: captions", "Language: en", ""];
const transcript = [
  "# Asylum: A Slow Start",
  "",
  "A patient first tour: the mental model, one task from blank composer to merge, and where secrets live.",
  "",
];
const chapterStarts: { title: string; start: number }[] = [];
let cursor = 0;
let previousChapter = "";
let captionIndex = 0;

for (const [index, scene] of sequence.entries()) {
  const speech = join(work, `${scene.id}.wav`);
  const clip = join(work, `${scene.id}.mp4`);
  const overlaySvg = join(work, `${scene.id}-overlay.svg`);
  const overlayPng = join(work, `${scene.id}-overlay.png`);

  console.log(`[${scene.id}/${sequence.length.toString().padStart(2, "0")}] ${scene.heading}`);
  if (!reuse || !(await Bun.file(speech).exists())) {
    await synthesize({ text: scene.narration, voice, speed, output: speech });
  }
  const spoken = await mediaDuration(speech);
  // Room to breathe: a beginner needs a beat to look at the screen after a
  // sentence ends, and the leading `adelay` has to fit inside the clip too.
  const length = Math.max(spoken + 1.5, 5);

  if (reuse && (await Bun.file(clip).exists())) {
    // A caption-only pass reuses the master, avoiding a lossy re-encode.
  } else {
    await Bun.write(overlaySvg, overlay(scene, index));
    await run(["rsvg-convert", "-o", overlayPng, overlaySvg], `overlay ${scene.id}`);

    const shot = scene.visual.startsWith("shot:") ? scene.visual.slice(5) : "";
    if (shot) {
      const dir = join(shots, shot);
      const manifest = join(dir, "frames.json");
      if (!(await Bun.file(manifest).exists())) {
        throw new Error(`no recorded frames for '${shot}' (run the recorder first)`);
      }
      const frames: Frame[] = JSON.parse(await Bun.file(manifest).text());
      if (frames.length === 0) throw new Error(`recorded scene '${shot}' is empty`);
      const list = join(work, `${scene.id}-frames.txt`);
      await Bun.write(list, concatList(dir, fit(frames, length)));
      await run(
        [
          "ffmpeg",
          "-y",
          "-f",
          "concat",
          "-safe",
          "0",
          "-i",
          list,
          "-loop",
          "1",
          "-framerate",
          String(FPS),
          "-i",
          overlayPng,
          "-i",
          speech,
          "-t",
          String(length),
          "-filter_complex",
          `[0:v]fps=${FPS},scale=1920:1080:flags=lanczos,setsar=1,format=rgba[base];` +
            `[1:v]format=rgba,fade=t=out:st=3.6:d=0.6:alpha=1[title];` +
            `[base][title]overlay,fade=t=in:st=0:d=0.3,fade=t=out:st=${Math.max(0, length - 0.4)}:d=0.4[v]`,
          "-map",
          "[v]",
          "-map",
          "2:a",
          "-af",
          `adelay=300|300,apad=pad_dur=${length}`,
          "-c:v",
          "libx264",
          "-preset",
          "medium",
          "-crf",
          "19",
          "-pix_fmt",
          "yuv420p",
          "-c:a",
          "aac",
          "-b:a",
          "128k",
          "-ac",
          "1",
          "-ar",
          "48000",
          "-shortest",
          clip,
        ],
        `shot scene ${scene.id}`,
      );
    } else {
      const svg = join(work, `${scene.id}.svg`);
      const frame = join(work, `${scene.id}.png`);
      await Bun.write(svg, diagram(scene, index, "Asylum · a slow start"));
      await run(["rsvg-convert", "-o", frame, svg], `frame ${scene.id}`);
      await run(
        [
          "ffmpeg",
          "-y",
          "-loop",
          "1",
          "-framerate",
          String(FPS),
          "-i",
          frame,
          "-i",
          speech,
          "-t",
          String(length),
          "-vf",
          `scale=1950:1097,crop=1920:1080:x='(iw-ow)/2+4*sin(t/9)':y='(ih-oh)/2',fade=t=in:st=0:d=0.3,fade=t=out:st=${Math.max(0, length - 0.4)}:d=0.4`,
          "-af",
          `adelay=300|300,apad=pad_dur=${length}`,
          "-c:v",
          "libx264",
          "-preset",
          "medium",
          "-crf",
          "19",
          "-pix_fmt",
          "yuv420p",
          "-c:a",
          "aac",
          "-b:a",
          "128k",
          "-ac",
          "1",
          "-ar",
          "48000",
          "-shortest",
          clip,
        ],
        `plate scene ${scene.id}`,
      );
    }
  }

  if (scene.chapter !== previousChapter) {
    chapterStarts.push({ title: scene.chapter, start: cursor });
    previousChapter = scene.chapter;
  }
  const chunks = captionChunks(scene.narration);
  const durations = captionDurations(chunks, spoken);
  let captionCursor = cursor + 0.3;
  for (const [chunkIndex, chunk] of chunks.entries()) {
    const end =
      chunkIndex === chunks.length - 1
        ? cursor + 0.3 + spoken
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
await run(
  ["ffmpeg", "-y", "-f", "concat", "-safe", "0", "-i", list, "-c", "copy", joined],
  "assemble beginner",
);

const video = join(output, "beginner.mp4");

// The mix lands in a WAV before it is muxed, rather than being filtered straight
// into the final encode. Running the graph alongside `-c:v copy` in one pass put
// the peak 3dB over what the limiter had just set it to; through a file it
// arrives where it was measured. It also makes the mix inspectable when
// something sounds wrong.
const mix = join(work, "mix.wav");
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
    // `alimiter` re-normalises back to full scale unless `level` is disabled,
    // which quietly undoes the limit it was added to impose.
    "[1:a]volume=0.004,tremolo=f=0.16:d=0.5[bed];[0:a][bed]amix=inputs=2:duration=first," +
      "loudnorm=I=-16:LRA=7:TP=-1.5,alimiter=limit=0.7:level=disabled:attack=5:release=60," +
      "aformat=sample_rates=48000:channel_layouts=mono[a]",
    "-map",
    "[a]",
    "-c:a",
    "pcm_s16le",
    mix,
  ],
  "mix beginner",
);

await run(
  [
    "ffmpeg",
    "-y",
    "-i",
    joined,
    "-i",
    mix,
    "-map",
    "0:v",
    "-map",
    "1:a",
    "-c:v",
    "copy",
    // Mono at 48kHz. `loudnorm` resamples to 192kHz internally and the encoder
    // follows it up, which is how the earlier videos ended up carrying 185kbps
    // stereo AAC at 96kHz for one speaking voice — more than the picture costs.
    // There is no stereo information in a single centred narrator to keep.
    "-c:a",
    "aac",
    "-b:a",
    "112k",
    "-ac",
    "1",
    "-ar",
    "48000",
    "-movflags",
    "+faststart",
    video,
  ],
  "encode beginner",
);

const chapters = ["WEBVTT", "Kind: chapters", "Language: en", ""];
for (const [index, chapter] of chapterStarts.entries()) {
  const end = chapterStarts[index + 1]?.start ?? cursor;
  chapters.push(String(index + 1), `${stamp(chapter.start)} --> ${stamp(end)}`, chapter.title, "");
}

await Bun.write(join(output, "beginner.vtt"), captions.join("\n"));
await Bun.write(join(output, "beginnerchapters.vtt"), chapters.join("\n"));
await Bun.write(join(output, "transcripts", "beginner.md"), transcript.join("\n"));
await copyFile(join(work, "01.png"), join(output, "beginner.png"));

const minutes = Math.floor(cursor / 60);
console.log(
  `beginner: ${sequence.length} scenes, ${minutes}m ${Math.round(cursor % 60)}s -> ${video}`,
);
