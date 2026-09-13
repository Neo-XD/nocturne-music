import { browser } from '$app/environment';

export interface MotionArtwork {
	url: string;
	title?: string;
	artist?: string;
	source: 'apple' | 'tidal' | 'nocturne';
}

const CACHE_KEY_PREFIX = 'nocturne_motion_art:';
const CACHE_TTL_MS = 24 * 60 * 60 * 1000; // 24 hours

interface CacheEntry {
	url: string | null;
	expires: number;
}

const memoryCache = new Map<string, CacheEntry>();

function getCached(key: string): string | null | undefined {
	const now = Date.now();
	const mem = memoryCache.get(key);
	if (mem) {
		if (mem.expires > now) return mem.url;
		memoryCache.delete(key);
	}
	if (browser) {
		try {
			const raw = localStorage.getItem(CACHE_KEY_PREFIX + key);
			if (raw) {
				const parsed = JSON.parse(raw) as CacheEntry;
				if (parsed.expires > now) {
					memoryCache.set(key, parsed);
					return parsed.url;
				}
				localStorage.removeItem(CACHE_KEY_PREFIX + key);
			}
		} catch {
			/* ignore storage errors */
		}
	}
	return undefined;
}

function setCached(key: string, url: string | null) {
	const entry: CacheEntry = {
		url,
		expires: Date.now() + CACHE_TTL_MS
	};
	memoryCache.set(key, entry);
	if (browser) {
		try {
			localStorage.setItem(CACHE_KEY_PREFIX + key, JSON.stringify(entry));
		} catch {
			/* ignore quota errors */
		}
	}
}

export function normalizeText(s: string): string {
	return s
		.toLowerCase()
		.normalize('NFD')
		.replace(/[\u0300-\u036f]/g, '') // remove diacritics
		.replace(/[\(\[\{].*?[\)\]\}]/g, '') // remove parenthesized info like (Deluxe) or [Remastered]
		.replace(/[^\w\s]/g, ' ') // remove punctuation
		.replace(/\s+/g, ' ')
		.trim();
}

// -------------------------------------------------------------------------------------------------
// Nocturne Canvas Provider (workers.dev manifest)
// -------------------------------------------------------------------------------------------------
interface NocturneItem {
	song: string;
	artist: string;
	album: string;
	url: string;
}

let manifestCache: NocturneItem[] | null = null;
let manifestFetchPromise: Promise<NocturneItem[] | null> | null = null;
let manifestExpiry = 0;

async function fetchNocturneManifest(): Promise<NocturneItem[] | null> {
	const now = Date.now();
	if (manifestCache && manifestExpiry > now) return manifestCache;
	if (manifestFetchPromise) return manifestFetchPromise;

	manifestFetchPromise = (async () => {
		try {
			const res = await fetch('https://vivimusicanvas.mkmdevilmi.workers.dev/canvas.json', {
				headers: { Accept: 'application/json' }
			});
			if (!res.ok) return null;
			const data = await res.json();
			const items: NocturneItem[] = Array.isArray(data.items) ? data.items : [];
			manifestCache = items;
			manifestExpiry = Date.now() + 5 * 60 * 1000; // 5 min cache
			return items;
		} catch {
			return null;
		} finally {
			manifestFetchPromise = null;
		}
	})();

	return manifestFetchPromise;
}

async function findInNocturneManifest(
	term: string,
	artist?: string,
	album?: string
): Promise<string | null> {
	const items = await fetchNocturneManifest();
	if (!items || !items.length) return null;

	const normTerm = normalizeText(term);
	const normArtist = artist ? normalizeText(artist) : '';
	const normAlbum = album ? normalizeText(album) : '';

	for (const it of items) {
		const itSong = normalizeText(it.song);
		const itArtist = normalizeText(it.artist);
		const itAlbum = normalizeText(it.album);

		// Match song title + artist
		if (normArtist && (itArtist.includes(normArtist) || normArtist.includes(itArtist))) {
			if (itSong === normTerm || itSong.includes(normTerm) || normTerm.includes(itSong)) {
				return it.url;
			}
			if (normAlbum && (itAlbum === normAlbum || itAlbum.includes(normAlbum))) {
				return it.url;
			}
		}

		// Match playlist/album title with album
		if (itAlbum && (itAlbum === normTerm || itAlbum.includes(normTerm) || normTerm.includes(itAlbum))) {
			if (!normArtist || itArtist.includes(normArtist) || normArtist.includes(itArtist)) {
				return it.url;
			}
		}
	}
	return null;
}

