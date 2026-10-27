import { browser } from '$app/environment';

export interface MotionArtwork {
	url: string;
	title?: string;
	artist?: string;
	source: 'apple' | 'tidal' | 'nocturne';
}

// Bumped cache key to v2 to automatically invalidate any previously cached loose/false matches
const CACHE_KEY_PREFIX = 'nocturne_motion_art_v2:';
const CACHE_TTL_MS = 24 * 60 * 60 * 1000; // 24 hours

interface CacheEntry {
	url: string | null;
	expires: number;
}

const memoryCache = new Map<string, CacheEntry>();

// Purge any stale legacy v1 entries once in browser
if (browser) {
	try {
		for (let i = localStorage.length - 1; i >= 0; i--) {
			const k = localStorage.key(i);
			if (k && k.startsWith('nocturne_motion_art:')) {
				localStorage.removeItem(k);
			}
		}
	} catch {
		/* ignore storage access errors */
	}
}

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
				// If negative lookup was previously stored, or if unplayable HLS (.m3u8) was stored, purge it
				if (!parsed.url || parsed.url.includes('.m3u8')) {
					localStorage.removeItem(CACHE_KEY_PREFIX + key);
					return undefined;
				}
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
	if (!url) {
		// Only keep negative lookups in short memory cache (30s) to avoid spamming network
		memoryCache.set(key, {
			url: null,
			expires: Date.now() + 30 * 1000
		});
		return;
	}
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

/**
 * Escapes characters for safe regular expression inclusion.
 */
function escapeRegex(s: string): string {
	return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * Normalizes text for comparison by removing diacritics, bracketed content,
 * punctuation, and standardizing whitespace.
 */
export function normalizeText(s: string): string {
	if (!s) return '';
	return s
		.toLowerCase()
		.normalize('NFD')
		.replace(/[\u0300-\u036f]/g, '') // remove diacritics
		.replace(/[\(\[\{].*?[\)\]\}]/g, '') // remove (Official Video), [Remastered], etc.
		.replace(/[^\w\s]/g, ' ') // remove punctuation
		.replace(/\s+/g, ' ')
		.trim();
}

/**
 * Strips noise like '(Official Video)', '[Remastered 2021]', 'feat. ...' from track titles.
 */
export function cleanTitle(raw: string): string {
	if (!raw) return '';
	return raw
		.replace(
			/\s*[\(\[](?:official|audio|video|music video|lyric video|lyrics|remaster(?:ed)?|deluxe(?: edition)?|bonus(?: track)?|live(?: at [^\)\]]+)?|expanded|special edition|anniversary(?: edition)?|feat\.?|ft\.?).*?[\)\]]/gi,
			''
		)
		.replace(
			/\s*-\s*(?:official|audio|video|lyric|remaster|deluxe|bonus|live|expanded|anniversary|radio edit).*$/gi,
			''
		)
		.trim();
}

/**
 * Extracts primary artist name before collaboration delimiters.
 */
export function getPrimaryArtist(raw: string): string {
	if (!raw) return '';
	const first = raw.split(
		/(?:,\s*|\s+&\s+|\s+×\s+|\s+x\s+|\s+feat\.?\s+|\s+ft\.?\s+|\s+featuring\s+|\s+with\s+)/i
	)[0];
	return first.trim();
}

/**
 * Splits raw artist string into individual artist names according to standard delimiters.
 */
export function splitAndNormalizeArtists(raw: string): string[] {
	if (!raw) return [];
	const delimiters = /(?:\s*,\s*|\s*&\s*|\s+×\s+|\s+x\s+|\bfeat\.?\b|\bft\.?\b|\bfeaturing\b|\bwith\b)/i;
	return raw
		.split(delimiters)
		.map((a) => normalizeText(a))
		.filter((a) => a.length > 0);
}

/**
 * Detects common generic album names ('Single', 'Greatest Hits', 'Unknown')
 * that should not trigger album-level fallback matches.
 */
export function isGenericAlbumName(name: string): boolean {
	const n = normalizeText(name);
	if (!n || n.length < 2) return true;
	const generic = new Set([
		'single',
		'singles',
		'ep',
		'unknown',
		'unknown album',
		'greatest hits',
		'the greatest hits',
		'best of',
		'the best of',
		'various artists',
		'top hits',
		'hit collection',
		'anthology',
		'live',
		'live album',
		'remixes',
		'deluxe',
		'soundtrack',
		'album'
	]);
	return generic.has(n);
}

