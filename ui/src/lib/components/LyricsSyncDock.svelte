<script lang="ts">
	import { onDestroy } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Time02Icon,
		RefreshIcon,
		Add01Icon,
		MinusSignIcon
	} from '@hugeicons/core-free-icons';
	import {
		lyricsSync,
		adjustCurrentLyricsOffset,
		resetCurrentLyricsOffset
	} from '$lib/player.svelte';

	let {
		compact = false,
		class: customClass = ''
	}: {
		compact?: boolean;
		class?: string;
	} = $props();

	const offsetMs = $derived(lyricsSync.currentOffsetMs);
	const formattedOffset = $derived(
		offsetMs === 0 ? '0 ms' : offsetMs > 0 ? `+${offsetMs} ms` : `${offsetMs} ms`
	);

	let repeatTimer: ReturnType<typeof setTimeout> | undefined;
	let intervalTimer: ReturnType<typeof setInterval> | undefined;

	function stopHold() {
		if (repeatTimer) {
			clearTimeout(repeatTimer);
			repeatTimer = undefined;
		}
		if (intervalTimer) {
			clearInterval(intervalTimer);
			intervalTimer = undefined;
		}
	}

	function startHold(deltaMs: number, e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		// Initial step
		const step = e.shiftKey ? deltaMs * 4 : deltaMs;
		adjustCurrentLyricsOffset(step);

		stopHold();
		repeatTimer = setTimeout(() => {
			intervalTimer = setInterval(() => {
				adjustCurrentLyricsOffset(deltaMs * 2);
			}, 60);
		}, 300);
	}

	function handleWheel(e: WheelEvent) {
		e.preventDefault();
		e.stopPropagation();
		const step = e.shiftKey ? 100 : 25;
		if (e.deltaY < 0) {
			adjustCurrentLyricsOffset(step); // Scroll up -> advance lyrics
		} else if (e.deltaY > 0) {
			adjustCurrentLyricsOffset(-step); // Scroll down -> delay lyrics
		}
	}

	function reset(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		resetCurrentLyricsOffset();
	}

	onDestroy(stopHold);
</script>

<svelte:window onpointerup={stopHold} onpointercancel={stopHold} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="lyrics-sync-dock inline-flex select-none items-center gap-1 rounded-full border border-border/70 bg-background/80 p-0.5 shadow-sm backdrop-blur-md transition-all duration-200 dark:bg-card/85 hover:border-primary/50 {customClass}"
	onwheel={handleWheel}
	title="Adjust lyrics sync timing in milliseconds (Scroll or click +/-)"
>
	<!-- Quick Jump -0.5s -->
	<button
		type="button"
		class="hidden sm:flex h-5 items-center justify-center rounded-full px-1.5 text-[10px] font-medium text-muted-foreground transition hover:bg-foreground/10 hover:text-foreground active:scale-95 cursor-pointer"
		onclick={(e) => {
			e.stopPropagation();
			adjustCurrentLyricsOffset(-500);
		}}
		title="Delay lyrics by 500 ms (-0.5s)"
		aria-label="Delay lyrics by 500 ms"
	>
		-0.5s
	</button>

	<!-- Precise Step Down (-) -->
	<button
		type="button"
		class="flex h-5 w-5 items-center justify-center rounded-full bg-foreground/5 text-muted-foreground transition hover:bg-foreground/15 hover:text-foreground active:scale-90 cursor-pointer"
		onpointerdown={(e) => startHold(-25, e)}
		title="Delay lyrics by 25 ms (-). Hold to adjust continuously, Shift+click for 100 ms"
		aria-label="Delay lyrics by 25 ms"
	>
		<HugeiconsIcon icon={MinusSignIcon} class="h-3 w-3" strokeWidth={2.5} />
	</button>

	<!-- Center Display / Reset button -->
	<button
		type="button"
		class="group flex h-5 min-w-16 cursor-pointer items-center justify-center gap-1 rounded-full px-2 text-[10px] font-semibold tabular-nums transition active:scale-95 {offsetMs !== 0
			? 'bg-primary/15 text-primary hover:bg-primary/25 shadow-xs'
			: 'text-muted-foreground hover:bg-foreground/10 hover:text-foreground'}"
		onclick={reset}
		title={offsetMs !== 0 ? `Current sync offset: ${formattedOffset} (Click to reset to 0 ms)` : 'Lyrics Sync Offset (Click to reset)'}
		aria-label="Lyrics sync offset: {formattedOffset}. Click to reset to 0 ms"
	>
		<HugeiconsIcon icon={Time02Icon} class="h-3 w-3 shrink-0 opacity-70" />
		<span>{formattedOffset}</span>
		{#if offsetMs !== 0}
			<HugeiconsIcon icon={RefreshIcon} class="h-2.5 w-2.5 opacity-60 group-hover:opacity-100" />
		{/if}
	</button>

	<!-- Precise Step Up (+) -->
	<button
		type="button"
		class="flex h-5 w-5 items-center justify-center rounded-full bg-foreground/5 text-muted-foreground transition hover:bg-foreground/15 hover:text-foreground active:scale-90 cursor-pointer"
		onpointerdown={(e) => startHold(25, e)}
		title="Advance lyrics by 25 ms (+). Hold to adjust continuously, Shift+click for 100 ms"
		aria-label="Advance lyrics by 25 ms"
	>
		<HugeiconsIcon icon={Add01Icon} class="h-3 w-3" strokeWidth={2.5} />
	</button>

	<!-- Quick Jump +0.5s -->
	<button
		type="button"
		class="hidden sm:flex h-5 items-center justify-center rounded-full px-1.5 text-[10px] font-medium text-muted-foreground transition hover:bg-foreground/10 hover:text-foreground active:scale-95 cursor-pointer"
		onclick={(e) => {
			e.stopPropagation();
			adjustCurrentLyricsOffset(500);
		}}
		title="Advance lyrics by 500 ms (+0.5s)"
		aria-label="Advance lyrics by 500 ms"
	>
		+0.5s
	</button>
</div>

