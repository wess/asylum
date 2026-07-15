type Speech = {
  text: string;
  voice: string;
  speed: number;
  output: string;
};

const endpoint = "http://tts.local/tts";

export const synthesize = async ({ text, voice, speed, output }: Speech) => {
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
};