/**
 * Strict title matching that prevents short common words ("Stay", "Run", "You", "Mine")
 * from matching unrelated titles ("Stay With Me", "Born to Run", "Young and Beautiful", "Mineral").
 */
export function isTitleMatch(req: string, candidate: string): boolean {
	if (!req || !candidate) return false;
	const normReq = normalizeText(req);
	const normCand = normalizeText(candidate);
	if (!normReq || !normCand) return false;

	// 1. Direct exact normalized match
	if (normReq === normCand) return true;

	// 2. Exact match after cleaning titles (stripping deluxe, remaster, etc.)
	const cleanReq = normalizeText(cleanTitle(req));
	const cleanCand = normalizeText(cleanTitle(candidate));
	if (cleanReq && cleanCand && cleanReq === cleanCand) return true;

	// 3. Strip leading "the " ("the hills" vs "hills")
	const noTheReq = cleanReq.replace(/^the\s+/, '');
	const noTheCand = cleanCand.replace(/^the\s+/, '');
	if (noTheReq && noTheCand && noTheReq === noTheCand) return true;

	// 4. For short titles (< 3 words), REQUIRE exact match above.
	// Never allow loose substring matching for 1- or 2-word titles!
	const wordsReq = cleanReq.split(/\s+/).filter(Boolean);
	const wordsCand = cleanCand.split(/\s+/).filter(Boolean);
	if (wordsReq.length < 3 || wordsCand.length < 3) {
		return false;
	}

	// 5. For longer titles (>= 3 words), allow matching if one is a version/subtitle extension
	// (Candidate must start with req or vice-versa on exact word boundary)
	if (cleanCand.startsWith(cleanReq + ' ') || cleanReq.startsWith(cleanCand + ' ')) {
		return true;
	}

	// 6. High-token overlap for long titles (all words of the shorter title must be in the longer title)
	const [shorter, longer] =
		wordsReq.length <= wordsCand.length ? [wordsReq, wordsCand] : [wordsCand, wordsReq];
	const longerSet = new Set(longer);
	const allInLonger = shorter.every((w) => longerSet.has(w));
	if (allInLonger && shorter.length >= 3 && shorter.length / longer.length >= 0.75) {
		return true;
	}

	return false;
}

/**
 * Strict artist matching:
 * Prevents short artist names ("Eve", "Nas", "HER", "US") from false-matching inside
 * longer names ("Steve Lacy", "Jonas Brothers", "Father John Misty").
 */
export function isArtistMatch(reqArtist: string, candidateArtists: string[]): boolean {
	if (!reqArtist || !candidateArtists || candidateArtists.length === 0) return false;

	const reqList = splitAndNormalizeArtists(reqArtist);
	const candList = candidateArtists.flatMap((a) => splitAndNormalizeArtists(a));
	if (reqList.length === 0 || candList.length === 0) return false;

	return reqList.some((req) => {
		if (!req) return false;
		const reqNoThe = req.replace(/^the\s+/, '');

		return candList.some((cand) => {
			if (!cand) return false;
			const candNoThe = cand.replace(/^the\s+/, '');

			// Exact match (with or without 'the')
			if (req === cand || reqNoThe === candNoThe) return true;

			// If name is short (< 4 chars), REQUIRE exact match to prevent false positives
			if (reqNoThe.length < 4 || candNoThe.length < 4) return false;

			// Word boundary match: 'Post Malone' inside 'Post Malone & Swae Lee'
			const reReq = new RegExp(`(?:^|\\s)${escapeRegex(reqNoThe)}(?:$|\\s)`);
			const reCand = new RegExp(`(?:^|\\s)${escapeRegex(candNoThe)}(?:$|\\s)`);
			return reReq.test(candNoThe) || reCand.test(reqNoThe);
		});
	});
}

/**
 * Strict validation matching algorithm.
 * Ensures the returned artwork accurately matches the requested song/artist.
 */
export function validateCanvasMatch(
	returnedTitle: string,
	returnedArtists: string[],
	returnedAlbum: string | null | undefined,
	reqTitle: string,
	reqArtist: string,
	reqAlbum?: string
): boolean {
	// 1. Strict Title match
	if (!isTitleMatch(reqTitle, returnedTitle)) {
		return false;
	}

	// 2. Strict Artist match
	if (!isArtistMatch(reqArtist, returnedArtists)) {
		return false;
	}

	return true;
}

