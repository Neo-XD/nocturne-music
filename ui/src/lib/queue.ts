// How the queue panel cuts one flat queue into blocks. Kept out of the component so it can be
// checked without a DOM (`queue.check.ts`) — the ordering bug it exists to prevent (an added
// playlist drawn under the playing playlist's name) is invisible until you look at real data.
import type { QueueState, SongItem } from './api';

export interface QueueRow {
	item: SongItem;
	/** video_id + occurrence, so `animate:flip` slides rows instead of recreating them. */
	key: string;
	/** Index in the backend queue — what play/remove act on, and what the row is numbered by. */
	i: number;
}

export interface QueueBlock {
	/** Stable id for keyed rendering (the first row's key). */
	key: string;
	/** What groups a run: same kind ⇒ same block. */
	kind: string;
	heading: string;
	autoplay: boolean;
	/** The "Clear queue" button goes on the first manual block only. */
	clearable: boolean;
	rows: QueueRow[];
}

/**
 * The tracks already played, then the playing track, then the upcoming queue in play order, split
 * wherever the tracks change origin: a manual block ("Play next" / "Add to queue", headed by what it
 * was added from), the playing context ("Next from: …"), autoplay's continuation.
 *
 * `prev` is what the panel shows behind "Load previous", oldest first, so the last thing heard sits
 * directly above the playing track. It runs from `playedFrom`, not from 0: the backend queue holds
 * the whole playlist even when playback started in the middle of it (`state.rs`), and those leading
 * tracks were never heard.
 *
 * Play order, not kind order: grouping by kind would draw an "Add to queue" block that sits at the
 * tail under the playing playlist's heading, naming a playlist those tracks never came from.
 *
 * Shuffle collapses that split. The backend deliberately interleaves everything it shuffles, so
 * origins alternate track by track and a per-origin split degenerates into one heading per row —
 * the shuffled run becomes a single block instead. What shuffle leaves alone keeps its own: the
 * pinned "Play next" block ahead of it, autoplay's filler behind it. Turning shuffle off restores
 * the real order, and with it the blocks.
 */
export function queueBlocks(q: QueueState): {
	prev: QueueRow[];
	now: QueueRow | null;
	blocks: QueueBlock[];
} {
	const { items, currentIndex, sourceName } = q;
	const playedFrom = Math.min(q.playedFrom ?? currentIndex, currentIndex);
	const seen = new Map<string, number>();
	const row = (i: number): QueueRow => {
		const item = items[i];
		const occ = seen.get(item.video_id) ?? 0;
		seen.set(item.video_id, occ + 1);
		return { item, key: `${item.video_id}:${occ}`, i };
	};
	// Occurrence counting must walk the whole prefix, not just the played part of it, or a repeated
	// track's key would collide with an earlier copy that isn't on screen.
	const prev: QueueRow[] = [];
	for (let i = 0; i < currentIndex; i++) {
		const r = row(i);
		if (i >= playedFrom) prev.push(r);
	}
	// Rows are drawn in queue order throughout, so the number is the queue index and nothing else:
	// counting per run restarted the playing track at 1 under the two tracks already heard (#25).
	const now = items[currentIndex] ? row(currentIndex) : null;

	// Where the shuffled run starts: shuffle pins the leading "Play next" block in place and
	// shuffles everything after it.
	let shuffledFrom = items.length;
	if (q.shuffle) {
		shuffledFrom = currentIndex + 1;
		while (items[shuffledFrom]?.queued) shuffledFrom++;
	}

	const blocks: QueueBlock[] = [];
	let cleared = false;
	for (let i = currentIndex + 1; i < items.length; i++) {
		const r = row(i);
		const it = r.item;
		const manual = !!(it.queued || it.queued_end);
		// `queued_from` splits two albums added back to back, and keeps a continuation walked in
		// later with the block it belongs to.
		const kind = it.autoplay
			? 'auto'
			: i >= shuffledFrom
				? 'shuffled'
				: manual
					? `manual:${it.queued ? 'next' : 'end'}:${it.queued_from ?? ''}`
					: 'context';
		const last = blocks.at(-1);
		if (last?.kind === kind) {
			last.rows.push(r);
			continue;
		}
		blocks.push({
			key: r.key,
			kind,
			heading: '',
			autoplay: !!it.autoplay,
			clearable: false,
			rows: [r]
		});
	}
	// Both need the whole block, not its first row: a shuffled run only has one origin to name if
	// every track in it agrees, and "Clear queue" belongs on the first block holding anything the
	// user queued — under shuffle that's a mixed block whose first row may well be a playlist track.
	for (const block of blocks) {
		block.heading = headingFor(block, sourceName);
		const manual = block.rows.some((r) => r.item.queued || r.item.queued_end);
		block.clearable = manual && !cleared;
		if (block.clearable) cleared = true;
	}
	return { prev, now, blocks };
}

/** The name a block goes under: its one origin if its tracks share one, else a neutral label. */
function headingFor(block: QueueBlock, sourceName?: string | null): string {
	if (block.autoplay) return 'Autoplay';
	// What each row would put on the heading: what it was added from, the queue's own source for a
	// plain context track, nothing for a single-song add.
	const names = new Set(
		block.rows.map(
			(r) => r.item.queued_from ?? (r.item.queued || r.item.queued_end ? '' : (sourceName ?? ''))
		)
	);
	const [name] = names;
	if (names.size === 1 && name) return `Next from: ${name}`;
	return block.rows.every((r) => r.item.queued || r.item.queued_end) ? 'Next in queue' : 'Next up';
}