// -------------------------------------------------------------------------------------------------
// Apple Music Motion Artwork Provider (editorialVideo)
// -------------------------------------------------------------------------------------------------
const DEFAULT_APPLE_TOKEN =
	'eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NiIsImtpZCI6IldlYlBsYXlLaWQifQ.eyJpc3MiOiJBTVBXZWJQbGF5IiwiaWF0IjoxNzgxMDMyODU1LCJleHAiOjE3ODQwNTY4NTUsInJvb3RfaHR0cHNfb3JpZ2luIjpbImFwcGxlLmNvbSJdfQ.fiMFcJWkfSlxKP9NVA0UW9CbItD1Rge0SISuepz203XcpU762OqdCpU9M-YkmtKkjRmaIWtjsfGgqZPrlMonpA';

let appleToken: string | null = null;
let appleTokenExpiry = 0;

async function getAppleToken(): Promise<string> {
	const now = Date.now();
	if (appleToken && appleTokenExpiry > now) return appleToken;

	try {
		const res = await fetch('https://music.apple.com/us/browse', {
			headers: {
				'User-Agent':
					'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
			}
		});
		if (res.ok) {
			const html = await res.text();
			const scriptMatches = html.match(/\/assets\/index(?:-legacy)?[~-][a-zA-Z0-9_-]+\.js/g);
			if (scriptMatches && scriptMatches.length > 0) {
				for (const scriptPath of scriptMatches.slice(0, 3)) {
					const scriptRes = await fetch(`https://music.apple.com${scriptPath}`);
					if (scriptRes.ok) {
						const scriptText = await scriptRes.text();
						const tokens = scriptText.match(/ey[a-zA-Z0-9_-]+\.ey[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+/g);
						if (tokens) {
							for (const tok of tokens) {
								try {
									const parts = tok.split('.');
									if (parts.length === 3) {
										const decoded = atob(parts[1].replace(/-/g, '+').replace(/_/g, '/'));
										if (decoded.includes('"iss"') && decoded.includes('"exp"')) {
											const expMatch = decoded.match(/"exp":\s*(\d+)/);
											if (expMatch) {
												const expSec = parseInt(expMatch[1], 10);
												if (expSec * 1000 > now) {
													appleToken = tok;
													appleTokenExpiry = expSec * 1000 - 60000;
													return tok;
												}
											}
										}
									}
								} catch {
									/* ignore base64 errors */
								}
							}
						}
					}
				}
			}
		}
	} catch {
		/* fallback */
	}

	appleToken = DEFAULT_APPLE_TOKEN;
	appleTokenExpiry = now + 12 * 60 * 60 * 1000;
	return appleToken;
}

function extractEditorialVideoUrl(ev: any): string | null {
	if (!ev || typeof ev !== 'object') return null;
	const candidates = [
		ev.motionDetailSquare,
		ev.motionDetailRaw,
		ev.motionDetailTall,
		ev.motionDetailStatic
	];
	for (const item of candidates) {
		if (item && typeof item === 'object') {
			const url = item.video || item.videoUrl || item.hlsUrl || item.url;
			if (typeof url === 'string' && url.length > 5) return url;
		}
	}
	return null;
}

async function searchAppleMusicMotion(
	term: string,
	types: 'playlists' | 'albums' = 'playlists'
): Promise<string | null> {
	try {
		const token = await getAppleToken();
		const storefront = 'us';
		const query = encodeURIComponent(term.trim());
		const url = `https://amp-api.music.apple.com/v1/catalog/${storefront}/search?term=${query}&types=${types}&limit=5&extend=editorialVideo`;

		const res = await fetch(url, {
			headers: {
				Authorization: `Bearer ${token}`,
				Origin: 'https://music.apple.com',
				Referer: 'https://music.apple.com/'
			}
		});

		if (!res.ok) return null;
		const json = await res.json();
		const data = json?.results?.[types]?.data;
		if (!Array.isArray(data) || !data.length) return null;

		for (const item of data) {
			const ev = item?.attributes?.editorialVideo;
			if (ev) {
				const videoUrl = extractEditorialVideoUrl(ev);
				if (videoUrl) return videoUrl;
			}
		}
		return null;
	} catch {
		return null;
	}
}

// -------------------------------------------------------------------------------------------------
// Public API Methods
// -------------------------------------------------------------------------------------------------

/**
 * Searches for motion album art / video cover for a playlist.
 * Tries:
 * 1. Nocturne Canvas Manifest (by playlist name or first song)
 * 2. Apple Music curated playlists (editorialVideo)
 * 3. Apple Music album match (if playlist is an album/release)
 */
export async function getPlaylistMotionArtwork(
	title?: string,
	subtitle?: string,
	firstSong?: { title?: string; artists?: string; album?: string }
): Promise<string | null> {
	if (!title || !title.trim()) return null;
	const cleanTitle = title.trim();
	const cacheKey = `pl:${cleanTitle}:${subtitle ?? ''}`;
	const cached = getCached(cacheKey);
	if (cached !== undefined) return cached;

	// 1. Check Nocturne Manifest
	let match = await findInNocturneManifest(cleanTitle, subtitle);
	if (!match && firstSong && firstSong.title) {
		match = await findInNocturneManifest(firstSong.title, firstSong.artists, firstSong.album);
	}

	// 2. Check Apple Music Playlists (e.g. curated playlists with motion covers)
	if (!match) {
		match = await searchAppleMusicMotion(cleanTitle, 'playlists');
	}

	// 3. If no playlist motion cover found, check Apple Music Albums
	if (!match) {
		match = await searchAppleMusicMotion(cleanTitle, 'albums');
	}

	setCached(cacheKey, match);
	return match;
}

/**
 * Searches for motion album art for an album.
 */
export async function getAlbumMotionArtwork(
	title?: string,
	artist?: string
): Promise<string | null> {
	if (!title || !title.trim()) return null;
	const cleanTitle = title.trim();
	const cleanArtist = (artist ?? '').trim();
	const cacheKey = `al:${cleanTitle}:${cleanArtist}`;
	const cached = getCached(cacheKey);
	if (cached !== undefined) return cached;

	// 1. Check Nocturne Manifest
	let match = await findInNocturneManifest(cleanTitle, cleanArtist);

	// 2. Check Apple Music Albums
	if (!match) {
		const query = cleanArtist ? `${cleanArtist} ${cleanTitle}` : cleanTitle;
		match = await searchAppleMusicMotion(query, 'albums');
	}

	setCached(cacheKey, match);
	return match;
}

/**
 * Searches for motion canvas for a track.
 */
export async function getSongMotionArtwork(
	title?: string,
	artist?: string,
	album?: string
): Promise<string | null> {
	if (!title || !artist) return null;
	const cacheKey = `tr:${title}:${artist}:${album ?? ''}`;
	const cached = getCached(cacheKey);
	if (cached !== undefined) return cached;

	// 1. Nocturne Manifest
	let match = await findInNocturneManifest(title, artist, album);

	// 2. Apple Music Album
	if (!match && album) {
		match = await searchAppleMusicMotion(`${artist} ${album}`, 'albums');
	}

	setCached(cacheKey, match);
	return match;
}
