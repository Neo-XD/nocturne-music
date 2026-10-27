<script lang="ts">
	import { fade, fly } from 'svelte/transition';
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
		Add01Icon,
		Mic01Icon,
		Queue01Icon,
		VolumeHighIcon,
		VolumeMute02Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { SongItem } from '$lib/api';
	import {
		playback,
		prefs,
		np,
		toggleNowPlayingLike,
		openAddToPlaylist,
		wheelVolume,
		nudgeVolume,
		dragVolume,
		commitVolume,
		toggleMute
	} from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';
	import { getSongMotionArtwork } from '$lib/canvas';
	import ExplicitIcon from './ExplicitIcon.svelte';
	import ArtistLine from './ArtistLine.svelte';
	import { Button } from '$lib/components/ui/button';
	import AnimatedArtwork from './AnimatedArtwork.svelte';
	import WaveformSeekbar from './WaveformSeekbar.svelte';
	import { appearance } from '$lib/theme.svelte';
	import LyricsView from './LyricsView.svelte';
	import QueueList from './QueueList.svelte';

	let activeTab = $state<'lyrics' | 'queue'>('lyrics');
	// Default to false for proper fullscreen theater view
	let userShowPanel = $state(false);
	let volDragging = $state(false);

	// Limusic v1.3.0 Theater auto-fading controls dock
	let controlsVisible = $state(true);
	let mouseTimer: any = null;

	function onUserActivity() {
		controlsVisible = true;
		if (mouseTimer) clearTimeout(mouseTimer);
		if (!playback.paused) {
			mouseTimer = setTimeout(() => {
				if (!playback.paused && !volDragging && seekDrag === null) {
					controlsVisible = false;
				}
			}, 3500);
		}
	}

	$effect(() => {
		if (playback.paused) {
			controlsVisible = true;
			if (mouseTimer) clearTimeout(mouseTimer);
		}
	});

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

	// --- Transport Controls ---
	let seekDrag = $state<number | null>(null);
	const currentPos = $derived(seekDrag ?? playback.position);
	const durationNum = $derived(durationSecs(playback.now?.duration) ?? playback.duration);
	const progressPct = $derived(
		durationNum > 0 ? Math.min(100, Math.max(0, (currentPos / durationNum) * 100)) : 0
	);
	const repeat = $derived(playback.queue.repeat ?? 'off');

	// --- Queue Items & Up Next Preview (Limusic v1.3.0) ---
	const currentSong = $derived.by(() => {
		const cur = playback.queue.items[playback.queue.currentIndex];
		return cur?.video_id === playback.now?.videoId ? cur : null;
	});

	const nextSong = $derived.by<SongItem | null>(() => {
		const items = playback.queue.items;
		const nextIndex = playback.queue.currentIndex + 1;
		if (nextIndex >= 0 && nextIndex < items.length) {
			return items[nextIndex];
		}
		return null;
	});

	const remainingSecs = $derived(Math.max(0, durationNum - currentPos));
	const showUpNext = $derived(
		!userShowPanel &&
		remainingSecs > 0 &&
		remainingSecs <= 22 &&
		nextSong !== null &&
		!playback.paused
	);

	// --- Animated Album Art (Canvases / Motion Video) ---
	let canvasUrl = $state<string | null>(null);
	let canvasReady = $state(false);
	let canvasVideoEl = $state<HTMLVideoElement | null>(null);
	let theaterCanvasVideoEl = $state<HTMLVideoElement | null>(null);
	let ambientCanvasVideoEl = $state<HTMLVideoElement | null>(null);

	$effect(() => {
		const now = playback.now;
		canvasUrl = null;
		canvasReady = false;
		if (!now || !prefs.animatedAlbumArt) return;

		let cancelled = false;
		const curSong = currentSong;
		getSongMotionArtwork(now.title, now.artists, curSong?.album ?? undefined)
			.then((u) => {
				if (!cancelled && u) {
					canvasUrl = u;
				}
			})
			.catch(() => {});

		return () => {
			cancelled = true;
		};
	});

	$effect(() => {
		if (canvasVideoEl) {
			if (playback.paused) {
				canvasVideoEl.pause();
			} else if (canvasUrl) {
				canvasVideoEl.play().catch(() => {});
			}
		}
		if (theaterCanvasVideoEl) {
			if (playback.paused) {
				theaterCanvasVideoEl.pause();
			} else if (canvasUrl) {
				theaterCanvasVideoEl.play().catch(() => {});
			}
		}
		if (ambientCanvasVideoEl) {
			if (playback.paused) {
				ambientCanvasVideoEl.pause();
			} else if (canvasUrl) {
				ambientCanvasVideoEl.play().catch(() => {});
			}
		}
	});

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
		onUserActivity();
		if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
		if (e.key === 'Escape' || e.key.toLowerCase() === 'f' || e.key === 'F11') {
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
		} else if (e.key.toLowerCase() === 'l') {
			e.preventDefault();
			if (userShowPanel && activeTab === 'lyrics') {
				userShowPanel = false;
			} else {
				activeTab = 'lyrics';
				userShowPanel = true;
			}
		} else if (e.key.toLowerCase() === 'q') {
			e.preventDefault();
			if (userShowPanel && activeTab === 'queue') {
				userShowPanel = false;
			} else {
				activeTab = 'queue';
				userShowPanel = true;
			}
		}
	}

	let justLiked = $state(false);
	function toggleLike() {
		justLiked = playback.rating !== 'like';
		toggleNowPlayingLike();
	}

	const coverSrc = $derived(thumb(playback.now?.thumbnail, 1080));
