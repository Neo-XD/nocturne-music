<script lang="ts">
	import * as api from '$lib/api';
	import { playback, lyricsSync } from '$lib/player.svelte';
	import LyricsSyncDock from '$lib/components/LyricsSyncDock.svelte';
	import LyricSelectorModal from '$lib/components/LyricSelectorModal.svelte';
	import { listen } from '@tauri-apps/api/event';

	// `expanded` only sizes the type and centres the column. The owner of the extra room (the side
	// panel, or the now-playing view) decides how much there is. Toggling it must not remount this
	// component, or the lyrics refetch and the scroll position is lost.
	// `compact` is the mini-player: a ~220px column with no room for the source footer or a
	// scrollbar. It only shrinks the type and chrome; the sync/auto-scroll logic is identical.
	let { expanded = false, compact = false }: { expanded?: boolean; compact?: boolean } =
		$props();

	/** "3:21" / "1:02:03" → seconds. */
	function durationSecs(d?: string): number | undefined {
		if (!d) return undefined;
		const parts = d.split(':').map(Number);
		if (!parts.length || parts.some(Number.isNaN)) return undefined;
		return parts.reduce((a, b) => a * 60 + b, 0);
	}

	let lyrics = $state<api.Lyrics | null>(null);
	let loading = $state(true);
	let selectorOpen = $state(false);
	let scroller: HTMLElement | undefined = $state();

	$effect(() => {
		const unlisten = listen<api.Lyrics>('lyrics-updated', (e) => {
			lyrics = e.payload;
		});
		return () => {
			unlisten.then((u) => u());
		};
	});

	// videoId of the fetch whose result is (or will be) shown — guards stale responses.
	let requested = '';
	let currentTrackId = '';

	$effect(() => {
		const vid = playback.now?.videoId;
		if (vid && vid !== currentTrackId) {
			currentTrackId = vid;
			hasScrolled = false;
			userScrollUntil = 0;
			if (scroller) {
				scroller.scrollTo({ top: 0, behavior: 'instant' });
			}
		}
	});

	$effect(() => {
		const now = playback.now;
		if (!now) {
			requested = '';
			lyrics = null;
			loading = false;
			return;
		}
		if (now.videoId === requested) return;
		const id = (requested = now.videoId);
		loading = true;
		lyrics = null;
		hasScrolled = false;
		userScrollUntil = 0;
		if (scroller) {
			scroller.scrollTo({ top: 0, behavior: 'instant' });
		}
		// Album isn't in now-playing, but the queue item usually has it — better LRCLIB matching.
		const album = playback.queue.items[playback.queue.currentIndex]?.album;
		api.getLyrics({
			videoId: id,
			title: now.title,
			artists: now.artists,
			album: album ?? undefined,
			// The track's own length — NOT playback.duration, which still holds the previous
			// track's value for a moment after a track change.
			duration: durationSecs(now.duration)
		})
			.then((l) => {
				if (requested !== id) return;
				lyrics = l;
				loading = false;
				hasScrolled = false; // first positioning on a new track is an instant jump
				userScrollUntil = 0;
				if (scroller) {
					scroller.scrollTo({ top: 0, behavior: 'instant' });
				}
			})
			.catch(() => {
				if (requested !== id) return;
				loading = false;
			});
	});

	// mpv's position arrives ~4x a second. Run a local clock forward from each one so the karaoke
	// sweep moves every frame instead of stepping four times a second.
	let interpolatedPosSecs = $state(playback.position);

	/** The rAF clock exists for the word sweep and nothing else. Unsynced lyrics have no cues, and
	 *  line-level-only lyrics move at most once a line, so both are served perfectly well by the
	 *  position tick they already get. Without this gate the loop ran at refresh rate for any
	 *  mounted lyrics panel, on every track, for the whole session, which meant the app never
	 *  reached an idle frame. */
	const needsFrameClock = $derived(
		!!lyrics?.synced && lyrics.lines.some((l) => (l.words?.length ?? 0) > 0)
	);

	$effect(() => {
		const pos = playback.position;
		if (playback.paused || !needsFrameClock) {
			interpolatedPosSecs = pos;
			return;
		}
		const base = pos;
		const baseAt = performance.now();
		interpolatedPosSecs = pos;
		let frameId = requestAnimationFrame(function tick() {
			interpolatedPosSecs = base + (performance.now() - baseAt) / 1000;
			frameId = requestAnimationFrame(tick);
		});
		return () => cancelAnimationFrame(frameId);
	});

	// Offset adjusted position in ms (BetterLyrics style offset sync)
	const posMs = $derived(interpolatedPosSecs * 1000 + lyricsSync.currentOffsetMs);

	// Last synced line whose cue has passed (lines arrive sorted by time).
	const activeIndex = $derived.by(() => {
		if (!lyrics?.synced) return -1;
		const currentMs = posMs;
		let i = -1;
		for (let j = 0; j < lyrics.lines.length; j++) {
			const t = lyrics.lines[j].time_ms;
			if (t === undefined) continue;
			if (t > currentMs) break;
			i = j;
		}
		return i;
	});

	// Auto-scroll pauses while the user is scrolling (wheel/touch/scrollbar), resumes after 3s.
	// Tracked via input events, not `scroll`, so our own smooth scrolls don't trip it.
	let userScrollUntil = 0;
	let hasScrolled = false;
	function onUserScroll() {
		userScrollUntil = Date.now() + 3000;
	}

	let wasExpanded: boolean | undefined;

	$effect(() => {
		const i = activeIndex;
		// Re-centre after the layout width/font changes, and jump rather than glide across it.
		// (Also fires on the first run, where both values are already at their defaults.)
		if (expanded !== wasExpanded) {
			wasExpanded = expanded;
			hasScrolled = false;
			userScrollUntil = 0;
		}
		if (i < 0) {
			if (!hasScrolled && scroller) {
				scroller.scrollTo({ top: 0, behavior: 'instant' });
			}
			return;
		}
		if (!scroller || Date.now() < userScrollUntil) return;
		if (i === 0) {
			scroller.scrollTo({
				top: 0,
				behavior: hasScrolled ? 'smooth' : 'instant'
			});
			hasScrolled = true;
			return;
		}
		const line = scroller.querySelector(`[data-line="${i}"]`);
		if (!line) return;
		const lineRect = line.getBoundingClientRect();
		const boxRect = scroller.getBoundingClientRect();
		scroller.scrollTo({
			top:
				scroller.scrollTop + (lineRect.top - boxRect.top) - (boxRect.height - lineRect.height) / 2,
			behavior: hasScrolled ? 'smooth' : 'instant'
		});
		hasScrolled = true;
	});

	function seekTo(line: api.LyricLine) {
		if (line.time_ms === undefined) return;
		const secs = line.time_ms / 1000;
		playback.position = secs; // optimistic — the mpv tick confirms
		userScrollUntil = 0; // jump the view along with the seek
		api.seek(secs);
	}

	function getWordProgress(word: api.LyricWord, currentMs: number): number {
		if (currentMs <= word.start_ms) return 0;
		if (currentMs >= word.end_ms) return 1;
		const dur = word.end_ms - word.start_ms;
		if (dur <= 0) return 1;
		return (currentMs - word.start_ms) / dur;
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -- handlers only detect scroll intent -->
<div
	bind:this={scroller}
	onwheel={onUserScroll}
	ontouchmove={onUserScroll}
	onpointerdown={onUserScroll}
	class="lyrics-scroller min-h-0 flex-1 overflow-y-auto {compact
		? 'px-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden'
		: expanded
			? 'px-10 py-6'
			: 'px-5 py-6'}"