// -------------------------------------------------------------------------------------------------
// Tidal Canvas Provider (direct api.tidal.com videoCover)
// -------------------------------------------------------------------------------------------------
const TIDAL_TOKEN = 'vNVdglQOjFJJGG2U';
const TIDAL_BASE = 'https://api.tidal.com/v1/';

function formatTidalVideoUrl(videoCover: string): string | null {
	if (!videoCover) return null;
	if (videoCover.startsWith('http')) return videoCover;
	const clean = videoCover.trim().replace(/^\/+|\/+$/g, '');
	const path = clean.includes('-') ? clean.split('-').join('/') : clean;
	return `https://resources.tidal.com/videos/${path}/1280x1280.mp4`;
}

/**
 * Searches Tidal for tracks with videoCover animations.
 */
async function searchTidalTrackCanvas(
	song: string,
	artist: string,
	album?: string
): Promise<string | null> {
	const cleanS = cleanTitle(song) || song;
	const primA = getPrimaryArtist(artist) || artist;

	// Build search queries in order of precision to avoid search syntax failures
	const queries = [
		`${primA} ${cleanS}`.trim(),
		cleanS !== song ? `${primA} ${song}`.trim() : null,
		album && album.trim() && !isGenericAlbumName(album)
			? `${primA} ${cleanS} ${cleanTitle(album)}`.trim()
			: null
	].filter(Boolean) as string[];

	for (const query of queries) {
		try {
			const url = `${TIDAL_BASE}search?query=${encodeURIComponent(query)}&limit=10&types=TRACKS&countryCode=US`;
			const res = await fetch(url, {
				headers: {
					'X-Tidal-Token': TIDAL_TOKEN,
					Accept: 'application/json'
				}
			});
			if (!res.ok) continue;
			const json = await res.json();
			const items = json?.tracks?.items;
			if (!Array.isArray(items) || !items.length) continue;

			for (const item of items) {
				const videoCover = item?.videoCover || item?.album?.videoCover;
				if (!videoCover) continue;

				const title = item?.title ?? '';
				const artistList: string[] = Array.isArray(item?.artists)
					? item.artists.map((a: { name?: string }) => a?.name ?? '').filter(Boolean)
					: [item?.artist?.name ?? ''];
				const albumTitle = item?.album?.title;

				if (validateCanvasMatch(title, artistList, albumTitle, cleanS, artist, album)) {
					const videoUrl = formatTidalVideoUrl(videoCover);
					if (videoUrl) return videoUrl;
				}
			}
		} catch {
			/* continue to next query */
		}
	}
	return null;
}

/**
 * Searches Tidal for album motion covers with strict title AND artist matching.
 */
async function searchTidalAlbumCanvas(album: string, artist?: string): Promise<string | null> {
	if (!album || !album.trim() || isGenericAlbumName(album)) return null;
	const cleanA = cleanTitle(album) || album.trim();
	const primArtist = artist ? getPrimaryArtist(artist) : '';

	const queries = [
		primArtist ? `${primArtist} ${cleanA}` : cleanA,
		cleanA
	];

	for (const query of queries) {
		try {
			const url = `${TIDAL_BASE}search?query=${encodeURIComponent(query)}&limit=10&types=ALBUMS&countryCode=US`;
			const res = await fetch(url, {
				headers: {
					'X-Tidal-Token': TIDAL_TOKEN,
					Accept: 'application/json'
				}
			});
			if (!res.ok) continue;
			const json = await res.json();
			const items = json?.albums?.items;
			if (!Array.isArray(items) || !items.length) continue;

			for (const item of items) {
				const videoCover = item?.videoCover;
				if (!videoCover) continue;

				const title = item?.title ?? '';
				if (!isTitleMatch(cleanA, title)) continue;

				// CRITICAL: Validate artist if provided to avoid matching another artist's album
				if (primArtist) {
					const itemArtists: string[] = Array.isArray(item?.artists)
						? item.artists.map((a: { name?: string }) => a?.name ?? '').filter(Boolean)
						: [item?.artist?.name ?? ''];
					if (!isArtistMatch(primArtist, itemArtists)) {
						continue; // Skip if artist does not match!
					}
				}

				const videoUrl = formatTidalVideoUrl(videoCover);
				if (videoUrl) return videoUrl;
			}
		} catch {
			/* continue to next query */
		}
	}
	return null;
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
			const items: NocturneItem[] = Array.isArray(data)
				? data
				: Array.isArray(data?.items)
				? data.items
				: Array.isArray(data?.data)
				? data.data
				: [];
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
	isSong: boolean = true
): Promise<string | null> {
	const items = await fetchNocturneManifest();
	if (!items || !items.length) return null;

	const primArtist = artist ? getPrimaryArtist(artist) : '';

	for (const it of items) {
		const itArtist = it.artist ? [it.artist] : [];

		if (isSong) {
			// For a song, the song title AND artist MUST match!
			// Never match by album alone for a song query.
			if (!isTitleMatch(term, it.song)) continue;
			if (primArtist && !isArtistMatch(primArtist, itArtist)) continue;
			return it.url;
		} else {
			// For an album or playlist
			const targetTitle = it.album || it.song;
			if (!isTitleMatch(term, targetTitle)) continue;
			if (primArtist && it.artist && !isArtistMatch(primArtist, itArtist)) continue;
			return it.url;
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
			const url = item.video || item.videoUrl || item.url;
			// Native HTML5 <video> in Chromium Windows cannot decode HLS (.m3u8) without hls.js. Only accept direct MP4/video files.
			if (typeof url === 'string' && url.length > 5 && !url.includes('.m3u8')) return url;
		}
	}
	return null;
}

