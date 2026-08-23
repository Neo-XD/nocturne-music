// One job: turn the playing track's cover into an accent colour the rest of the theme can use.
// Used only when "Adapt colors to artwork" is on (theme.svelte.ts).
//
// The picking is deliberately crude — 32x32, buckets, one winner. A cover has one or two colours
// that read as "its" colour, and a 3-bit-per-channel histogram finds them; anything smarter
// (k-means, palette libraries) is a dependency and a frame budget for a result nobody can tell
// apart at accent size.

import { hexToHsv, hsvToHex } from './color.ts';

const SIZE = 32;

/**
 * Winning colour of an RGBA buffer, normalized into the band an accent has to live in (saturated
 * enough to read as a colour, mid-light so black or white text can sit on it). `null` when the
 * artwork has no colour worth taking — a greyscale cover would otherwise have a hue invented for
 * it out of JPEG noise.
 */
export function pickAccent(data: Uint8ClampedArray): string | null {
	// key = 3 bits per channel. score favours saturated, mid-value pixels: the near-black and
	// near-white that dominate most covers must not win just by area.
	const buckets = new Map<number, { n: number; r: number; g: number; b: number; score: number }>();
	for (let i = 0; i < data.length; i += 4) {
		if (data[i + 3] < 128) continue; // transparent
		const [r, g, b] = [data[i], data[i + 1], data[i + 2]];
		const max = Math.max(r, g, b) / 255;
		const min = Math.min(r, g, b) / 255;
		const sat = max ? (max - min) / max : 0;
		const score = sat * (1 - Math.abs(max - 0.65));
		const key = ((r >> 5) << 6) | ((g >> 5) << 3) | (b >> 5);
		const cur = buckets.get(key) ?? { n: 0, r: 0, g: 0, b: 0, score: 0 };
		buckets.set(key, {
			n: cur.n + 1,
			r: cur.r + r,
			g: cur.g + g,
			b: cur.b + b,
			score: cur.score + score
		});
	}
	let best: { n: number; r: number; g: number; b: number; score: number } | null = null;
	for (const bucket of buckets.values()) if (!best || bucket.score > best.score) best = bucket;
	if (!best) return null;

	const hex =
		'#' +
		[best.r, best.g, best.b]
			.map((c) => Math.round(c / best!.n).toString(16).padStart(2, '0'))
			.join('');
	const hsv = hexToHsv(hex);
	if (!hsv || hsv.s < 0.15) return null; // greyscale cover: leave the user's theme alone
	return hsvToHex({ h: hsv.h, s: Math.min(0.85, Math.max(0.5, hsv.s)), v: Math.min(0.9, Math.max(0.62, hsv.v)) });
}

/**
 * Accent for a cover URL, or `null` if it can't be read. The image is fetched with CORS so the
 * canvas stays untainted (googleusercontent and ytimg both send `access-control-allow-origin: *`);
 * a host that doesn't throws on `getImageData` and lands in the same `null`.
 */
export async function artworkAccent(url: string): Promise<string | null> {
	try {
		const img = new Image();
		img.crossOrigin = 'anonymous';
		img.src = url;
		await img.decode();
		const canvas = document.createElement('canvas');
		canvas.width = canvas.height = SIZE;
		const ctx = canvas.getContext('2d', { willReadFrequently: true });
		if (!ctx) return null;
		ctx.drawImage(img, 0, 0, SIZE, SIZE);
		return pickAccent(ctx.getImageData(0, 0, SIZE, SIZE).data);
	} catch {
		return null; // offline, 404, throttled, tainted — the current accent just stays
	}
}
