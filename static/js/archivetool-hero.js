// dream_archivetool's hero: `extract-all`, staged as a machine, rendered live with three.js.
//
// The archive is Morrowind.bsa: a lacquered strongbox with brass bands, its name-hash table glowing
// down its flanks, soldered onto a circuit board like any other component. Its lid opens and its
// entries come out as file cards printed with the names the archive stored, spelled however its
// maker spelled them. A belt carries them past the policy gate, the icon's two crossed wrenches, where
// a scanner reads each name and rewrites it to its normalized form (lowercase, forward slashes) and
// the terminal the jaws hold prints the target. Nothing is written while the plan is being built:
// the cards wait on the plan table until every target has been checked, then drop into the folders
// of `out/`, a glass case they cannot land outside of, each one appearing as a temporary file and
// turning solid in one rename.
//
// The runs rotate through what the tool actually does. A clean run extracts everything. A dry run
// prints its plan as JSON and writes nothing. A run with an unsafe name (`..\..\Windows\notepad.exe`)
// stops at that name and puts every card back: an extract-all that fails has written nothing. A run
// with --skip-existing leaves the files already in out/ alone. Pulses run along the board's traces
// from the archive to the gate to the case while a run is live.
//
// The pointer is a lamp and tilts the view a little. Resting it on the archive makes its pressure
// gauge tremble. Holding the archive down is `--compress`: see `Press` below.
//
// Rendering follows the other DreamWeave heroes: a half-float scene target, bloom, ACES and dither in
// the composite, colours from the site's CSS tokens, adaptive resolution, nothing running while the
// hero is off screen or the tab hidden, one frame under prefers-reduced-motion, and a still of the
// machine in its place until the first frame and without WebGL.

import * as THREE from './vendor/three.module.min.js';

const reduceMotion = matchMedia('(prefers-reduced-motion: reduce)').matches;

// The machine, in its own units: the board is at FLOOR_Y, and everything stands on it.
const FLOOR_Y = -0.95;
const CHIP = { x: -1.42, w: 1.04, d: 0.8, h: 0.24 };
const CHEST = { x: -1.42, w: 0.88, d: 0.62, h: 0.5, lid: 0.12 };
const CHEST_BOTTOM = FLOOR_Y + CHIP.h;
const CHEST_TOP = CHEST_BOTTOM + CHEST.h;
const BELT = { x0: -0.9, x1: 0.3, y: -0.52, half: 0.17 };
const PLAN = { x0: 0.3, x1: 1.08 };
const GATE = { x: 0.02, z: -0.42, hub: 0.12, tilt: 0.6, scale: 0.9 };
const SCREEN = { w: 1.36, h: 0.76, y: 1.06, z: -0.33 };
const CASE = { x0: 1.32, x1: 2.24, y1: -0.34, z0: -0.44, z1: 0.44 };
const CARD = { w: 0.11, h: 0.15, t: 0.012 };
const CARD_Y = BELT.y + CARD.h / 2 + 0.004;
const BELT_START = BELT.x0 + 0.07;
const SPEED = 0.42;
// The composition's box, for placing and sizing it: x from the chip's edge to the case's, y from
// the board to the terminal's top.
const BOX = { x0: -1.96, x1: 2.26, y0: FLOOR_Y, y1: SCREEN.y + SCREEN.h / 2 + 0.05 };
const ASPECT = (BOX.x1 - BOX.x0) / (BOX.y1 - BOX.y0);
const CENTER = new THREE.Vector3((BOX.x0 + BOX.x1) / 2, (BOX.y0 + BOX.y1) / 2, 0);

// What Morrowind.bsa holds, as the archive spells it, and where each lands under out/.
const TRAYS = ['meshes/', 'textures/', 'icons/', 'sound/'];
const ENTRIES = [
  { raw: 'Meshes\\x\\Ex_De_Ship.NIF', kind: 'NIF', tray: 0 },
  { raw: 'Textures\\Tx_Wood_01.DDS', kind: 'DDS', tray: 1 },
  { raw: 'Icons\\Tx_GoldIcon.dds', kind: 'DDS', tray: 2 },
  { raw: 'Sound\\Fx\\magic\\frostH.wav', kind: 'WAV', tray: 3 },
  { raw: 'meshes\\f\\Furn_De_Bed_01.nif', kind: 'NIF', tray: 0 },
  { raw: 'Textures\\Tx_BM_Snow_01.dds', kind: 'DDS', tray: 1 },
  { raw: 'Icons\\w\\Tx_Glass_Longsword.DDS', kind: 'DDS', tray: 2 },
  { raw: 'Sound\\Vo\\d\\m\\Hlo_DM001.mp3', kind: 'MP3', tray: 3 },
  { raw: 'Meshes\\r\\XBase_Anim.KF', kind: 'KF', tray: 0 },
  { raw: 'Textures\\Menu_Head_Block.dds', kind: 'DDS', tray: 1 },
  { raw: 'Icons\\m\\Tx_Ebony_Dagger.dds', kind: 'DDS', tray: 2 },
  { raw: 'Sound\\Fx\\FOOT\\drtR.wav', kind: 'WAV', tray: 3 },
];
// Names extraction refuses: a `..` component, and a component with `:`.
const HOSTILE = [
  { raw: '..\\..\\Windows\\notepad.exe', kind: 'EXE', tray: 0 },
  { raw: 'Textures\\..\\..\\C:\\Boot.ini', kind: 'INI', tray: 1 },
];
const normalize = (raw) => raw.replace(/[\\/]+/g, '/').replace(/^\//, '').replace(/[A-Z]/g, (c) => c.toLowerCase());
for (const entry of [...ENTRIES, ...HOSTILE]) entry.path = normalize(entry.raw);

const KIND_COLORS = { NIF: '#6f8fb8', DDS: '#c0504f', KF: '#c9a24b', WAV: '#4f9f98', MP3: '#4f9f98', EXE: '#8a8f99', INI: '#8a8f99' };
const RUNS = ['clean', 'dry', 'unsafe', 'skip'];

function cssColor(name, fallback) {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const color = new THREE.Color(fallback);
  if (raw) {
    try { color.setStyle(raw); } catch { /* an unparsable token keeps the fallback */ }
  }
  return color;
}

const clamp01 = (value) => Math.min(1, Math.max(0, value));
const smooth = (value) => {
  const t = clamp01(value);
  return t * t * (3 - 2 * t);
};
const lerp = (a, b, t) => a + (b - a) * t;
// An elastic settle from 0 to 1, for things that spring back.
const elastic = (value) => {
  const t = clamp01(value);
  return t === 0 || t === 1 ? t : 1 - Math.exp(-6.5 * t) * Math.cos(t * 13);
};

function random(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function valueNoise(seed) {
  const next = random(seed);
  const table = new Float32Array(256 * 256);
  for (let i = 0; i < table.length; i++) table[i] = next();
  return (x, y) => {
    const xi = Math.floor(x);
    const yi = Math.floor(y);
    const fx = x - xi;
    const fy = y - yi;
    const u = fx * fx * (3 - 2 * fx);
    const v = fy * fy * (3 - 2 * fy);
    const x0 = xi & 255;
    const y0 = yi & 255;
    const x1 = (x0 + 1) & 255;
    const y1 = (y0 + 1) & 255;
    const a = table[y0 * 256 + x0];
    const b = table[y0 * 256 + x1];
    const c = table[y1 * 256 + x0];
    const d = table[y1 * 256 + x1];
    return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v;
  };
}

function fbm(noise, x, y, octaves) {
  let sum = 0;
  let amplitude = 0.5;
  let frequency = 1;
  let norm = 0;
  for (let i = 0; i < octaves; i++) {
    sum += amplitude * noise(x * frequency, y * frequency);
    norm += amplitude;
    amplitude *= 0.5;
    frequency *= 2.03;
  }
  return sum / norm;
}

function canvas2d(width, height) {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  return { canvas, context: canvas.getContext('2d') };
}

function textureFrom(canvas, colorSpace, anisotropy) {
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = colorSpace;
  texture.anisotropy = anisotropy;
  texture.needsUpdate = true;
  return texture;
}

function hex(color) {
  return `#${color.clone().convertLinearToSRGB().getHexString()}`;
}

// Surfaces -------------------------------------------------------------------------------------

// Lacquer over wood: deep red with orange-peel, darker in the grain, worn through to brown at every
// edge. Returns a colour map and a roughness map; each face of a box gets the whole map, so the
// wear sits on its edges.
function lacquerMaps(size, base, anisotropy) {
  const { canvas: color, context: colorContext } = canvas2d(size, size);
  const { canvas: rough, context: roughContext } = canvas2d(size, size);
  const colorImage = colorContext.createImageData(size, size);
  const roughImage = roughContext.createImageData(size, size);
  const noise = valueNoise(7);
  const grain = valueNoise(19);
  const lacquer = base.clone().convertLinearToSRGB();
  const wood = new THREE.Color(0.24, 0.12, 0.07);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const u = x / size;
      const v = y / size;
      const edge = Math.min(u, 1 - u, v, 1 - v);
      const wobble = fbm(noise, u * 22, v * 22, 3);
      const wear = clamp01(1 - (edge - 0.012 - wobble * 0.03) / 0.03);
      const lines = 0.5 + 0.5 * Math.sin((v * 90 + fbm(grain, u * 3, v * 40, 3) * 9) * 1.3);
      const peel = fbm(noise, u * 180, v * 180, 2);
      const shade = 0.78 + 0.16 * peel + 0.1 * (lines - 0.5);
      const r = lerp(lacquer.r * shade, wood.r * (0.7 + 0.5 * lines), wear);
      const g = lerp(lacquer.g * shade, wood.g * (0.7 + 0.5 * lines), wear);
      const b = lerp(lacquer.b * shade, wood.b * (0.7 + 0.5 * lines), wear);
      const i = (y * size + x) * 4;
      colorImage.data[i] = r * 255;
      colorImage.data[i + 1] = g * 255;
      colorImage.data[i + 2] = b * 255;
      colorImage.data[i + 3] = 255;
      const roughness = lerp(0.2 + peel * 0.12, 0.7, wear);
      roughImage.data[i] = 255;
      roughImage.data[i + 1] = roughness * 255;
      roughImage.data[i + 2] = 0;
      roughImage.data[i + 3] = 255;
    }
  }
  colorContext.putImageData(colorImage, 0, 0);
  roughContext.putImageData(roughImage, 0, 0);
  return { map: textureFrom(color, THREE.SRGBColorSpace, anisotropy), roughness: textureFrom(rough, THREE.NoColorSpace, anisotropy) };
}

// Brushed brass, streaked along u.
function brassMaps(width, height, anisotropy) {
  const { canvas: color, context: colorContext } = canvas2d(width, height);
  const { canvas: rough, context: roughContext } = canvas2d(width, height);
  const colorImage = colorContext.createImageData(width, height);
  const roughImage = roughContext.createImageData(width, height);
  const noise = valueNoise(31);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const streak = fbm(noise, x * 0.01, y * 1.3, 3);
      const tarnish = fbm(noise, x * 0.03 + 50, y * 0.03, 4);
      const i = (y * width + x) * 4;
      const shade = 0.82 + streak * 0.3 - clamp01(tarnish - 0.55) * 0.9;
      colorImage.data[i] = 214 * shade;
      colorImage.data[i + 1] = 168 * shade;
      colorImage.data[i + 2] = 96 * shade;
      colorImage.data[i + 3] = 255;
      roughImage.data[i] = 255;
      roughImage.data[i + 1] = (0.22 + streak * 0.18 + clamp01(tarnish - 0.5) * 0.6) * 255;
      roughImage.data[i + 2] = 255;
      roughImage.data[i + 3] = 255;
    }
  }
  colorContext.putImageData(colorImage, 0, 0);
  roughContext.putImageData(roughImage, 0, 0);
  const map = textureFrom(color, THREE.SRGBColorSpace, anisotropy);
  const roughness = textureFrom(rough, THREE.NoColorSpace, anisotropy);
  for (const texture of [map, roughness]) texture.wrapS = texture.wrapT = THREE.RepeatWrapping;
  return { map, roughness };
}

// The archive's header plate: its name and format, engraved.
function plateTexture(font, anisotropy) {
  const { canvas, context } = canvas2d(768, 272);
  const gradient = context.createLinearGradient(0, 0, 768, 272);
  gradient.addColorStop(0, '#c9a260');
  gradient.addColorStop(0.5, '#e6c787');
  gradient.addColorStop(1, '#a8823f');
  context.fillStyle = gradient;
  context.fillRect(0, 0, 768, 272);
  context.strokeStyle = 'rgba(60, 36, 10, 0.8)';
  context.lineWidth = 8;
  context.strokeRect(14, 14, 740, 244);
  context.fillStyle = 'rgba(52, 30, 8, 0.92)';
  context.textAlign = 'center';
  context.textBaseline = 'middle';
  context.font = `700 84px ${font}`;
  context.fillText('Morrowind.bsa', 384, 108);
  context.font = `500 40px ${font}`;
  context.fillText('TES3 \u00b7 0x100 \u00b7 3886 files', 384, 196);
  for (const [x, y] of [[40, 40], [728, 40], [40, 232], [728, 232]]) {
    context.beginPath();
    context.arc(x, y, 12, 0, Math.PI * 2);
    context.fillStyle = '#7a5a26';
    context.fill();
  }
  return textureFrom(canvas, THREE.SRGBColorSpace, anisotropy);
}

// The name-hash table down the archive's flanks: sorted 64-bit hashes, glowing.
function hashTexture(font, anisotropy) {
  const { canvas, context } = canvas2d(512, 1024);
  context.fillStyle = '#000';
  context.fillRect(0, 0, 512, 1024);
  const next = random(401);
  const hashes = Array.from({ length: 36 }, () => Array.from({ length: 16 }, () => '0123456789abcdef'[Math.floor(next() * 16)]).join('')).sort();
  context.font = `600 34px ${font}`;
  context.textBaseline = 'top';
  hashes.forEach((hash, i) => {
    const alpha = 0.35 + next() * 0.65;
    context.fillStyle = `rgba(255, 255, 255, ${alpha})`;
    context.fillText(`0x${hash}`, 28, 10 + i * 28);
  });
  const texture = textureFrom(canvas, THREE.NoColorSpace, anisotropy);
  texture.wrapT = THREE.RepeatWrapping;
  return texture;
}

// The gauge's dial: ticks, a red zone, and the only thing on the archive that moves before you touch
// it the right way.
function dialTexture(font) {
  const { canvas, context } = canvas2d(256, 256);
  context.fillStyle = '#efe6d2';
  context.beginPath();
  context.arc(128, 128, 124, 0, Math.PI * 2);
  context.fill();
  context.translate(128, 128);
  for (let i = 0; i <= 24; i++) {
    const angle = (-120 + i * 10) * Math.PI / 180 - Math.PI / 2;
    const major = i % 4 === 0;
    context.strokeStyle = i >= 19 ? '#b3261e' : '#2b2118';
    context.lineWidth = major ? 7 : 3;
    context.beginPath();
    context.moveTo(Math.cos(angle) * (major ? 84 : 94), Math.sin(angle) * (major ? 84 : 94));
    context.lineTo(Math.cos(angle) * 110, Math.sin(angle) * 110);
    context.stroke();
  }
  context.strokeStyle = 'rgba(179, 38, 30, 0.85)';
  context.lineWidth = 16;
  context.beginPath();
  context.arc(0, 0, 102, (70 - 90) * Math.PI / 180, (120 - 90) * Math.PI / 180);
  context.stroke();
  context.fillStyle = '#2b2118';
  context.font = `700 26px ${font}`;
  context.textAlign = 'center';
  context.fillText('PSI', 0, 58);
  return textureFrom(canvas, THREE.SRGBColorSpace, 4);
}

// Hazard stripes for the press, which only appears when it is wanted.
function hazardTexture() {
  const { canvas, context } = canvas2d(512, 64);
  context.fillStyle = '#16100c';
  context.fillRect(0, 0, 512, 64);
  context.fillStyle = '#e0a526';
  for (let x = -64; x < 576; x += 64) {
    context.beginPath();
    context.moveTo(x, 64);
    context.lineTo(x + 32, 0);
    context.lineTo(x + 64, 0);
    context.lineTo(x + 32, 64);
    context.fill();
  }
  return textureFrom(canvas, THREE.SRGBColorSpace, 4);
}

