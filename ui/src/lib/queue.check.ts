// Self-check for the queue panel's block split (`queue.ts`). Same deal as `dnd.check.ts` — no test
// runner in `ui/`, node 22 runs TypeScript directly:
//
//     node --experimental-strip-types ui/src/lib/queue.check.ts
//
// Prints "ok" and exits 0, or throws on the first broken invariant. The bug this guards: an album
// added with "Add to queue" sits at the tail of a playlist queue, and grouping the panel by kind
// instead of by play order drew it under the *playing* playlist's "Next from" heading.
import type { QueueState, SongItem } from './api.ts';
import { queueBlocks } from './queue.ts';

function ok(cond: boolean, what: string): void {
	if (!cond) throw new Error(`FAIL: ${what}`);
}

const song = (id: string, extra: Partial<SongItem> = {}): SongItem => ({
	video_id: id,
	title: id,
	artists: '',
	...extra
});
const q = (
	items: SongItem[],
	currentIndex: number,
	sourceName?: string,
	shuffle = false,
	playedFrom?: number
): QueueState => ({ items, currentIndex, sourceName, shuffle, playedFrom });

// Playing "Afro", an "Add to queue" of "Nightcore Bangers" behind it, autoplay filler behind that.
const added = (id: string) => song(id, { queued_end: true, queued_from: 'Nightcore Bangers' });
let v = queueBlocks(
	q(
		[
			song('a1'),
			song('a2'),
			added('n1'),
			added('n2'),
			song('r1', { autoplay: true })
		],
		0,
		'Afro'
	)
);
ok(v.now?.item.video_id === 'a1', 'the playing track is its own row');
ok(v.blocks.length === 3, 'context, added block, autoplay');
ok(v.blocks[0].heading === 'Next from: Afro', 'the playing playlist keeps its name');
ok(v.blocks[0].rows.length === 1, 'only the rest of Afro is under it');
ok(v.blocks[1].heading === 'Next from: Nightcore Bangers', 'the added block is named after itself');
ok(v.blocks[1].rows.map((r) => r.item.video_id).join() === 'n1,n2', 'the whole added block');
ok(v.blocks[2].autoplay, 'autoplay filler is last and marked');
ok(v.blocks[1].clearable && !v.blocks[0].clearable, 'Clear queue sits on the manual block');
// Row numbering runs 1..n over what's visible, and indices still point at the backend queue.
ok(v.now?.n === 1 && v.blocks[0].rows[0].n === 2 && v.blocks[2].rows[0].n === 5, 'numbered in order');
ok(v.blocks[1].rows[0].i === 2, 'index is the queue index, not the display number');

// "Play next" (queued) stays right behind the current track, ahead of the context.
v = queueBlocks(q([song('a1'), song('p1', { queued: true }), song('a2')], 0, 'Afro'));
ok(v.blocks[0].heading === 'Next in queue', 'a single-song add has no source to name');
ok(v.blocks[1].heading === 'Next from: Afro', 'the context follows it');

// Two albums added back to back are two blocks, in the order they were added.
v = queueBlocks(
	q(
		[
			song('a1'),
			song('x1', { queued_end: true, queued_from: 'X' }),
			song('y1', { queued_end: true, queued_from: 'Y' })
		],
		0
	)
);
ok(v.blocks.map((b) => b.heading).join() === 'Next from: X,Next from: Y', 'one block each');
ok(v.blocks[0].key !== v.blocks[1].key, 'block keys are distinct (keyed rendering)');

// No source name and nothing manual: the plain "Next up" case still works, as one block.
v = queueBlocks(q([song('a'), song('b'), song('c')], 0));
ok(v.blocks.length === 1 && v.blocks[0].heading === 'Next up', 'unnamed context is one block');

// Played tracks are out of the upcoming blocks, and a repeated track doesn't collide with its
// earlier copy's key.
v = queueBlocks(q([song('dup'), song('now'), song('dup')], 1));
ok(v.blocks[0].rows.length === 1 && v.now?.item.video_id === 'now', 'history is not drawn');
ok(v.blocks[0].rows[0].key === 'dup:1', 'the second copy of a track gets its own key');

