import { mkdir } from "node:fs/promises";
import { basename, dirname, join } from "node:path";

const site = dirname(import.meta.dir);
const videos = join(site, "public", "videos");
const screens = join(import.meta.dir, "screen");
const work = join(site, ".video", "remake");

const lessons: Record<string, string[]> = {
  trailer: ["notes.png", "integrations.png", "diff.png", "settings.png"],
  episode01: ["notes.png", "diff.png", "integrations.png"],
  episode02: ["settings.png", "integrations.png"],
  episode03: ["integrations.png", "notes.png", "diff.png"],
  episode04: ["integrations.png", "diff.png", "notes.png"],
  episode05: ["diff.png", "notes.png", "integrations.png"],
  episode06: ["diff.png", "settings.png"],
  episode07: ["diff.png", "integrations.png"],
  episode08: ["notes.png", "settings.png", "integrations.png"],
  episode09: ["integrations.png", "settings.png", "notes.png", "diff.png"],
  episode10: ["settings.png", "diff.png", "integrations.png", "notes.png"],
};

const run = async (args: string[], label: string) => {
  const child = Bun.spawn(args, { stdout: "ignore", stderr: "pipe" });
  const error = await new Response(child.stderr).text();
  if (await child.exited !== 0) throw new Error(`${label}: ${error}`);
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

const segment = (length: number) =>
  `[0:v]scale=1920:1080:force_original_aspect_ratio=increase,crop=1920:1080,gblur=sigma=28,eq=brightness=-0.30[back];` +
  `[0:v]scale=1760:990:force_original_aspect_ratio=decrease[screen];` +
  `[back][screen]overlay=(W-w)/2:(H-h)/2,drawbox=x=(iw-1766)/2:y=(ih-996)/2:w=1766:h=996:color=0x33d6ff@0.72:t=3,` +
  `fade=t=in:st=0:d=0.28,fade=t=out:st=${Math.max(0, length - 0.28)}:d=0.28,format=yuv420p[video]`;

const postersegment =
  `[0:v]scale=1920:1080:force_original_aspect_ratio=increase,crop=1920:1080,gblur=sigma=28,eq=brightness=-0.30[back];` +
  `[0:v]scale=1760:990:force_original_aspect_ratio=decrease[screen];` +
  `[back][screen]overlay=(W-w)/2:(H-h)/2,drawbox=x=(iw-1766)/2:y=(ih-996)/2:w=1766:h=996:color=0x33d6ff@0.72:t=3,format=yuv420p[video]`;

const makeposter = async (screen: string, output: string) => run([
  "ffmpeg", "-y", "-i", screen, "-filter_complex", postersegment,
  "-map", "[video]", "-frames:v", "1", output,
], `poster ${basename(output)}`);

await mkdir(work, { recursive: true });

for (const [id, images] of Object.entries(lessons)) {
  const source = join(videos, `${id}.mp4`);
  if (!(await Bun.file(source).exists())) continue;
  const length = await duration(source);
  const part = length / images.length;
  const clips: string[] = [];

  for (const [index, image] of images.entries()) {
    const clip = join(work, `${id}${index + 1}.mp4`);
    const remaining = length - part * index;
    const cliplength = index === images.length - 1 ? remaining : part;
    await run([
      "ffmpeg", "-y", "-loop", "1", "-i", join(screens, image),
      "-t", String(cliplength), "-filter_complex", segment(cliplength),
      "-map", "[video]", "-an", "-r", "30", "-c:v", "libx264",
      "-preset", "veryfast", "-crf", "21", "-pix_fmt", "yuv420p", clip,
    ], `${id} screenshot ${index + 1}`);
    clips.push(clip);
  }

  const list = join(work, `${id}.txt`);
  const silent = join(work, `${id}.mp4`);
  const rebuilt = join(work, `${id}final.mp4`);
  await Bun.write(list, clips.map((path) => `file '${path}'`).join("\n"));
  await run(["ffmpeg", "-y", "-f", "concat", "-safe", "0", "-i", list, "-c", "copy", silent], `${id} assembly`);
  await run([
    "ffmpeg", "-y", "-i", silent, "-i", source, "-map", "0:v", "-map", "1:a?",
    "-c:v", "copy", "-c:a", "copy", "-t", String(length), "-movflags", "+faststart", rebuilt,
  ], `${id} audio`);
  await Bun.write(source, Bun.file(rebuilt));
  await makeposter(join(screens, images[0]), join(videos, `${id}.png`));
  console.log(`${id}: ${images.length} screenshots, ${length.toFixed(1)} seconds`);
}
