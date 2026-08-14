// Playlist sorting. Pure so it can be checked without a DOM (`sort.check.ts`).
//
// The playlist page keeps `pl.items` in YouTube's order and treats the sort as a view over it, so
// every optimistic mutation (add, remove, setVideoId backfill, loadMore) keeps working on the real
// list and switching back to Default costs nothing.
import type { SongItem } from './api';

export type SortKey = 'default' | 'newest' | 'oldest' | 'title' | 'artist' | 'album';

export const SORTS: { key: SortKey; label: string }[] = [
	{ key: 'default', label: 'Default' },
	{ key: 'newest', label: 'Newest first' },
	{ key: 'oldest', label: 'Oldest first' },
	{ key: 'title', label: 'Title' },
	{ key: 'artist', label: 'Artist' },
	{ key: 'album', label: 'Album' }
];

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

/**
 * Sort a playlist's tracks. Returns the input array untouched for `default` (and for anything too
 * short to reorder), a new array otherwise — the page compares identity to skip re-rendering.
 *
 * `baseNewestFirst` says which end of the incoming order is the newest addition. Playlist rows
 * carry no timestamp, so newest/oldest is add order, i.e. the position YouTube hands them back in:
 * Liked Music arrives most-recently-liked first, every other playlist arrives oldest-added first.
 */
export function sortSongs(
	items: SongItem[],
	sort: SortKey,
	baseNewestFirst: boolean
): SongItem[] {
	if (sort === 'default' || items.length < 2) return items;
	if (sort === 'newest' || sort === 'oldest')
		return (sort === 'newest') === baseNewestFirst ? items : items.slice().reverse();
	// '￿' parks albumless tracks at the end rather than the top.
	const key = (t: SongItem) =>
		sort === 'title' ? t.title : sort === 'artist' ? t.artists : t.album || '￿';
	// Array#sort is stable, so ties keep playlist order; the title tiebreak only decides the order
	// inside one artist's or album's block.
	return items
		.slice()
		.sort((a, b) => collator.compare(key(a), key(b)) || collator.compare(a.title, b.title));
}