>
	{#if loading}
		<div class="space-y-3">
			{#each { length: 8 } as _, i (i)}
				<div class="h-5 animate-pulse rounded bg-muted" style="width:{55 + ((i * 17) % 40)}%"></div>
			{/each}
		</div>
	{:else if lyrics && lyrics.synced}
		<!-- Padding lets the first/last lines center-scroll. -->
		<div class="py-[35vh] {expanded ? 'mx-auto max-w-3xl' : ''}">
			{#each lyrics.lines as line, i (i)}
				{@const isActive = i === activeIndex}
				{@const isPast = i < activeIndex}
				<button
					data-line={i}
					onclick={() => seekTo(line)}
					style="font-family: var(--font-lyrics, var(--font-heading, inherit));"
					class="group/lyric-line block w-full origin-left cursor-pointer text-left font-bold leading-snug transition-all duration-300 ease-out hover:text-foreground
						{expanded ? 'py-3.5 text-3xl sm:text-4xl' : compact ? 'py-1 text-sm' : 'py-2.5 text-xl'}
						{isActive
						? 'scale-[1.03] text-foreground opacity-100'
						: isPast
							? 'text-muted-foreground/45 opacity-60 blur-[0.3px] hover:blur-none hover:opacity-90'
							: 'text-muted-foreground/75 opacity-75 blur-[0.2px] hover:blur-none hover:opacity-100'}"
				>
					{#if line.words && line.words.length > 0}
						<!-- Word-by-Word Karaoke Sweep Animation (Glassy Turbo luminous syllable sweep) -->
						<span class="inline-flex flex-wrap items-baseline">
							{#each line.words as word, wIdx (wIdx)}
								{@const isWordEnd = word.text.endsWith(' ')}
								{@const cleanText = word.text.trimEnd()}
								{#if isActive}
									{@const progress = getWordProgress(word, posMs)}
									{@const pct = Math.round(Math.min(1, Math.max(0, progress)) * 100)}
									{@const isCurrentWord = progress > 0 && progress < 1}
									<span
										class="inline-block bg-clip-text text-transparent [-webkit-text-fill-color:transparent] transition-transform duration-100 ease-out {isWordEnd ? 'mr-[0.26em]' : ''} {isCurrentWord
											? 'scale-[1.04]'
											: ''}"
										style="background-image: linear-gradient(90deg, var(--foreground) {pct}%, var(--muted-foreground) {pct}%)"
									>
										{cleanText}
									</span>
								{:else}
									<span class="inline-block {isWordEnd ? 'mr-[0.26em]' : ''} {isPast ? 'text-muted-foreground/40' : 'text-muted-foreground/70'}">
										{cleanText}
									</span>
								{/if}
							{/each}
						</span>
					{:else}
						<span class="{isActive ? 'bg-gradient-to-r from-foreground via-foreground to-primary/80 bg-clip-text' : ''}">{line.text || '♪'}</span>
					{/if}

					<!-- Translation line rendering -->
					{#if line.translation}
						<p class="mt-1 text-sm font-normal italic tracking-wide opacity-80 transition-opacity">
							{line.translation}
						</p>
					{/if}
				</button>
			{/each}
		</div>
	{:else if lyrics}
		<div
			style="font-family: var(--font-lyrics, var(--font-heading, inherit));"
			class="space-y-2 leading-relaxed text-foreground/90 {expanded
				? 'mx-auto max-w-3xl text-xl'
				: compact
					? 'text-xs'
					: 'text-[15px]'}"
		>
			{#each lyrics.lines as line, i (i)}
				{#if line.text}
					<div>
						<p>{line.text}</p>
						{#if line.translation}
							<p class="text-xs italic text-muted-foreground">{line.translation}</p>
						{/if}
					</div>
				{:else}
					<div class="h-4"></div>
				{/if}
			{/each}
		</div>
	{:else}
		<div class="py-8 text-center flex flex-col items-center gap-2">
			<p class="text-sm text-muted-foreground">No lyrics found for this track.</p>
			{#if !loading && playback.now}
				<button
					onclick={() => (selectorOpen = true)}
					class="px-3 py-1.5 rounded-lg bg-muted/60 hover:bg-muted text-xs font-medium text-foreground transition-colors inline-flex items-center gap-1.5"
				>
					<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
					</svg>
					Search other lyric sources
				</button>
			{/if}
		</div>
	{/if}
</div>
{#if lyrics && !loading && !compact}
	<div class="flex items-center justify-between border-t border-border/40 px-4 py-2 text-xs text-muted-foreground">
		<div class="flex items-center gap-2">
			<span>{lyrics.source.startsWith('Source:') ? lyrics.source : `Lyrics from ${lyrics.source}`}</span>
			<button
				onclick={() => (selectorOpen = true)}
				class="hover:text-foreground inline-flex items-center gap-1 transition-colors px-1.5 py-0.5 rounded hover:bg-foreground/5 text-[11px]"
				title="Change lyrics / search other sources"
			>
				<svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
				</svg>
				Change source
			</button>
		</div>
		<LyricsSyncDock />
	</div>
{/if}

{#if playback.now}
	<LyricSelectorModal
		bind:open={selectorOpen}
		videoId={playback.now.videoId}
		initialTitle={playback.now.title}
		initialArtist={playback.now.artists}
		duration={durationSecs(playback.now.duration)}
		onApplied={(l) => {
			lyrics = l;
		}}
	/>
{/if}

