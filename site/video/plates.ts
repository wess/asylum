import { mkdir } from "node:fs/promises";
import { join } from "node:path";

const output = join(import.meta.dir, "plate");

const prompts = [
  "original cinematic cyber action film, lone operator silhouette on a rain swept rooftop overlooking an immense black glass digital megacity, cyan energy grid igniting street by street, amber horizon, volumetric fog, anamorphic lighting, dynamic low camera, no text, no logos, no recognizable characters",
  "original cinematic cyber action film, abstract artificial intelligence agents materializing as five distinct armored energy silhouettes inside a vast dark command chamber, cyan teal and amber light trails, sparks and atmospheric smoke, dramatic wide composition, no text, no logos, no recognizable characters",
  "original cinematic cyber action film, high speed chase through a luminous data tunnel, black architecture with cyan circuitry, electric orange debris, extreme forward motion, tilted camera, premium theatrical lighting, no vehicles, no text, no logos",
  "original cinematic cyber action film, multiple parallel neon pathways splitting across a bottomless digital city, each path contains a fast moving energy figure, enormous scale, storm clouds, teal cyan and amber, dramatic aerial camera, no text, no logos",
  "original cinematic cyber action film, colossal arena made from floating code fragments and translucent glass, two abstract energy forces collide at the center creating a cyan shockwave, sparks, debris, dramatic action composition, no text, no logos, no recognizable characters",
  "original cinematic cyber action film, close view of holographic evidence panels and code diffs locking into a luminous geometric shield, mechanical precision, dark black background, cyan and red accents, powerful central composition, no readable text, no logos",
  "original cinematic cyber action film, victorious fleet of seven abstract armored silhouettes standing before a monumental portal of white cyan energy, black reflective floor, smoke, god rays, heroic low angle, orange rim light, no text, no logos, no recognizable characters",
  "original cinematic cyber action film, enormous angular letter A shaped monolith emerging from darkness above a glowing cyan horizon, black metal and glass, electric particles, symmetrical premium title reveal background, no words, no logos",
];

await mkdir(output, { recursive: true });

for (const [index, prompt] of prompts.entries()) {
  const response = await fetch("http://ai.local/image", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ prompt, size: "1920x1080", provider: "local" }),
    signal: AbortSignal.timeout(300_000),
  });
  if (!response.ok) throw new Error(`plate ${index + 1}: ${response.status} ${await response.text()}`);
  const body = await response.json() as { data: { b64_json: string }[] };
  await Bun.write(join(output, `${String(index + 1).padStart(2, "0")}.png`), Buffer.from(body.data[0].b64_json, "base64"));
  console.log(`plate ${index + 1} of ${prompts.length}`);
}
