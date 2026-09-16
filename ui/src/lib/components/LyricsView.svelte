<script lang="ts">
	import * as api from '$lib/api';
	import {
		playback,
		lyricsSync,
		prefs,
		type LyricsAnimationStyle
	} from '$lib/player.svelte';
	import LyricsSyncDock from '$lib/components/LyricsSyncDock.svelte';
	import LyricSelectorModal from '$lib/components/LyricSelectorModal.svelte';
	import { listen } from '@tauri-apps/api/event';

	// `expanded` only sizes the type and centres the column. The owner of the extra room (the side
	// panel, or the now-playing view) decides how much there is. Toggling it must not remount this
	// component, or the lyrics refetch and the scroll position is lost.
	// `compact` is the mini-player: a ~220px column with no room for the source footer or a
	// scrollbar. It only shrinks the type and chrome; the sync/auto-scroll logic is identical.
	let {
		expanded = false,
		compact = false
	}: {
		expanded?: boolean;
		compact?: boolean;
	} = $props();

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
			duration: durationSecs(now.duration)
		})
			.then((l) => {
				if (requested !== id) return;
				lyrics = l;
				loading = false;
				hasScrolled = false;
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

	// Compute start and end times for every line for timing-accurate state and simultaneous lyrics
	const lineTimings = $derived.by(() => {
		const lines = lyrics?.lines;
		if (!lines) return [];
		return lines.map((line, j) => {
			const startMs = line.time_ms ?? 0;
			let endMs = line.end_time_ms;
			if (!endMs || endMs <= startMs) {
				if (line.words && line.words.length > 0) {
					const lastWord = line.words[line.words.length - 1];
					if (lastWord.end_ms && lastWord.end_ms > startMs) {
						endMs = lastWord.end_ms;
					}
				}
			}
			if (!endMs || endMs <= startMs) {
				const next = lines[j + 1];
				if (next?.time_ms !== undefined && next.time_ms > startMs) {
					endMs = next.time_ms;
				} else {
					endMs = startMs + 4000;
				}
			}
			return { startMs, endMs };
		});
	});

	// Linger duration based on chosen animation style
	const currentAnimStyle = $derived(prefs.lyricsAnimationStyle || 'wave');
	const lingerMs = $derived.by(() => {
		const s = currentAnimStyle;
		if (s === 'wave') return 180;
		if (s === 'apple') return 120;
		if (s === 'glow') return 200;
		if (s === 'fade') return 180;
		if (s === 'slide') return 140;
		return 0; // og, karaoke, none
	});

	// Active singing lines (supports simultaneous lyrics!)
	const activeIndices = $derived.by(() => {
		if (!lyrics?.synced || !lineTimings.length) return [];
		const currentMs = posMs;
		const result: number[] = [];

		for (let j = 0; j < lineTimings.length; j++) {
			const { startMs, endMs } = lineTimings[j];
			if (currentMs >= startMs && currentMs < endMs) {
				result.push(j);
			}
		}

		// If pos is in between lines, find last line that started
		if (result.length === 0) {
			let last = -1;
			for (let j = 0; j < lineTimings.length; j++) {
				if (lineTimings[j].startMs <= currentMs) {
					last = j;
				} else {
					break;
				}
			}
			if (last >= 0 && currentMs < lineTimings[last].endMs + lingerMs) {
				result.push(last);
			}
		}

		return result;
	});

	// Lingering lines (completed within lingerMs)
	const lingeredIndices = $derived.by(() => {
		if (!lyrics?.synced || lingerMs <= 0 || !lineTimings.length) return [];
		const currentMs = posMs;
		const result: number[] = [];
		for (let j = 0; j < lineTimings.length; j++) {
			const { endMs } = lineTimings[j];
			if (currentMs >= endMs && currentMs < endMs + lingerMs) {
				result.push(j);
			}
		}
		return result;
	});

	// Auto-scroll target: Only scroll to next lyric when the previous line is actually done!
	const scrollTargetIndex = $derived.by(() => {
		if (!lyrics?.synced || !lineTimings.length) return -1;
		const currentMs = posMs;

		if (activeIndices.length > 0) {
			// If active line(s) exist, find the first active line that is NOT yet done.
			// As long as the line is singing, keep focus on it!
			const firstNotDone = activeIndices.find((idx) => currentMs < lineTimings[idx].endMs);
			if (firstNotDone !== undefined) {
				return firstNotDone;
			}
			return activeIndices[activeIndices.length - 1];
		}

		// Gap between cues: stay on last finished line until next line is starting
		let lastDone = -1;
		for (let j = 0; j < lineTimings.length; j++) {
			if (currentMs >= lineTimings[j].endMs) {
				lastDone = j;
			} else {
				break;
			}
		}
		if (lastDone + 1 < lineTimings.length) {
			if (currentMs >= lineTimings[lastDone + 1].startMs - 350) {
				return lastDone + 1;
			}
		}
		return lastDone;
	});

	// Auto-scroll pauses while the user is scrolling (wheel/touch/scrollbar), resumes after 3s.
	let userScrollUntil = 0;
	let hasScrolled = false;
	function onUserScroll() {
		userScrollUntil = Date.now() + 3000;
	}

	let wasExpanded: boolean | undefined;

	$effect(() => {
		const i = scrollTargetIndex;
		if (expanded !== wasExpanded) {
			wasExpanded = expanded;
			hasScrolled = false;
			userScrollUntil = 0;
		}
		if (!scroller) return;
		if (i < 0) {
			if (!hasScrolled) {
				// Lyrics loaded but no active cue yet (lead-in silence) — scroll to first line
				// so the user can read ahead rather than staring at a blank top.
				const firstLine = lyrics?.lines?.length ? scroller.querySelector('[data-line="0"]') : null;
				if (firstLine) {
					const lineRect = firstLine.getBoundingClientRect();
					const boxRect = scroller.getBoundingClientRect();
					scroller.scrollTo({
						top: scroller.scrollTop + (lineRect.top - boxRect.top) - (boxRect.height - lineRect.height) / 2,
						behavior: 'instant'
					});
				} else {
					scroller.scrollTo({ top: 0, behavior: 'instant' });
				}
			}
			return;
		}
		if (Date.now() < userScrollUntil) return;
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

	// Autoscroll for unsynced (plain) lyrics based on playback progress
	const durationNum = $derived(playback.duration || durationSecs(playback.now?.duration) || 0);
	const unsyncedProgress = $derived(
		durationNum > 6 ? Math.min(Math.max((playback.position - 2) / (durationNum - 4), 0), 1) : 0
	);

	$effect(() => {
		if (lyrics?.synced || !lyrics || !scroller || loading || Date.now() < userScrollUntil) return;
		const progress = unsyncedProgress;
		const maxScroll = scroller.scrollHeight - scroller.clientHeight;
		if (maxScroll > 0) {
			scroller.scrollTo({
				top: progress * maxScroll,
				behavior: 'smooth'
			});
		}
	});

	function seekTo(line: api.LyricLine) {
		if (line.time_ms === undefined) return;
		const secs = line.time_ms / 1000;
		playback.position = secs;
		userScrollUntil = 0;
		api.seek(secs);
	}

	function getWordProgress(word: api.LyricWord, currentMs: number): number {
		if (currentMs <= word.start_ms) return 0;
		if (currentMs >= word.end_ms) return 1;
		const dur = word.end_ms - word.start_ms;
		if (dur <= 0) return 1;
		return (currentMs - word.start_ms) / dur;
	}

	function getWordRenderInfo(word: api.LyricWord, currentMs: number, style: LyricsAnimationStyle) {
		const progress = getWordProgress(word, currentMs);
		const isCurrent = progress > 0 && progress < 1;
		const isFinished = progress >= 1;

		if (style === 'wave') {
			const pct = Math.min(100, Math.max(0, Math.round(progress * 100)));
			const tail = Math.min(100, pct + 14);
			return {
				bg: `linear-gradient(90deg, var(--foreground) 0%, var(--foreground) ${pct}%, color-mix(in srgb, var(--foreground) 75%, var(--primary, #6366f1)) ${Math.min(100, pct + 4)}%, var(--muted-foreground) ${tail}%, var(--muted-foreground) 100%)`,
				cls: isCurrent ? 'scale-[1.05] drop-shadow-[0_0_12px_rgba(255,255,255,0.45)]' : ''
			};
		}
		if (style === 'apple') {
			const smooth = progress * progress * (3 - 2 * progress);
			const pct = Math.round(smooth * 100);
			return {
				bg: `linear-gradient(90deg, #ffffff 0%, #ffffff ${pct}%, rgba(255,255,255,0.38) ${pct}%, rgba(255,255,255,0.38) 100%)`,
				cls: isCurrent ? 'scale-[1.035] drop-shadow-[0_0_14px_rgba(255,255,255,0.6)] font-extrabold' : ''
			};
		}
		if (style === 'karaoke') {
			const pct = Math.min(100, Math.max(0, Math.round(progress * 100)));
			return {
				bg: `linear-gradient(90deg, var(--primary) 0%, var(--primary) ${pct}%, var(--muted-foreground) ${pct}%, var(--muted-foreground) 100%)`,
				cls: isCurrent ? 'scale-[1.03] drop-shadow-[0_0_12px_var(--primary)] font-black text-primary' : ''
			};
		}
		if (style === 'glow') {
			const pct = Math.min(100, Math.max(0, Math.round(progress * 100)));
			return {
				bg: `linear-gradient(90deg, var(--foreground) 0%, var(--primary) ${pct}%, var(--muted-foreground) ${Math.min(100, pct + 15)}%, var(--muted-foreground) 100%)`,
				cls: isCurrent ? 'scale-[1.06] drop-shadow-[0_0_22px_var(--primary)] font-extrabold' : ''
			};
		}
		if (style === 'slide') {
			const pct = Math.min(100, Math.max(0, Math.round(progress * 100)));
			return {
				bg: `linear-gradient(90deg, var(--foreground) 0%, var(--primary) ${pct}%, var(--muted-foreground) ${pct}%, var(--muted-foreground) 100%)`,
				cls: isCurrent ? 'scale-[1.03] translate-x-0.5 drop-shadow-[0_0_12px_var(--primary)] font-extrabold' : ''
			};
		}
		if (style === 'fade') {
			const pct = Math.min(100, Math.max(0, Math.round(progress * 100)));
			return {
				bg: `linear-gradient(90deg, var(--foreground) 0%, var(--foreground) ${pct}%, var(--muted-foreground) ${Math.min(100, pct + 8)}%, var(--muted-foreground) 100%)`,
				cls: isCurrent ? 'drop-shadow-[0_0_10px_rgba(255,255,255,0.3)] font-bold' : ''
			};
		}
		if (style === 'og') {
			const pct = Math.min(100, Math.max(0, Math.round(progress * 100)));
			return {
				bg: `linear-gradient(90deg, var(--foreground) ${pct}%, var(--muted-foreground) ${pct}%)`,
				cls: isCurrent ? 'scale-[1.03] font-bold' : ''
			};
		}
		// 'none' / static
		const solid = isFinished || isCurrent ? 100 : 0;
		return {
			bg: `linear-gradient(90deg, var(--foreground) ${solid}%, var(--muted-foreground) ${solid}%)`,
			cls: ''
		};
	}

	function getLineFallbackClass(isHighlight: boolean, style: LyricsAnimationStyle): string {
		if (!isHighlight) return '';
		if (style === 'wave') return 'bg-gradient-to-r from-foreground via-foreground to-primary/85 bg-clip-text text-transparent drop-shadow-[0_0_16px_rgba(var(--color-primary-rgb,99,102,241),0.4)]';
		if (style === 'apple') return 'text-foreground drop-shadow-[0_0_16px_rgba(255,255,255,0.35)]';
		if (style === 'karaoke') return 'text-primary drop-shadow-[0_0_14px_rgba(var(--color-primary-rgb,99,102,241),0.5)]';
		if (style === 'glow') return 'text-foreground drop-shadow-[0_0_24px_var(--primary)]';
		if (style === 'slide') return 'translate-x-2 text-foreground drop-shadow-[0_0_12px_rgba(var(--color-primary-rgb,99,102,241),0.3)]';
		if (style === 'fade') return 'text-foreground drop-shadow-[0_0_8px_rgba(255,255,255,0.2)]';
		if (style === 'og') return 'text-foreground font-bold drop-shadow-[0_0_10px_rgba(255,255,255,0.25)]';
		return 'text-foreground font-bold';
	}

	function getLineClasses(isHighlight: boolean, isPast: boolean, dist: number, style: LyricsAnimationStyle): string {
		if (isHighlight) {
			if (style === 'wave') return 'scale-[1.045] -translate-y-0.5 text-foreground font-extrabold opacity-100 blur-0 drop-shadow-[0_0_18px_rgba(var(--color-primary-rgb,99,102,241),0.35)]';
			if (style === 'apple') return 'scale-[1.035] -translate-y-0.5 text-foreground font-bold opacity-100 blur-0 drop-shadow-[0_0_20px_rgba(255,255,255,0.35)]';
			if (style === 'karaoke') return 'scale-[1.02] translate-y-0 text-primary font-extrabold opacity-100 blur-0 drop-shadow-[0_0_14px_rgba(var(--color-primary-rgb,99,102,241),0.5)]';
			if (style === 'glow') return 'scale-[1.04] -translate-y-0.5 text-foreground font-extrabold opacity-100 blur-0 drop-shadow-[0_0_24px_var(--primary)]';
			if (style === 'slide') return 'scale-[1.02] translate-x-2 text-foreground font-extrabold opacity-100 blur-0 drop-shadow-[0_0_14px_rgba(var(--color-primary-rgb,99,102,241),0.35)]';
			if (style === 'fade') return 'scale-100 translate-y-0 text-foreground font-bold opacity-100 blur-0 drop-shadow-[0_0_8px_rgba(255,255,255,0.2)]';
			if (style === 'og') return 'scale-[1.03] translate-y-0 text-foreground font-bold opacity-100 blur-0 drop-shadow-[0_0_10px_rgba(255,255,255,0.25)]';
			return 'scale-100 translate-y-0 text-foreground font-bold opacity-100 blur-0';
		}

		if (isPast) {
			if (style === 'wave') {
				if (dist === 1) return 'scale-100 translate-y-0 font-bold text-muted-foreground/80 opacity-75 blur-0 hover:blur-none hover:opacity-100';
				if (dist === 2) return 'scale-100 translate-y-0 font-bold text-muted-foreground/60 opacity-50 blur-[0.6px] hover:blur-none hover:opacity-90';
				if (dist === 3) return 'scale-100 translate-y-0 font-semibold text-muted-foreground/45 opacity-30 blur-[1.4px] hover:blur-none hover:opacity-80';
				return 'scale-100 translate-y-0 font-medium text-muted-foreground/35 opacity-20 blur-[2.4px] hover:blur-none hover:opacity-75';
			}
			if (style === 'apple') {
				if (dist === 1) return 'scale-100 translate-y-0 font-semibold text-muted-foreground/75 opacity-70 blur-0 hover:opacity-95';
				if (dist === 2) return 'scale-100 translate-y-0 font-medium text-muted-foreground/50 opacity-45 blur-[0.4px] hover:blur-none hover:opacity-90';
				return 'scale-100 translate-y-0 font-normal text-muted-foreground/35 opacity-25 blur-[0.8px] hover:blur-none hover:opacity-80';
			}
			if (style === 'karaoke') {
				return 'scale-100 translate-y-0 font-medium text-muted-foreground/45 opacity-45 blur-0 hover:opacity-85';
			}
			if (style === 'glow') {
				if (dist === 1) return 'scale-100 translate-y-0 font-bold text-muted-foreground/70 opacity-65 blur-0 hover:opacity-90';
				return 'scale-100 translate-y-0 font-medium text-muted-foreground/40 opacity-30 blur-[1px] hover:blur-none hover:opacity-80';
			}
			if (style === 'slide') {
				return 'translate-x-0 font-medium text-muted-foreground/50 opacity-40 blur-0 hover:opacity-85';
			}
			if (style === 'fade') {
				if (dist === 1) return 'scale-100 translate-y-0 font-semibold text-muted-foreground/75 opacity-65 blur-0 hover:opacity-90';
				return 'scale-100 translate-y-0 font-medium text-muted-foreground/40 opacity-35 blur-0 hover:opacity-80';
			}
			if (style === 'og') {
				return 'scale-100 translate-y-0 font-semibold text-muted-foreground/60 opacity-60 blur-0 hover:opacity-90';
			}
			return 'scale-100 translate-y-0 font-normal text-muted-foreground/40 opacity-40 blur-0 hover:opacity-80';
		}

		// Upcoming lines
		if (style === 'wave') {
			if (dist === 1) return 'scale-100 translate-y-0 font-bold text-muted-foreground/80 opacity-75 blur-0 hover:blur-none hover:opacity-100';
			if (dist === 2) return 'scale-100 translate-y-0 font-bold text-muted-foreground/60 opacity-50 blur-[0.6px] hover:blur-none hover:opacity-90';
			if (dist === 3) return 'scale-100 translate-y-0 font-semibold text-muted-foreground/45 opacity-30 blur-[1.4px] hover:blur-none hover:opacity-80';
			return 'scale-100 translate-y-0 font-medium text-muted-foreground/35 opacity-20 blur-[2.4px] hover:blur-none hover:opacity-75';
		}
		if (style === 'apple') {
			if (dist === 1) return 'scale-100 translate-y-0 font-semibold text-muted-foreground/80 opacity-75 blur-0 hover:opacity-100';
			if (dist === 2) return 'scale-100 translate-y-0 font-medium text-muted-foreground/55 opacity-50 blur-[0.4px] hover:blur-none hover:opacity-90';
			return 'scale-100 translate-y-0 font-normal text-muted-foreground/40 opacity-30 blur-[0.8px] hover:blur-none hover:opacity-80';
		}
		if (style === 'karaoke') {
			return 'scale-100 translate-y-0 font-medium text-muted-foreground/65 opacity-65 blur-0 hover:opacity-90';
		}
		if (style === 'glow') {
			if (dist === 1) return 'scale-100 translate-y-0 font-bold text-muted-foreground/75 opacity-70 blur-0 hover:opacity-95';
			return 'scale-100 translate-y-0 font-medium text-muted-foreground/45 opacity-35 blur-[1px] hover:blur-none hover:opacity-85';
		}
		if (style === 'slide') {
			return 'translate-x-0 font-medium text-muted-foreground/60 opacity-55 blur-0 hover:opacity-90';
		}
		if (style === 'fade') {
			if (dist === 1) return 'scale-100 translate-y-0 font-semibold text-muted-foreground/80 opacity-70 blur-0 hover:opacity-95';
			return 'scale-100 translate-y-0 font-medium text-muted-foreground/45 opacity-40 blur-0 hover:opacity-85';
		}
		if (style === 'og') {
			return 'scale-100 translate-y-0 font-semibold text-muted-foreground/75 opacity-75 blur-0 hover:opacity-100';
		}
		return 'scale-100 translate-y-0 font-normal text-muted-foreground/55 opacity-55 blur-0 hover:opacity-85';
	}
</script>

<div class="relative flex min-h-0 flex-1 flex-col overflow-hidden">


	<!-- svelte-ignore a11y_no_static_element_interactions -- handlers only detect scroll intent -->
	<div
		bind:this={scroller}
		onwheel={onUserScroll}
		ontouchmove={onUserScroll}
		onpointerdown={onUserScroll}
		class="lyrics-scroller min-h-0 flex-1 overflow-y-auto {compact
			? 'px-3 py-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden'
			: expanded
				? 'px-10 py-6'
				: 'px-5 py-4'}"
		style="mask-image: linear-gradient(to bottom, transparent 0%, black 6%, black 94%, transparent 100%); -webkit-mask-image: linear-gradient(to bottom, transparent 0%, black 6%, black 94%, transparent 100%);"
	>
		{#if loading}
			<div class="space-y-3">
				{#each { length: 8 } as _, i (i)}
					<div class="h-5 animate-pulse rounded bg-muted" style="width:{55 + ((i * 17) % 40)}%"></div>
				{/each}
			</div>
		{:else if lyrics && lyrics.synced}
			<!-- BetterLyrics-style dynamic vertical spacing: tight padding on sidebar/compact, expansive on fullscreen -->
			<div class="{compact ? 'py-4' : expanded ? 'py-[25vh] mx-auto max-w-3xl' : 'py-8'}">
				{#each lyrics.lines as line, i (i)}
					{@const timing = lineTimings[i]}
					{@const isActive = activeIndices.includes(i)}
					{@const isLingered = lingeredIndices.includes(i)}
					{@const isHighlight = isActive || isLingered}
					{@const isPast = timing ? posMs >= timing.endMs + lingerMs : false}
					{@const dist = isHighlight
						? 0
						: activeIndices.length > 0
							? Math.min(...activeIndices.map((a) => Math.abs(i - a)))
							: scrollTargetIndex >= 0
								? Math.abs(i - scrollTargetIndex)
								: 99}
					<button
						data-line={i}
						onclick={() => seekTo(line)}
						style="font-family: var(--font-lyrics, var(--font-heading, inherit));"
						class="group/lyric-line block w-full origin-left cursor-pointer text-left leading-snug transition-[transform,opacity,filter] duration-300 ease-out hover:text-foreground
							{expanded ? 'py-3.5 text-3xl sm:text-4xl' : compact ? 'py-1 text-sm' : 'py-2.5 text-xl'}
							{getLineClasses(isHighlight, isPast, dist, currentAnimStyle)}"
					>
						{#if line.words && line.words.length > 0}
							<!-- Word-by-Word Animation Sweep with selectable styles -->
							<span class="inline-flex flex-wrap items-baseline">
								{#each line.words as word, wIdx (wIdx)}
									{@const isWordEnd = word.text.endsWith(' ')}
									{@const cleanText = word.text.trimEnd()}
									{#if isHighlight}
										{@const renderInfo = getWordRenderInfo(word, posMs, currentAnimStyle)}
										<span
											class="inline-block bg-clip-text text-transparent [-webkit-text-fill-color:transparent] transition-transform duration-100 ease-out {isWordEnd
												? 'mr-[0.26em]'
												: ''} {renderInfo.cls}"
											style="background-image: {renderInfo.bg}"
										>
											{cleanText}
										</span>
									{:else}
										<span
											class="inline-block {isWordEnd ? 'mr-[0.26em]' : ''} {isPast
												? 'text-muted-foreground/40'
												: 'text-muted-foreground/70'}"
										>
											{cleanText}
										</span>
									{/if}
								{/each}
							</span>
						{:else}
							<span class="{getLineFallbackClass(isHighlight, currentAnimStyle)}">
								{line.text || '♪'}
							</span>
						{/if}

						<!-- Translation line rendering -->
						{#if line.translation}
							<p
								class="mt-1 text-sm font-normal italic tracking-wide transition-opacity {isHighlight
									? 'text-foreground/80 opacity-90'
									: 'text-muted-foreground/50 opacity-60'}"
							>
								{line.translation}
							</p>
						{/if}
					</button>
				{/each}
			</div>
		{:else if lyrics}
			<div
				style="font-family: var(--font-lyrics, var(--font-heading, inherit));"
				class="space-y-3 leading-relaxed text-foreground/90 {expanded
					? 'mx-auto max-w-3xl text-2xl py-12 space-y-5 font-bold'
					: compact
						? 'text-xs py-2'
						: 'text-[16px] py-4 font-semibold'}"
			>
				{#each lyrics.lines as line, i (i)}
					{#if line.text}
						<div class="transition-opacity hover:opacity-100 opacity-80">
							<p>{line.text}</p>
							{#if line.translation}
								<p class="text-xs italic text-muted-foreground mt-1">{line.translation}</p>
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
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
							/>
						</svg>
						Search other lyric sources
					</button>
				{/if}
			</div>
		{/if}
	</div>

	<!-- Bottom Dock / Toolbar -->
	{#if lyrics && !loading && !compact}
		<div class="flex items-center justify-between border-t border-border/40 px-4 py-2 text-xs text-muted-foreground">
			<div class="flex items-center gap-2.5">
				<span>{lyrics.source.startsWith('Source:') ? lyrics.source : `Lyrics from ${lyrics.source}`}</span>
				<button
					onclick={() => (selectorOpen = true)}
					class="hover:text-foreground inline-flex items-center gap-1 transition-colors px-1.5 py-0.5 rounded hover:bg-foreground/5 text-[11px]"
					title="Change lyrics / search other sources"
				>
					<svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width="2"
							d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
						/>
					</svg>
					Change source
				</button>
			</div>
			<LyricsSyncDock />
		</div>
	{/if}
</div>

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