// Every card face, in one atlas: for each entry its stored spelling and its normalized one, the
// unsafe names, the refusal stamp and the back.
const ATLAS = { columns: 8, rows: 4 };
function cardAtlas(font, cellWidth) {
  const cellHeight = Math.round(cellWidth * 1.25);
  const { canvas, context } = canvas2d(cellWidth * ATLAS.columns, cellHeight * ATLAS.rows);
  const cells = {};
  let index = 0;
  const unit = cellWidth / 256;
  const place = () => {
    const column = index % ATLAS.columns;
    const row = Math.floor(index / ATLAS.columns);
    index++;
    return { x: column * cellWidth, y: row * cellHeight, index: index - 1 };
  };
  const glyph = (kind, x, y, size) => {
    context.save();
    context.translate(x, y);
    context.strokeStyle = '#2a211b';
    context.fillStyle = '#2a211b';
    context.lineWidth = 5 * unit;
    if (kind === 'DDS' || kind === 'INI') {
      for (let j = 0; j < 4; j++) {
        for (let i = 0; i < 4; i++) {
          context.fillStyle = (i + j) % 2 ? KIND_COLORS.DDS : '#e9dcc4';
          context.fillRect(-size / 2 + i * size / 4, -size / 2 + j * size / 4, size / 4, size / 4);
        }
      }
      context.strokeRect(-size / 2, -size / 2, size, size);
    } else if (kind === 'NIF') {
      const s = size * 0.36;
      const o = size * 0.18;
      context.strokeRect(-s - o / 2, -s + o / 2, s * 2, s * 2);
      context.strokeRect(-s + o / 2, -s - o / 2, s * 2, s * 2);
      for (const [dx, dy] of [[-1, -1], [1, -1], [-1, 1], [1, 1]]) {
        context.beginPath();
        context.moveTo(dx * s - o / 2, dy * s + o / 2);
        context.lineTo(dx * s + o / 2, dy * s - o / 2);
        context.stroke();
      }
    } else if (kind === 'KF') {
      context.beginPath();
      context.moveTo(-size / 2, size * 0.3);
      context.bezierCurveTo(-size * 0.1, size * 0.3, -size * 0.2, -size * 0.4, size * 0.1, -size * 0.35);
      context.bezierCurveTo(size * 0.35, -size * 0.3, size * 0.2, size * 0.2, size / 2, size * 0.1);
      context.stroke();
      for (const [dx, dy] of [[-0.5, 0.3], [0.1, -0.35], [0.5, 0.1]]) {
        context.fillRect(dx * size - 7 * unit, dy * size - 7 * unit, 14 * unit, 14 * unit);
      }
    } else if (kind === 'WAV' || kind === 'MP3') {
      context.beginPath();
      for (let i = 0; i <= 40; i++) {
        const u = i / 40;
        const amplitude = Math.sin(u * Math.PI) * (0.4 + 0.6 * Math.abs(Math.sin(u * 23)));
        context.moveTo(-size / 2 + u * size, -amplitude * size * 0.42);
        context.lineTo(-size / 2 + u * size, amplitude * size * 0.42);
      }
      context.stroke();
    } else {
      context.beginPath();
      context.moveTo(0, -size * 0.42);
      context.lineTo(size * 0.42, size * 0.36);
      context.lineTo(-size * 0.42, size * 0.36);
      context.closePath();
      context.stroke();
      context.fillRect(-3 * unit, -size * 0.12, 6 * unit, size * 0.26);
      context.fillRect(-3 * unit, size * 0.2, 6 * unit, 6 * unit);
    }
    context.restore();
  };
  const face = (entry, spelling) => {
    const { x, y, index: cell } = place();
    context.save();
    context.translate(x, y);
    context.fillStyle = '#efe5cf';
    context.fillRect(0, 0, cellWidth, cellHeight);
    context.fillStyle = KIND_COLORS[entry.kind];
    context.fillRect(0, 0, cellWidth, 52 * unit);
    context.fillStyle = '#fff6e6';
    context.font = `700 ${34 * unit}px ${font}`;
    context.textAlign = 'center';
    context.textBaseline = 'middle';
    context.fillText(`.${spelling === 'raw' ? entry.kind : entry.kind.toLowerCase()}`, cellWidth / 2, 27 * unit);
    glyph(entry.kind, cellWidth / 2, 138 * unit, 110 * unit);
    const name = spelling === 'raw' ? entry.raw : entry.path;
    const separator = spelling === 'raw' ? /[\\]/ : /[/]/;
    const parts = name.split(separator);
    const base = parts.pop();
    const folder = parts.length ? `${parts.join(spelling === 'raw' ? '\\' : '/')}${spelling === 'raw' ? '\\' : '/'}` : '';
    context.fillStyle = '#5a4a3c';
    context.font = `500 ${22 * unit}px ${font}`;
    context.fillText(folder.length > 18 ? `\u2026${folder.slice(-17)}` : folder, cellWidth / 2, 232 * unit);
    context.fillStyle = '#1f1914';
    let size = 30;
    context.font = `700 ${size * unit}px ${font}`;
    while (context.measureText(base).width > cellWidth - 22 * unit && size > 14) {
      size -= 2;
      context.font = `700 ${size * unit}px ${font}`;
    }
    context.fillText(base, cellWidth / 2, 272 * unit);
    context.strokeStyle = 'rgba(40, 30, 22, 0.35)';
    context.lineWidth = 3 * unit;
    context.strokeRect(1.5 * unit, 1.5 * unit, cellWidth - 3 * unit, cellHeight - 3 * unit);
    context.restore();
    return cell;
  };
  for (const entry of ENTRIES) {
    entry.rawCell = face(entry, 'raw');
    entry.pathCell = face(entry, 'path');
  }
  for (const entry of HOSTILE) {
    entry.rawCell = face(entry, 'raw');
    entry.pathCell = entry.rawCell;
  }
  {
    const { x, y, index: cell } = place();
    context.save();
    context.translate(x + cellWidth / 2, y + cellHeight / 2);
    context.rotate(-0.42);
    context.strokeStyle = '#d0342c';
    context.fillStyle = '#d0342c';
    context.lineWidth = 9 * unit;
    context.strokeRect(-cellWidth * 0.46, -34 * unit, cellWidth * 0.92, 68 * unit);
    context.font = `800 ${40 * unit}px ${font}`;
    context.textAlign = 'center';
    context.textBaseline = 'middle';
    context.fillText('REFUSED', 0, 2 * unit);
    context.restore();
    cells.stamp = cell;
  }
  {
    const { x, y, index: cell } = place();
    context.fillStyle = '#5c1a1f';
    context.fillRect(x, y, cellWidth, cellHeight);
    context.strokeStyle = 'rgba(255, 220, 190, 0.25)';
    context.lineWidth = 3 * unit;
    for (let i = -cellHeight; i < cellWidth + cellHeight; i += 22 * unit) {
      context.beginPath();
      context.moveTo(x + i, y);
      context.lineTo(x + i - cellHeight, y + cellHeight);
      context.stroke();
    }
    cells.back = cell;
  }
  return { canvas, cells };
}

function trayLabels(font, anisotropy) {
  const { canvas, context } = canvas2d(512, 512);
  context.fillStyle = '#1a1210';
  context.fillRect(0, 0, 512, 512);
  context.font = `700 64px ${font}`;
  context.textAlign = 'center';
  context.textBaseline = 'middle';
  TRAYS.forEach((name, i) => {
    context.fillStyle = '#f2e3c6';
    context.fillText(name, 256, 64 + i * 128);
  });
  return textureFrom(canvas, THREE.SRGBColorSpace, anisotropy);
}

function caseLabel(font) {
  const { canvas, context } = canvas2d(512, 256);
  context.clearRect(0, 0, 512, 256);
  context.fillStyle = '#fff';
  context.font = `700 132px ${font}`;
  context.textAlign = 'center';
  context.textBaseline = 'middle';
  context.fillText('out/', 256, 128);
  return textureFrom(canvas, THREE.NoColorSpace, 4);
}

// The circuit board under everything, as a data texture: copper in red, the distance along each
// bus trace in green (for pulses that run from the archive to the case), pads and vias in blue,
// silkscreen in alpha. It covers BOARD, in the machine's units.
const BOARD = { x0: -3.1, z0: -1.7, width: 6.4, depth: 3.4 };
function boardTexture(width, font) {
  const height = Math.round(width * BOARD.depth / BOARD.width);
  const toPixel = (x, z) => [(x - BOARD.x0) / BOARD.width * width, (z - BOARD.z0) / BOARD.depth * height];
  const pixelsPerUnit = width / BOARD.width;
  const layers = ['copper', 'distance', 'pads', 'silk'].map(() => canvas2d(width, height));
  const [copper, distance, pads, silk] = layers.map((layer) => layer.context);
  for (const context of [copper, distance, pads, silk]) {
    context.fillStyle = '#000';
    context.fillRect(0, 0, width, height);
    context.lineCap = 'round';
    context.lineJoin = 'round';
  }
  const next = random(2024);
  // A trace through points in the machine's units; bus traces also write their running length.
  const trace = (points, widthUnits, bus) => {
    const pixels = points.map(([x, z]) => toPixel(x, z));
    copper.strokeStyle = '#fff';
    copper.lineWidth = widthUnits * pixelsPerUnit;
    copper.beginPath();
    pixels.forEach(([x, y], i) => (i ? copper.lineTo(x, y) : copper.moveTo(x, y)));
    copper.stroke();
    if (!bus) return;
    let run = next() * 300;
    distance.lineWidth = widthUnits * pixelsPerUnit;
    for (let i = 1; i < pixels.length; i++) {
      const [ax, ay] = pixels[i - 1];
      const [bx, by] = pixels[i];
      const length = Math.hypot(bx - ax, by - ay);
      const steps = Math.max(1, Math.ceil(length / 3));
      for (let s = 0; s < steps; s++) {
        const t0 = s / steps;
        const t1 = (s + 1) / steps;
        const value = 1 + Math.floor(((run + length * t0) / 260) % 1 * 254);
        distance.strokeStyle = `rgb(${value}, ${value}, ${value})`;
        distance.beginPath();
        distance.moveTo(ax + (bx - ax) * t0, ay + (by - ay) * t0);
        distance.lineTo(ax + (bx - ax) * t1, ay + (by - ay) * t1);
        distance.stroke();
      }
      run += length;
    }
  };
  const via = (x, z, radius) => {
    const [px, py] = toPixel(x, z);
    pads.fillStyle = '#fff';
    pads.beginPath();
    pads.arc(px, py, radius * pixelsPerUnit, 0, Math.PI * 2);
    pads.fill();
    pads.fillStyle = '#000';
    pads.beginPath();
    pads.arc(px, py, radius * pixelsPerUnit * 0.45, 0, Math.PI * 2);
    pads.fill();
    copper.fillStyle = '#fff';
    copper.beginPath();
    copper.arc(px, py, radius * pixelsPerUnit * 1.1, 0, Math.PI * 2);
    copper.fill();
  };
  // The bus: from under the archive's pins, along under the belt, through the gate's footing, to
  // the case, with the forty-five degree jogs a router makes.
  const lanes = 9;
  for (let i = 0; i < lanes; i++) {
    const z = -0.3 + i * (0.6 / (lanes - 1));
    const lift = (i - (lanes - 1) / 2) * 0.03;
    trace([
      [CHIP.x + CHIP.w / 2 + 0.06, z * 0.9],
      [BELT.x0 + 0.2, z * 0.9],
      [BELT.x0 + 0.32, z * 0.55 + lift],
      [GATE.x - 0.3, z * 0.55 + lift],
      [GATE.x - 0.18, z * 0.35 - 0.12],
      [GATE.x + 0.24, z * 0.35 - 0.12],
      [GATE.x + 0.36, z * 0.55 + lift],
      [PLAN.x1 - 0.1, z * 0.55 + lift],
      [CASE.x0 - 0.04, z * 0.8],
      [CASE.x0 + 0.18, z * 0.8],
    ], 0.018, true);
  }
  // Pads for the archive chip's legs, and for everything's feet.
  for (let i = 0; i < 12; i++) {
    const x = CHIP.x - CHIP.w / 2 + 0.1 + i * ((CHIP.w - 0.2) / 11);
    for (const side of [-1, 1]) {
      const [px, py] = toPixel(x, side * (CHIP.d / 2 + 0.05));
      pads.fillStyle = '#fff';
      pads.fillRect(px - 0.022 * pixelsPerUnit, py - 0.05 * pixelsPerUnit, 0.044 * pixelsPerUnit, 0.1 * pixelsPerUnit);
    }
  }
  // Decoration: a router's mess of short traces, vias and the odd package outline.
  for (let i = 0; i < 55; i++) {
    let x = BOARD.x0 + next() * BOARD.width;
    let z = BOARD.z0 + next() * BOARD.depth;
    if (Math.abs(z) < 0.45 && x > CHIP.x - 0.7 && x < CASE.x1 + 0.2) continue;
    const points = [[x, z]];
    for (let s = 0; s < 3 + Math.floor(next() * 4); s++) {
      const direction = Math.floor(next() * 8) * Math.PI / 4;
      const length = 0.1 + next() * 0.5;
      x += Math.cos(direction) * length;
      z += Math.sin(direction) * length;
      points.push([x, z]);
    }
    trace(points, 0.008 + next() * 0.012, false);
    via(points.at(-1)[0], points.at(-1)[1], 0.018);
    if (next() < 0.5) via(points[0][0], points[0][1], 0.014);
  }
  silk.strokeStyle = '#fff';
  silk.fillStyle = '#fff';
  silk.lineWidth = 0.008 * pixelsPerUnit;
  silk.font = `600 ${0.07 * pixelsPerUnit}px ${font}`;
  silk.textBaseline = 'middle';
  const label = (text, x, z, size = 0.07) => {
    const [px, py] = toPixel(x, z);
    silk.font = `600 ${size * pixelsPerUnit}px ${font}`;
    silk.fillText(text, px, py);
  };
  label('U1  MORROWIND.BSA', CHIP.x - CHIP.w / 2, CHIP.d / 2 + 0.2);
  label('U2  POLICY', GATE.x - 0.2, 0.62);
  label('J1  OUT/', CASE.x0, CASE.z1 + 0.14);
  label('DREAM_ARCHIVETOOL 1.1.0   MIT OR Apache-2.0', -2.9, 1.45, 0.06);
  label('--dry-run', 0.5, -0.62, 0.05);
  label('R12', -0.4, 0.8, 0.05);
  label('C7', 0.9, 0.95, 0.05);
  {
    const [px, py] = toPixel(CHIP.x - CHIP.w / 2 - 0.06, -CHIP.d / 2 - 0.06);
    silk.beginPath();
    silk.arc(px, py, 0.02 * pixelsPerUnit, 0, Math.PI * 2);
    silk.fill();
  }
  const data = new Uint8Array(width * height * 4);
  const images = layers.map(({ context }) => context.getImageData(0, 0, width, height).data);
  for (let i = 0; i < width * height; i++) {
    data[i * 4] = images[0][i * 4];
    data[i * 4 + 1] = images[1][i * 4];
    data[i * 4 + 2] = images[2][i * 4];
    data[i * 4 + 3] = images[3][i * 4];
  }
  const texture = new THREE.DataTexture(data, width, height, THREE.RGBAFormat);
  texture.flipY = false;
  texture.generateMipmaps = true;
  texture.minFilter = THREE.LinearMipmapLinearFilter;
  texture.magFilter = THREE.LinearFilter;
  texture.anisotropy = 8;
  texture.needsUpdate = true;
  return texture;
}

// Geometry -------------------------------------------------------------------------------------

// A box with rounded edges and analytic normals, so the rounding shades smoothly across faces.
function roundedBox(width, height, depth, radius, segments = 3) {
  const n = segments * 2 + 1;
  const geometry = new THREE.BoxGeometry(1, 1, 1, n, n, n);
  const position = geometry.attributes.position;
  const normal = geometry.attributes.normal;
  const half = new THREE.Vector3(width / 2, height / 2, depth / 2);
  const inner = new THREE.Vector3(half.x - radius, half.y - radius, half.z - radius);
  const a = 0.5 / n;
  const map = (t, halfSize, innerSize) => {
    if (Math.abs(t) <= a) return t / a * innerSize;
    return Math.sign(t) * (innerSize + (Math.abs(t) - a) / (0.5 - a) * (halfSize - innerSize));
  };
  const p = new THREE.Vector3();
  const c = new THREE.Vector3();
  const d = new THREE.Vector3();
  for (let i = 0; i < position.count; i++) {
    p.set(map(position.getX(i), half.x, inner.x), map(position.getY(i), half.y, inner.y), map(position.getZ(i), half.z, inner.z));
    c.set(THREE.MathUtils.clamp(p.x, -inner.x, inner.x), THREE.MathUtils.clamp(p.y, -inner.y, inner.y), THREE.MathUtils.clamp(p.z, -inner.z, inner.z));
    d.subVectors(p, c);
    if (d.lengthSq() > 1e-12) {
      d.normalize();
      p.copy(c).addScaledVector(d, radius);
      normal.setXYZ(i, d.x, d.y, d.z);
    }
    position.setXYZ(i, p.x, p.y, p.z);
  }
  geometry.computeBoundingBox();
  geometry.computeBoundingSphere();
  return geometry;
}

// A combination wrench, as the icon draws it: an open jaw angled fifteen degrees at one end, a
// twelve-point ring at the other, on a slim handle. Length runs along x.
function wrenchGeometry() {
  const handle = 0.056;
  const ring = { x: -0.78, r: 0.165 };
  const jaw = { x: 0.8, r: 0.2, angle: 0.26, slot: 0.078 };
  const outline = [];
  const arc = (cx, cy, radius, from, to, steps) => {
    for (let i = 0; i <= steps; i++) {
      const angle = from + (to - from) * i / steps;
      outline.push(new THREE.Vector2(cx + Math.cos(angle) * radius, cy + Math.sin(angle) * radius));
    }
  };
  const ringAttach = Math.asin(handle / ring.r);
  const jawAttach = Math.asin(handle / jaw.r);
  const slotSpread = Math.asin(jaw.slot / jaw.r);
  const reach = Math.sqrt(jaw.r * jaw.r - jaw.slot * jaw.slot);
  const u = new THREE.Vector2(Math.cos(jaw.angle), Math.sin(jaw.angle));
  const nrm = new THREE.Vector2(-u.y, u.x);
  const centre = new THREE.Vector2(jaw.x, 0);
  // Along the handle's top, over the jaw to its slot, into the slot and round its bottom, out, on
  // round the jaw, back along the handle, and round the ring.
  outline.push(new THREE.Vector2(ring.x + Math.cos(ringAttach) * ring.r, handle));
  arc(jaw.x, 0, jaw.r, Math.PI - jawAttach, jaw.angle + slotSpread, 36);
  outline.push(centre.clone().addScaledVector(nrm, jaw.slot).addScaledVector(u, reach * 0.35));
  arc(jaw.x, 0, jaw.slot, jaw.angle + Math.PI / 2, jaw.angle + Math.PI * 1.5, 18);
  outline.push(centre.clone().addScaledVector(nrm, -jaw.slot).addScaledVector(u, reach * 0.35));
  arc(jaw.x, 0, jaw.r, jaw.angle - slotSpread, -Math.PI + jawAttach, 36);
  outline.push(new THREE.Vector2(ring.x + Math.cos(ringAttach) * ring.r, -handle));
  arc(ring.x, 0, ring.r, -ringAttach, -Math.PI * 2 + ringAttach, 40);
  const shape = new THREE.Shape(outline);
  const hole = new THREE.Path();
  for (let i = 0; i <= 24; i++) {
    const angle = i / 24 * Math.PI * 2 + Math.PI / 12;
    const radius = i % 2 ? 0.093 : 0.108;
    const point = new THREE.Vector2(ring.x + Math.cos(angle) * radius, Math.sin(angle) * radius);
    if (i === 0) hole.moveTo(point.x, point.y);
    else hole.lineTo(point.x, point.y);
  }
  shape.holes.push(hole);
  const geometry = new THREE.ExtrudeGeometry(shape, {
    depth: 0.05,
    bevelEnabled: true,
    bevelThickness: 0.014,
    bevelSize: 0.012,
    bevelSegments: 4,
    curveSegments: 12,
  });
  geometry.translate(0, 0, -0.025);
  geometry.computeVertexNormals();
  return geometry;
}

