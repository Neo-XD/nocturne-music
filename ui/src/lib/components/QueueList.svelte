<script lang="ts">
	import { tick } from 'svelte';
	import { flip } from 'svelte/animate';
	import { cubicOut } from 'svelte/easing';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { HistoryIcon, InfinityIcon } from '@hugeicons/core-free-icons';
	import TrackRow from '$lib/components/TrackRow.svelte';
	import * as api from '$lib/api';
	import { queueBlocks, type QueueRow } from '$lib/queue';
	import { blockWindows, fullWindow, type RowWindow } from '$lib/rows';
	import { rowScroller } from '$lib/rows.svelte';
	import { playback, openAddToPlaylist } from '$lib/player.svelte';
	import { lt } from '$lib/lt.svelte';

	// Guests are add-only in a session — no removing (theirs or anyone's). The playing row can't
	// be removed either (backend guards it too).
	const canRemove = $derived(lt.role !== 'guest');

	// Blocks in play order, cut wherever the upcoming tracks change origin (`queue.ts`).
	const view = $derived(queueBlocks(playback.queue));

	// The tracks already heard, hidden until asked for: a queue played deep into has hundreds of
	// them, and they sit above everything anyone opened the panel to look at.
	let showPrev = $state(false);
	let el: HTMLElement;

	// Playing a playlist queues the whole playlist, so this panel can be handed five figures of
	// rows the moment it opens, at roughly 165 KB of web-process memory each (`rows.ts`). Past a
	// couple of hundred it renders only what is near the viewport.
	//
	// Below that it is exactly what it always was, flip animation included: windowing costs the
	// reorder animation (flip measures against the viewport, so it would fight the scroll), and
	// that is a bad trade for a queue you can see the end of.
	const WINDOW_ABOVE = 200;
	const sc = rowScroller();
	// One entry per block, previously-played first. Collapsed it is 0 rows but still charged a
	// heading it doesn't draw, which shifts every window's *choice* of slice by 40px and none of
	// their heights: the overscan swallows it (see HEADING_PX).
	const counts = $derived([
		showPrev ? view.prev.length : 0,
		view.now ? 1 : 0,
		...view.blocks.map((b) => b.rows.length)
	]);
	const windowed = $derived(counts.reduce((a, c) => a + c, 0) > WINDOW_ABOVE);
	const wins = $derived(
		windowed
			? blockWindows(sc.scrollTop, sc.viewportPx, counts, sc.rowPx)
			: counts.map(fullWindow)
	);

	async function togglePrev() {
		const before = el.scrollHeight;
		showPrev = !showPrev;
		await tick();
		// Rows appear (or vanish) above the viewport, and WebKit implements no scroll anchoring, so
		// without this the panel jumps by the whole height of the history. Keeps Now playing still.
		el.scrollTop += el.scrollHeight - before;
	}
</script>

{#snippet rows(list: QueueRow[], w: RowWindow)}
	<!-- The padding stands in for the rows outside the window, so this block is exactly as tall as
	     all of its rows and every heading below it stays where it was. -->
	<div style="padding-top:{w.padTop}px;padding-bottom:{w.padBottom}px">
		{#each list.slice(w.start, w.end) as { item, key, i, n } (key)}
			<!-- data-row: what the scroller measures a row's real height from. -->
			<div data-row animate:flip={{ duration: windowed ? 0 : 200, easing: cubicOut }}>
				<TrackRow
					song={item}
					index={n - 1}
					active={i === playback.queue.currentIndex}
					hideRating
					onplay={() => api.playIndex(i)}
					onAdd={() => openAddToPlaylist(item)}
					onRemove={canRemove && i !== playback.queue.currentIndex
						? () => api.removeFromQueue(i)
						: undefined}
					removeLabel="Remove from queue"
				/>
			</div>
		{/each}
	</div>
{/snippet}

<!-- The list on its own, so the side panel and the now-playing view's Queue tab render the same
     one instead of drifting apart. -->
<div class="min-h-0 flex-1 overflow-y-auto p-2" bind:this={el} {@attach sc.attach}>
	{#if view.now}
		{#if showPrev && view.prev.length}
			<h3 class="px-2 pt-2 pb-1.5 text-sm font-semibold text-muted-foreground">
				Previously played
			</h3>
			{@render rows(view.prev, wins[0])}
		{/if}
		<div class="flex items-center justify-between gap-2 px-2 pt-2 pb-1.5">
			<h3 class="truncate text-sm font-semibold">Now playing</h3>
			{#if view.prev.length}
				<button
					class="flex shrink-0 cursor-pointer items-center gap-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
					onclick={togglePrev}
				>
					<HugeiconsIcon icon={HistoryIcon} class="h-3.5 w-3.5" />
					{showPrev ? 'Hide previous' : 'Load previous'}
				</button>
			{/if}
		</div>
		{@render rows([view.now], wins[1])}

		{#each view.blocks as block, b (block.key)}
			{#if block.autoplay}
				<div
					class="mt-3 flex items-center gap-2 border-t px-2 pt-2.5 pb-1.5 text-muted-foreground"
					title="Autoplay keeps the music going with similar songs. Turn it off in Settings ▸ Playback."
				>
					<HugeiconsIcon icon={InfinityIcon} class="h-3.5 w-3.5" />
					<span class="text-xs font-medium">Autoplay</span>
					<span class="truncate text-xs">· similar music</span>
				</div>
			{:else}
				<div class="mt-3 flex items-center justify-between gap-2 px-2 pb-1.5">
					<h3 class="truncate text-sm font-semibold">{block.heading}</h3>
					{#if block.clearable && canRemove}
						<button
							class="shrink-0 cursor-pointer text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
							onclick={() => api.clearQueued()}
						>
							Clear queue
						</button>
					{/if}
				</div>
			{/if}
			{@render rows(block.rows, wins[b + 2])}
		{/each}
	{:else}
		<p class="p-4 text-sm text-muted-foreground">The queue is empty.</p>
	{/if}
</div>
