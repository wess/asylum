import { unlink } from "node:fs/promises";
import { dirname, join } from "node:path";

type Speech = {
  text: string;
  voice: string;
  speed: number;
  output: string;
};

type Provider = {
  name: string;
  synthesize: (text: string, voice: string) => Promise<ArrayBuffer>;
};

const repo = dirname(dirname(import.meta.dir));

// Bun only reads `.env` from the working directory, and these scripts are run
// from several. Parsing it here keeps the key in the process and out of argv,
// which is where a `curl -H` would have put it.
const env = await (async () => {
  const file = Bun.file(join(repo, ".env"));
  const result: Record<string, string> = { ...process.env } as Record<string, string>;
  if (!(await file.exists())) return result;
  for (const line of (await file.text()).split("\n")) {
    const match = line.match(/^\s*([A-Z0-9_]+)\s*=\s*(.*)$/);
    if (!match) continue;
    result[match[1]] = match[2].trim().replace(/^["']|["']$/g, "");
  }
  return result;
})();

const fish: Provider = {
  name: "fish",
  synthesize: async (text, voice) => {
    const key = env.FISH_API_KEY;
    if (!key) throw new Error("FISH_API_KEY is not set");
    const reference = voice === "female" ? env.FISH_FEMALE_ID : env.FISH_MALE_ID;
    const response = await fetch("https://api.fish.audio/v1/tts", {
      method: "POST",
      headers: {
        authorization: `Bearer ${key}`,
        model: env.FISH_MODEL ?? "s2.1-pro",
        "content-type": "application/json",
      },
      body: JSON.stringify({ text, reference_id: reference, format: "mp3" }),
      signal: AbortSignal.timeout(180_000),
    });
    if (!response.ok) throw new Error(`fish ${response.status}: ${await response.text()}`);
    return response.arrayBuffer();
  },
};

const elevenlabs: Provider = {
  name: "elevenlabs",
  synthesize: async (text, voice) => {
    const key = env.ELEVENLABS_API_KEY;
    if (!key) throw new Error("ELEVENLABS_API_KEY is not set");
    const id = voice === "female" ? env.ELEVENLABS_FEMALE_VOICE_ID : env.ELEVENLABS_MALE_VOICE_ID;
    if (!id) throw new Error("no ElevenLabs voice id for this voice");
    const response = await fetch(`https://api.elevenlabs.io/v1/text-to-speech/${id}`, {
      method: "POST",
      headers: { "xi-api-key": key, "content-type": "application/json" },
      body: JSON.stringify({
        text,
        model_id: env.ELEVENLABS_MODEL ?? "eleven_multilingual_v2",
        // Steady over expressive: a tutorial read should not drift in tone
        // between scenes that were synthesized minutes apart.
        voice_settings: { stability: 0.55, similarity_boost: 0.75, style: 0.15 },
      }),
      signal: AbortSignal.timeout(180_000),
    });
    if (!response.ok) throw new Error(`elevenlabs ${response.status}: ${await response.text()}`);
    return response.arrayBuffer();
  },
};

const providers: Record<string, Provider> = { fish, elevenlabs };

/// The order to try: the configured provider first, then the other one. A key
/// that has expired should cost a warning line, not the whole render.
const order = () => {
  const preferred = (env.TTS_PROVIDER ?? "fish").toLowerCase();
  const rest = Object.keys(providers).filter((name) => name !== preferred);
  return [preferred, ...rest].map((name) => providers[name]).filter(Boolean);
};

const reported = new Set<string>();

export const synthesize = async ({ text, voice, speed, output }: Speech) => {
  const raw = `${output}.raw`;
  for (const provider of order()) {
    try {
      const audio = await provider.synthesize(text, voice);
      if (audio.byteLength < 1_000) throw new Error("returned an empty clip");
      await Bun.write(raw, audio);
      await encode(raw, speed, output);
      await unlink(raw).catch(() => {});
      if (!reported.has(provider.name)) {
        reported.add(provider.name);
        console.log(`  voice: ${provider.name}`);
      }
      return;
    } catch (error) {
      if (!reported.has(`fail:${provider.name}`)) {
        reported.add(`fail:${provider.name}`);
        console.warn(`  voice: ${provider.name} unavailable (${(error as Error).message.slice(0, 120)})`);
      }
    }
  }
  if (process.platform !== "darwin") throw new Error("no speech provider available");
  await local(text, speed, output);
};

/// Normalise whatever the provider returned to one format, and apply the pace
/// here rather than asking each provider for it: `atempo` sounds the same
/// whichever service produced the clip.
const encode = async (source: string, speed: number, output: string) => {
  const filters = ["dynaudnorm=p=0.9:m=6"];
  if (Math.abs(speed - 1) > 0.01) filters.unshift(`atempo=${speed.toFixed(3)}`);
  await run([
    "ffmpeg",
    "-y",
    "-i",
    source,
    "-af",
    filters.join(","),
    "-ar",
    "48000",
    "-ac",
    "2",
    output,
  ]);
};

const local = async (text: string, speed: number, output: string) => {
  const source = `${output}.aiff`;
  const rate = String(Math.round(175 * speed));
  await run(["say", "-v", "Samantha", "-r", rate, "-o", source, text]);
  await run(["ffmpeg", "-y", "-i", source, "-ar", "48000", "-ac", "2", output]);
  await unlink(source).catch(() => {});
};

const run = async (args: string[]) => {
  const child = Bun.spawn(args, { stdout: "ignore", stderr: "pipe" });
  const error = await new Response(child.stderr).text();
  if ((await child.exited) !== 0) throw new Error(`${args[0]} failed: ${error}`);
};