// A light shaft: a frustum from a small square to a larger rectangle, open at both ends, with v
// running from the source (0) to the far end (1).
function frustum(top, bottom) {
  const positions = [];
  const uvs = [];
  const indices = [];
  for (let side = 0; side < 4; side++) {
    const a = side;
    const b = (side + 1) % 4;
    const base = positions.length / 3;
    for (const [corner, v] of [[top[a], 0], [top[b], 0], [bottom[b], 1], [bottom[a], 1]]) {
      positions.push(corner.x, corner.y, corner.z);
      uvs.push(side / 4 + (corner === top[b] || corner === bottom[b] ? 0.25 : 0), v);
    }
    indices.push(base, base + 1, base + 2, base, base + 2, base + 3);
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2));
  geometry.setIndex(indices);
  return geometry;
}

// Something for polished metal to reflect: a dark room with a warm softbox overhead, a cool strip
// on one side and a strip in the site's accent on the other.
function environment(renderer, accent) {
  const room = new THREE.Scene();
  room.background = new THREE.Color(0.012, 0.008, 0.01);
  const panel = (color, intensity, size, position, look) => {
    const mesh = new THREE.Mesh(new THREE.PlaneGeometry(size[0], size[1]), new THREE.MeshBasicMaterial({ color: color.clone().multiplyScalar(intensity), side: THREE.DoubleSide }));
    mesh.position.copy(position);
    mesh.lookAt(look);
    room.add(mesh);
  };
  const origin = new THREE.Vector3();
  panel(new THREE.Color(1, 0.9, 0.78), 4.5, [6, 3], new THREE.Vector3(0, 6, 1), origin);
  panel(new THREE.Color(0.6, 0.72, 1), 2.2, [1.2, 6], new THREE.Vector3(-6, 1, 2), origin);
  panel(accent, 3.2, [1.2, 6], new THREE.Vector3(6, 0.5, -1), origin);
  panel(new THREE.Color(1, 0.8, 0.6), 1.2, [8, 1], new THREE.Vector3(0, -1, -6), origin);
  const pmrem = new THREE.PMREMGenerator(renderer);
  const target = pmrem.fromScene(room, 0.03, 0.1, 100, { size: 128 });
  pmrem.dispose();
  return target;
}

// Shaders --------------------------------------------------------------------------------------

const FULLSCREEN_VERTEX = /* glsl */ `
  varying vec2 vUv;
  void main() {
    vUv = uv;
    gl_Position = vec4(position.xy, 0.0, 1.0);
  }
`;

// Any NaN or infinity a driver produces is zeroed and bright values capped before the bloom, which
// would otherwise smear a single bad pixel into a black square.
const SCRUB = /* glsl */ `
  vec3 scrub(vec3 c) {
    if (any(isnan(c)) || any(isinf(c)) || c.r != c.r || c.g != c.g || c.b != c.b) return vec3(0.0);
    return clamp(c, 0.0, 64.0);
  }
`;

const SKY_FRAGMENT = /* glsl */ `
  uniform vec3 uTop;
  uniform vec3 uBottom;
  uniform vec3 uGlow;
  uniform vec2 uCenter;
  uniform vec2 uResolution;
  uniform float uRadius;
  uniform float uTime;
  varying vec2 vUv;
  void main() {
    vec3 color = mix(uBottom, uTop, smoothstep(0.0, 1.0, vUv.y));
    vec2 p = (vUv - uCenter) * vec2(uResolution.x / max(uResolution.y, 1.0), 1.0);
    float r = length(p) / max(uRadius, 1e-3);
    color += uGlow * exp(-r * r * 1.3);
    // Dust in the air over the machine, catching the light.
    float shafts = 0.0;
    for (int i = 0; i < 3; i++) {
      float fi = float(i);
      float x = p.x - (fi - 1.0) * uRadius * 0.55 + p.y * (0.22 - 0.12 * fi);
      float width = uRadius * (0.05 + 0.03 * fi);
      shafts += exp(-(x * x) / max(width * width, 1e-5)) * (0.55 + 0.45 * sin(uTime * (0.23 + 0.05 * fi) + fi * 2.3));
    }
    shafts *= smoothstep(-0.2, 0.9, p.y / max(uRadius, 1e-3)) * exp(-r * 0.35);
    color += uGlow * shafts * 0.35;
    gl_FragColor = vec4(color, 1.0);
  }
`;

const BOARD_VERTEX = /* glsl */ `
  varying vec2 vXZ;
  varying vec3 vWorld;
  void main() {
    vXZ = vec2(position.x, -position.y);
    vec4 world = modelMatrix * vec4(position, 1.0);
    vWorld = world.xyz;
    gl_Position = projectionMatrix * viewMatrix * world;
  }
`;

const BOARD_FRAGMENT = /* glsl */ `
  uniform sampler2D tBoard;
  uniform vec4 uRect;
  uniform vec3 uMask;
  uniform vec3 uCopper;
  uniform vec3 uPad;
  uniform vec3 uSilk;
  uniform vec3 uPulse;
  uniform float uFlow;
  uniform float uTime;
  uniform vec3 uLamp;
  uniform float uLampPower;
  uniform vec4 uShadows[3];
  varying vec2 vXZ;
  varying vec3 vWorld;
  float sdBox(vec2 p, vec2 b) {
    vec2 d = abs(p) - b;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
  }
  void main() {
    vec2 uv = (vXZ - uRect.xy) / uRect.zw;
    float inside = step(0.0, uv.x) * step(uv.x, 1.0) * step(0.0, uv.y) * step(uv.y, 1.0);
    vec4 board = texture2D(tBoard, clamp(uv, 0.0, 1.0)) * inside;
    float copper = board.r;
    float flow = board.g;
    float pad = board.b;
    float silk = board.a;
    // Solder mask over copper, pads in bright tin, silkscreen in white.
    vec3 color = uMask * (0.75 + 0.25 * fract(sin(dot(floor(vXZ * 90.0), vec2(12.9898, 78.233))) * 43758.5453));
    color = mix(color, uCopper, copper * 0.8);
    color = mix(color, uPad, pad);
    color = mix(color, uSilk, silk * 0.55);
    // The lamp's highlight on copper and tin.
    vec3 toCamera = normalize(cameraPosition - vWorld);
    vec3 toLamp = uLamp - vWorld;
    float lampDistance = max(length(toLamp), 1e-3);
    vec3 halfway = normalize(toLamp / lampDistance + toCamera);
    float spec = pow(max(halfway.y, 0.0), 60.0) * (copper * 0.7 + pad) * uLampPower / (1.0 + lampDistance * lampDistance);
    color += vec3(1.0, 0.86, 0.7) * spec;
    // Data running along the bus while a run is live.
    float live = step(0.004, flow) * copper;
    float pulse = smoothstep(0.86, 1.0, fract(flow * 3.0 - uTime * 0.55)) + 0.5 * smoothstep(0.93, 1.0, fract(flow * 7.0 - uTime * 1.1));
    color += uPulse * live * pulse * uFlow * 1.6 + uPulse * live * 0.03;
    // Contact shadows under the chip, the belt and the case.
    float shade = 1.0;
    for (int i = 0; i < 3; i++) {
      float d = sdBox(vXZ - uShadows[i].xy, uShadows[i].zw);
      shade *= 1.0 - 0.7 * (1.0 - smoothstep(-0.02, 0.16, d));
    }
    color *= shade;
    float fade = 1.0 - smoothstep(1.6, 3.2, length(vXZ * vec2(0.8, 1.2) - vec2(0.2, 0.0)));
    gl_FragColor = vec4(color * fade, fade);
  }
`;

const BELT_FRAGMENT = /* glsl */ `
  uniform float uOffset;
  uniform vec3 uScan;
  uniform float uScanX;
  uniform float uScanPower;
  varying vec2 vXZ;
  varying vec3 vWorld;
  void main() {
    float x = vXZ.x;
    float cleat = smoothstep(0.3, 0.5, abs(fract((x - uOffset) / 0.07) - 0.5));
    float edge = smoothstep(0.155, 0.17, abs(vXZ.y));
    vec3 color = vec3(0.028, 0.022, 0.02) * (0.7 + 0.5 * cleat) + vec3(0.06, 0.05, 0.045) * edge;
    float window = exp(-pow((x - uScanX) / 0.085, 2.0)) * (1.0 - edge);
    float line = exp(-pow((x - uScanX) / 0.012, 2.0));
    color += uScan * (window * 0.8 + line * 2.2) * uScanPower;
    gl_FragColor = vec4(color, 1.0);
  }
`;

const BEAM_VERTEX = /* glsl */ `
  varying vec2 vUv;
  varying vec3 vWorld;
  void main() {
    vUv = uv;
    vec4 world = modelMatrix * vec4(position, 1.0);
    vWorld = world.xyz;
    gl_Position = projectionMatrix * viewMatrix * world;
  }
`;

const BEAM_FRAGMENT = /* glsl */ `
  uniform vec3 uColor;
  uniform float uPower;
  uniform float uTime;
  uniform float uDashed;
  varying vec2 vUv;
  void main() {
    float along = vUv.y;
    float falloff = mix(0.35, 1.0, along) * smoothstep(0.0, 0.08, along);
    float lines = 0.55 + 0.45 * sin(along * 60.0 - uTime * 9.0);
    float dash = mix(1.0, step(0.5, fract(along * 14.0 - uTime * 2.0)), uDashed);
    float edge = pow(1.0 - abs(fract(vUv.x * 4.0) - 0.5) * 2.0, 3.0);
    float intensity = falloff * (0.25 + 0.75 * lines) * dash * (0.35 + 0.65 * edge) * uPower;
    gl_FragColor = vec4(uColor * intensity * 0.55, 1.0);
  }
`;

const SCREEN_FRAGMENT = /* glsl */ `
  uniform sampler2D tText;
  uniform vec3 uTint;
  uniform float uTime;
  uniform float uPower;
  varying vec2 vUv;
  void main() {
    vec2 uv = vUv - 0.5;
    uv *= 1.0 + dot(uv, uv) * 0.06;
    uv += 0.5;
    vec3 text = texture2D(tText, uv).rgb;
    float scan = 0.82 + 0.18 * sin(vUv.y * 420.0);
    float roll = 0.94 + 0.06 * sin(vUv.y * 6.0 - uTime * 1.2);
    vec2 v = vUv - 0.5;
    float vignette = 1.0 - smoothstep(0.25, 0.75, length(v * vec2(1.0, 1.4)));
    vec3 glass = uTint * 0.05 * vignette;
    vec3 color = (glass + text * 2.1 * scan * roll) * (0.5 + 0.5 * vignette + 0.2) * uPower;
    gl_FragColor = vec4(color, 1.0);
  }
`;

// The cards: an instanced box whose front shows an atlas cell. The scan wipes the stored spelling
// into the normalized one; a refused card carries a stamp; ghosts are temporary files and plans.
const CARD_VERTEX = /* glsl */ `
  attribute vec4 aCells;
  attribute vec4 aLook;
  varying vec2 vUv;
  varying vec3 vNormal;
  varying vec3 vObjectNormal;
  varying vec3 vWorld;
  varying vec4 vCells;
  varying vec4 vLook;
  void main() {
    vUv = uv;
    vCells = aCells;
    vLook = aLook;
    vObjectNormal = normal;
    mat4 model = modelMatrix;
    #ifdef USE_INSTANCING
      model = modelMatrix * instanceMatrix;
    #endif
    vec4 world = model * vec4(position, 1.0);
    vWorld = world.xyz;
    vNormal = normalize(mat3(model) * normal);
    gl_Position = projectionMatrix * viewMatrix * world;
  }
`;

const CARD_FRAGMENT = /* glsl */ `
  uniform sampler2D tAtlas;
  uniform vec2 uGrid;
  uniform float uStamp;
  uniform float uBack;
  uniform vec3 uKey;
  uniform vec3 uLamp;
  uniform float uLampPower;
  uniform vec3 uGlowOk;
  uniform vec3 uGlowBad;
  uniform vec3 uGlowSkip;
  uniform float uGhost;
  uniform float uTime;
  varying vec2 vUv;
  varying vec3 vNormal;
  varying vec3 vObjectNormal;
  varying vec3 vWorld;
  varying vec4 vCells;
  varying vec4 vLook;
  vec3 cell(float index, vec2 uv) {
    vec2 at = vec2(mod(index, uGrid.x), floor(index / uGrid.x));
    vec2 inset = mix(vec2(0.012), vec2(0.988), uv);
    return texture2D(tAtlas, (at + vec2(inset.x, 1.0 - inset.y)) / uGrid).rgb;
  }
  vec4 stampOver(vec2 uv) {
    vec2 at = vec2(mod(uStamp, uGrid.x), floor(uStamp / uGrid.x));
    vec3 c = texture2D(tAtlas, (at + vec2(uv.x, 1.0 - uv.y)) / uGrid).rgb;
    float a = smoothstep(0.1, 0.35, c.r - c.g);
    return vec4(c, a);
  }
  void main() {
    float wipe = vLook.x;
    float verdict = vLook.y;
    float glow = vLook.z;
    float alpha = vLook.w;
    vec3 base;
    bool front = vObjectNormal.z > 0.5;
    bool back = vObjectNormal.z < -0.5;
    if (front) {
      bool normalized = vUv.x < wipe;
      base = cell(normalized ? vCells.y : vCells.x, vUv);
      float seam = exp(-pow((vUv.x - wipe) / 0.02, 2.0)) * step(0.001, wipe) * step(wipe, 0.999);
      base += vec3(1.0, 0.85, 0.8) * seam * 1.5;
      if (verdict > 0.5 && verdict < 1.5) {
        vec4 stamp = stampOver(vUv);
        base = mix(base * vec3(1.0, 0.55, 0.5), stamp.rgb, stamp.a);
      }
    } else if (back) {
      base = cell(uBack, vUv);
    } else {
      base = vec3(0.86, 0.8, 0.7);
    }
    vec3 n = normalize(vNormal) * (gl_FrontFacing ? 1.0 : -1.0);
    vec3 toCamera = normalize(cameraPosition - vWorld);
    float key = max(dot(n, normalize(uKey)), 0.0);
    vec3 toLamp = uLamp - vWorld;
    float lampDistance = max(length(toLamp), 1e-3);
    float lamp = max(dot(n, toLamp / lampDistance), 0.0) * uLampPower / (1.0 + lampDistance * lampDistance * 4.0);
    float rim = pow(1.0 - max(dot(n, toCamera), 0.0), 3.0);
    vec3 light = vec3(0.22, 0.2, 0.22) + vec3(1.0, 0.94, 0.86) * key * 0.95 + vec3(1.0, 0.88, 0.76) * lamp;
    vec3 color = base * base * light;
    vec3 tint = verdict < 0.5 ? uGlowOk : (verdict < 1.5 ? uGlowBad : uGlowSkip);
    color += tint * (glow * 1.6 + rim * 0.15);
    if (uGhost > 0.5) {
      float scan = 0.6 + 0.4 * sin(vWorld.y * 260.0 - uTime * 6.0);
      vec2 e = min(vUv, 1.0 - vUv);
      float frame = 1.0 - smoothstep(0.0, 0.06, min(e.x, e.y));
      vec3 holo = tint * (0.25 + 0.75 * frame) * scan + base * tint * 0.35;
      gl_FragColor = vec4(holo * alpha * (1.0 + glow * 2.0), 1.0);
      return;
    }
    gl_FragColor = vec4(color, 1.0);
  }
`;

const GLASS_FRAGMENT = /* glsl */ `
  uniform vec3 uColor;
  uniform float uPower;
  uniform sampler2D tLabel;
  uniform float uHasLabel;
  varying vec2 vUv;
  varying vec3 vNormal;
  varying vec3 vWorld;
  void main() {
    vec3 n = normalize(vNormal) * (gl_FrontFacing ? 1.0 : -1.0);
    float facing = abs(dot(n, normalize(cameraPosition - vWorld)));
    float fresnel = pow(1.0 - facing, 3.0);
    vec2 cell = abs(fract(vUv * vec2(18.0, 12.0)) - 0.5);
    float grid = smoothstep(0.47, 0.5, max(cell.x, cell.y)) * 0.25;
    float label = uHasLabel * texture2D(tLabel, vUv).r;
    vec3 color = uColor * (0.03 + fresnel * 0.9 + grid * 0.12 + label * 0.9) * uPower;
    gl_FragColor = vec4(color, 1.0);
  }
`;

const GLASS_VERTEX = /* glsl */ `
  varying vec2 vUv;
  varying vec3 vNormal;
  varying vec3 vWorld;
  void main() {
    vUv = uv;
    vec4 world = modelMatrix * vec4(position, 1.0);
    vWorld = world.xyz;
    vNormal = normalize(mat3(modelMatrix) * normal);
    gl_Position = projectionMatrix * viewMatrix * world;
  }
`;

const PUFF_VERTEX = /* glsl */ `
  attribute vec4 aPuff;
  uniform float uPixel;
  varying float vLife;
  varying float vKind;
  void main() {
    vLife = aPuff.x;
    vKind = aPuff.z;
    vec4 view = modelViewMatrix * vec4(position, 1.0);
    gl_PointSize = aPuff.y * uPixel / max(-view.z, 0.1);
    gl_Position = projectionMatrix * view;
  }
`;

const PUFF_FRAGMENT = /* glsl */ `
  uniform vec3 uSteam;
  uniform vec3 uSpark;
  varying float vLife;
  varying float vKind;
  void main() {
    vec2 p = gl_PointCoord - 0.5;
    float d = length(p) * 2.0;
    if (vLife <= 0.0 || d > 1.0) discard;
    float soft = 1.0 - smoothstep(0.0, 1.0, d);
    vec3 color = vKind > 0.5 ? uSpark * (1.0 - smoothstep(0.0, 0.6, d)) * vLife * 3.0 : uSteam * soft * soft * vLife * 0.35;
    gl_FragColor = vec4(color, 1.0);
  }
`;

