import { convertFileSrc } from '@tauri-apps/api/core';

// Rewrite image URLs to high-resolution by default.
// Google CDN (googleusercontent / yt3) supports arbitrary sizes up to 1080p/1200p dynamically.
// By default, upgrade sizes to 544px which provides crisp retina fidelity while loading dramatically faster.
export function thumb(
	url: string | undefined | null,
	px: number = 544,
	exact: boolean = false
): string | undefined {
	if (!url) return undefined;
	// Local library artwork is a path on this machine, not a URL. Hand it through Tauri's asset protocol.
	if (url.startsWith('/') || /^[A-Za-z]:[\\/]/.test(url)) return convertFileSrc(url);

	const targetPx = px;
	if (/=w\d+-h\d+/.test(url)) return url.replace(/=w\d+-h\d+/, `=w${targetPx}-h${targetPx}`);
	if (/=s\d+/.test(url)) return url.replace(/=s\d+/, `=s${targetPx}`);
	return url;
}
