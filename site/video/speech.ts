import { unlink } from "node:fs/promises";

type Speech = {
  text: string;
  voice: string;
  speed: number;
  output: string;
};

const endpoint = "http://tts.local/tts";

export const synthesize = async ({ text, voice, speed, output }: Speech) => {
  try {
    const response = await fetch(endpoint, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ text, voice, speed }),
      signal: AbortSignal.timeout(120_000),
    });

    if (!response.ok) {
      throw new Error(`speech synthesis failed (${response.status}): ${await response.text()}`);
    }

    const audio = await response.arrayBuffer();
    if (audio.byteLength < 1_000) throw new Error("speech synthesis returned an empty clip");
    await Bun.write(output, audio);
  } catch (error) {
    if (process.platform !== "darwin") throw error;
    await local(text, speed, output);
  }
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
  if (await child.exited !== 0) throw new Error(`${args[0]} failed: ${error}`);
};