async function searchAppleMusicMotion(
	term: string,
	types: 'playlists' | 'albums' | 'songs' = 'playlists',
	artist?: string
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
			if (!ev) continue;

			const name = item?.attributes?.name ?? '';
			const artistName = item?.attributes?.artistName ?? '';

			// Validate title
			if (!isTitleMatch(term, name)) continue;

			// Validate artist for albums and songs if artist is provided
			if (artist && artistName) {
				if (!isArtistMatch(artist, [artistName])) continue;
			}

			const videoUrl = extractEditorialVideoUrl(ev);
			if (videoUrl) return videoUrl;
		}
		return null;
	} catch {
		return null;
	}
}

// -------------------------------------------------------------------------------------------------
// BetterLyrics Motion Artwork Provider (artwork.boidu.dev / api.betterlyrics.org)
// -------------------------------------------------------------------------------------------------
const BETTERLYRICS_ENDPOINTS = [
	'https://artwork.boidu.dev/artwork',
	'https://api.betterlyrics.org/artwork'
];

async function searchBetterLyricsMotion(
	song: string,
	artist: string,
	album?: string
): Promise<string | null> {
	const cleanS = cleanTitle(song) || song;
	const primA = getPrimaryArtist(artist) || artist;
	if (!cleanS || !primA) return null;

	for (const endpoint of BETTERLYRICS_ENDPOINTS) {
		try {
			const u = new URL(endpoint);
			u.searchParams.set('s', cleanS);
			u.searchParams.set('a', primA);
			if (album && !isGenericAlbumName(album)) {
				u.searchParams.set('al', cleanTitle(album));
			}

			const res = await fetch(u.toString(), {
				headers: { Accept: 'application/json' },
				signal: AbortSignal.timeout(4500)
			});
			if (!res.ok) continue;

			const data = await res.json();
			if (!data || typeof data !== 'object') continue;

			// Direct MP4 video stream URL from BetterLyrics / Apple Music
			const directVideo =
				(typeof data.videoUrl === 'string' && data.videoUrl.length > 5 && !data.videoUrl.includes('.m3u8')
					? data.videoUrl
					: null) ||
				(typeof data.videoUrlVertical === 'string' && data.videoUrlVertical.length > 5 && !data.videoUrlVertical.includes('.m3u8')
					? data.videoUrlVertical
					: null) ||
				(typeof data.animated === 'string' && data.animated.length > 5 && !data.animated.includes('.m3u8')
					? data.animated
					: null);

			if (!directVideo) continue;

			// Strict metadata verification
			const retTitle = data.name || data.title || '';
			const retArtist = data.artist || '';
			if (retTitle && !isTitleMatch(cleanS, retTitle)) continue;
			if (retArtist && !isArtistMatch(primA, [retArtist])) continue;

			return directVideo;
		} catch {
			/* try next endpoint */
		}
	}
	return null;
}