</script>

<svelte:window onkeydown={onKey} />

<!-- Fullscreen Root Container: Covers whole screen at z-[90] -->
<div
	class="fixed inset-0 z-[90] flex flex-col select-none overflow-hidden bg-background text-foreground animate-in fade-in-0 duration-300 {!userShowPanel && (!controlsVisible && !playback.paused) ? 'cursor-none' : ''}"
	onmousemove={onUserActivity}
	onpointerdown={onUserActivity}
	tabindex="-1"
>
	<!-- Dynamic Background: Canvas Motion OR Shaders OR Static Cover Wash -->
	{#if prefs.animatedAlbumArt && canvasUrl}
		<!-- svelte-ignore a11y_media_has_caption -->
		<video
			bind:this={ambientCanvasVideoEl}
			src={canvasUrl}
			autoplay
			loop
			muted
			playsinline
			class="pointer-events-none absolute inset-0 h-full w-full scale-110 object-cover"
			style="opacity: {appearance.fullscreenLightness}; filter: blur({appearance.fullscreenBlur}px) saturate({Math.round(appearance.fullscreenSaturation * 100)}%);"
		></video>
	{:else if coverSrc}
		{#if prefs.animatedArtwork}
			<AnimatedArtwork
				src={coverSrc}
				class="pointer-events-none absolute inset-0 h-full w-full object-cover"
				style="opacity: {appearance.fullscreenLightness}; filter: blur({appearance.fullscreenBlur}px) saturate({Math.round(appearance.fullscreenSaturation * 100)}%);"
				intensity={appearance.fullscreenWarp}
				speed={appearance.fullscreenSpeed}
			/>
		{:else}
			<img
				src={coverSrc}
				alt=""
				class="pointer-events-none absolute inset-0 h-full w-full scale-110 object-cover"
				style="opacity: {appearance.fullscreenLightness}; filter: blur({appearance.fullscreenBlur}px) saturate({Math.round(appearance.fullscreenSaturation * 100)}%);"
			/>
		{/if}
	{/if}

	<!-- Ambient User Theme Tint (Subtle glow matching active primary accent) -->
	<div
		class="pointer-events-none absolute inset-0 opacity-20 transition-opacity duration-500"
		style="background: radial-gradient(120% 120% at 50% 15%, color-mix(in srgb, var(--primary) 45%, transparent) 0%, transparent 65%), radial-gradient(90% 90% at 85% 85%, color-mix(in srgb, var(--primary) 25%, transparent) 0%, transparent 70%);"
	></div>

	<!-- Top Header Bar (With generous top padding from screen edge) -->
	<header
		class="relative z-20 flex shrink-0 items-center justify-between px-8 pt-16 pb-6 sm:px-14 sm:pt-20 xl:px-20 xl:pt-24 transition-opacity duration-500 {!userShowPanel && (!controlsVisible && !playback.paused) ? 'opacity-0 pointer-events-none' : 'opacity-100 pointer-events-auto'}"
	>
		<div class="flex items-center gap-3.5 min-w-0">
			<div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-foreground/10 text-foreground backdrop-blur-md shadow-sm border border-white/10">
				<HugeiconsIcon icon={MusicNote01Icon} class="h-4.5 w-4.5" />
			</div>
			<div class="truncate text-xs sm:text-sm font-semibold uppercase tracking-widest text-foreground/80 drop-shadow-sm">
				{playback.queue.sourceName || 'Nocturne Music'}
			</div>
		</div>

		<!-- Top Right Action Cluster (Lyrics / Queue Switcher + Exit) -->
		<div class="flex items-center gap-3 shrink-0">
			<!-- Translucent glass switcher between Lyrics and Queue -->
			<div class="flex items-center rounded-[calc(var(--radius,0.45rem)+4px)] border border-white/20 dark:border-white/10 bg-white/10 dark:bg-black/25 p-0.5 backdrop-blur-xl shadow-lg">
				<button
					type="button"
					onclick={() => {
						if (activeTab === 'lyrics' && userShowPanel) {
							userShowPanel = false;
						} else {
							activeTab = 'lyrics';
							userShowPanel = true;
						}
					}}
					aria-label="Toggle Lyrics (L)"
					class="flex items-center gap-1.5 rounded-[var(--radius,0.45rem)] px-3 py-1.5 text-xs font-medium transition-all duration-200 cursor-pointer {userShowPanel && activeTab === 'lyrics'
						? 'bg-white/25 dark:bg-white/15 text-foreground shadow-xs font-semibold'
						: 'text-foreground/70 hover:text-foreground hover:bg-white/10'}"
				>
					<HugeiconsIcon icon={Mic01Icon} class="h-3.5 w-3.5 {userShowPanel && activeTab === 'lyrics' ? 'text-primary' : ''}" />
					<span>Lyrics</span>
				</button>
				<button
					type="button"
					onclick={() => {
						if (activeTab === 'queue' && userShowPanel) {
							userShowPanel = false;
						} else {
							activeTab = 'queue';
							userShowPanel = true;
						}
					}}
					aria-label="Toggle Queue (Q)"
					class="flex items-center gap-1.5 rounded-[var(--radius,0.45rem)] px-3 py-1.5 text-xs font-medium transition-all duration-200 cursor-pointer {userShowPanel && activeTab === 'queue'
						? 'bg-white/25 dark:bg-white/15 text-foreground shadow-xs font-semibold'
						: 'text-foreground/70 hover:text-foreground hover:bg-white/10'}"
				>
					<HugeiconsIcon icon={Queue01Icon} class="h-3.5 w-3.5 {userShowPanel && activeTab === 'queue' ? 'text-primary' : ''}" />
					<span>Queue</span>
				</button>
			</div>

			<Button
				variant="outline"
				size="sm"
				onclick={() => (np.fullscreenOpen = false)}
				aria-label="Exit Fullscreen (Esc)"
				class="gap-1.5 rounded-[var(--radius,0.45rem)] border-white/20 dark:border-white/10 bg-white/10 dark:bg-black/25 px-3.5 py-1.5 text-xs font-medium text-foreground backdrop-blur-xl transition-all duration-200 hover:bg-white/20 dark:hover:bg-black/40 hover:scale-[1.02] cursor-pointer shadow-lg"
			>
				<HugeiconsIcon icon={Cancel01Icon} class="h-3.5 w-3.5" />
				<span>Exit</span>
				<kbd class="ml-0.5 rounded bg-white/15 dark:bg-white/10 px-1 py-0.5 text-[10px] text-foreground/80 border border-white/10">Esc</kbd>
			</Button>
		</div>
	</header>

	<!-- Main Content Area -->
	<div class="relative z-10 flex min-h-0 flex-1 w-full overflow-hidden">
		{#if userShowPanel}
			<!-- Two-Column View (Cover & Controls Left, Lyrics or Queue Right) -->
			<main class="mx-auto grid h-full w-full max-w-[100rem] min-h-0 grid-rows-[minmax(0,1fr)] gap-10 px-8 pb-8 sm:px-12 xl:gap-20 xl:px-16 lg:grid-cols-[minmax(20rem,0.85fr)_minmax(0,1.15fr)] items-center">
				<!-- Left Section: Cover & Playback Transport -->
				<section
					class="relative flex w-full max-w-sm shrink-0 self-stretch flex-col items-center justify-center mx-auto pt-14 sm:pt-18 xl:pt-22 sm:max-w-md xl:max-w-[26rem]"
					onwheel={wheelVolume}
				>
					<!-- Artwork Card: Click to Pause/Play + Scroll Volume + Canvas Support -->
					<div
						role="button"
						tabindex="0"
						onclick={() => api.togglePause()}
						onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && api.togglePause()}
						class="group/art relative w-full max-h-[44vh] max-w-[19rem] sm:max-w-[23rem] lg:max-w-[26rem] aspect-square overflow-hidden rounded-3xl ring-1 ring-white/10 shadow-2xl transition-transform duration-300 hover:scale-[1.01] cursor-pointer"
						title={playback.paused ? 'Play' : 'Pause'}
					>
						{#if coverSrc}
							<img
								src={coverSrc}
								alt={playback.now?.title ?? 'Album Art'}
								class="h-full w-full object-cover transition duration-500 group-hover/art:scale-102"
								in:fade={{ duration: 250 }}
							/>
							{#if prefs.animatedAlbumArt && canvasUrl}
								<!-- svelte-ignore a11y_media_has_caption -->
								<video
									bind:this={canvasVideoEl}
									src={canvasUrl}
									autoplay
									loop
									muted
									playsinline
									onloadeddata={() => (canvasReady = true)}
									oncanplay={() => (canvasReady = true)}
									class="pointer-events-none absolute inset-0 h-full w-full object-cover transition-opacity duration-700 ease-out {canvasReady ? 'opacity-100' : 'opacity-0'} group-hover/art:scale-102"
								></video>
							{/if}
						{:else}
							<div class="flex h-full w-full items-center justify-center bg-muted text-muted-foreground/40">
								<HugeiconsIcon icon={MusicNote01Icon} class="h-24 w-24" />
							</div>
						{/if}

						<!-- Subtle play/pause hover overlay -->
						<div class="pointer-events-none absolute inset-0 flex items-center justify-center bg-black/20 opacity-0 transition-opacity duration-200 group-hover/art:opacity-100">
							<div class="flex h-12 w-12 items-center justify-center rounded-full bg-black/60 text-white backdrop-blur-md shadow-lg">
								<HugeiconsIcon icon={playback.paused ? PlayIcon : PauseIcon} class="h-6 w-6 fill-current" />
							</div>
						</div>
					</div>

					<!-- Track Info & Metadata -->
					<div class="mt-6 w-full text-left">
						<div class="flex items-center justify-between gap-4">
							<div class="min-w-0 flex-1">
								<h2 class="font-heading text-2xl sm:text-3xl font-extrabold tracking-tight text-foreground truncate">
									{playback.now?.title ?? 'Nothing playing'}
								</h2>
								<div class="mt-1 flex items-center gap-1.5 text-sm sm:text-base font-medium text-muted-foreground truncate">
									{#if playback.now?.explicit}
										<ExplicitIcon class="h-3.5 w-3.5 shrink-0" />
									{/if}
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

						<!-- Upstream-style Seek Scrubber -->
						<div class="mt-6 w-full">
							{#if prefs.waveformSeekbar}
								<WaveformSeekbar
									position={currentPos}
									duration={durationNum || 0}
									height={26}
									barCount={72}
									onSeek={(v) => (seekDrag = v)}
									onCommit={(v) => {
										playback.position = v;
										seekDrag = null;
										api.seek(v);
									}}
								/>
							{:else}
								<input
									type="range"
									class="range theater-range w-full cursor-pointer"
									style="--pct:{progressPct}%"
									min="0"
									max={durationNum || 0}
									value={currentPos}
									oninput={onSeekInput}
									onchange={onSeekCommit}
									aria-label="Seek position"
								/>
							{/if}
							<div class="mt-2 flex justify-between text-xs font-medium tabular-nums text-muted-foreground">
								<span>{fmt(currentPos)}</span>
								<span>{playback.now?.duration ?? fmt(durationNum)}</span>
							</div>
						</div>

						<!-- Playback Transport Controls -->
						<div class="mt-4 flex items-center justify-center gap-4 sm:gap-6">
							<Button
								variant="ghost"
								size="icon-sm"
								onclick={() => api.toggleShuffle()}
								class="relative text-muted-foreground hover:text-foreground cursor-pointer"
								aria-label="Shuffle"
							>
								<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4 {playback.queue.shuffle ? 'text-primary' : ''}" />
								{#if playback.queue.shuffle}
									<span class="absolute bottom-0.5 left-1/2 -translate-x-1/2 size-1 rounded-full bg-primary pointer-events-none"></span>
								{/if}
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
								class="flex h-14 w-14 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg shadow-primary/25 transition hover:scale-105 active:scale-95 cursor-pointer"
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
								class="relative text-muted-foreground hover:text-foreground cursor-pointer"
								aria-label="Repeat: {repeat}"
							>
								<HugeiconsIcon
									icon={repeat === 'one' ? RepeatOne01Icon : RepeatIcon}
									class="h-4 w-4 {repeat !== 'off' ? 'text-primary' : ''}"
								/>
								{#if repeat !== 'off'}
									<span class="absolute bottom-0.5 left-1/2 -translate-x-1/2 size-1 rounded-full bg-primary pointer-events-none"></span>
								{/if}
							</Button>
						</div>

						<!-- Volume Slider Under Transport -->
						<div class="mt-4 flex items-center justify-center gap-3">
							<button
								type="button"
								onclick={toggleMute}
								class="flex h-8 w-8 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground"
								aria-label={playback.volume === 0 ? 'Unmute' : 'Mute'}
							>
								<HugeiconsIcon
									icon={playback.volume === 0 ? VolumeMute02Icon : VolumeHighIcon}
									class="h-4 w-4"
								/>
							</button>
							<input
								type="range"
								class="range w-32 sm:w-36 cursor-pointer"
								style="--pct:{playback.volume}%"
								min="0"
								max="100"
								value={playback.volume}
								onpointerdown={() => (volDragging = true)}
								oninput={(e) => dragVolume(Number(e.currentTarget.value))}
								onchange={(e) => commitVolume(Number(e.currentTarget.value))}
								aria-label="Volume"
							/>
							<span class="w-8 text-left text-xs font-medium tabular-nums text-muted-foreground">
								{playback.volume}%
							</span>
						</div>
					</div>
				</section>

				<!-- Right Section: Clean transparent Lyrics or Translucent blurred Queue -->
				{#if activeTab === 'lyrics'}
					<section class="relative flex min-h-0 flex-1 h-[72vh] flex-col overflow-hidden [mask-image:linear-gradient(to_bottom,transparent_0%,black_6%,black_94%,transparent_100%)]">
						<LyricsView expanded={true} />
					</section>
				{:else}
					<section class="relative flex min-h-0 flex-1 h-[72vh] flex-col overflow-hidden rounded-2xl border border-white/15 bg-card/40 dark:bg-black/35 backdrop-blur-2xl shadow-2xl p-3">
						<QueueList />
					</section>
				{/if}
			</main>
		{:else}
			<!-- Centered Theater View (Click artwork to pause/play, scroll volume, auto-fade controls) -->
			<main
				class="mx-auto flex h-full w-full max-w-2xl min-h-0 flex-col items-center justify-center px-8 pb-8 sm:px-12 text-center"
				onwheel={wheelVolume}
			>
				<!-- Artwork Card: Click to pause/play + scroll volume -->
				<div
					role="button"
					tabindex="0"
					onclick={() => api.togglePause()}
					onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && api.togglePause()}
					class="group/art relative w-full max-h-[50vh] sm:max-h-[56vh] max-w-[22rem] sm:max-w-[28rem] lg:max-w-[34rem] aspect-square overflow-hidden rounded-3xl ring-1 ring-white/10 shadow-2xl transition-transform duration-300 hover:scale-[1.01] cursor-pointer"
					title={playback.paused ? 'Play' : 'Pause'}
				>
					{#if coverSrc}
						<img
							src={coverSrc}
							alt={playback.now?.title ?? 'Album Art'}
							class="h-full w-full object-cover transition duration-500 group-hover/art:scale-102"
							in:fade={{ duration: 250 }}
						/>
						{#if prefs.animatedAlbumArt && canvasUrl}
							<!-- svelte-ignore a11y_media_has_caption -->
							<video
								bind:this={theaterCanvasVideoEl}
								src={canvasUrl}
								autoplay
								loop
								muted
								playsinline
								onloadeddata={() => (canvasReady = true)}
								oncanplay={() => (canvasReady = true)}
								class="pointer-events-none absolute inset-0 h-full w-full object-cover transition-opacity duration-700 ease-out {canvasReady ? 'opacity-100' : 'opacity-0'} group-hover/art:scale-102"
							></video>
						{/if}
					{:else}
						<div class="flex h-full w-full items-center justify-center bg-muted text-muted-foreground/40">
							<HugeiconsIcon icon={MusicNote01Icon} class="h-28 w-28" />
						</div>
					{/if}

					<!-- Subtle play/pause hover overlay -->
					<div class="pointer-events-none absolute inset-0 flex items-center justify-center bg-black/20 opacity-0 transition-opacity duration-200 group-hover/art:opacity-100">
						<div class="flex h-14 w-14 items-center justify-center rounded-full bg-black/60 text-white backdrop-blur-md shadow-lg">
							<HugeiconsIcon icon={playback.paused ? PlayIcon : PauseIcon} class="h-7 w-7 fill-current" />
						</div>
					</div>
				</div>

				<!-- Track Info & Metadata -->
				<div class="mt-6 w-full max-w-lg">
					<div class="flex flex-col items-center gap-1">
						<h2 class="font-heading text-2xl sm:text-3xl font-extrabold tracking-tight text-foreground truncate max-w-full">
							{playback.now?.title ?? 'Nothing playing'}
						</h2>
						<div class="text-sm sm:text-base font-medium text-muted-foreground truncate max-w-full">
							<ArtistLine
								runs={playback.now?.artistRuns}
								text={playback.now?.artists ?? ''}
								class="text-muted-foreground hover:text-foreground transition-colors"
							/>
						</div>
					</div>

					<!-- Theater Controls Dock with auto-fade on inactivity -->
					<div class="transition-opacity duration-500 {controlsVisible || playback.paused ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none'}">
						<!-- Upstream-style Seek Scrubber -->
						<div class="mt-6 w-full">
							{#if prefs.waveformSeekbar}
								<WaveformSeekbar
									position={currentPos}
									duration={durationNum || 0}
									height={26}
									barCount={72}
									onSeek={(v) => (seekDrag = v)}
									onCommit={(v) => {
										playback.position = v;
										seekDrag = null;
										api.seek(v);
									}}
								/>
							{:else}
								<input
									type="range"
									class="range theater-range w-full cursor-pointer"
									style="--pct:{progressPct}%"
									min="0"
									max={durationNum || 0}
									value={currentPos}
									oninput={onSeekInput}
									onchange={onSeekCommit}
									aria-label="Seek position"
								/>
							{/if}
							<div class="mt-2 flex justify-between text-xs font-medium tabular-nums text-muted-foreground">
								<span>{fmt(currentPos)}</span>
								<span>{playback.now?.duration ?? fmt(durationNum)}</span>
							</div>
						</div>

						<!-- Playback Transport Controls -->
						<div class="mt-5 flex items-center justify-center gap-4 sm:gap-6">
							<Button
								variant="ghost"
								size="icon-sm"
								onclick={() => api.toggleShuffle()}
								class="relative text-muted-foreground hover:text-foreground cursor-pointer"
								aria-label="Shuffle"
							>
								<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4 {playback.queue.shuffle ? 'text-primary' : ''}" />
								{#if playback.queue.shuffle}
									<span class="absolute bottom-0.5 left-1/2 -translate-x-1/2 size-1 rounded-full bg-primary pointer-events-none"></span>
								{/if}
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
								class="flex h-14 w-14 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg shadow-primary/25 transition hover:scale-105 active:scale-95 cursor-pointer"
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
								class="relative text-muted-foreground hover:text-foreground cursor-pointer"
								aria-label="Repeat: {repeat}"
							>
								<HugeiconsIcon
									icon={repeat === 'one' ? RepeatOne01Icon : RepeatIcon}
									class="h-4 w-4 {repeat !== 'off' ? 'text-primary' : ''}"
								/>
								{#if repeat !== 'off'}
									<span class="absolute bottom-0.5 left-1/2 -translate-x-1/2 size-1 rounded-full bg-primary pointer-events-none"></span>
								{/if}
							</Button>
						</div>

						<!-- Volume Slider Under Transport -->
						<div class="mt-4 flex items-center justify-center gap-3">
							<button
								type="button"
								onclick={toggleMute}
								class="flex h-8 w-8 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground"
								aria-label={playback.volume === 0 ? 'Unmute' : 'Mute'}
							>
								<HugeiconsIcon
									icon={playback.volume === 0 ? VolumeMute02Icon : VolumeHighIcon}
									class="h-4 w-4"
								/>
							</button>
							<input
								type="range"
								class="range w-32 sm:w-36 cursor-pointer"
								style="--pct:{playback.volume}%"
								min="0"
								max="100"
								value={playback.volume}
								onpointerdown={() => (volDragging = true)}
								oninput={(e) => dragVolume(Number(e.currentTarget.value))}
								onchange={(e) => commitVolume(Number(e.currentTarget.value))}
								aria-label="Volume"
							/>
							<span class="w-8 text-left text-xs font-medium tabular-nums text-muted-foreground">
								{playback.volume}%
							</span>
						</div>
					</div>
				</div>
			</main>
		{/if}
	</div>

	<!-- Limusic v1.3.0 "Up Next" preview card (Slides in near track end) -->
	{#if showUpNext && nextSong}
		<div
			transition:fly={{ y: 24, duration: 350 }}
			class="fixed bottom-8 right-8 z-30 flex items-center gap-3.5 rounded-2xl border border-white/20 bg-black/60 p-2.5 pr-4 backdrop-blur-2xl shadow-2xl text-left max-w-xs sm:max-w-sm select-none"
		>
			<img
				src={thumb(nextSong.thumbnail, 120)}
				alt={nextSong.title}
				class="h-12 w-12 rounded-xl object-cover ring-1 ring-white/10 shrink-0 shadow-md"
			/>
			<div class="min-w-0 flex-1">
				<div class="text-[10px] font-bold uppercase tracking-wider text-primary">Up Next</div>
				<div class="truncate text-xs font-semibold text-white">{nextSong.title}</div>
				<div class="truncate text-[11px] text-white/70">{nextSong.artists}</div>
			</div>
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={() => api.nextTrack()}
				class="h-8 w-8 rounded-full bg-white/15 hover:bg-white/25 text-white shrink-0 cursor-pointer transition-transform hover:scale-105 active:scale-95"
				aria-label="Play next track now"
				title="Play next track now"
			>
				<HugeiconsIcon icon={NextIcon} class="h-4 w-4 fill-current" />
			</Button>
		</div>
	{/if}
</div>

<style>
	.theater-range::-webkit-slider-runnable-track {
		height: 6px;
	}
	.theater-range::-webkit-slider-thumb {
		margin-top: -4px;
		height: 14px;
		width: 14px;
	}
</style>
