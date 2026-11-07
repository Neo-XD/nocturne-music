<script lang="ts">
	import { playback } from '$lib/player.svelte';

	let {
		position = 0,
		duration = 0,
		class: className = '',
		height = 24,
		barCount = 72,
		onSeek,
		onCommit
	}: {
		position?: number;
		duration?: number;
		class?: string;
		height?: number;
		barCount?: number;
		onSeek?: (pos: number) => void;
		onCommit?: (pos: number) => void;
	} = $props();

	let containerEl: HTMLDivElement | undefined = $state();
	let isHovering = $state(false);
	let hoverRatio = $state(0);
	let isDragging = $state(false);

	// Generate deterministic, realistic waveform amplitudes for the current track
	const waveformBars = $derived.by(() => {
		const trackKey = `${playback.now?.videoId ?? 'default'}_${Math.round(duration)}`;
		let seed = 0;
		for (let i = 0; i < trackKey.length; i++) {
			seed = (seed << 5) - seed + trackKey.charCodeAt(i);
			seed |= 0;
		}

		// Simple pseudo-random generator
		const pseudoRand = (s: number) => {
			const x = Math.sin(s) * 10000;
			return x - Math.floor(x);
		};

		const bars: number[] = [];
		const total = barCount;

		// Generate continuous, musical dynamic energy curves
		for (let i = 0; i < total; i++) {
			const norm = i / total;
			// Intro fade in, energetic middle choruses, outro fade
			const envelope = Math.sin(norm * Math.PI);
			const octave1 = Math.sin(norm * 14.0 + seed) * 0.25;
			const octave2 = Math.cos(norm * 28.0 + seed * 1.5) * 0.15;
			const noise = pseudoRand(seed + i * 37) * 0.45;

			const amp = 0.15 + (envelope * 0.5 + octave1 + octave2 + noise) * 0.75;
			bars.push(Math.max(0.12, Math.min(1.0, amp)));
		}

		return bars;
	});

	const progressRatio = $derived(duration > 0 ? Math.max(0, Math.min(1, position / duration)) : 0);

	function formatTime(secs: number) {
		if (!secs || secs < 0 || isNaN(secs)) return '0:00';
		const m = Math.floor(secs / 60);
		const s = Math.floor(secs % 60);
		return `${m}:${s.toString().padStart(2, '0')}`;
	}

	function updateSeek(e: PointerEvent) {
		if (!containerEl || duration <= 0) return;
		const rect = containerEl.getBoundingClientRect();
		const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
		hoverRatio = ratio;
		onSeek?.(ratio * duration);
	}

	function handlePointerDown(e: PointerEvent) {
		e.stopPropagation();
		if (!containerEl || duration <= 0) return;
		isDragging = true;
		containerEl.setPointerCapture(e.pointerId);
		updateSeek(e);
	}

	function handlePointerMove(e: PointerEvent) {
		e.stopPropagation();
		if (!containerEl) return;
		const rect = containerEl.getBoundingClientRect();
		const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
		hoverRatio = ratio;

		if (isDragging) {
			const newPos = ratio * duration;
			onSeek?.(newPos);
		}
	}

	function handlePointerUp(e: PointerEvent) {
		e.stopPropagation();
		if (!isDragging || !containerEl || duration <= 0) return;
		isDragging = false;
		try {
			containerEl.releasePointerCapture(e.pointerId);
		} catch {}
		const rect = containerEl.getBoundingClientRect();
		const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
		const newPos = ratio * duration;
		onCommit?.(newPos);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={containerEl}
	class="group/waveform relative flex w-full cursor-pointer select-none items-center py-1 transition-all {className}"
	style="height: {height + 8}px;"
	onclick={(e) => e.stopPropagation()}
	onpointerdown={handlePointerDown}
	onpointermove={handlePointerMove}
	onpointerup={handlePointerUp}
	onpointerenter={() => (isHovering = true)}
	onpointerleave={() => (isHovering = false)}
	role="slider"
	aria-valuenow={Math.round(position)}
	aria-valuemin="0"
	aria-valuemax={Math.round(duration)}
	aria-label="Seek track position with audio waveform"
	tabindex="0"
	onkeydown={(e) => {
		if (e.key === 'ArrowRight') onCommit?.(Math.min(duration, position + 5));
		else if (e.key === 'ArrowLeft') onCommit?.(Math.max(0, position - 5));
	}}
>
	<!-- Background (Unplayed) Bars Track -->
	<div class="flex h-full w-full items-center justify-between gap-[2px] opacity-35">
		{#each waveformBars as bar, i (i)}
			<div
				class="w-full flex-1 rounded-[var(--radius,2px)] bg-muted-foreground transition-[height] duration-200"
				style="height: {Math.max(3, bar * height)}px; min-width: 1.5px; max-width: 4px;"
			></div>
		{/each}
	</div>

	<!-- Hover Preview Bars Track -->
	{#if isHovering}
		<div
			class="pointer-events-none absolute inset-0 flex h-full w-full items-center justify-between gap-[2px]"
			style="clip-path: inset(0 calc(100% - {hoverRatio * 100}%) 0 0);"
		>
			{#each waveformBars as bar, i (i)}
				<div
					class="w-full flex-1 rounded-[var(--radius,2px)] bg-primary/40 transition-[height] duration-200"
					style="height: {Math.max(3, bar * height)}px; min-width: 1.5px; max-width: 4px;"
				></div>
			{/each}
		</div>
	{/if}

	<!-- Active (Played) Bars Track - Continuous smooth clip-path fill -->
	<div
		class="pointer-events-none absolute inset-0 flex h-full w-full items-center justify-between gap-[2px] {isDragging
			? ''
			: 'transition-[clip-path] duration-150 ease-linear'}"
		style="clip-path: inset(0 calc(100% - {progressRatio * 100}%) 0 0);"
	>
		{#each waveformBars as bar, i (i)}
			<div
				class="w-full flex-1 rounded-[var(--radius,2px)] bg-primary shadow-[0_0_8px_rgba(var(--primary-rgb,225,29,72),0.45)] transition-[height] duration-200"
				style="height: {Math.max(3, bar * height)}px; min-width: 1.5px; max-width: 4px;"
			></div>
		{/each}
	</div>

	<!-- Hover Tooltip -->
	{#if isHovering && duration > 0}
		<div
			class="pointer-events-none absolute -top-6 -translate-x-1/2 rounded bg-popover/95 px-1.5 py-0.5 text-[10px] font-semibold text-popover-foreground shadow-md backdrop-blur border border-border/80 transition-opacity"
			style="left: {hoverRatio * 100}%;"
		>
			{formatTime(hoverRatio * duration)}
		</div>
	{/if}
</div>