// --- previously played ------------------------------------------------------------------------
// The prefix is not the history: opening a playlist at track 3 leaves two untouched tracks in front
// of the playing one, and the backend says so with `playedFrom`.
v = queueBlocks(q([song('a'), song('b'), song('now'), song('d')], 2, 'Afro', false, 2));
ok(v.prev.length === 0, 'starting mid-playlist has played nothing');

// Jumping forward counts everything passed over, oldest first, so the last thing heard sits
// directly above the playing track.
v = queueBlocks(q([song('a'), song('b'), song('c'), song('now')], 3, 'Afro', false, 1));
ok(v.prev.map((r) => r.item.video_id).join() === 'b,c', 'skipped-over tracks count as played');
ok(v.prev.map((r) => r.i).join() === '1,2', 'indices still point at the backend queue');
ok(v.prev.map((r) => r.n).join() === '1,2', 'the played run numbers itself from 1');
ok(v.now?.n === 1, 'the playing row keeps its number when history loads');

// A blob saved before this existed (no `playedFrom`) restores with nothing played, rather than
// claiming the whole prefix was heard.
v = queueBlocks(q([song('a'), song('b'), song('now')], 2));
ok(v.prev.length === 0, 'no playedFrom ⇒ empty history');

// Keys stay unique when the same track shows up in the history and again ahead of the playing one.
v = queueBlocks(q([song('dup'), song('now'), song('dup')], 1, undefined, false, 0));
ok(v.prev[0].key === 'dup:0' && v.blocks[0].rows[0].key === 'dup:1', 'each copy keeps its own key');

// An empty queue draws nothing rather than throwing.
v = queueBlocks(q([], 0));
ok(v.now === null && v.blocks.length === 0, 'empty queue');

// --- shuffle: the interleaved run is one block ------------------------------------------------
// Shuffle mixes the playing playlist with everything added to it, so origins alternate row by row.
// Split per origin, that's a heading above every single track (what the owner reported).
const mixed = [
	song('now'),
	song('a1'),
	added('n1'),
	song('a2'),
	added('n2'),
	song('r1', { autoplay: true })
];
v = queueBlocks(q(mixed, 0, 'Afro', true));
ok(v.blocks.length === 2, 'the shuffled run is one block, autoplay still its own');
ok(v.blocks[0].heading === 'Next up', 'two origins in one block ⇒ neither name is the truth');
ok(v.blocks[0].rows.length === 4, 'every shuffled track is in it');
ok(v.blocks[0].clearable, 'Clear queue is offered even though the block opens on a playlist track');
ok(v.blocks[1].autoplay, 'filler is not merged into the shuffle');

// Same queue unshuffled: the blocks come back (this is what turning shuffle off restores).
v = queueBlocks(q([song('now'), song('a1'), song('a2'), added('n1'), added('n2')], 0, 'Afro'));
ok(
	v.blocks.map((b) => b.heading).join() === 'Next from: Afro,Next from: Nightcore Bangers',
	'unshuffled ⇒ back to one block per origin'
);

// Shuffling a plain playlist (nothing added) still names it — no regression on the common case.
v = queueBlocks(q([song('now'), song('a1'), song('a2')], 0, 'Afro', true));
ok(v.blocks.length === 1 && v.blocks[0].heading === 'Next from: Afro', 'one origin keeps its name');

// The "Play next" block is pinned ahead of the shuffle, so it keeps its own heading.
v = queueBlocks(
	q([song('now'), song('p1', { queued: true }), song('a1'), added('n1')], 0, 'Afro', true)
);
ok(v.blocks[0].heading === 'Next in queue' && v.blocks[0].rows.length === 1, 'pinned block stands');
ok(v.blocks[1].heading === 'Next up' && v.blocks[1].rows.length === 2, 'the rest is the shuffle');
ok(v.blocks[0].clearable && !v.blocks[1].clearable, 'Clear queue on the first manual block only');

// Only added playlists left in the shuffled run (the context ran out): still the queue, unnamed.
v = queueBlocks(
	q([song('now'), added('n1'), song('x1', { queued_end: true, queued_from: 'X' })], 0, 'Afro', true)
);
ok(v.blocks[0].heading === 'Next in queue', 'all manual, two origins ⇒ "Next in queue"');

console.log('ok');
