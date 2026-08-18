/// The drawn scenes and the text furniture shared by every video script: title
/// plates, the lower third laid over recorded footage, caption chunking, and
/// WebVTT timestamps. Kept in one place so two videos cannot drift into two
/// different-looking title cards.

export type Scene = {
  id: string;
  visual: string;
  chapter: string;
  heading: string;
  narration: string;
};

export const accents = ["#33d6ff", "#80f0c0", "#ffb347", "#ff7085"];

export const clean = (value: string) =>
  value
    .trim()
    .replace(/^"|"$/g, "")
    .replace(/\*\*/g, "")
    .replace(/`/g, "")
    .replace(/<[^>]+>/g, "")
    .replace(/\s+/g, " ");

export const xml = (value: string) =>
  value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");

/// Parse a shot-list table out of a script. Rows are `| ID | Visual | Chapter |
/// Heading | Narration |`, and anything that is not a numbered row is prose.
export const scenesFrom = async (path: string) => {
  const text = await Bun.file(path).text();
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

export const wrap = (value: string, width: number, limit = 4) => {
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

export const text = (
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

export const stamp = (seconds: number) => {
  const value = Math.round(seconds * 1000);
  const hours = Math.floor(value / 3_600_000);
  const minutes = Math.floor((value % 3_600_000) / 60_000);
  const secs = Math.floor((value % 60_000) / 1000);
  const millis = value % 1000;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
};

export const captionChunks = (value: string) => {
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
    const piece = raw[index];
    const previous = result[result.length - 1];
    if (previous && piece.split(/\s+/).length <= 2 && fits(previous, piece)) {
      result[result.length - 1] = `${previous} ${piece}`;
      continue;
    }
    result.push(piece);
  }
  return result;
};

/// Split a scene's spoken length across its caption chunks by character count,
/// so a long line stays up longer than a short one.
export const captionDurations = (chunks: string[], total: number) => {
  const weights = chunks.map((chunk) => Math.max(chunk.length, 12));
  const sum = weights.reduce((left, right) => left + right, 0);
  return weights.map((weight) => (weight / sum) * total);
};

export const captionText = (value: string) => wrap(value, 42, 2).join("\n");

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

/// A drawn scene. `eyebrow` is the strip above the heading, so a second video
/// can label its plates without redefining the artwork.
export const diagram = (scene: Scene, index: number, eyebrow: string) => {
  const accent = accents[index % accents.length];
  const header = `
    <text x="138" y="105" fill="${accent}" font-family="Arial, sans-serif" font-size="24" font-weight="700" letter-spacing="4">${xml(eyebrow.toUpperCase())} · ${xml(scene.chapter.toUpperCase())}</text>
    ${text(wrap(scene.heading, 29, 3), 138, 235, 68, "#f4fbff", 750, 1.08)}
  `;
  let art = "";

  if (scene.visual === "title") {
    art = `
      <path d="M1260 455h390M1260 455v250M1455 455v250M1650 455v250" stroke="#34485b" stroke-width="5"/>
      ${card(1170, 390, 180, 170, "ASK", accents[0])}
      ${card(1365, 575, 180, 170, "CHECK", accents[1])}
      ${card(1560, 390, 180, 170, "KEEP", accents[2])}
      <text x="138" y="725" fill="#91a4b4" font-family="Arial, sans-serif" font-size="30">One safe workflow, from first launch to a deliberate merge.</text>
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
    art =
      labels
        .map((label, position) => {
          const angle = -Math.PI / 2 + position * (Math.PI / 2);
          const x = 1410 + Math.cos(angle) * 260;
          const y = 570 + Math.sin(angle) * 260;
          return `${card(x - 100, y - 65, 200, 130, label, accents[position])}`;
        })
        .join("") +
      `<circle cx="1410" cy="570" r="260" fill="none" stroke="#354a5e" stroke-width="5" stroke-dasharray="15 14"/><text x="1410" y="585" text-anchor="middle" fill="#f4fbff" font-family="Arial" font-size="42" font-weight="700">REPEAT</text>`;
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
    art =
      `<path d="M985 610H1770" stroke="#354a5e" stroke-width="10"/>` +
      labels
        .map((label, position) => {
          const x = 1030 + position * 175;
          return `<circle cx="${x}" cy="610" r="48" fill="#101722" stroke="${accents[position % accents.length]}" stroke-width="5"/><text x="${x}" y="705" text-anchor="middle" fill="#dce9f2" font-family="Arial" font-size="24" font-weight="700">${label}</text><text x="${x}" y="621" text-anchor="middle" fill="#f4fbff" font-family="Arial" font-size="28" font-weight="700">${position + 1}</text>`;
        })
        .join("") +
      `<text x="1380" y="850" text-anchor="middle" fill="#91a4b4" font-family="Arial" font-size="28">Separate tables. Visible evidence. One deliberate winner.</text>`;
  }

  return `<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080">${background(accent)}${header}${art}<text x="1780" y="980" text-anchor="end" fill="#344554" font-family="Arial" font-size="118" font-weight="700">${scene.id}</text></svg>`;
};

/// The lower third laid over recorded footage: a chapter strip and the heading,
/// on a gradient that keeps both readable over a bright interface.
export const overlay = (scene: Scene, index: number) => {
  const accent = accents[index % accents.length];
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080">
    <defs><linearGradient id="shade" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#03060a" stop-opacity=".94"/><stop offset="1" stop-color="#03060a" stop-opacity="0"/></linearGradient></defs>
    <rect width="1920" height="270" fill="url(#shade)"/>
    <rect x="60" y="62" width="8" height="140" rx="4" fill="${accent}"/>
    <text x="98" y="105" fill="${accent}" font-family="Arial" font-size="21" font-weight="700" letter-spacing="3">${xml(scene.chapter.toUpperCase())}</text>
    ${text(wrap(scene.heading, 42, 2), 98, 171, 44, "#f4fbff", 750, 1.05)}
  </svg>`;
};