const BRIGHT_FRAGMENT = /* glsl */ `
  uniform sampler2D tInput;
  uniform float uThreshold;
  varying vec2 vUv;
  ${SCRUB}
  void main() {
    vec3 c = scrub(texture2D(tInput, vUv).rgb);
    float luma = dot(c, vec3(0.2126, 0.7152, 0.0722));
    gl_FragColor = vec4(c * smoothstep(uThreshold, uThreshold + 0.8, luma), 1.0);
  }
`;

const BLUR_FRAGMENT = /* glsl */ `
  uniform sampler2D tInput;
  uniform vec2 uDirection;
  varying vec2 vUv;
  void main() {
    vec3 sum = texture2D(tInput, vUv).rgb * 0.2270270270;
    sum += texture2D(tInput, vUv + uDirection * 1.3846153846).rgb * 0.3162162162;
    sum += texture2D(tInput, vUv - uDirection * 1.3846153846).rgb * 0.3162162162;
    sum += texture2D(tInput, vUv + uDirection * 3.2307692308).rgb * 0.0702702703;
    sum += texture2D(tInput, vUv - uDirection * 3.2307692308).rgb * 0.0702702703;
    gl_FragColor = vec4(sum, 1.0);
  }
`;

const COMPOSITE_FRAGMENT = /* glsl */ `
  uniform sampler2D tScene;
  uniform sampler2D tBloomNear;
  uniform sampler2D tBloomFar;
  uniform float uTime;
  uniform float uFlash;
  uniform vec3 uFlashColor;
  uniform vec2 uFlashAt;
  uniform vec2 uCenter;
  uniform vec2 uHalf;
  uniform vec2 uResolution;
  uniform float uScrim;
  varying vec2 vUv;
  vec3 aces(vec3 x) {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
  }
  float dither(vec2 p) {
    return fract(sin(dot(p + fract(uTime), vec2(12.9898, 78.233))) * 43758.5453) - 0.5;
  }
  ${SCRUB}
  void main() {
    vec3 color = scrub(texture2D(tScene, vUv).rgb);
    color += scrub(texture2D(tBloomNear, vUv).rgb) * 0.4 + scrub(texture2D(tBloomFar, vUv).rgb) * 0.3;
    vec2 f = (vUv - uFlashAt) * vec2(uResolution.x / max(uResolution.y, 1.0), 1.0) / max(uHalf.y, 1e-3);
    color += uFlashColor * uFlash * exp(-dot(f, f) * 2.5);
    // Keep the words calm: darken what lies well away from the machine.
    vec2 p = (vUv - uCenter) / max(uHalf, vec2(1e-3));
    float away = smoothstep(1.05, 1.9, length(p * vec2(0.85, 1.0)));
    color *= 1.0 - away * uScrim;
    color = aces(color * 0.95);
    color = pow(color, vec3(1.0 / 2.2));
    color += dither(gl_FragCoord.xy) / 255.0;
    gl_FragColor = vec4(color, 1.0);
  }
`;

function fullscreenMaterial(fragmentShader, uniforms) {
  return new THREE.ShaderMaterial({ vertexShader: FULLSCREEN_VERTEX, fragmentShader, uniforms, depthTest: false, depthWrite: false });
}

function additive(vertexShader, fragmentShader, uniforms, extra = {}) {
  return new THREE.ShaderMaterial({
    vertexShader,
    fragmentShader,
    uniforms,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    side: THREE.DoubleSide,
    ...extra,
  });
}

// The terminal -----------------------------------------------------------------------------------

// What the jaws hold up: dream_archivetool's own output, typed as it runs. Short lines, so they
// stay legible at the size the machine is drawn.
class Terminal {
  constructor(font, colors) {
    const { canvas, context } = canvas2d(1024, 572);
    this.canvas = canvas;
    this.context = context;
    this.font = font;
    this.colors = colors;
    this.lines = [];
    this.texture = textureFrom(canvas, THREE.SRGBColorSpace, 4);
    this.dirty = true;
    this.typing = false;
  }

  clear() {
    this.lines = [];
    this.dirty = true;
  }

  // Lines are typed at `rate` characters a second, starting at `at` (the machine's clock).
  print(text, tone = 'text', at = 0, rate = 0) {
    this.lines.push({ text, tone, at, rate });
    if (this.lines.length > 40) this.lines.splice(0, this.lines.length - 40);
    this.dirty = true;
  }

  draw(time) {
    const shown = this.lines.filter((line) => time >= line.at);
    const typing = shown.some((line) => line.rate && (time - line.at) * line.rate < line.text.length);
    const blink = Math.floor(time * 1.25) % 2;
    if (!this.dirty && !typing && !this.typing && shown.length === this.shown && blink === this.blink) return;
    this.typing = typing;
    this.shown = shown.length;
    this.blink = blink;
    this.dirty = false;
    const { context } = this;
    context.fillStyle = '#060304';
    context.fillRect(0, 0, 1024, 572);
    context.font = `600 52px ${this.font}`;
    context.textBaseline = 'top';
    const visible = shown.slice(-8);
    visible.forEach((line, i) => {
      const count = line.rate ? Math.min(line.text.length, Math.floor((time - line.at) * line.rate)) : line.text.length;
      const text = line.text.slice(0, count);
      const y = 22 + i * 67;
      if (line.tone === 'prompt') {
        context.fillStyle = this.colors.accent;
        context.fillText(text.slice(0, 1), 34, y);
        context.fillStyle = this.colors.text;
        context.fillText(text.slice(1), 34 + context.measureText(text.slice(0, 1)).width, y);
      } else {
        context.fillStyle = this.colors[line.tone] || this.colors.text;
        context.fillText(text, 34, y);
      }
      if (i === visible.length - 1 && blink === 0) {
        context.fillStyle = this.colors.text;
        context.fillRect(34 + context.measureText(text).width + 6, y + 4, 26, 50);
      }
    });
    this.texture.needsUpdate = true;
  }
}

// Layout -----------------------------------------------------------------------------------------

function textRects(text) {
  const rects = [];
  const range = document.createRange();
  const walker = document.createTreeWalker(text, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => (node.nodeValue.trim() ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT),
  });
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    range.selectNodeContents(node);
    for (const rect of range.getClientRects()) rects.push(rect);
  }
  for (const element of text.querySelectorAll('a, button, input, select, img, svg, .dw-command, .dw-badge')) rects.push(element.getBoundingClientRect());
  return rects.filter((rect) => rect.width > 0 && rect.height > 0);
}

// The largest box of the machine's shape clear of the text: beside it, beside the title rows, or
// above it all where the stylesheet leaves room on a phone. Relative to the art; `size` is its
// height.
function placement(root) {
  const hero = root.closest('.dw-hero') || root.parentElement;
  const box = root.getBoundingClientRect();
  const text = hero.querySelector('.dw-hero__text') || hero.querySelector('.dw-shell');
  const strip = hero.querySelector('.dw-strip');
  const shellElement = hero.querySelector('.dw-hero__grid') || hero.querySelector('.dw-shell') || hero;
  const shellStyle = getComputedStyle(shellElement);
  const shellBox = shellElement.getBoundingClientRect();
  const shell = strip ? strip.getBoundingClientRect() : { left: shellBox.left + parseFloat(shellStyle.paddingLeft), right: shellBox.right - parseFloat(shellStyle.paddingRight) };
  const summary = hero.querySelector('.dw-hero__summary');
  const floor = strip ? strip.getBoundingClientRect().top : box.bottom - 24;
  const rects = text ? textRects(text) : [];
  if (!rects.length) return { x: box.width * 0.7, y: box.height * 0.45, size: Math.min(box.width * 0.5 / ASPECT, box.height * 0.7), above: false };
  const gap = 28;
  const right = Math.max(...rects.map((rect) => rect.right));
  const top = Math.min(...rects.map((rect) => rect.top));
  const summaryTop = summary ? summary.getBoundingClientRect().top : floor;
  const headRects = rects.filter((rect) => rect.bottom <= summaryTop + 1);
  const headRight = headRects.length ? Math.max(...headRects.map((rect) => rect.right)) : right;
  const candidates = [
    { x0: right + gap, x1: shell.right, y0: box.top + 14, y1: floor - 14, above: false },
    { x0: headRight + gap, x1: shell.right, y0: box.top + 10, y1: summaryTop - 10, above: false },
    { x0: shell.left, x1: shell.right, y0: box.top + 8, y1: top - 12, above: true },
  ].map((region) => {
    const width = region.x1 - region.x0;
    const height = region.y1 - region.y0;
    return { ...region, size: Math.max(0, Math.min(height, width / ASPECT)) };
  });
  const best = candidates.reduce((a, b) => (b.size > a.size ? b : a));
  const size = Math.min(best.size * 0.94, 340);
  const width = size * ASPECT;
  const x = best.above ? (best.x0 + best.x1) / 2 : Math.min(best.x1 - width / 2, (best.x0 + best.x1) / 2 + (best.x1 - best.x0 - width) * 0.3);
  return { x: x - box.left, y: (best.y0 + best.y1) / 2 - box.top, size, above: best.above };
}

// The runs -----------------------------------------------------------------------------------------

// A run of extract-all: which cards come out, and when each does what. Everything is a function
// of the clock, so a frame can be drawn for any moment.
function planRun(kind, start, serial, count) {
  const offset = (serial * 5) % ENTRIES.length;
  const cards = [];
  const hostileAt = kind === 'unsafe' ? 2 : -1;
  const skipped = kind === 'skip' ? new Set([1, 4]) : new Set();
  for (let i = 0; i < count; i++) {
    const entry = i === hostileAt ? HOSTILE[serial % HOSTILE.length] : ENTRIES[(offset + i) % ENTRIES.length];
    const emergeAt = start + 1.35 + i * 0.55;
    const beltAt = emergeAt + 0.85;
    const stageX = PLAN.x1 - 0.06 - i * ((PLAN.x1 - PLAN.x0 - 0.14) / Math.max(1, count - 1));
    const scanAt = beltAt + (GATE.x - BELT_START) / SPEED;
    const arriveAt = beltAt + (stageX - BELT_START) / SPEED + 0.12;
    cards.push({ entry, emergeAt, beltAt, stageX, scanAt, arriveAt, hostile: i === hostileAt, skip: skipped.has(i), writeAt: Infinity, landAt: Infinity, slot: 0 });
  }
  const run = { kind, start, serial, cards, abortAt: Infinity, abortWhy: '', end: Infinity, flowUntil: Infinity };
  const lastArrive = Math.max(...cards.map((card) => card.arriveAt));
  const perTray = [0, 0, 0, 0];
  // Existing files a skip run finds in out/: they take their slots first.
  for (const card of cards) {
    if (card.skip) card.slot = perTray[card.entry.tray]++;
  }
  if (kind === 'unsafe') {
    const hostile = cards[hostileAt];
    run.abortAt = hostile.scanAt + 0.4;
    run.abortWhy = 'unsafe';
    run.end = run.abortAt + 3.0;
    run.flowUntil = hostile.scanAt;
  } else {
    let order = 0;
    for (const card of cards) {
      if (card.skip) continue;
      card.writeAt = lastArrive + 0.55 + order * 0.18;
      card.landAt = card.writeAt + 0.62;
      if (kind !== 'dry') card.slot = perTray[card.entry.tray]++;
      order++;
    }
    const last = Math.max(...cards.map((card) => (Number.isFinite(card.landAt) ? card.landAt : card.arriveAt)));
    run.end = last + (kind === 'dry' ? 1.6 : 2.2);
    run.flowUntil = last;
  }
  return run;
}

const TRAY_CENTERS = [
  new THREE.Vector3(1.54, 0, -0.2),
  new THREE.Vector3(2.02, 0, -0.2),
  new THREE.Vector3(1.54, 0, 0.2),
  new THREE.Vector3(2.02, 0, 0.2),
];
const TRAY = { w: 0.4, d: 0.28, h: 0.08 };
const TRAY_FLOOR = FLOOR_Y + 0.035;

function traySlot(tray, slot, out) {
  const centre = TRAY_CENTERS[tray];
  return out.set(centre.x - 0.12 + slot * 0.075, TRAY_FLOOR + CARD.h / 2, centre.z - 0.02);
}

const RISE_FROM = new THREE.Vector3(CHEST.x, CHEST_TOP - 0.06, 0.02);
const RISE_TO = new THREE.Vector3(CHEST.x + 0.08, CHEST_TOP + 0.3, 0.05);
const HOP_TO = new THREE.Vector3(BELT_START, CARD_Y, 0);

// Where a card is at time t, where it is headed and how it looks. `out` is reused.
function cardPose(run, card, t, out) {
  out.visible = t >= card.emergeAt && (!(t >= run.abortAt) || card.emergeAt < run.abortAt);
  out.ghost = run.kind === 'dry';
  out.verdict = 0;
  out.glow = 0;
  out.alpha = 1;
  out.wipe = 0;
  out.rx = 0;
  out.ry = 0;
  out.rz = 0;
  out.scale = 1;
  if (!out.visible) return out;
  const p = out.position;
  const since = t - card.emergeAt;
  if (since < 0.35) {
    const k = smooth(since / 0.35);
    p.lerpVectors(RISE_FROM, RISE_TO, k);
    out.rz = 0.25 * (1 - k);
    out.scale = 0.4 + 0.6 * k;
  } else if (t < card.beltAt) {
    const k = clamp01((t - card.emergeAt - 0.35) / 0.5);
    p.lerpVectors(RISE_TO, HOP_TO, smooth(k));
    p.y = lerp(RISE_TO.y, HOP_TO.y, k * k) + 0.18 * Math.sin(Math.PI * k);
    out.rz = -0.6 * Math.sin(Math.PI * k);
  } else {
    const linear = BELT_START + SPEED * (t - card.beltAt);
    const settle = card.stageX - 0.09;
    p.set(linear < settle ? linear : settle + 0.09 * (1 - Math.exp(-(linear - settle) / 0.05)), CARD_Y, 0);
    p.x = Math.min(p.x, card.stageX);
  }
  out.wipe = card.hostile ? 0 : smooth((t - card.scanAt + 0.12) / 0.26);
  const scanned = t >= card.scanAt;
  if (scanned) {
    out.verdict = card.hostile ? 1 : card.skip ? 2 : 0;
    out.glow = Math.exp(-(t - card.scanAt) * 3.5) * (card.hostile ? 1.4 : 0.7);
  }
  if (t >= card.writeAt) {
    const k = clamp01((t - card.writeAt) / 0.62);
    const from = out.from.set(card.stageX, CARD_Y, 0);
    const to = run.kind === 'dry' ? out.to.set(card.stageX, CARD_Y + 0.5, 0) : traySlot(card.entry.tray, card.slot, out.to);
    if (run.kind === 'dry') {
      p.lerpVectors(from, to, smooth(k));
      out.alpha = 1 - smooth(k);
      out.glow = Math.max(out.glow, 1 - k);
      out.visible = k < 1;
    } else {
      p.lerpVectors(from, to, smooth(k));
      p.y = lerp(from.y, to.y, k * k) + 0.42 * Math.sin(Math.PI * Math.min(1, k * 1.05));
      out.rz = 0.35 * Math.sin(Math.PI * k);
      out.ghost = t < card.landAt;
      out.glow = Math.max(out.glow, t >= card.landAt ? Math.exp(-(t - card.landAt) * 5) * 1.6 : 0.4);
    }
  } else if (card.skip && t >= run.end - 1.2) {
    const k = clamp01((t - (run.end - 1.2)) / 0.6);
    out.alpha = 1 - k;
    out.ghost = true;
    out.visible = k < 1;
  }
  if (t >= run.abortAt && card.writeAt > run.abortAt) {
    const k = clamp01((t - run.abortAt) / 0.75);
    if (card.hostile) {
      const shake = Math.sin((t - run.abortAt) * 40) * 0.02 * (1 - k);
      p.x += shake;
      out.scale *= 1 - smooth((t - run.abortAt - 0.35) / 0.5);
      out.visible = out.scale > 0.01;
      out.glow = 1.2;
    } else {
      out.from.copy(p);
      out.to.copy(RISE_FROM);
      p.lerpVectors(out.from, out.to, smooth(k));
      p.y += 0.3 * Math.sin(Math.PI * k);
      out.scale *= 1 - 0.6 * smooth(k);
      out.visible = k < 1;
      out.rz = 0.8 * k;
    }
  }
  return out;
}

// The press ------------------------------------------------------------------------------------

// Hold the archive down and nothing happens. Keep holding: after a second and a half a hydraulic
// ram comes down out of the dark and the terminal types `create Morrowind.bsa out --format tes3
// --compress`. The needle climbs, steam leaks from the seams, the rivets let go one by one, and at
// full pressure the archive crushes into a white-hot brick. The brick rides the belt into the gate,
// which refuses it with dream_archivetool's own error, because Morrowind's BSA has no compression,
// and flings it away. The archive springs back, lid open, papers everywhere. Let go early and the
// ram backs off; the terminal says ^C.
const PRESS = { delay: 1.5, charge: 4.0, move: 12 };

// The scene --------------------------------------------------------------------------------------

const loaded = document.readyState === 'complete' ? Promise.resolve() : new Promise((resolve) => addEventListener('load', resolve, { once: true }));
const breathe = () => new Promise((resolve) => setTimeout(resolve, 0));

