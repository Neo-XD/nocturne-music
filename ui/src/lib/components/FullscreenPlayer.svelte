<script lang="ts">
	import { onMount } from 'svelte';
	import { fade, fly, scale } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Cancel01Icon,
		FavouriteIcon,
		PlayIcon,
		PauseIcon,
		PreviousIcon,
		NextIcon,
		ShuffleIcon,
		RepeatIcon,
		RepeatOne01Icon,
		MusicNote01Icon,
		VolumeHighIcon,
		VolumeMute02Icon,
		Add01Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import {
		playback,
		np,
		toggleNowPlayingLike,
		openAddToPlaylist,
		wheelVolume,
		nudgeVolume
	} from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';
	import ArtistLine from './ArtistLine.svelte';
	import { Button } from '$lib/components/ui/button';

	// --- Lyrics Loading & Sync ---
	let lyrics = $state<api.Lyrics | null>(null);
	let loadingLyrics = $state(true);
	let scroller: HTMLElement | undefined = $state();
	let requestedId = '';

	function durationSecs(d?: string): number | undefined {
		if (!d) return undefined;
		const parts = d.split(':').map(Number);
		if (!parts.length || parts.some(Number.isNaN)) return undefined;
		return parts.reduce((a, b) => a * 60 + b, 0);
	}

	const fmt = (secs: number) => {
		if (!secs || secs < 0) return '0:00';
		const t = Math.floor(secs);
		const h = Math.floor(t / 3600);
		const m = Math.floor((t % 3600) / 60);
		const s = t % 60;
		const mm = h ? m.toString().padStart(2, '0') : `${m}`;
		return `${h ? `${h}:` : ''}${mm}:${s.toString().padStart(2, '0')}`;
	};

	$effect(() => {
		const now = playback.now;
		if (!now) {
			requestedId = '';
			lyrics = null;
			loadingLyrics = false;
			return;
		}
		if (now.videoId === requestedId) return;
		const id = (requestedId = now.videoId);
		loadingLyrics = true;
		lyrics = null;
		const album = playback.queue.items[playback.queue.currentIndex]?.album;
		api.getLyrics({
			videoId: id,
			title: now.title,
			artists: now.artists,
			album: album ?? undefined,
			duration: durationSecs(now.duration)
		})
			.then((l) => {
				if (requestedId !== id) return;
				lyrics = l;
				loadingLyrics = false;
				hasScrolled = false;
			})
			.catch(() => {
				if (requestedId !== id) return;
				loadingLyrics = false;
			});
	});

	// High-precision clock for 60fps karaoke word sweep
	let interpolatedPosSecs = $state(playback.position);

	$effect(() => {
		const pos = playback.position;
		if (playback.paused) {
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

	const posMs = $derived(interpolatedPosSecs * 1000);

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

	let userScrollUntil = 0;
	let hasScrolled = false;

	function onUserScroll() {
		userScrollUntil = Date.now() + 3000;
	}

	$effect(() => {
		const i = activeIndex;
		if (i < 0 || !scroller || Date.now() < userScrollUntil) return;
		scroller.querySelector(`[data-line="${i}"]`)?.scrollIntoView({
			behavior: hasScrolled ? 'smooth' : 'instant',
			block: 'center'
		});
		hasScrolled = true;
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

	// --- Transport Controls ---
	let seekDrag = $state<number | null>(null);
	const currentPos = $derived(seekDrag ?? playback.position);
	const durationNum = $derived(durationSecs(playback.now?.duration) ?? playback.duration);
	const repeat = $derived(playback.queue.repeat ?? 'off');

	function onSeekInput(e: Event) {
		seekDrag = Number((e.target as HTMLInputElement).value);
	}

	function onSeekCommit(e: Event) {
		const v = Number((e.target as HTMLInputElement).value);
		playback.position = v;
		seekDrag = null;
		api.seek(v);
	}

	function cycleRepeat() {
		const order: api.RepeatMode[] = ['off', 'all', 'one'];
		const next = order[(order.indexOf(repeat) + 1) % order.length];
		api.setRepeat(next);
	}

	function onKey(e: KeyboardEvent) {
		if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
		if (e.key === 'Escape') {
			e.preventDefault();
			np.fullscreenOpen = false;
		} else if (e.key === ' ' || e.code === 'Space') {
			e.preventDefault();
			api.togglePause();
		} else if (e.key === 'ArrowRight') {
			e.preventDefault();
			api.seek(Math.min(durationNum, playback.position + 5));
		} else if (e.key === 'ArrowLeft') {
			e.preventDefault();
			api.seek(Math.max(0, playback.position - 5));
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			nudgeVolume(5);
		} else if (e.key === 'ArrowDown') {
			e.preventDefault();
			nudgeVolume(-5);
		}
	}

	let justLiked = $state(false);
	function toggleLike() {
		justLiked = playback.rating !== 'like';
		toggleNowPlayingLike();
	}

	const bgSrc = $derived(thumb(playback.now?.thumbnail, 400));
	const coverSrc = $derived(thumb(playback.now?.thumbnail, 720));

	// This is intentionally the window's native fullscreen mode, not a larger in-app overlay. It
	// removes the desktop chrome and fills the monitor like a borderless game view; returning to
	// the normal player restores the state the window had before this view opened.
	onMount(() => {
		let windowWasFullscreen = false;
		let disposed = false;
		let appWindow: ReturnType<typeof getCurrentWindow> | null = null;
		try {
			appWindow = getCurrentWindow();
			appWindow
				.isFullscreen()
				.then((wasFullscreen) => {
					windowWasFullscreen = wasFullscreen;
					if (!wasFullscreen && !disposed) return appWindow?.setFullscreen(true);
				})
				.catch(() => {});
		} catch {
			// Browser preview: the in-app fullscreen layout still works without Tauri's window API.
		}
		return () => {
			disposed = true;
			if (!windowWasFullscreen) appWindow?.setFullscreen(false).catch(() => {});
		};
	});
</script>

<svelte:window onkeydown={onKey} />

<!-- Fullscreen Root Container: Covers entire screen, elevates above all chrome (z-50) -->
<div
	transition:fade={{ duration: 250 }}
	class="fixed inset-0 z-50 flex h-full w-full flex-col overflow-hidden bg-background text-foreground select-none"
>
	<!-- Blurred Background Album Art Wash (Seamless whole-window wash identical to NowPlaying) -->
	{#if coverSrc}
		<img
			src={coverSrc}
			alt=""
			class="pointer-events-none absolute inset-0 h-full w-full art-wash scale-125 object-cover opacity-30 blur-3xl dark:opacity-40"
		/>
	{/if}

	<!-- Top Floating Header Bar with generous top distance -->
	<header class="absolute top-0 inset-x-0 z-30 flex items-center justify-between px-8 pt-24 pb-4 sm:px-16 sm:pt-28 lg:px-24 lg:pt-32">
		<div class="flex items-center gap-3.5">
			<div class="flex h-9 w-9 items-center justify-center rounded-xl bg-foreground/10 text-foreground backdrop-blur-md shadow-sm">
				<HugeiconsIcon icon={MusicNote01Icon} class="h-4.5 w-4.5" />
			</div>
			<div class="truncate text-xs sm:text-sm font-semibold uppercase tracking-widest text-foreground/80 drop-shadow-sm">
				{playback.queue.sourceName || 'Nocturne Music'}
			</div>
		</div>

		<!-- Exit Fullscreen Button -->
		<Button
			variant="outline"
			size="sm"
			onclick={() => (np.fullscreenOpen = false)}
			aria-label="Exit Fullscreen (Esc)"
			class="gap-1.5 rounded-full border-border/70 bg-background/70 px-3 py-1.5 text-xs font-medium text-foreground backdrop-blur-md transition hover:bg-background cursor-pointer shadow-md"
		>
			<HugeiconsIcon icon={Cancel01Icon} class="h-3.5 w-3.5" />
			<span>Exit</span>
			<kbd class="ml-0.5 rounded bg-muted/80 px-1 py-0.5 text-[10px] text-muted-foreground">Esc</kbd>
		</Button>
	</header>

	<!-- Main Two-Column View with spacious top clearance -->
	<main class="relative z-10 mx-auto flex min-h-0 w-full max-w-7xl flex-1 items-center gap-16 px-8 pt-52 pb-6 sm:gap-20 sm:px-16 sm:pt-56 lg:gap-28 lg:px-24 lg:pt-60 xl:gap-32">
		<!-- Left Column: Album Artwork & Transport Controls (Middle-Left) -->
		<section
			class="relative flex w-full max-w-sm shrink-0 self-stretch flex-col items-center justify-center sm:max-w-md xl:max-w-[26rem] 2xl:max-w-[28rem]"
			onwheel={wheelVolume}
		>
			<!-- Artwork Card -->
			<div class="relative w-full max-h-[40vh] max-w-[17rem] sm:max-w-[21rem] lg:max-w-[24rem] aspect-square overflow-hidden rounded-3xl ring-1 ring-white/10 shadow-2xl transition-transform duration-300 hover:scale-[1.01]">
				{#if coverSrc}
					<img
						src={coverSrc}
						alt={playback.now?.title ?? 'Album Art'}
						class="h-full w-full object-cover"
						in:fade={{ duration: 250 }}
					/>
				{:else}
					<div class="flex h-full w-full items-center justify-center bg-muted text-muted-foreground/40">
						<HugeiconsIcon icon={MusicNote01Icon} class="h-24 w-24" />
					</div>
				{/if}
			</div>

			<!-- Track Info & Metadata -->
			<div class="mt-6 w-full text-left">
				<div class="flex items-center justify-between gap-4">
					<div class="min-w-0 flex-1">
						<h2 class="font-heading text-2xl sm:text-3xl font-extrabold tracking-tight text-foreground truncate">
							{playback.now?.title ?? 'Nothing playing'}
						</h2>
						<div class="mt-1 text-sm sm:text-base font-medium text-muted-foreground truncate">
							<ArtistLine
								runs={playback.now?.artistRuns}
								text={playback.now?.artists ?? ''}
								class="text-muted-foreground hover:text-foreground transition-colors"
							/>
						</div>
					</div>

					<!-- Like & Playlist Buttons -->
					{#if playback.now && !api.isLocalId(playback.now.videoId)}
						<div class="flex items-center gap-1 shrink-0">
							<Button
								variant="ghost"
								size="icon-sm"
								onclick={toggleLike}
								aria-label="Like track"
								class="hover:text-foreground cursor-pointer"
							>
								<span
									class="inline-flex"
									class:animate-heart-pop={justLiked}
									onanimationend={() => (justLiked = false)}
								>
									<HugeiconsIcon
										icon={FavouriteIcon}
										class="h-5 w-5 {playback.rating === 'like' ? 'fill-current text-primary' : 'text-muted-foreground'}"
									/>
								</span>
							</Button>
							<Button
								variant="ghost"
								size="icon-sm"
								onclick={() => {
									const now = playback.now!;
									openAddToPlaylist({
										video_id: now.videoId,
										title: now.title,
										artists: now.artists,
										artist_id: now.artistId,
										thumbnail: now.thumbnail,
										duration: now.duration
									});
								}}
								aria-label="Add to playlist"
								class="hover:text-foreground cursor-pointer"
							>
								<HugeiconsIcon icon={Add01Icon} class="h-5 w-5 text-muted-foreground" />
							</Button>
						</div>
					{/if}
				</div>

				<!-- Seek Bar -->
				<div class="mt-4 flex items-center gap-3">
					<span class="w-10 text-right font-mono text-xs text-muted-foreground">
						{fmt(currentPos)}
					</span>
					<div class="relative flex-1 group">
						<input
							type="range"
							min="0"
							max={durationNum || 100}
							value={currentPos}
							oninput={onSeekInput}
							onchange={onSeekCommit}
							aria-label="Seek position"
							class="h-1.5 w-full cursor-pointer appearance-none rounded-full bg-foreground/20 accent-foreground transition group-hover:h-2"
						/>
					</div>
					<span class="w-10 text-left font-mono text-xs text-muted-foreground">
						{playback.now?.duration ?? fmt(durationNum)}
					</span>
				</div>

				<!-- Playback Transport Row -->
				<div class="absolute inset-x-0 bottom-0 flex items-center justify-center gap-4 sm:gap-6">
					<Button
						variant="ghost"
						size="icon-sm"
						onclick={() => api.toggleShuffle()}
						class="text-muted-foreground hover:text-foreground cursor-pointer"
						aria-label="Shuffle"
					>
						<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4 {playback.queue.shuffle ? 'text-primary' : ''}" />
					</Button>
					<Button
						variant="ghost"
						size="icon"
						onclick={() => api.prevTrack()}
						class="text-foreground hover:text-primary cursor-pointer"
						aria-label="Previous track"
					>
						<HugeiconsIcon icon={PreviousIcon} class="h-6 w-6" />
					</Button>
					<button
						type="button"
						onclick={() => api.togglePause()}
						class="flex h-14 w-14 items-center justify-center rounded-full bg-foreground text-background shadow-lg transition hover:scale-105 active:scale-95 cursor-pointer"
						aria-label={playback.paused ? 'Play' : 'Pause'}
					>
						<HugeiconsIcon icon={playback.paused ? PlayIcon : PauseIcon} class="h-7 w-7 fill-current" />
					</button>
					<Button
						variant="ghost"
						size="icon"
						onclick={() => api.nextTrack()}
						class="text-foreground hover:text-primary cursor-pointer"
						aria-label="Next track"
					>
						<HugeiconsIcon icon={NextIcon} class="h-6 w-6" />
					</Button>
					<Button
						variant="ghost"
						size="icon-sm"
						onclick={cycleRepeat}
						class="text-muted-foreground hover:text-foreground cursor-pointer"
						aria-label="Repeat: {repeat}"
					>
						<HugeiconsIcon
							icon={repeat === 'one' ? RepeatOne01Icon : RepeatIcon}
							class="h-4 w-4 {repeat !== 'off' ? 'text-primary' : ''}"
						/>
					</Button>
				</div>
			</div>
		</section>

		<!-- Right Column: ONLY Lyrics (Clean, Immersive Live-Synced Stream) -->
		<section class="relative flex min-h-0 flex-1 h-[75vh] flex-col overflow-hidden [mask-image:linear-gradient(to_bottom,transparent_0%,black_10%,black_90%,transparent_100%)]">
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				bind:this={scroller}
				onwheel={onUserScroll}
				ontouchmove={onUserScroll}
				onpointerdown={onUserScroll}
				class="min-h-0 flex-1 overflow-y-auto px-4 lg:px-10 py-10 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
			>
				{#if loadingLyrics}
					<div class="space-y-6 py-20">
						{#each { length: 7 } as _, i (i)}
							<div
								class="h-8 animate-pulse rounded-lg bg-foreground/10"
								style="width:{50 + ((i * 23) % 45)}%"
							></div>
						{/each}
					</div>
				{:else if lyrics?.instrumental}
					<div class="flex h-full items-center justify-center">
						<p class="font-heading text-2xl lg:text-3xl font-bold tracking-wide text-muted-foreground/60">
							Instrumental ♪
						</p>
					</div>
				{:else if lyrics && lyrics.synced}
					<div class="py-[32vh] space-y-4">
						{#each lyrics.lines as line, i (i)}
							{@const isActive = i === activeIndex}
							{@const isPast = i < activeIndex}
							<button
								data-line={i}
								onclick={() => seekTo(line)}
								class="block w-full origin-left cursor-pointer text-left font-heading font-extrabold leading-snug transition-all duration-300 ease-out hover:text-foreground
									text-2xl sm:text-3xl lg:text-4xl
									{isActive
									? 'text-foreground opacity-100'
									: isPast
										? 'text-muted-foreground/35 opacity-50'
										: 'text-muted-foreground/75 opacity-75'}"
							>
								{#if line.words && line.words.length > 0}
									<span class="inline-flex flex-wrap items-baseline">
										{#each line.words as word, wIdx (wIdx)}
											{@const isWordEnd = word.text.endsWith(' ')}
											{@const cleanText = word.text.trimEnd()}
											{#if isActive}
												{@const progress = getWordProgress(word, posMs)}
												{@const pct = Math.round(Math.min(1, Math.max(0, progress)) * 100)}
												<span
													class="inline-block bg-clip-text text-transparent [-webkit-text-fill-color:transparent] transition-colors duration-100 ease-out {isWordEnd ? 'mr-[0.26em]' : ''}"
													style="background-image: linear-gradient(90deg, var(--foreground) {pct}%, var(--muted-foreground) {pct}%)"
												>
													{cleanText}
												</span>
											{:else}
												<span class="inline-block {isWordEnd ? 'mr-[0.26em]' : ''} {isPast ? 'text-muted-foreground/35' : 'text-muted-foreground/75'}">
													{cleanText}
												</span>
											{/if}
										{/each}
									</span>
								{:else}
									<span>{line.text || '♪'}</span>
								{/if}

								{#if line.translation}
									<p class="mt-2 text-base font-normal italic tracking-wide opacity-75">
										{line.translation}
									</p>
								{/if}
							</button>
						{/each}
					</div>
				{:else if lyrics}
					<!-- Plain text unsynced lyrics -->
					<div class="py-16 space-y-4 text-xl sm:text-2xl lg:text-3xl font-heading font-semibold text-foreground/80 leading-relaxed">
						{#each lyrics.lines as line, i (i)}
							{#if line.text}
								<div>
									<p>{line.text}</p>
									{#if line.translation}
										<p class="text-sm font-normal italic text-muted-foreground">{line.translation}</p>
									{/if}
								</div>
							{:else}
								<div class="h-6"></div>
							{/if}
						{/each}
					</div>
				{:else}
					<div class="flex h-full items-center justify-center">
						<p class="text-lg font-medium text-muted-foreground/60">No lyrics available for this track.</p>
					</div>
				{/if}
			</div>
		</section>
	</main>
</div>
