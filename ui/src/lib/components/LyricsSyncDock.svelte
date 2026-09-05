<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Time02Icon,
		RefreshIcon,
		ArrowLeft01Icon,
		ArrowRight01Icon
	} from '@hugeicons/core-free-icons';
	import {
		lyricsSync,
		adjustCurrentLyricsOffset,
		resetCurrentLyricsOffset,
		playback
	} from '$lib/player.svelte';

	let {
		compact = false,
		class: customClass = ''
	}: {
		compact?: boolean;
		class?: string;
	} = $props();

	const offsetMs = $derived(lyricsSync.currentOffsetMs);
	const offsetSecs = $derived((offsetMs / 1000).toFixed(1));
	const formattedOffset = $derived(
		offsetMs === 0 ? '0.0s' : offsetMs > 0 ? `+${offsetSecs}s` : `${offsetSecs}s`
	);

	function nudge(deltaMs: number, e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		adjustCurrentLyricsOffset(deltaMs);
	}

	function reset(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		resetCurrentLyricsOffset();
	}
</script>

<div
	class="lyrics-sync-dock inline-flex items-center gap-0.5 rounded-full border border-border/60 bg-background/70 p-0.5 shadow-sm backdrop-blur-md transition-all duration-200 dark:bg-card/75 hover:border-primary/40 {customClass}"
	title="Adjust lyrics sync timing for this song"
>
	<div class="flex items-center gap-0.5">
		<!-- Step down -0.5s -->
		<button
			type="button"
			class="flex h-5 items-center justify-center rounded-full px-1.5 text-[10px] font-medium text-muted-foreground transition hover:bg-foreground/10 hover:text-foreground active:scale-95 cursor-pointer"
			onclick={(e) => nudge(-500, e)}
			title="Delay lyrics by 0.5s"
			aria-label="Delay lyrics by 0.5s"
		>
			-0.5s
		</button>
		<!-- Step down -0.1s -->
		<button
			type="button"
			class="flex h-5 items-center justify-center rounded-full px-1.5 text-[10px] font-medium text-muted-foreground transition hover:bg-foreground/10 hover:text-foreground active:scale-95 cursor-pointer"
			onclick={(e) => nudge(-100, e)}
			title="Delay lyrics by 0.1s"
			aria-label="Delay lyrics by 0.1s"
		>
			-0.1s
		</button>
	</div>

	<!-- Center Display / Reset button -->
	<button
		type="button"
		class="group flex h-5 min-w-14 cursor-pointer items-center justify-center gap-1 rounded-full px-2 text-[10px] font-semibold tabular-nums transition active:scale-95 {offsetMs !== 0
			? 'bg-primary/15 text-primary hover:bg-primary/25 shadow-xs'
			: 'text-muted-foreground hover:bg-foreground/10 hover:text-foreground'}"
		onclick={reset}
		title={offsetMs !== 0 ? 'Click to reset offset to 0.0s' : 'Sync Offset'}
		aria-label="Lyrics sync offset: {formattedOffset}. Click to reset"
	>
		<HugeiconsIcon icon={Time02Icon} class="h-3 w-3 shrink-0 opacity-70" />
		<span>{formattedOffset}</span>
		{#if offsetMs !== 0}
			<HugeiconsIcon icon={RefreshIcon} class="h-2.5 w-2.5 opacity-60 group-hover:opacity-100" />
		{/if}
	</button>

	<div class="flex items-center gap-0.5">
		<!-- Step up +0.1s -->
		<button
			type="button"
			class="flex h-5 items-center justify-center rounded-full px-1.5 text-[10px] font-medium text-muted-foreground transition hover:bg-foreground/10 hover:text-foreground active:scale-95 cursor-pointer"
			onclick={(e) => nudge(100, e)}
			title="Advance lyrics by 0.1s"
			aria-label="Advance lyrics by 0.1s"
		>
			+0.1s
		</button>
		<!-- Step up +0.5s -->
		<button
			type="button"
			class="flex h-5 items-center justify-center rounded-full px-1.5 text-[10px] font-medium text-muted-foreground transition hover:bg-foreground/10 hover:text-foreground active:scale-95 cursor-pointer"
			onclick={(e) => nudge(500, e)}
			title="Advance lyrics by 0.5s"
			aria-label="Advance lyrics by 0.5s"
		>
			+0.5s
		</button>
	</div>
</div>