async function mount(root) {
  const hero = root.closest('.dw-hero') || root;
  hero.classList.add('dat-hero-on');
  const still = document.createElement('img');
  still.className = 'dat-hero__still';
  still.alt = '';
  still.decoding = 'async';
  still.src = new URL('../img/archivetool-hero.webp', import.meta.url).href;
  root.append(still);
  requestAnimationFrame(placeStill);

  // The page comes first: the machine is built once it has loaded, the still standing in until then.
  const fontFamily = '"deja-vu-sans-mono", ui-monospace, "DejaVu Sans Mono", monospace';
  const fontReady = document.fonts ? Promise.race([document.fonts.load(`600 40px ${fontFamily}`).catch(() => {}), new Promise((resolve) => setTimeout(resolve, 1200))]) : Promise.resolve();
  await loaded;
  await breathe();

  const canvas = document.createElement('canvas');
  canvas.className = 'dat-hero__canvas';
  // Ask for WebGL 2 before three.js does, so a browser without it gets the still and no console noise.
  if (!document.createElement('canvas').getContext('webgl2')) {
    placeStill();
    return;
  }
  let renderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, antialias: false, alpha: false, powerPreference: 'high-performance' });
  } catch {
    placeStill();
    return;
  }
  if (!renderer.capabilities.isWebGL2) {
    renderer.dispose();
    placeStill();
    return;
  }

  renderer.autoClear = false;
  renderer.outputColorSpace = THREE.LinearSRGBColorSpace;
  root.append(canvas);

  const small = Math.min(innerWidth, innerHeight) < 700;
  const anisotropy = Math.min(8, renderer.capabilities.getMaxAnisotropy());
  const floatTargets = renderer.extensions.has('EXT_color_buffer_float') || renderer.extensions.has('EXT_color_buffer_half_float');
  const targetType = floatTargets ? THREE.HalfFloatType : THREE.UnsignedByteType;
  const makeTarget = () => new THREE.WebGLRenderTarget(1, 1, { type: targetType, depthBuffer: false });
  const sceneTarget = new THREE.WebGLRenderTarget(1, 1, { type: targetType, samples: small ? 0 : 4 });
  const bloomTargets = [makeTarget(), makeTarget(), makeTarget(), makeTarget()];

  const accent = cssColor('--dw-accent', '#e0616b');
  const top = cssColor('--dw-bg-1', '#1a0f12');
  const bottom = cssColor('--dw-bg-0', '#0f080a');
  const text = cssColor('--dw-text', '#f3e7e8');
  const ok = accent.clone().lerp(new THREE.Color(1, 1, 1), 0.35);
  const bad = new THREE.Color(1.0, 0.12, 0.08);
  const amber = new THREE.Color(1.0, 0.62, 0.12);

  const camera = new THREE.PerspectiveCamera(26, 1, 0.1, 80);
  const scene = new THREE.Scene();
  const envTarget = environment(renderer, accent);
  scene.environment = envTarget.texture;

  const quad = new THREE.PlaneGeometry(2, 2);
  const skyUniforms = {
    uTop: { value: top },
    uBottom: { value: bottom },
    uGlow: { value: accent.clone().multiplyScalar(0.03) },
    uCenter: { value: new THREE.Vector2(0.7, 0.5) },
    uResolution: { value: new THREE.Vector2(1, 1) },
    uRadius: { value: 0.35 },
    uTime: { value: 0 },
  };
  const sky = new THREE.Mesh(quad, fullscreenMaterial(SKY_FRAGMENT, skyUniforms));
  sky.frustumCulled = false;
  sky.renderOrder = -10;
  scene.add(sky);

  // The machine hangs off one rig: placed, scaled and tilted as a whole.
  const rig = new THREE.Group();
  const machine = new THREE.Group();
  machine.position.set(-CENTER.x, -CENTER.y, 0);
  rig.add(machine);
  scene.add(rig);

  // Textures: the plain ones while the font arrives, then the lettered ones.
  const lacquer = lacquerMaps(256, accent.clone().multiplyScalar(0.42).lerp(new THREE.Color(0.25, 0.01, 0.02), 0.5), anisotropy);
  await breathe();
  const brass = brassMaps(256, 32, anisotropy);
  await fontReady;
  await breathe();
  const atlas = cardAtlas(fontFamily, small ? 128 : 192);
  const atlasTexture = textureFrom(atlas.canvas, THREE.SRGBColorSpace, anisotropy);
  atlasTexture.flipY = false;

  // Materials.
  const lacquerMaterial = new THREE.MeshPhysicalMaterial({
    map: lacquer.map,
    roughnessMap: lacquer.roughness,
    roughness: 1,
    metalness: 0,
    clearcoat: 1,
    clearcoatRoughness: 0.08,
    envMapIntensity: 1.1,
    emissive: new THREE.Color(1.0, 0.3, 0.1),
    emissiveIntensity: 0,
  });
  const brassMaterial = new THREE.MeshStandardMaterial({ map: brass.map, roughnessMap: brass.roughness, roughness: 1, metalness: 1, envMapIntensity: 1.2, emissive: new THREE.Color(1.0, 0.4, 0.1), emissiveIntensity: 0 });
  const steelMaterial = new THREE.MeshStandardMaterial({ color: new THREE.Color(0.8, 0.8, 0.84), metalness: 1, roughness: 0.36, envMapIntensity: 2.2 });
  const railMaterial = new THREE.MeshStandardMaterial({ color: new THREE.Color(0.72, 0.52, 0.26), metalness: 1, roughness: 0.34, envMapIntensity: 1.1 });
  const darkSteel = new THREE.MeshStandardMaterial({ color: new THREE.Color(0.06, 0.055, 0.06), metalness: 0.8, roughness: 0.42, envMapIntensity: 0.8 });
  const epoxy = new THREE.MeshStandardMaterial({ color: new THREE.Color(0.018, 0.016, 0.018), metalness: 0.1, roughness: 0.55, envMapIntensity: 0.6 });
  const tin = new THREE.MeshStandardMaterial({ color: new THREE.Color(0.78, 0.78, 0.8), metalness: 1, roughness: 0.3, envMapIntensity: 1.2 });

  // The board.
  const boardUniforms = {
    tBoard: { value: boardTexture(small ? 1024 : 1536, fontFamily) },
    uRect: { value: new THREE.Vector4(BOARD.x0, BOARD.z0, BOARD.width, BOARD.depth) },
    uMask: { value: accent.clone().multiplyScalar(0.018).lerp(new THREE.Color(0.012, 0.004, 0.005), 0.5) },
    uCopper: { value: new THREE.Color(0.12, 0.05, 0.022) },
    uPad: { value: new THREE.Color(0.22, 0.21, 0.2) },
    uSilk: { value: new THREE.Color(0.24, 0.22, 0.21) },
    uPulse: { value: accent.clone().lerp(new THREE.Color(1, 0.8, 0.6), 0.25) },
    uFlow: { value: 0 },
    uTime: { value: 0 },
    uLamp: { value: new THREE.Vector3() },
    uLampPower: { value: 0 },
    uShadows: { value: [
      new THREE.Vector4(CHIP.x, 0, CHIP.w / 2, CHIP.d / 2),
      new THREE.Vector4((BELT.x0 + PLAN.x1) / 2, 0, (PLAN.x1 - BELT.x0) / 2, BELT.half),
      new THREE.Vector4((CASE.x0 + CASE.x1) / 2, 0, (CASE.x1 - CASE.x0) / 2, (CASE.z1 - CASE.z0) / 2),
    ] },
  };
  const board = new THREE.Mesh(new THREE.PlaneGeometry(9, 5.4), new THREE.ShaderMaterial({
    vertexShader: BOARD_VERTEX,
    fragmentShader: BOARD_FRAGMENT,
    uniforms: boardUniforms,
    transparent: true,
    depthWrite: false,
  }));
  board.rotation.x = -Math.PI / 2;
  board.position.set(0, FLOOR_Y, 0);
  board.renderOrder = -5;
  machine.add(board);

  // The archive chip the archive sits on, legs and all.
  const chip = new THREE.Mesh(roundedBox(CHIP.w, CHIP.h, CHIP.d, 0.02, 2), epoxy);
  chip.position.set(CHIP.x, FLOOR_Y + CHIP.h / 2, 0);
  machine.add(chip);
  const legGeometry = new THREE.BoxGeometry(0.036, 0.024, 0.09);
  const legs = new THREE.InstancedMesh(legGeometry, tin, 24);
  {
    const dummy = new THREE.Object3D();
    let i = 0;
    for (let n = 0; n < 12; n++) {
      const x = CHIP.x - CHIP.w / 2 + 0.1 + n * ((CHIP.w - 0.2) / 11);
      for (const side of [-1, 1]) {
        dummy.position.set(x, FLOOR_Y + 0.03, side * (CHIP.d / 2 + 0.035));
        dummy.rotation.set(side * 0.35, 0, 0);
        dummy.updateMatrix();
        legs.setMatrixAt(i++, dummy.matrix);
      }
    }
  }
  machine.add(legs);

  // The archive.
  const chest = new THREE.Group();
  chest.position.set(CHEST.x, CHEST_BOTTOM, 0);
  const squash = new THREE.Group();
  chest.add(squash);
  const body = new THREE.Mesh(roundedBox(CHEST.w, CHEST.h, CHEST.d, 0.035, 3), lacquerMaterial);
  body.position.y = CHEST.h / 2;
  squash.add(body);
  const lidPivot = new THREE.Group();
  lidPivot.position.set(0, CHEST.h, -CHEST.d / 2);
  squash.add(lidPivot);
  const lid = new THREE.Mesh(roundedBox(CHEST.w + 0.02, CHEST.lid, CHEST.d + 0.02, 0.04, 3), lacquerMaterial);
  lid.position.set(0, CHEST.lid / 2, CHEST.d / 2);
  lidPivot.add(lid);
  // Brass: two bands over body and lid, and the plate with the archive's name.
  const bandGeometry = new THREE.BoxGeometry(0.07, CHEST.h + 0.004, CHEST.d + 0.016);
  for (const x of [-0.27, 0.27]) {
    const band = new THREE.Mesh(bandGeometry, brassMaterial);
    band.position.set(x, CHEST.h / 2, 0);
    squash.add(band);
    const lidBand = new THREE.Mesh(new THREE.BoxGeometry(0.07, CHEST.lid + 0.012, CHEST.d + 0.036), brassMaterial);
    lidBand.position.set(x, CHEST.lid / 2, CHEST.d / 2);
    lidPivot.add(lidBand);
  }
  const plate = new THREE.Mesh(new THREE.BoxGeometry(0.34, 0.12, 0.012), [brassMaterial, brassMaterial, brassMaterial, brassMaterial, new THREE.MeshStandardMaterial({ map: plateTexture(fontFamily, anisotropy), metalness: 1, roughness: 0.32, envMapIntensity: 1.2 }), brassMaterial]);
  plate.position.set(-0.02, CHEST.h * 0.62, CHEST.d / 2 + 0.012);
  squash.add(plate);
  // The name-hash table, down both flanks.
  const hashes = hashTexture(fontFamily, anisotropy);
  const hashMaterial = additive(GLASS_VERTEX, /* glsl */ `
    uniform sampler2D tHash;
    uniform vec3 uColor;
    uniform float uOffset;
    varying vec2 vUv;
    void main() {
      float t = texture2D(tHash, vec2(vUv.x, vUv.y * 0.6 + uOffset)).r;
      float fade = smoothstep(0.0, 0.12, vUv.y) * smoothstep(1.0, 0.88, vUv.y);
      gl_FragColor = vec4(uColor * t * fade * 0.9, 1.0);
    }
  `, { tHash: { value: hashes }, uColor: { value: accent.clone().multiplyScalar(1.2) }, uOffset: { value: 0 } });
  for (const side of [-1, 1]) {
    const flank = new THREE.Mesh(new THREE.PlaneGeometry(CHEST.d * 0.78, CHEST.h * 0.8), hashMaterial);
    flank.position.set(side * (CHEST.w / 2 + 0.003), CHEST.h / 2, 0);
    flank.rotation.y = side * Math.PI / 2;
    squash.add(flank);
  }
  // The gauge: brass rim, dial, needle, glass. The hint.
  const gauge = new THREE.Group();
  gauge.position.set(0.3, CHEST.h * 0.3, CHEST.d / 2 + 0.008);
  const rim = new THREE.Mesh(new THREE.TorusGeometry(0.052, 0.01, 10, 32), brassMaterial);
  gauge.add(rim);
  const dial = new THREE.Mesh(new THREE.CircleGeometry(0.05, 32), new THREE.MeshStandardMaterial({ map: dialTexture(fontFamily), roughness: 0.6, metalness: 0 }));
  dial.position.z = 0.002;
  gauge.add(dial);
  const needlePivot = new THREE.Group();
  needlePivot.position.z = 0.006;
  const needle = new THREE.Mesh(new THREE.BoxGeometry(0.005, 0.042, 0.003), new THREE.MeshStandardMaterial({ color: 0x8b1a14, roughness: 0.4 }));
  needle.position.y = 0.017;
  needlePivot.add(needle);
  gauge.add(needlePivot);
  const gaugeGlass = new THREE.Mesh(new THREE.CircleGeometry(0.05, 32), new THREE.MeshStandardMaterial({ transparent: true, opacity: 0.16, roughness: 0.04, metalness: 0, envMapIntensity: 2 }));
  gaugeGlass.position.z = 0.012;
  gauge.add(gaugeGlass);
  squash.add(gauge);
  // Rivets on the bands, which the press can pop.
  const rivetPlaces = [];
  for (const x of [-0.27, 0.27]) {
    for (const y of [0.08, 0.25, 0.42]) rivetPlaces.push(new THREE.Vector3(x, y, CHEST.d / 2 + 0.012));
  }
  for (const side of [-1, 1]) {
    for (const y of [0.12, 0.38]) rivetPlaces.push(new THREE.Vector3(side * 0.27, y, -CHEST.d / 2 - 0.012));
  }
  const rivets = new THREE.InstancedMesh(new THREE.SphereGeometry(0.016, 10, 8), brassMaterial, rivetPlaces.length);
  rivets.frustumCulled = false;
  machine.add(rivets);
  const rivetState = rivetPlaces.map(() => ({ popped: false, at: 0, position: new THREE.Vector3(), velocity: new THREE.Vector3(), spin: 0 }));
  // Light from inside, when the lid is up.
  const innerGlow = new THREE.Mesh(new THREE.PlaneGeometry(CHEST.w * 0.86, CHEST.d * 0.84), additive(GLASS_VERTEX, /* glsl */ `
    uniform vec3 uColor;
    uniform float uPower;
    varying vec2 vUv;
    void main() {
      vec2 p = vUv - 0.5;
      float g = 1.0 - smoothstep(0.0, 0.5, length(p * vec2(1.0, 1.3)));
      gl_FragColor = vec4(uColor * g * uPower, 1.0);
    }
  `, { uColor: { value: new THREE.Color(1.0, 0.72, 0.5).lerp(accent, 0.3) }, uPower: { value: 0 } }));
  innerGlow.rotation.x = -Math.PI / 2;
  innerGlow.position.y = CHEST.h + 0.003;
  squash.add(innerGlow);
  const shaftTop = [[-0.36, -0.24], [0.36, -0.24], [0.36, 0.24], [-0.36, 0.24]].map(([x, z]) => new THREE.Vector3(x, 0, z));
  const shaftBottom = [[-0.7, -0.5], [0.7, -0.5], [0.7, 0.5], [-0.7, 0.5]].map(([x, z]) => new THREE.Vector3(x, 1.1, z));
  const shaftUniforms = { uColor: { value: new THREE.Color(1.0, 0.7, 0.45).lerp(accent, 0.35) }, uPower: { value: 0 }, uTime: { value: 0 }, uDashed: { value: 0 } };
  const shaft = new THREE.Mesh(frustum(shaftTop, shaftBottom), additive(BEAM_VERTEX, /* glsl */ `
    uniform vec3 uColor;
    uniform float uPower;
    uniform float uTime;
    varying vec2 vUv;
    void main() {
      float along = vUv.y;
      float fade = (1.0 - along) * (1.0 - along) * smoothstep(0.0, 0.06, along);
      float motes = 0.75 + 0.25 * sin(along * 24.0 - uTime * 1.5 + vUv.x * 30.0);
      float edge = pow(1.0 - abs(fract(vUv.x * 4.0) - 0.5) * 2.0, 2.0);
      gl_FragColor = vec4(uColor * fade * motes * (0.25 + 0.75 * edge) * uPower * 0.35, 1.0);
    }
  `, shaftUniforms));
  shaft.position.y = CHEST.h + 0.01;
  squash.add(shaft);
  machine.add(chest);

  // The belt, the plan table and their legs.
  const beltBed = new THREE.Mesh(new THREE.BoxGeometry(BELT.x1 - BELT.x0, 0.06, BELT.half * 2 + 0.04), darkSteel);
  beltBed.position.set((BELT.x0 + BELT.x1) / 2, BELT.y - 0.03, 0);
  machine.add(beltBed);
  const beltUniforms = { uOffset: { value: 0 }, uScan: { value: ok.clone() }, uScanX: { value: GATE.x }, uScanPower: { value: 0.4 } };
  const beltSurface = new THREE.Mesh(new THREE.PlaneGeometry(BELT.x1 - BELT.x0, BELT.half * 2), new THREE.ShaderMaterial({ vertexShader: BOARD_VERTEX, fragmentShader: BELT_FRAGMENT, uniforms: beltUniforms }));
  beltSurface.geometry.translate((BELT.x0 + BELT.x1) / 2, 0, 0);
  beltSurface.rotation.x = -Math.PI / 2;
  beltSurface.position.set(0, BELT.y + 0.001, 0);
  machine.add(beltSurface);
  for (const x of [BELT.x0, BELT.x1]) {
    const roller = new THREE.Mesh(new THREE.CylinderGeometry(0.034, 0.034, BELT.half * 2 + 0.05, 20), steelMaterial);
    roller.rotation.x = Math.PI / 2;
    roller.position.set(x, BELT.y - 0.03, 0);
    machine.add(roller);
  }
  for (const side of [-1, 1]) {
    const rail = new THREE.Mesh(new THREE.BoxGeometry(PLAN.x1 - BELT.x0 + 0.04, 0.034, 0.018), railMaterial);
    rail.position.set((BELT.x0 + PLAN.x1) / 2, BELT.y + 0.012, side * (BELT.half + 0.02));
    machine.add(rail);
  }
  const planTable = new THREE.Mesh(new THREE.BoxGeometry(PLAN.x1 - PLAN.x0, 0.06, BELT.half * 2 + 0.04), new THREE.MeshStandardMaterial({ color: new THREE.Color(0.05, 0.02, 0.025), metalness: 0.3, roughness: 0.14, envMapIntensity: 1.1 }));
  planTable.position.set((PLAN.x0 + PLAN.x1) / 2, BELT.y - 0.03, 0);
  machine.add(planTable);
  const legPosts = new THREE.InstancedMesh(new THREE.CylinderGeometry(0.022, 0.026, 1, 12), darkSteel, 8);
  {
    const dummy = new THREE.Object3D();
    const posts = [[BELT.x0 + 0.08, -0.14], [BELT.x0 + 0.08, 0.14], [PLAN.x1 - 0.06, -0.14], [PLAN.x1 - 0.06, 0.14], [PLAN.x0, -0.14], [PLAN.x0, 0.14]];
    posts.forEach(([x, z], i) => {
      const height = BELT.y - 0.06 - FLOOR_Y;
      dummy.position.set(x, FLOOR_Y + height / 2, z);
      dummy.scale.set(1, height, 1);
      dummy.updateMatrix();
      legPosts.setMatrixAt(i, dummy.matrix);
    });
    legPosts.count = posts.length;
  }
  machine.add(legPosts);

  // The gate: the icon's crossed wrenches, the pivot bolt that is the scanner, its beam, and the
  // terminal the jaws hold up.
  const gate = new THREE.Group();
  gate.position.set(GATE.x, GATE.hub, GATE.z);
  const wrench = wrenchGeometry();
  const wrenchRings = [];
  for (const [side, depth] of [[1, 0.034], [-1, -0.034]]) {
    const mesh = new THREE.Mesh(wrench, steelMaterial);
    mesh.scale.setScalar(GATE.scale);
    mesh.rotation.z = Math.PI / 2 - side * GATE.tilt;
    mesh.position.z = depth;
    gate.add(mesh);
    const direction = new THREE.Vector2(Math.cos(mesh.rotation.z), Math.sin(mesh.rotation.z));
    wrenchRings.push(new THREE.Vector3(direction.x * -0.78 * GATE.scale, direction.y * -0.78 * GATE.scale, depth));
  }
  const bolt = new THREE.Mesh(new THREE.CylinderGeometry(0.07, 0.07, 0.16, 6), steelMaterial);
  bolt.rotation.x = Math.PI / 2;
  gate.add(bolt);
  const lens = new THREE.Mesh(new THREE.CircleGeometry(0.048, 32), additive(GLASS_VERTEX, /* glsl */ `
    uniform vec3 uColor;
    uniform float uPower;
    varying vec2 vUv;
    void main() {
      float r = length(vUv - 0.5) * 2.0;
      gl_FragColor = vec4(uColor * (1.0 - smoothstep(0.2, 1.0, r)) * uPower * 3.0, 1.0);
    }
  `, { uColor: { value: ok.clone() }, uPower: { value: 1 } }));
  lens.position.z = 0.082;
  gate.add(lens);
  for (const ring of wrenchRings) {
    const post = new THREE.Mesh(new THREE.CylinderGeometry(0.02, 0.026, 1, 12), darkSteel);
    const height = GATE.hub + ring.y - FLOOR_Y;
    post.scale.y = height;
    post.position.set(ring.x, ring.y - height / 2, ring.z);
    gate.add(post);
  }
  machine.add(gate);
  const beamTop = [[-0.03, -0.03], [0.03, -0.03], [0.03, 0.03], [-0.03, 0.03]].map(([x, z]) => new THREE.Vector3(GATE.x + x, GATE.hub, GATE.z + 0.08 + z));
  const beamBottom = [[-0.09, -BELT.half], [0.09, -BELT.half], [0.09, BELT.half], [-0.09, BELT.half]].map(([x, z]) => new THREE.Vector3(GATE.x + x, BELT.y + 0.004, z));
  const beamUniforms = { uColor: { value: ok.clone() }, uPower: { value: 0.3 }, uTime: { value: 0 }, uDashed: { value: 0 } };
  const beam = new THREE.Mesh(frustum(beamTop, beamBottom), additive(BEAM_VERTEX, BEAM_FRAGMENT, beamUniforms));
  machine.add(beam);
  const terminal = new Terminal(fontFamily, { text: `#${text.clone().convertLinearToSRGB().getHexString()}`, accent: hex(accent.clone().lerp(new THREE.Color(1, 1, 1), 0.2)), ok: hex(ok), bad: '#ff5a4f', amber: '#f0b04a', dim: '#9a8a8c' });
  const screenGroup = new THREE.Group();
  screenGroup.position.set(GATE.x, SCREEN.y, SCREEN.z);
  const bezel = new THREE.Mesh(roundedBox(SCREEN.w + 0.08, SCREEN.h + 0.08, 0.07, 0.03, 2), darkSteel);
  bezel.position.z = -0.03;
  screenGroup.add(bezel);
  const screenUniforms = { tText: { value: terminal.texture }, uTint: { value: accent.clone() }, uTime: { value: 0 }, uPower: { value: 1 } };
  const screen = new THREE.Mesh(new THREE.PlaneGeometry(SCREEN.w, SCREEN.h), new THREE.ShaderMaterial({ vertexShader: GLASS_VERTEX, fragmentShader: SCREEN_FRAGMENT, uniforms: screenUniforms }));
  screen.position.z = 0.008;
  screenGroup.add(screen);
  for (const x of [-1, 1]) {
    const corner = new THREE.Mesh(new THREE.BoxGeometry(0.1, 0.1, 0.09), brassMaterial);
    corner.position.set(x * (SCREEN.w / 2 + 0.02), -(SCREEN.h / 2 + 0.02), -0.02);
    screenGroup.add(corner);
  }
  machine.add(screenGroup);

  // out/: a glass case with glowing edges, four card-file trays inside.
  const caseGroup = new THREE.Group();
  const caseSize = new THREE.Vector3(CASE.x1 - CASE.x0, CASE.y1 - FLOOR_Y, CASE.z1 - CASE.z0);
  caseGroup.position.set((CASE.x0 + CASE.x1) / 2, FLOOR_Y + caseSize.y / 2, 0);
  const glassUniforms = { uColor: { value: accent.clone().lerp(new THREE.Color(1, 1, 1), 0.2) }, uPower: { value: 1 }, tLabel: { value: caseLabel(fontFamily) }, uHasLabel: { value: 0 } };
  const glassMaterial = additive(GLASS_VERTEX, GLASS_FRAGMENT, glassUniforms);
  const frontGlassMaterial = additive(GLASS_VERTEX, GLASS_FRAGMENT, { ...glassUniforms, uHasLabel: { value: 1 } });
  for (const [size, position, rotation, material] of [
    [[caseSize.x, caseSize.y], [0, 0, caseSize.z / 2], [0, 0, 0], frontGlassMaterial],
    [[caseSize.x, caseSize.y], [0, 0, -caseSize.z / 2], [0, Math.PI, 0], glassMaterial],
    [[caseSize.z, caseSize.y], [caseSize.x / 2, 0, 0], [0, Math.PI / 2, 0], glassMaterial],
    [[caseSize.z, caseSize.y], [-caseSize.x / 2, 0, 0], [0, -Math.PI / 2, 0], glassMaterial],
  ]) {
    const pane = new THREE.Mesh(new THREE.PlaneGeometry(...size), material);
    pane.position.set(...position);
    pane.rotation.set(...rotation);
    caseGroup.add(pane);
  }
  const edgeMaterial = new THREE.MeshBasicMaterial({ color: accent.clone().multiplyScalar(1.5) });
  const edgeThickness = 0.012;
  for (const [x, z] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
    const post = new THREE.Mesh(new THREE.BoxGeometry(edgeThickness, caseSize.y, edgeThickness), edgeMaterial);
    post.position.set(x * caseSize.x / 2, 0, z * caseSize.z / 2);
    caseGroup.add(post);
  }
  for (const y of [-1, 1]) {
    for (const z of [-1, 1]) {
      const bar = new THREE.Mesh(new THREE.BoxGeometry(caseSize.x, edgeThickness, edgeThickness), edgeMaterial);
      bar.position.set(0, y * caseSize.y / 2, z * caseSize.z / 2);
      caseGroup.add(bar);
    }
    for (const x of [-1, 1]) {
      const bar = new THREE.Mesh(new THREE.BoxGeometry(edgeThickness, edgeThickness, caseSize.z), edgeMaterial);
      bar.position.set(x * caseSize.x / 2, y * caseSize.y / 2, 0);
      caseGroup.add(bar);
    }
  }
  machine.add(caseGroup);
  const labels = trayLabels(fontFamily, anisotropy);
  const trayMaterial = new THREE.MeshStandardMaterial({ color: new THREE.Color(0.1, 0.035, 0.03), metalness: 0.6, roughness: 0.35, envMapIntensity: 1.1 });
  TRAY_CENTERS.forEach((centre, i) => {
    const tray = new THREE.Group();
    tray.position.set(centre.x, FLOOR_Y + 0.004, centre.z);
    const floor = new THREE.Mesh(new THREE.BoxGeometry(TRAY.w, 0.02, TRAY.d), trayMaterial);
    floor.position.y = 0.02;
    tray.add(floor);
    for (const [w, d, x, z] of [[TRAY.w, 0.012, 0, -TRAY.d / 2], [0.012, TRAY.d, -TRAY.w / 2, 0], [0.012, TRAY.d, TRAY.w / 2, 0]]) {
      const wall = new THREE.Mesh(new THREE.BoxGeometry(w, TRAY.h, d), trayMaterial);
      wall.position.set(x, TRAY.h / 2, z);
      tray.add(wall);
    }
    const labelTexture = labels.clone();
    labelTexture.repeat.set(1, 0.25);
    labelTexture.offset.set(0, 0.75 - i * 0.25);
    labelTexture.needsUpdate = true;
    const front = new THREE.Mesh(new THREE.BoxGeometry(TRAY.w, TRAY.h * 0.7, 0.012), [trayMaterial, trayMaterial, trayMaterial, trayMaterial, new THREE.MeshStandardMaterial({ map: labelTexture, roughness: 0.5, metalness: 0.2, emissive: new THREE.Color(0.2, 0.15, 0.12), emissiveMap: labelTexture }), trayMaterial]);
    front.position.set(0, TRAY.h * 0.35, TRAY.d / 2);
    tray.add(front);
    machine.add(tray);
  });

  // The press, hidden until wanted.
  const press = new THREE.Group();
  press.position.set(CHEST.x, 3, 0);
  const hazard = hazardTexture();
  hazard.wrapS = THREE.RepeatWrapping;
  const plateMaterial = new THREE.MeshStandardMaterial({ map: hazard, metalness: 0.5, roughness: 0.45, envMapIntensity: 1 });
  const pressPlate = new THREE.Mesh(new THREE.BoxGeometry(CHEST.w + 0.14, 0.1, CHEST.d + 0.12), [plateMaterial, plateMaterial, darkSteel, darkSteel, plateMaterial, plateMaterial]);
  press.add(pressPlate);
  const rod = new THREE.Mesh(new THREE.CylinderGeometry(0.06, 0.06, 3.2, 20), steelMaterial);
  rod.position.y = 1.65;
  press.add(rod);
  const sleeve = new THREE.Mesh(new THREE.CylinderGeometry(0.11, 0.11, 1.6, 20), darkSteel);
  sleeve.position.y = 2.8;
  press.add(sleeve);
  press.visible = false;
  machine.add(press);

  // The brick the archive becomes, briefly.
  const brickMaterial = new THREE.MeshStandardMaterial({ color: new THREE.Color(0.3, 0.04, 0.03), emissive: new THREE.Color(1.0, 0.45, 0.2), emissiveIntensity: 3, metalness: 0.2, roughness: 0.35 });
  const brick = new THREE.Mesh(roundedBox(0.2, 0.11, 0.15, 0.02, 2), brickMaterial);
  brick.visible = false;
  machine.add(brick);

  // Cards: current run, previous run (clearing out of the trays), confetti; and their ghosts.
  const cardGeometry = new THREE.BoxGeometry(CARD.w, CARD.h, CARD.t);
  const CURRENT = 8;
  const CONFETTI = 24;
  const solidCount = CURRENT * 2 + CONFETTI;
  const ghostCount = CURRENT + 3;
  const cardMaterial = (ghost) => new THREE.ShaderMaterial({
    vertexShader: CARD_VERTEX,
    fragmentShader: CARD_FRAGMENT,
    uniforms: {
      tAtlas: { value: atlasTexture },
      uGrid: { value: new THREE.Vector2(ATLAS.columns, ATLAS.rows) },
      uStamp: { value: atlas.cells.stamp },
      uBack: { value: atlas.cells.back },
      uKey: { value: new THREE.Vector3(-0.4, 0.9, 0.6) },
      uLamp: { value: new THREE.Vector3() },
      uLampPower: { value: 0 },
      uGlowOk: { value: ok.clone() },
      uGlowBad: { value: bad.clone() },
      uGlowSkip: { value: amber.clone() },
      uGhost: { value: ghost ? 1 : 0 },
      uTime: { value: 0 },
    },
    ...(ghost ? { transparent: true, depthWrite: false, blending: THREE.AdditiveBlending } : {}),
  });
  const cardMesh = (count, ghost) => {
    const geometry = cardGeometry.clone();
    const cells = new THREE.InstancedBufferAttribute(new Float32Array(count * 4), 4);
    const look = new THREE.InstancedBufferAttribute(new Float32Array(count * 4), 4);
    cells.setUsage(THREE.DynamicDrawUsage);
    look.setUsage(THREE.DynamicDrawUsage);
    geometry.setAttribute('aCells', cells);
    geometry.setAttribute('aLook', look);
    const mesh = new THREE.InstancedMesh(geometry, cardMaterial(ghost), count);
    mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    mesh.frustumCulled = false;
    mesh.renderOrder = ghost ? 6 : 0;
    machine.add(mesh);
    return { mesh, cells, look };
  };
  const solids = cardMesh(solidCount, false);
  const ghosts = cardMesh(ghostCount, true);

  // Steam and sparks.
  const PUFFS = small ? 90 : 160;
  const puffGeometry = new THREE.BufferGeometry();
  const puffPositions = new Float32Array(PUFFS * 3);
  const puffData = new Float32Array(PUFFS * 4);
  puffGeometry.setAttribute('position', new THREE.BufferAttribute(puffPositions, 3).setUsage(THREE.DynamicDrawUsage));
  puffGeometry.setAttribute('aPuff', new THREE.BufferAttribute(puffData, 4).setUsage(THREE.DynamicDrawUsage));
  const puffUniforms = { uPixel: { value: 100 }, uSteam: { value: new THREE.Color(0.9, 0.86, 0.84) }, uSpark: { value: new THREE.Color(1.0, 0.45, 0.2) } };
  const puffs = new THREE.Points(puffGeometry, new THREE.ShaderMaterial({ vertexShader: PUFF_VERTEX, fragmentShader: PUFF_FRAGMENT, uniforms: puffUniforms, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending }));
  puffs.frustumCulled = false;
  puffs.renderOrder = 8;
  machine.add(puffs);
  const particles = Array.from({ length: PUFFS }, () => ({ life: 0, age: 0, span: 1, size: 0.1, kind: 0, position: new THREE.Vector3(), velocity: new THREE.Vector3() }));
  let particleCursor = 0;
  function emit(kind, position, velocity, span, size) {
    const particle = particles[particleCursor];
    particleCursor = (particleCursor + 1) % PUFFS;
    particle.kind = kind;
    particle.position.copy(position);
    particle.velocity.copy(velocity);
    particle.age = 0;
    particle.span = span;
    particle.size = size;
    particle.life = 1;
  }

  // Lights: a warm key, a cool fill, a rim in the accent, the pointer's lamp, the archive's inner
  // light and the scanner's.
  const key = new THREE.DirectionalLight(new THREE.Color(1.0, 0.93, 0.84), 2.1);
  key.position.set(-3, 5, 4);
  const rimLight = new THREE.DirectionalLight(accent, 2.4);
  rimLight.position.set(2, 3, -5);
  scene.add(key, rimLight);
  const lamp = new THREE.PointLight(new THREE.Color(1.0, 0.9, 0.8), 0, 0, 2);
  const chestLight = new THREE.PointLight(new THREE.Color(1.0, 0.7, 0.45), 0, 0, 2);
  const scanLight = new THREE.PointLight(ok.clone(), 0, 0, 2);
  scene.add(lamp, chestLight, scanLight);

  // Post-processing.
  const postScene = new THREE.Scene();
  const postCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  const postQuad = new THREE.Mesh(quad);
  postQuad.frustumCulled = false;
  postScene.add(postQuad);
  const brightMaterial = fullscreenMaterial(BRIGHT_FRAGMENT, { tInput: { value: sceneTarget.texture }, uThreshold: { value: 1.25 } });
  const blurMaterial = fullscreenMaterial(BLUR_FRAGMENT, { tInput: { value: null }, uDirection: { value: new THREE.Vector2() } });
  const copyMaterial = fullscreenMaterial(/* glsl */ `
    uniform sampler2D tInput;
    varying vec2 vUv;
    void main() { gl_FragColor = texture2D(tInput, vUv); }
  `, { tInput: { value: null } });
  const compositeUniforms = {
    tScene: { value: sceneTarget.texture },
    tBloomNear: { value: bloomTargets[0].texture },
    tBloomFar: { value: bloomTargets[2].texture },
    uTime: { value: 0 },
    uFlash: { value: 0 },
    uFlashColor: { value: new THREE.Color(1.0, 0.7, 0.5) },
    uFlashAt: { value: new THREE.Vector2(0.5, 0.5) },
    uCenter: { value: new THREE.Vector2(0.7, 0.5) },
    uHalf: { value: new THREE.Vector2(0.3, 0.3) },
    uResolution: { value: new THREE.Vector2(1, 1) },
    uScrim: { value: 0.6 },
  };
  const compositeMaterial = fullscreenMaterial(COMPOSITE_FRAGMENT, compositeUniforms);
  function pass(material, target) {
    postQuad.material = material;
    renderer.setRenderTarget(target);
    renderer.render(postScene, postCamera);
  }
  function blur(target, scratch, radius) {
    blurMaterial.uniforms.tInput.value = target.texture;
    blurMaterial.uniforms.uDirection.value.set(radius / target.width, 0);
    pass(blurMaterial, scratch);
    blurMaterial.uniforms.tInput.value = scratch.texture;
    blurMaterial.uniforms.uDirection.value.set(0, radius / target.height);
    pass(blurMaterial, target);
  }

  // Layout.
  const quality = { level: 1, slow: 0 };
  let width = 1;
  let height = 1;
  let scale = 1;
  let place = { x: 0, y: 0, size: 0, above: false };
  const anchor = new THREE.Vector3();
  const raycaster = new THREE.Raycaster();
  const plane = new THREE.Plane(new THREE.Vector3(0, 0, 1), 0);
  const ndc = new THREE.Vector2();
  const tmp = new THREE.Vector3();
  const YAW = -0.22;
  const PITCH = 0.2;
  camera.position.set(0, Math.sin(PITCH) * 10, Math.cos(PITCH) * 10);
  camera.lookAt(0, 0, 0);

  // The machine's picture on screen: the centre and size, in pixels, of its box's projection.
  const corners = [];
  for (const x of [BOX.x0, BOX.x1]) {
    for (const y of [BOX.y0, BOX.y1]) {
      for (const z of [-0.62, 0.56]) corners.push(new THREE.Vector3(x, y, z).sub(CENTER));
    }
  }
  function projectedBounds() {
    let minX = Infinity;
    let maxX = -Infinity;
    let minY = Infinity;
    let maxY = -Infinity;
    for (const corner of corners) {
      tmp.copy(corner);
      rig.localToWorld(tmp).project(camera);
      const x = (tmp.x + 1) / 2 * width;
      const y = (1 - tmp.y) / 2 * height;
      minX = Math.min(minX, x);
      maxX = Math.max(maxX, x);
      minY = Math.min(minY, y);
      maxY = Math.max(maxY, y);
    }
    return { x: (minX + maxX) / 2, y: (minY + maxY) / 2, width: maxX - minX, height: maxY - minY };
  }

  function placeStill() {
    const spot = placement(root);
    const stillWidth = spot.size * ASPECT;
    Object.assign(still.style, {
      left: `${spot.x - stillWidth / 2}px`,
      top: `${spot.y - spot.size / 2}px`,
      width: `${stillWidth}px`,
      height: `${spot.size}px`,
    });
    root.classList.add('is-placed');
  }

  function layout() {
    const rect = root.getBoundingClientRect();
    width = Math.max(1, Math.round(rect.width));
    height = Math.max(1, Math.round(rect.height));
    const dpr = Math.min(window.devicePixelRatio || 1, small ? 1.5 : 1.75) * quality.level;
    renderer.setPixelRatio(dpr);
    renderer.setSize(width, height, false);
    const w = Math.max(1, Math.floor(width * dpr));
    const h = Math.max(1, Math.floor(height * dpr));
    sceneTarget.setSize(w, h);
    bloomTargets[0].setSize(Math.max(1, w >> 2), Math.max(1, h >> 2));
    bloomTargets[1].setSize(Math.max(1, w >> 2), Math.max(1, h >> 2));
    bloomTargets[2].setSize(Math.max(1, w >> 3), Math.max(1, h >> 3));
    bloomTargets[3].setSize(Math.max(1, w >> 3), Math.max(1, h >> 3));
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    camera.updateMatrixWorld();
    skyUniforms.uResolution.value.set(width, height);
    compositeUniforms.uResolution.value.set(width, height);

    place = placement(root);
    placeStill();
    ndc.set(place.x / width * 2 - 1, -(place.y / height * 2 - 1));
    raycaster.setFromCamera(ndc, camera);
    raycaster.ray.intersectPlane(plane, anchor);
    const unitsPerPixel = 2 * camera.position.distanceTo(anchor) * Math.tan(THREE.MathUtils.degToRad(camera.fov / 2)) / height;
    scale = Math.max(0.02, place.size * unitsPerPixel / (BOX.y1 - BOX.y0));
    // Perspective and the three-quarter turn make the machine's picture larger than its box and off
    // centre: measure the picture and fit that, a few times over.
    rig.rotation.set(0, YAW, 0);
    for (let step = 0; step < 4; step++) {
      rig.position.copy(anchor);
      rig.scale.setScalar(scale);
      rig.updateMatrixWorld(true);
      const bounds = projectedBounds();
      const fit = Math.min(place.size * ASPECT / Math.max(1, bounds.width), place.size / Math.max(1, bounds.height));
      scale *= fit;
      rig.scale.setScalar(scale);
      rig.updateMatrixWorld(true);
      const refit = projectedBounds();
      tmp.copy(anchor).project(camera);
      ndc.set(tmp.x + (place.x - refit.x) / width * 2, tmp.y - (place.y - refit.y) / height * 2);
      raycaster.setFromCamera(ndc, camera);
      raycaster.ray.intersectPlane(plane, anchor);
    }
    rig.position.copy(anchor);
    rig.scale.setScalar(scale);
    const dpr2 = renderer.getPixelRatio();
    puffUniforms.uPixel.value = height * dpr2 / (2 * Math.tan(THREE.MathUtils.degToRad(camera.fov / 2))) * scale;
    skyUniforms.uCenter.value.set(place.x / width, 1 - place.y / height);
    skyUniforms.uRadius.value = place.size / height * 0.9;
    compositeUniforms.uCenter.value.set(place.x / width, 1 - place.y / height);
    compositeUniforms.uHalf.value.set(place.size * ASPECT / width * 0.5, place.size / height * 0.5);
    compositeUniforms.uScrim.value = place.above ? 0.25 : 0.55;
  }

  // Input: the pointer lamp and tilt, the gauge's tremble, and the press.
  const pointer = new THREE.Vector2(0, 0);
  let pointerActive = false;
  let lastPointer = 0;
  let presence = 0;
  let overChest = false;
  const lampTarget = new THREE.Vector3();
  const lampPosition = new THREE.Vector3();
  const tilt = new THREE.Vector2();
  const hold = { state: 'idle', downAt: 0, x: 0, y: 0, pointerId: -1, charge: 0, releasedAt: -99, doneAt: -99, releaseCharge: 0 };
  const chestBox = new THREE.Box3();
  const interactive = 'a, button, input, select, textarea, summary, label, [role="button"], [contenteditable], .dw-command';

  function setNdc(event) {
    const rect = root.getBoundingClientRect();
    pointer.set((event.clientX - rect.left) / rect.width * 2 - 1, -((event.clientY - rect.top) / rect.height * 2 - 1));
  }
  const lidBox = new THREE.Box3();
  function hitsChest() {
    if (!squash.visible) return false;
    raycaster.setFromCamera(pointer, camera);
    chestBox.setFromObject(body);
    chestBox.union(lidBox.setFromObject(lid));
    return raycaster.ray.intersectsBox(chestBox);
  }
  function onMove(event) {
    setNdc(event);
    pointerActive = event.pointerType === 'mouse';
    lastPointer = performance.now();
    if (hold.state !== 'idle' && event.pointerId === hold.pointerId && Math.hypot(event.clientX - hold.x, event.clientY - hold.y) > PRESS.move) release();
  }
  function onDown(event) {
    if (!event.isPrimary || (event.pointerType === 'mouse' && event.button !== 0)) return;
    if (event.target instanceof Element && event.target.closest(interactive)) return;
    if (hold.state !== 'idle' || time < pressReadyAt) return;
    setNdc(event);
    if (!hitsChest()) return;
    hold.state = 'armed';
    hold.downAt = performance.now();
    hold.x = event.clientX;
    hold.y = event.clientY;
    hold.pointerId = event.pointerId;
    root.dataset.press = 'armed';
    requestFrame();
  }
  function onUp(event) {
    if (event && event.pointerId !== undefined && event.pointerId !== hold.pointerId) return;
    release();
  }
  function release() {
    if (hold.state === 'charging') {
      hold.state = 'releasing';
      hold.releasedAt = time;
      hold.releaseCharge = hold.charge;
      terminal.print('^C', 'dim', time);
      root.dataset.press = 'released';
    } else if (hold.state === 'armed') {
      hold.state = 'idle';
      root.dataset.press = 'idle';
    }
    hold.pointerId = -1;
  }
  const preventWhileHeld = (event) => {
    if (hold.state === 'armed' || hold.state === 'charging') event.preventDefault();
  };
  if (!reduceMotion) {
    hero.addEventListener('pointermove', onMove, { passive: true });
    hero.addEventListener('pointerdown', onDown, { passive: true });
    hero.addEventListener('pointerleave', () => { pointerActive = false; }, { passive: true });
    window.addEventListener('pointerup', onUp, { passive: true });
    window.addEventListener('pointercancel', onUp, { passive: true });
    window.addEventListener('blur', () => release());
    hero.addEventListener('contextmenu', preventWhileHeld);
    document.addEventListener('selectstart', preventWhileHeld);
  }
  root.dataset.press = 'idle';

  // The machine's clock and state.
  const clock = new THREE.Clock();
  let time = 0;
  let serial = 0;
  const cardsPerRun = small ? 6 : 7;
  let run = null;
  let previous = null;
  let pressReadyAt = 0;
  let gag = null;

  function startRun(at) {
    previous = run;
    const kind = RUNS[serial % RUNS.length];
    run = planRun(kind, at, serial, cardsPerRun);
    serial++;
    terminal.clear();
    const flags = kind === 'dry' ? '--dry-run' : kind === 'skip' ? '--skip-existing' : '';
    terminal.print('$ dream_archivetool \\', 'prompt', at, 40);
    terminal.print('  extract-all \\', 'text', at + 0.55, 40);
    terminal.print(`  Morrowind.bsa -o out${flags ? ' \\' : ''}`, 'text', at + 0.95, 40);
    if (flags) terminal.print(`  ${flags}`, 'text', at + 1.5, 40);
    if (kind === 'dry') terminal.print('{"entries": [', 'dim', run.cards[0].scanAt - 0.3);
    for (const card of run.cards) {
      if (run.abortAt <= card.scanAt) continue;
      if (card.hostile) continue;
      const name = card.entry.path.split('/').pop();
      if (kind === 'dry') terminal.print(`  "\u2026/${name}",`, 'ok', card.scanAt);
      else if (card.skip) terminal.print(`~ ${name}`, 'amber', card.scanAt);
      else terminal.print(`+ ${name}`, 'ok', card.scanAt);
    }
    if (kind === 'unsafe') {
      const hostile = run.cards.find((card) => card.hostile);
      terminal.print('ERROR: unsafe archive path:', 'bad', hostile.scanAt + 0.15);
      terminal.print(hostile.entry.path, 'bad', hostile.scanAt + 0.2);
    } else if (kind === 'dry') {
      terminal.print(']}', 'dim', run.flowUntil);
    } else {
      const written = run.cards.filter((card) => !card.skip).length;
      terminal.print(`extracted: ${written}`, 'text', run.flowUntil + 0.2);
      if (kind === 'skip') terminal.print(`skipped: ${run.cards.length - written}`, 'amber', run.flowUntil + 0.25);
    }
  }

  // The still frame under reduced motion: mid-run, cards on the belt and in the trays.
  if (reduceMotion) {
    serial = 0;
    startRun(-6.6);
    previous = null;
  } else {
    startRun(0.4);
  }

  const pose = { position: new THREE.Vector3(), from: new THREE.Vector3(), to: new THREE.Vector3() };
  const dummy = new THREE.Object3D();
  const hidden = new THREE.Matrix4().makeScale(0, 0, 0);

  function writeCard(set, index, poseValue, cells, look) {
    if (!poseValue.visible) {
      set.mesh.setMatrixAt(index, hidden);
      return;
    }
    dummy.position.copy(poseValue.position);
    dummy.rotation.set(poseValue.rx, poseValue.ry, poseValue.rz);
    dummy.scale.setScalar(poseValue.scale);
    dummy.updateMatrix();
    set.mesh.setMatrixAt(index, dummy.matrix);
    set.cells.setXYZW(index, cells[0], cells[1], 0, 0);
    set.look.setXYZW(index, look[0], look[1], look[2], look[3]);
  }

  const confetti = Array.from({ length: CONFETTI }, (_, i) => ({ entry: ENTRIES[i % ENTRIES.length], position: new THREE.Vector3(), velocity: new THREE.Vector3(), spin: new THREE.Vector3(), rotation: new THREE.Euler(), life: 0 }));

  function frame() {
    running = false;
    if (lost) return;
    const rawDt = clock.getDelta();
    const dt = Math.min(rawDt, 0.1);
    // Slow frames step the resolution down, as far as a third, until the machine keeps up.
    if (!reduceMotion && rawDt < 2) {
      quality.slow = rawDt > 1 / 40 ? quality.slow + Math.min(rawDt, 0.5) : Math.max(0, quality.slow - rawDt * 0.5);
      if (quality.slow > 1.5 && quality.level > 0.35) {
        quality.level = Math.max(0.35, quality.level - 0.2);
        quality.slow = 0;
        layout();
      }
    }
    if (!reduceMotion) time += dt;
    const now = performance.now();

    // The press state machine.
    if (hold.state === 'armed' && (now - hold.downAt) / 1000 >= PRESS.delay) {
      hold.state = 'charging';
      hold.charge = 0;
      root.dataset.press = 'charging';
      terminal.clear();
      terminal.print('$ dream_archivetool \\', 'prompt', time, 40);
      terminal.print('  create Morrowind.bsa \\', 'text', time + 0.55, 40);
      terminal.print('  out --format tes3 \\', 'text', time + 1.2, 40);
      terminal.print('  --compress', 'text', time + 1.8, 40);
      if (run && run.abortAt > time) {
        run.abortAt = time;
        run.abortWhy = 'press';
        run.end = Infinity;
        run.flowUntil = time;
      }
    }
    if (hold.state === 'charging') {
      hold.charge = clamp01(((now - hold.downAt) / 1000 - PRESS.delay) / PRESS.charge);
      if (hold.charge >= 1) {
        hold.state = 'done';
        hold.doneAt = time;
        hold.pointerId = -1;
        root.dataset.press = 'done';
        gag = { at: time, brick: new THREE.Vector3(), flung: false };
      }
    }
    let charge = 0;
    if (hold.state === 'charging') charge = hold.charge;
    else if (hold.state === 'releasing') {
      const k = clamp01((time - hold.releasedAt) / 1.1);
      charge = hold.releaseCharge * (1 - smooth(k));
      if (k >= 1) {
        hold.state = 'idle';
        root.dataset.press = 'idle';
        pressReadyAt = time + 0.5;
        startRun(time + 0.3);
      }
    }

    // Runs follow one another; the press and its aftermath hold them off.
    if (run && time >= run.end && hold.state === 'idle' && !gag) startRun(time);
    if (previous && time > run.start + 1.2) previous = null;

    // The lamp and the tilt follow the pointer; idle, the lamp drifts round the machine.
    const idle = !pointerActive || now - lastPointer > 4000;
    if (idle) {
      tmp.set(Math.sin(time * 0.35) * 1.8, 0.3 + Math.cos(time * 0.27) * 0.5, 2.8).multiplyScalar(scale).add(anchor);
      lampTarget.copy(tmp);
    } else {
      raycaster.setFromCamera(pointer, camera);
      plane.constant = -(anchor.z + 2.6 * scale);
      if (raycaster.ray.intersectPlane(plane, tmp)) lampTarget.copy(tmp);
      plane.constant = 0;
    }
    presence += ((idle ? 0.4 : 1) - presence) * (reduceMotion ? 1 : Math.min(1, dt * 3));
    lampPosition.lerp(lampTarget, reduceMotion ? 1 : Math.min(1, dt * 6));
    lamp.position.copy(lampPosition);
    lamp.intensity = presence * 2.4 * scale * scale;
    const follow = reduceMotion ? 1 : Math.min(1, dt * 2);
    tilt.x += ((idle ? 0 : pointer.y * 0.05) - tilt.x) * follow;
    tilt.y += ((idle ? Math.sin(time * 0.13) * 0.03 : pointer.x * 0.08) - tilt.y) * follow;
    const shake = charge * charge * 0.006 * scale;
    rig.rotation.set(tilt.x * -0.6, YAW + tilt.y, 0);
    rig.position.set(anchor.x + (Math.random() - 0.5) * shake, anchor.y + (Math.random() - 0.5) * shake, anchor.z);
    rig.updateMatrixWorld();
    overChest = !idle && hold.state === 'idle' && hitsChest();

    // The archive: lid, squash, heat, gauge, rivets.
    const runKind = run ? run.kind : 'clean';
    let lidOpen = 0;
    if (run) {
      const opening = smooth((time - run.start - 0.4) / 0.6);
      const closing = Number.isFinite(run.abortAt) ? smooth((time - run.abortAt - 0.9) / 0.3) : smooth((time - (run.cards.at(-1).beltAt + 0.2)) / 0.6);
      lidOpen = opening * (1 - closing);
    }
    let chestScale = 1;
    let chestVisible = true;
    if (gag) {
      const since = time - gag.at;
      chestVisible = since < 0.12 || since > 3.3;
      chestScale = since < 0.12 ? 1 - since / 0.12 : since > 3.3 ? elastic((since - 3.3) / 0.9) : 0;
      lidOpen = since > 3.3 ? smooth((since - 3.35) / 0.25) * (1 - smooth((since - 5.2) / 0.6)) : 0;
    }
    const pressDepth = smooth(charge / 0.25);
    const crush = smooth((charge - 0.2) / 0.8);
    squash.scale.set(chestScale * (1 + 0.2 * crush), chestScale * (1 - 0.58 * crush), chestScale * (1 + 0.16 * crush));
    squash.visible = chestVisible && chestScale > 0.001;
    lidPivot.rotation.x = -1.15 * lidOpen * (1 - pressDepth);
    const heat = crush * crush;
    lacquerMaterial.emissiveIntensity = heat * 1.4;
    brassMaterial.emissiveIntensity = heat * 0.8;
    innerGlow.material.uniforms.uPower.value = lidOpen * 0.45;
    shaftUniforms.uPower.value = lidOpen * 0.3;
    shaftUniforms.uTime.value = time;
    chest.updateMatrixWorld();
    chestLight.position.set(CHEST.x, CHEST_TOP + 0.25, 0.1).sub(CENTER);
    rig.localToWorld(chestLight.position);
    chestLight.intensity = (lidOpen * 1.2 + heat * 5) * scale * scale;
    // The needle: resting, trembling under the pointer, climbing under the press.
    const needleAngle = THREE.MathUtils.degToRad(120) - THREE.MathUtils.degToRad(240) * (charge + (overChest ? 0.015 + Math.sin(time * 37) * 0.012 + Math.sin(time * 23) * 0.008 : 0));
    needlePivot.rotation.z = needleAngle + (charge > 0.9 ? Math.sin(time * 50) * 0.05 : 0);
    // The press itself.
    press.visible = charge > 0.001 || Boolean(gag && time - gag.at < 1.6);
    const lidTopY = CHEST_BOTTOM + CHEST.h * (1 - 0.58 * crush) + CHEST.lid + 0.05;
    let pressY = lerp(CHEST_TOP + 2.4, lidTopY, pressDepth);
    if (gag) pressY = lerp(CHEST_BOTTOM + 0.18, CHEST_TOP + 2.4, smooth((time - gag.at - 0.3) / 1.2));
    press.position.set(CHEST.x, pressY, 0);
    // Steam from the seams while it builds.
    if (!reduceMotion && crush > 0.05 && Math.random() < crush * dt * 40) {
      const side = Math.random() < 0.5 ? -1 : 1;
      tmp.set(CHEST.x + side * (CHEST.w / 2 + 0.02) * (1 + 0.2 * crush), CHEST_BOTTOM + CHEST.h * (1 - 0.58 * crush) * (0.3 + Math.random() * 0.6), (Math.random() - 0.5) * CHEST.d);
      emit(0, tmp, new THREE.Vector3(side * (0.25 + Math.random() * 0.3), 0.25 + Math.random() * 0.3, (Math.random() - 0.5) * 0.2), 1.1 + Math.random() * 0.6, 0.1 + Math.random() * 0.08);
    }
    // Rivets pop from 80% pressure and come back with the archive.
    rivetState.forEach((rivet, i) => {
      const place = rivetPlaces[i];
      const threshold = 0.8 + (i / rivetPlaces.length) * 0.17;
      if (!rivet.popped && charge >= threshold) {
        rivet.popped = true;
        rivet.at = time;
        tmp.copy(place);
        tmp.x *= 1 + 0.2 * crush;
        tmp.y *= 1 - 0.58 * crush;
        tmp.z *= 1 + 0.16 * crush;
        rivet.position.set(CHEST.x + tmp.x, CHEST_BOTTOM + tmp.y, tmp.z);
        rivet.velocity.set((Math.random() - 0.5) * 1.4, 1.2 + Math.random(), Math.sign(place.z) * (0.8 + Math.random() * 0.8));
        for (let s = 0; s < 3; s++) emit(1, rivet.position, new THREE.Vector3((Math.random() - 0.5) * 1.2, Math.random() * 1.2, (Math.random() - 0.5) * 1.2), 0.35, 0.03);
      }
      if (rivet.popped && ((hold.state === 'idle' && !gag) || (gag && time - gag.at >= 3.3))) rivet.popped = false;
      if (rivet.popped) {
        rivet.velocity.y -= 4.5 * dt;
        rivet.position.addScaledVector(rivet.velocity, dt);
        dummy.position.copy(rivet.position);
      } else {
        dummy.position.set(CHEST.x + place.x * squash.scale.x, CHEST_BOTTOM + place.y * squash.scale.y, place.z * squash.scale.z);
      }
      dummy.scale.setScalar(squash.visible || rivet.popped ? 1 : 0);
      dummy.rotation.set(0, 0, 0);
      dummy.updateMatrix();
      rivets.setMatrixAt(i, dummy.matrix);
    });
    rivets.instanceMatrix.needsUpdate = true;

    // The gag: the brick, the gate's refusal, the archive's return.
    let scanTint = ok;
    let scanPower = 0.35;
    if (gag) {
      const since = time - gag.at;
      brick.visible = since >= 0.05 && since < 4.6;
      compositeUniforms.uFlash.value = since < 0.8 ? Math.exp(-since * 6) * 0.9 : 0;
      tmp.set(CHEST.x, CHEST_BOTTOM + 0.15, 0).sub(CENTER);
      rig.localToWorld(tmp).project(camera);
      compositeUniforms.uFlashAt.value.set((tmp.x + 1) / 2, (tmp.y + 1) / 2);
      if (since < 0.75) {
        const k = clamp01((since - 0.05) / 0.7);
        brick.position.set(lerp(CHEST.x, BELT_START, smooth(k)), lerp(CHEST_BOTTOM + 0.1, BELT.y + 0.06, k * k) + 0.5 * Math.sin(Math.PI * k), 0);
        brick.rotation.set(0, 0, -k * Math.PI * 2);
      } else if (since < 2.9) {
        const x = Math.min(GATE.x, BELT_START + SPEED * 1.4 * (since - 0.75));
        brick.position.set(x, BELT.y + 0.06, 0);
        brick.rotation.set(0, 0, 0);
      } else {
        if (!gag.flung) {
          gag.flung = true;
          terminal.print('ERROR: archive error:', 'bad', time);
          terminal.print('compress is not valid', 'bad', time + 0.05);
          terminal.print('with TES3 BSA archives', 'bad', time + 0.1);
          gag.from = brick.position.clone();
          for (let s = 0; s < 18; s++) emit(1, brick.position, new THREE.Vector3((Math.random() - 0.5) * 2, Math.random() * 2, (Math.random() - 0.2) * 2), 0.6, 0.035);
        }
        const k = since - 2.9;
        brick.position.set(gag.from.x - 0.9 * k, gag.from.y + 2.0 * k - 3.2 * k * k, gag.from.z + 1.6 * k);
        brick.rotation.set(k * 7, k * 5, k * 9);
        brick.scale.setScalar(Math.max(0.001, 1 - smooth((since - 3.6) / 1.0)));
      }
      brickMaterial.emissiveIntensity = 3 * Math.exp(-since * 0.25);
      if (since >= 2.75 && since < 3.8) {
        scanTint = bad;
        scanPower = 1.2;
      }
      // Papers everywhere as the archive springs back.
      if (since >= 3.35 && !gag.burst) {
        gag.burst = true;
        confetti.forEach((paper, i) => {
          paper.position.set(CHEST.x + (Math.random() - 0.5) * 0.3, CHEST_TOP + 0.05, (Math.random() - 0.5) * 0.2);
          paper.velocity.set((Math.random() - 0.5) * 2.2, 2.2 + Math.random() * 1.6, (Math.random() - 0.2) * 1.8);
          paper.spin.set((Math.random() - 0.5) * 12, (Math.random() - 0.5) * 12, (Math.random() - 0.5) * 12);
          paper.rotation.set(0, 0, 0);
          paper.life = 1;
          paper.entry = ENTRIES[(i * 7) % ENTRIES.length];
        });
      }
      if (since > 6.2) {
        gag = null;
        hold.state = 'idle';
        root.dataset.press = 'idle';
        pressReadyAt = time + 1;
        startRun(time + 0.2);
      }
    }

    // The cards. Slots: the current run's cards take 0..7 in both meshes, drawn in one and hidden in
    // the other; the previous run's, clearing out of the trays, take 8..15 of the solids; a skip
    // run's existing files take 8..10 of the ghosts; confetti takes the rest of the solids.
    let scanning = 0;
    let verdictNow = 0;
    const cellsTemp = [0, 0];
    const lookTemp = [0, 0, 0, 1];
    const lookOf = (card) => {
      cellsTemp[0] = card.entry.rawCell;
      cellsTemp[1] = card.entry.pathCell;
      lookTemp[0] = pose.wipe;
      lookTemp[1] = pose.verdict;
      lookTemp[2] = pose.glow;
      lookTemp[3] = pose.alpha;
    };
    for (let i = 0; i < CURRENT; i++) {
      const card = run && run.cards[i];
      if (!card) {
        solids.mesh.setMatrixAt(i, hidden);
        ghosts.mesh.setMatrixAt(i, hidden);
        continue;
      }
      cardPose(run, card, time, pose);
      lookOf(card);
      writeCard(pose.ghost ? ghosts : solids, i, pose, cellsTemp, lookTemp);
      (pose.ghost ? solids : ghosts).mesh.setMatrixAt(i, hidden);
      if (pose.visible && Math.abs(pose.position.x - GATE.x) < 0.1 && pose.position.y < CARD_Y + 0.02 && time < card.writeAt) {
        scanning = Math.max(scanning, 1 - Math.abs(pose.position.x - GATE.x) / 0.1);
        if (time >= card.scanAt) verdictNow = card.hostile ? 1 : card.skip ? 2 : 0;
      }
    }
    for (let i = 0; i < CURRENT; i++) {
      const card = previous && previous.cards[i];
      if (!card) {
        solids.mesh.setMatrixAt(CURRENT + i, hidden);
        continue;
      }
      cardPose(previous, card, time, pose);
      pose.scale *= 1 - smooth((time - run.start) / 0.9);
      if (pose.ghost || pose.scale < 0.01) pose.visible = false;
      lookOf(card);
      writeCard(solids, CURRENT + i, pose, cellsTemp, lookTemp);
    }
    let existing = CURRENT;
    if (run && run.kind === 'skip') {
      for (const card of run.cards) {
        if (!card.skip) continue;
        const appear = smooth((time - run.start) / 0.6) * (1 - smooth((time - run.end + 0.6) / 0.6));
        traySlot(card.entry.tray, card.slot, pose.position);
        pose.visible = appear > 0.01;
        pose.rx = 0;
        pose.ry = 0;
        pose.rz = 0;
        pose.scale = 1;
        cellsTemp[0] = card.entry.pathCell;
        cellsTemp[1] = card.entry.pathCell;
        lookTemp[0] = 1;
        lookTemp[1] = 2;
        lookTemp[2] = time >= card.scanAt ? Math.exp(-(time - card.scanAt) * 2) * 1.5 : 0;
        lookTemp[3] = appear * 0.8;
        writeCard(ghosts, existing++, pose, cellsTemp, lookTemp);
      }
    }
    for (; existing < ghostCount; existing++) ghosts.mesh.setMatrixAt(existing, hidden);
    // Confetti.
    confetti.forEach((paper, i) => {
      const index = CURRENT * 2 + i;
      if (paper.life <= 0) {
        solids.mesh.setMatrixAt(index, hidden);
        return;
      }
      paper.velocity.y -= 3.2 * dt;
      paper.velocity.multiplyScalar(Math.exp(-dt * 0.9));
      paper.position.addScaledVector(paper.velocity, dt);
      if (paper.position.y < FLOOR_Y + 0.02) {
        paper.position.y = FLOOR_Y + 0.02;
        paper.velocity.set(0, 0, 0);
        paper.spin.multiplyScalar(0.5);
      }
      paper.rotation.x += paper.spin.x * dt;
      paper.rotation.y += paper.spin.y * dt;
      paper.rotation.z += paper.spin.z * dt;
      paper.life -= dt / 3.2;
      pose.visible = true;
      pose.position.copy(paper.position);
      pose.rx = paper.rotation.x;
      pose.ry = paper.rotation.y;
      pose.rz = paper.rotation.z;
      pose.scale = Math.min(1, paper.life * 4);
      cellsTemp[0] = paper.entry.rawCell;
      cellsTemp[1] = paper.entry.pathCell;
      lookTemp[0] = 0;
      lookTemp[1] = 0;
      lookTemp[2] = 0;
      lookTemp[3] = 1;
      writeCard(solids, index, pose, cellsTemp, lookTemp);
    });
    for (const set of [solids, ghosts]) {
      set.mesh.instanceMatrix.needsUpdate = true;
      set.cells.needsUpdate = true;
      set.look.needsUpdate = true;
    }

    // The scanner: its colour follows the verdict of what is under it.
    if (!gag) {
      const refusing = run && Number.isFinite(run.abortAt) && run.abortWhy === 'unsafe' && time >= run.abortAt - 0.4 && time < run.abortAt + 1.0;
      scanTint = refusing ? bad : verdictNow === 2 ? amber : ok;
      scanPower = 0.3 + scanning * 0.6 + (refusing ? 0.7 : 0);
    }
    beamUniforms.uColor.value.copy(scanTint);
    beamUniforms.uPower.value = scanPower;
    beamUniforms.uTime.value = time;
    beamUniforms.uDashed.value = runKind === 'dry' && !gag ? 1 : 0;
    beltUniforms.uScan.value.copy(scanTint);
    beltUniforms.uScanPower.value = scanPower;
    lens.material.uniforms.uColor.value.copy(scanTint);
    lens.material.uniforms.uPower.value = 0.3 + scanPower * 0.5;
    scanLight.color.copy(scanTint);
    tmp.set(GATE.x, BELT.y + 0.15, 0.1).sub(CENTER);
    rig.localToWorld(tmp);
    scanLight.position.copy(tmp);
    scanLight.intensity = scanPower * 0.8 * scale * scale;
    // The belt moves while cards ride it.
    const beltMoving = run && time > run.start + 1.5 && time < (Number.isFinite(run.abortAt) ? run.abortAt : run.flowUntil) || (gag && time - gag.at > 0.6 && time - gag.at < 2.9);
    beltUniforms.uOffset.value += (beltMoving ? SPEED : 0) * dt;
    const flowing = run && time > run.start + 1.2 && time < run.flowUntil + 0.8 && !gag;
    boardUniforms.uFlow.value += ((flowing ? 1 : 0.15) - boardUniforms.uFlow.value) * Math.min(1, dt * 2);
    boardUniforms.uTime.value = time;
    boardUniforms.uLamp.value.copy(lampPosition);
    boardUniforms.uLampPower.value = presence * 1.2;
    hashMaterial.uniforms.uOffset.value = time * 0.02;
    glassUniforms.uPower.value = 0.45 + 0.1 * Math.sin(time * 0.8);
    skyUniforms.uTime.value = time;
    compositeUniforms.uTime.value = time;
    screenUniforms.uTime.value = time;
    for (const set of [solids, ghosts]) {
      set.mesh.material.uniforms.uTime.value = time;
      set.mesh.material.uniforms.uLamp.value.copy(lampPosition);
      set.mesh.material.uniforms.uLampPower.value = presence * 2.5 * scale;
    }
    terminal.draw(time);

    // Particles.
    particles.forEach((particle, i) => {
      if (particle.life > 0) {
        particle.age += dt;
        particle.life = Math.max(0, 1 - particle.age / particle.span);
        if (particle.kind === 1) particle.velocity.y -= 5 * dt;
        else particle.velocity.multiplyScalar(Math.exp(-dt * 1.2));
        particle.position.addScaledVector(particle.velocity, dt);
      }
      puffPositions[i * 3] = particle.position.x;
      puffPositions[i * 3 + 1] = particle.position.y;
      puffPositions[i * 3 + 2] = particle.position.z;
      puffData[i * 4] = particle.life;
      puffData[i * 4 + 1] = particle.kind === 0 ? particle.size * (1.2 + (1 - particle.life) * 2.5) : particle.size;
      puffData[i * 4 + 2] = particle.kind;
    });
    puffGeometry.attributes.position.needsUpdate = true;
    puffGeometry.attributes.aPuff.needsUpdate = true;

    // Draw: the scene, the bloom, the composite.
    scene.updateMatrixWorld();
    renderer.setRenderTarget(sceneTarget);
    renderer.clear();
    renderer.render(scene, camera);
    pass(brightMaterial, bloomTargets[0]);
    blur(bloomTargets[0], bloomTargets[1], 1.0);
    blur(bloomTargets[0], bloomTargets[1], 2.0);
    copyMaterial.uniforms.tInput.value = bloomTargets[0].texture;
    pass(copyMaterial, bloomTargets[2]);
    blur(bloomTargets[2], bloomTargets[3], 1.5);
    blur(bloomTargets[2], bloomTargets[3], 3.0);
    pass(compositeMaterial, null);

    if (first) {
      first = false;
      root.classList.add('is-live');
    }
    if (visible && !reduceMotion && !document.hidden) requestFrame();
  }

  let visible = false;
  let running = false;
  let first = true;
  let lost = false;

  function requestFrame() {
    if (running || lost) return;
    running = true;
    requestAnimationFrame(frame);
  }

  canvas.addEventListener('webglcontextlost', (event) => {
    event.preventDefault();
    lost = true;
    root.classList.remove('is-live');
  });
  canvas.addEventListener('webglcontextrestored', () => {
    canvas.remove();
    still.remove();
    root.classList.remove('is-live', 'is-placed');
    mount(root);
  });

  layout();
  // Compile every program and upload the large textures off the main thread's critical path, so the
  // first frame draws rather than stalls.
  const warm = new THREE.Scene();
  for (const material of [brightMaterial, blurMaterial, copyMaterial, compositeMaterial]) {
    const mesh = new THREE.Mesh(quad, material);
    mesh.frustumCulled = false;
    warm.add(mesh);
  }
  for (const texture of [atlasTexture, boardUniforms.tBoard.value, lacquer.map, lacquer.roughness, terminal.texture]) renderer.initTexture(texture);
  try {
    await Promise.all([renderer.compileAsync(scene, camera), renderer.compileAsync(warm, postCamera)]);
  } catch { /* compiled on first use instead */ }
  new ResizeObserver(() => {
    layout();
    requestFrame();
  }).observe(root);
  if (document.fonts) {
    document.fonts.ready.then(() => {
      layout();
      requestFrame();
    });
  }
  new IntersectionObserver((entries) => {
    visible = entries.some((entry) => entry.isIntersecting);
    if (visible) {
      clock.getDelta();
      requestFrame();
    }
  }).observe(root);
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) release();
    else if (visible) {
      clock.getDelta();
      requestFrame();
    }
  });
}

for (const root of document.querySelectorAll('[data-dw-hero-art]')) mount(root);
