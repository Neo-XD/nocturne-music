<script lang="ts">
	import { flip } from 'svelte/animate';
	import { cubicOut } from 'svelte/easing';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { InfinityIcon } from '@hugeicons/core-free-icons';
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

	// Playing a playlist queues the whole playlist, so this panel can be handed five figures of
	// rows the moment it opens, at roughly 165 KB of web-process memory each (`rows.ts`). Past a
	// couple of hundred it renders only what is near the viewport.
	//
	// Below that it is exactly what it always was, flip animation included: windowing costs the
	// reorder animation (flip measures against the viewport, so it would fight the scroll), and
	// that is a bad trade for a queue you can see the end of.
	const WINDOW_ABOVE = 200;
	const sc = rowScroller();
	const counts = $derived([view.now ? 1 : 0, ...view.blocks.map((b) => b.rows.length)]);
	const windowed = $derived(counts.reduce((a, c) => a + c, 0) > WINDOW_ABOVE);
	const wins = $derived(
		windowed
			? blockWindows(sc.scrollTop, sc.viewportPx, counts, sc.rowPx)
			: counts.map(fullWindow)
	);
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
<div class="min-h-0 flex-1 overflow-y-auto p-2" {@attach sc.attach}>
	{#if view.now}
		<h3 class="px-2 pt-2 pb-1.5 text-sm font-semibold">Now playing</h3>
		{@render rows([view.now], wins[0])}

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
			{@render rows(block.rows, wins[b + 1])}
		{/each}
	{:else}
		<p class="p-4 text-sm text-muted-foreground">The queue is empty.</p>
	{/if}
</div>