// -------------------------------------------------------------------------------------------------
// Public API Methods
// -------------------------------------------------------------------------------------------------

/**
 * Searches for motion album art / video cover for a playlist.
 */
export async function getPlaylistMotionArtwork(
	title?: string,
	subtitle?: string,
	firstSong?: { title?: string; artists?: string; album?: string }
): Promise<string | null> {
	if (!title || !title.trim()) return null;
	const cleanT = cleanTitle(title) || title.trim();
	const cacheKey = `pl:${cleanT}:${subtitle ?? ''}`;
	const cached = getCached(cacheKey);
	if (cached !== undefined) return cached;

	// 1. Check Nocturne Manifest (playlist title)
	let match = await findInNocturneManifest(cleanT, subtitle, false);

	// 2. Check Apple Music Playlists
	if (!match) {
		match = await searchAppleMusicMotion(cleanT, 'playlists');
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
	const cleanT = cleanTitle(title) || title.trim();
	const primArtist = artist ? getPrimaryArtist(artist) : '';
	if (isGenericAlbumName(cleanT)) return null;

	const cacheKey = `al:${cleanT}:${primArtist}`;
	const cached = getCached(cacheKey);
	if (cached !== undefined) return cached;

	// 1. Check Nocturne Manifest
	let match = await findInNocturneManifest(cleanT, primArtist, false);

	// 2. Check BetterLyrics / Boidu Artwork
	if (!match) {
		match = await searchBetterLyricsMotion(cleanT, primArtist);
	}

	// 3. Check Tidal Album VideoCover (strictly verifies title and artist)
	if (!match) {
		match = await searchTidalAlbumCanvas(cleanT, primArtist);
	}

	// 4. Check Apple Music Albums (strictly verifies title and artist)
	if (!match) {
		match = await searchAppleMusicMotion(cleanT, 'albums', primArtist);
	}

	setCached(cacheKey, match);
	return match;
}

/**
 * Searches for motion canvas / animated album art for a song track.
 * Follows strict Nocturne priority waterfall with full metadata verification:
 * 1. Nocturne Canvas Manifest (strict song + artist match)
 * 2. BetterLyrics / Boidu Animated Artwork (Apple Music motion MP4s with strict match)
 * 3. Tidal Track VideoCover (direct 1280x1280 MP4 with strict track and artist verification)
 * 4. Tidal Album search for single release (only if album title matches track AND artist matches)
 * 5. Tidal Album search for the track's album (only if non-generic and artist matches)
 * 6. Apple Music Album (only if non-generic and artist matches)
 */
export async function getSongMotionArtwork(
	title?: string,
	artist?: string,
	album?: string
): Promise<string | null> {
	if (!title || !artist) return null;
	const cleanS = cleanTitle(title) || title;
	const primA = getPrimaryArtist(artist) || artist;
	const validAlbum = album && !isGenericAlbumName(album) ? album.trim() : '';

	const cacheKey = `tr:${cleanS}:${primA}:${validAlbum}`;
	const cached = getCached(cacheKey);
	if (cached !== undefined) return cached;

	// 1. Nocturne Manifest (strict song title + artist match)
	let match = await findInNocturneManifest(title, artist, true);
	if (!match && cleanS !== title) {
		match = await findInNocturneManifest(cleanS, primA, true);
	}

	// 2. BetterLyrics / Boidu Animated Artwork (Apple Music motion MP4s with strict match)
	if (!match) {
		match = await searchBetterLyricsMotion(title, artist, validAlbum);
	}

	// 3. Tidal Track Search (direct 1280x1280 MP4 with strict track and artist verification)
	if (!match) {
		match = await searchTidalTrackCanvas(title, artist, validAlbum);
	}

	// 4. Tidal Album search for single release (only if album title matches song AND artist matches)
	if (!match) {
		match = await searchTidalAlbumCanvas(cleanS, primA);
	}

	// 5. Tidal Album search for the track's album (only if specific album is given and artist matches)
	if (!match && validAlbum) {
		match = await searchTidalAlbumCanvas(validAlbum, primA);
	}

	// 6. Apple Music Album (only if specific album is given and artist matches)
	if (!match && validAlbum) {
		match = await searchAppleMusicMotion(validAlbum, 'albums', primA);
	}

	setCached(cacheKey, match);
	return match;
}
