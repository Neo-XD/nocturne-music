// Self-check for playlist sorting (`sort.ts`). Same deal as `rows.check.ts` — no test runner in
// `ui/`, node 22 runs TypeScript directly:
//
//     node --experimental-strip-types ui/src/lib/sort.check.ts
//
// Prints "ok" and exits 0, or throws on the first broken invariant. What this guards is the part
// the queue depends on: the sorted list has to hold exactly the same tracks as the input (never
// dropping or duplicating one), and `default` has to hand back the original array untouched.
import type { SongItem } from './api.ts';
import { sortSongs } from './sort.ts';

function ok(cond: boolean, what: string): void {
	if (!cond) throw new Error(`FAIL: ${what}`);
}

const song = (video_id: string, title: string, artists: string, album?: string): SongItem =>
	({ video_id, title, artists, album }) as SongItem;

// Deliberately messy: mixed case, a numeric run, a missing album, two tracks that tie on artist.
const items = [
	song('a', 'Zebra', 'beta', 'Second'),
	song('b', 'track 10', 'Alpha', 'First'),
	song('c', 'track 9', 'Alpha'),
	song('d', 'apple', 'Gamma', 'First')
];
const ids = (list: SongItem[]) => list.map((t) => t.video_id).join('');

// --- default is a no-op, identity included ---------------------------------------------------
ok(sortSongs(items, 'default', false) === items, 'default returns the very same array');
ok(sortSongs(items.slice(0, 1), 'title', false).length === 1, 'a one-track list survives');

// --- nothing is lost or duplicated ------------------------------------------------------------
// The queue is built from this list, so a comparator that drops a track silently shortens it.
for (const key of ['newest', 'oldest', 'title', 'artist', 'album'] as const) {
	const out = sortSongs(items, key, false);
	ok(out.length === items.length, `${key} keeps every track`);
	ok(
		[...out].map((t) => t.video_id).sort().join('') === 'abcd',
		`${key} is a permutation, no dupes`
	);
}
// Sorting in place would reorder `pl.items` itself, and the page relies on that staying YouTube's
// order (it is what Default and the setVideoId backfill both read).
ok(ids(items) === 'abcd', 'the input array is still in playlist order afterwards');

// --- add order --------------------------------------------------------------------------------
// A normal playlist arrives oldest-added first, so newest-first is the reverse and oldest-first is
// the order as given. Liked Music arrives the other way round and both flip with it.
ok(ids(sortSongs(items, 'oldest', false)) === 'abcd', 'oldest-first on an oldest-first list');
ok(ids(sortSongs(items, 'newest', false)) === 'dcba', 'newest-first reverses it');
ok(ids(sortSongs(items, 'newest', true)) === 'abcd', 'newest-first on Liked Music');
ok(ids(sortSongs(items, 'oldest', true)) === 'dcba', 'oldest-first reverses Liked Music');

// --- title: case-insensitive, and "10" after "9" ----------------------------------------------
ok(ids(sortSongs(items, 'title', false)) === 'dcba', 'title sorts apple, track 9, track 10, Zebra');

// --- artist: case-insensitive, title orders the group -----------------------------------------
// b and c are both "Alpha", so the title tiebreak decides: "track 9" before "track 10".
ok(ids(sortSongs(items, 'artist', false)) === 'cbad', 'artist groups, title orders inside a group');

// Equal on both keys: nothing left to compare, so playlist order survives.
const tied = [song('x', 'Same', 'Alpha'), song('y', 'Same', 'Alpha')];
ok(ids(sortSongs(tied, 'artist', false)) === 'xy', 'a total tie keeps playlist order');

// --- album: groups, keeps playlist order inside a group, albumless last -----------------------
// b and d are both on "First"; b is first in the playlist, so it stays first (no title tiebreak
// here, or an album added in one go would come back alphabetised instead of in track order).
ok(ids(sortSongs(items, 'album', false)) === 'bdac', 'album groups First, Second, then no album');

console.log('ok');
