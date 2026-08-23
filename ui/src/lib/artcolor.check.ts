// Self-check for the artwork accent picker (`artcolor.ts`). No test runner in `ui/` (see
// color.check.ts) — node 22 runs TypeScript directly:
//
//     node --experimental-strip-types ui/src/lib/artcolor.check.ts
//
// Prints "ok" and exits 0, or throws on the first broken invariant.
import { pickAccent } from './artcolor.ts';
import { hexToHsv, lerpHue } from './color.ts';

/** An RGBA buffer: `n` pixels of each colour, in order. */
const buf = (...runs: [[number, number, number], number][]) =>
	new Uint8ClampedArray(runs.flatMap(([[r, g, b], n]) => Array.from({ length: n * 4 }, (_, i) => [r, g, b, 255][i % 4])));

const hue = (hex: string) => Math.round(hexToHsv(hex)!.h);

// A cover that is mostly black with a red band: area must not beat colour, or every dark cover
// themes the app grey.
const red = pickAccent(buf([[8, 8, 8], 900], [[200, 30, 30], 124]))!;
if (Math.abs(hue(red) - 0) > 20 && Math.abs(hue(red) - 360) > 20) throw new Error(`hue: ${red}`);

// Whatever comes back is usable as an accent: saturated and mid-light in both themes.
const hsv = hexToHsv(red)!;
if (hsv.s < 0.5 || hsv.v < 0.62 || hsv.v > 0.9) throw new Error(`out of band: ${JSON.stringify(hsv)}`);

// Greyscale artwork keeps the user's theme instead of inventing a hue out of noise.
if (pickAccent(buf([[20, 20, 20], 512], [[210, 210, 212], 512])) !== null) throw new Error('grey');
if (pickAccent(new Uint8ClampedArray(0)) !== null) throw new Error('empty');

// The crossfade takes the short way round the wheel: 350 -> 10 must pass through 0, not 180. Get
// this wrong and every track change sweeps the whole UI through an unrelated colour.
if (Math.round(lerpHue(350, 10, 0.5)) !== 0) throw new Error(`wrap: ${lerpHue(350, 10, 0.5)}`);
if (Math.round(lerpHue(10, 350, 0.5)) !== 0) throw new Error(`wrap back: ${lerpHue(10, 350, 0.5)}`);
if (Math.round(lerpHue(20, 200, 0.5)) % 180 !== 110 % 180) throw new Error('half turn');
if (lerpHue(120, 240, 0) !== 120 || lerpHue(120, 240, 1) !== 240) throw new Error('hue ends');

console.log('ok');
