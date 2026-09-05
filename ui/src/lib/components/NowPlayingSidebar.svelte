<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Cancel01Icon,
		FavouriteIcon,
		Add01Icon,
		PlayIcon,
		PauseIcon,
		MusicNote01Icon,
		Video01Icon,
		VideoOffIcon,
		UserIcon,
		Queue01Icon,
		Mic01Icon,
		InfinityIcon,
		ArrowRight01Icon,
		FullScreenIcon,
		Maximize01Icon,
		Minimize01Icon
	} from '@hugeicons/core-free-icons';
	import { goto } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import * as api from '$lib/api';
	import type { ArtistPage, SongItem } from '$lib/api';
	import {
		playback,
		prefs,
		np,
		lyricsSync,
		toggleNowPlayingLike,
		openAddToPlaylist,
		wheelVolume
	} from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';
	import ExplicitIcon from './ExplicitIcon.svelte';
	import ArtistLine from './ArtistLine.svelte';
	import Marquee from './Marquee.svelte';
	import TrackMenu from './TrackMenu.svelte';
	import LyricsSyncDock from './LyricsSyncDock.svelte';

	let {
		onClose,
		onOpenQueue,
		onOpenLyrics
	}: {
		onClose: () => void;
		onOpenQueue?: () => void;
		onOpenLyrics?: () => void;
	} = $props();

	// --- Artist Info Fetching ---
	let artistInfo = $state<ArtistPage | null>(null);
	let loadingArtist = $state(false);

	$effect(() => {
		const artistId = playback.now?.artistId;
		artistInfo = null;
		if (!artistId || api.isLocalId(artistId)) return;

		let cancelled = false;
		loadingArtist = true;
		api.getArtist(artistId)
			.then((data) => {
				if (!cancelled) artistInfo = data;
			})
			.catch(() => {})
			.finally(() => {
				if (!cancelled) loadingArtist = false;
			});

		return () => {
			cancelled = true;
		};
	});

	// --- Lyrics Fetching & Sync ---
	function durationSecs(d?: string): number | undefined {
		if (!d) return undefined;
		const parts = d.split(':').map(Number);
		if (!parts.length || parts.some(Number.isNaN)) return undefined;
		return parts.reduce((a, b) => a * 60 + b, 0);
	}

	let lyrics = $state<api.Lyrics | null>(null);
	let loadingLyrics = $state(true);
	let requestedLyricsId = '';
	let lyricsScroller: HTMLElement | undefined = $state();
	let expandedLyrics = $state(false);

	$effect(() => {
		const now = playback.now;
		if (!now) {
			requestedLyricsId = '';
			lyrics = null;
			loadingLyrics = false;
			return;
		}
		if (now.videoId === requestedLyricsId) return;
		const id = (requestedLyricsId = now.videoId);
		loadingLyrics = true;
		lyrics = null;
		hasScrolledLyrics = false;
		userScrollUntil = 0;
		if (lyricsScroller) {
			lyricsScroller.scrollTo({ top: 0, behavior: 'instant' });
		}
		const album = playback.queue.items[playback.queue.currentIndex]?.album;
		api.getLyrics({
			videoId: id,
			title: now.title,
			artists: now.artists,
			album: album ?? undefined,
			duration: durationSecs(now.duration)
		})
			.then((l) => {
				if (requestedLyricsId !== id) return;
				lyrics = l;
				loadingLyrics = false;
				hasScrolledLyrics = false;
				userScrollUntil = 0;
				if (lyricsScroller) {
					lyricsScroller.scrollTo({ top: 0, behavior: 'instant' });
				}
			})
			.catch(() => {
				if (requestedLyricsId !== id) return;
				loadingLyrics = false;
			});
	});

	// Active lyrics line calculation
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

	// High frequency position interpolation for silky smooth karaoke highlight
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

	const posMs = $derived(interpolatedPosSecs * 1000 + lyricsSync.currentOffsetMs);

	function getWordProgress(word: api.LyricWord, currentMs: number): number {
		if (currentMs <= word.start_ms) return 0;
		if (currentMs >= word.end_ms) return 1;
		const dur = word.end_ms - word.start_ms;
		if (dur <= 0) return 1;
		return (currentMs - word.start_ms) / dur;
	}

	let userScrollUntil = 0;
	let hasScrolledLyrics = false;
	function onUserLyricsScroll() {
		userScrollUntil = Date.now() + 3000;
	}

	$effect(() => {
		const i = activeIndex;
		if (!lyricsScroller || Date.now() < userScrollUntil) return;
		if (i < 0) return;
		if (i === 0) {
			lyricsScroller.scrollTo({
				top: 0,
				behavior: hasScrolledLyrics ? 'smooth' : 'instant'
			});
			hasScrolledLyrics = true;
			return;
		}
		const lineEl = lyricsScroller.querySelector(`[data-line="${i}"]`) as HTMLElement | null;
		if (!lineEl) return;
		const scrollerRect = lyricsScroller.getBoundingClientRect();
		const lineRect = lineEl.getBoundingClientRect();
		const lineRelativeTop = lineRect.top - scrollerRect.top + lyricsScroller.scrollTop;
		const targetTop = lineRelativeTop - (lyricsScroller.clientHeight / 2) + (lineRect.height / 2);
		lyricsScroller.scrollTo({
			top: Math.max(0, targetTop),
			behavior: hasScrolledLyrics ? 'smooth' : 'instant'
		});
		hasScrolledLyrics = true;
	});

	function seekTo(line: api.LyricLine) {
		if (line.time_ms === undefined) return;
		const secs = line.time_ms / 1000;
		playback.position = secs;
		userScrollUntil = 0;
		api.seek(secs);
	}

	// --- Like State Animation ---
	let justLiked = $state(false);
	function toggleLike() {
		if (playback.rating !== 'like') justLiked = true;
		toggleNowPlayingLike();
	}

	// --- Queue Items ---
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

	// --- Music Video Logic ---
	let wantVideo = $state(true);
	let videoUrl = $state<string | null>(null);
	let videoEl = $state<HTMLVideoElement | null>(null);
	const canVideo = $derived(prefs.musicVideos && !!playback.now?.isVideo);
	const showVideo = $derived(canVideo && wantVideo && !!videoUrl);

	$effect(() => {
		const id = playback.now?.videoId;
		videoUrl = null;
		if (!id || !canVideo || !wantVideo) return;
		let cancelled = false;
		api.videoStream(id, 480)
			.then((u) => !cancelled && (videoUrl = u))
			.catch(() => {});
		return () => {
			cancelled = true;
		};
	});

	$effect(() => {
		if (videoEl && showVideo) {
			const onTimeUpdate = () => {
				if (!videoEl || playback.paused) return;
				const audioPos = playback.position;
				const videoPos = videoEl.currentTime;
				if (Math.abs(audioPos - videoPos) > 0.3) {
					videoEl.currentTime = audioPos;
				}
			};
			videoEl.addEventListener('timeupdate', onTimeUpdate);
			return () => videoEl?.removeEventListener('timeupdate', onTimeUpdate);
		}
	});

	$effect(() => {
		if (videoEl) {
			if (playback.paused) {
				videoEl.pause();
			} else if (showVideo) {
				videoEl.play().catch(() => {});
			}
		}
	});

	const sourceTitle = $derived(playback.queue.sourceName || 'Now Playing');
</script>

<button
	class="fixed inset-0 z-30 cursor-default bg-black/20 backdrop-blur-xs lg:hidden"
	onclick={onClose}
	aria-label="Close now playing sidebar"
	transition:fade={{ duration: 150 }}
></button>

<aside
	transition:fly={{ x: 320, duration: 250, easing: cubicOut }}
	class="info-sidebar flex h-full w-80 max-w-[90vw] shrink-0 flex-col border-l border-border/70 bg-card/90 shadow-2xl backdrop-blur-2xl transition-all select-none overflow-hidden z-20"
>
	<!-- Header -->
	<div class="flex items-center justify-between border-b px-4 py-3 shrink-0">
		<div class="flex items-center gap-2">
			<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4 text-primary" />
			<h2 class="font-heading text-sm font-semibold tracking-tight">Now Playing</h2>
		</div>
		<div class="flex items-center gap-1">
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={() => (np.fullscreenOpen = true)}
				aria-label="Open fullscreen player"
				class="hover:text-foreground"
				title="Fullscreen (F)"
			>
				<HugeiconsIcon icon={FullScreenIcon} class="h-4 w-4" />
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={onClose}
				aria-label="Close now playing"
				class="hover:text-foreground"
			>
				<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4" />
			</Button>
		</div>
	</div>

	<!-- Scrollable Content Body -->
	<div class="min-h-0 flex-1 overflow-y-auto p-4 space-y-5">
		<!-- Large Artwork / Music Video Header -->
		<div class="group relative aspect-square w-full overflow-hidden rounded-2xl border bg-muted shadow-lg">
			{#if showVideo}
				<!-- svelte-ignore a11y_media_has_caption -->
				<video
					bind:this={videoEl}
					src={videoUrl ?? ''}
					autoplay
					muted
					playsinline
					class="h-full w-full object-cover"
				></video>
				<button
					onclick={() => (wantVideo = false)}
					class="absolute right-2 top-2 z-10 flex h-7 items-center gap-1 rounded-full bg-black/60 px-2 text-[11px] font-medium text-white backdrop-blur-md transition hover:bg-black/80"
					title="Switch to album artwork"
				>
					<HugeiconsIcon icon={VideoOffIcon} class="h-3.5 w-3.5" />
					<span>Audio</span>
				</button>
			{:else}
				{#if playback.now?.thumbnail}
					<img
						src={thumb(playback.now.thumbnail, 600)}
						alt={playback.now?.title ?? 'Album Art'}
						class="h-full w-full object-cover transition duration-500 group-hover:scale-105"
					/>
				{:else}
					<div class="flex h-full w-full items-center justify-center text-muted-foreground">
						<HugeiconsIcon icon={MusicNote01Icon} class="h-16 w-16" />
					</div>
				{/if}

				{#if canVideo}
					<button
						onclick={() => (wantVideo = true)}
						class="absolute right-2 top-2 z-10 flex h-7 items-center gap-1 rounded-full bg-black/60 px-2 text-[11px] font-medium text-white backdrop-blur-md transition hover:bg-black/80"
						title="Watch music video"
					>
						<HugeiconsIcon icon={Video01Icon} class="h-3.5 w-3.5 text-primary" />
						<span>Video</span>
					</button>
				{/if}
			{/if}
		</div>

		<!-- Track Title, Artists & Primary Actions -->
		<div class="space-y-2">
			<div class="flex items-start justify-between gap-3">
				<div class="min-w-0 flex-1">
					<h3 class="font-heading text-lg font-bold leading-tight text-foreground truncate">
						{playback.now?.title ?? 'Nothing playing'}
					</h3>
					<div class="mt-0.5 truncate text-sm text-muted-foreground">
						<ArtistLine
							runs={playback.now?.artistRuns}
							text={playback.now?.artists ?? 'Unknown artist'}
							class="block max-w-full text-sm text-muted-foreground hover:text-foreground"
						/>
					</div>
				</div>

				{#if playback.now}
					<div class="flex items-center gap-0.5 shrink-0 pt-0.5">
						{#if !api.isLocalId(playback.now.videoId)}
							<Button
								variant="ghost"
								size="icon-sm"
								onclick={toggleLike}
								aria-label="Like"
								class="hover:text-primary"
							>
								<span
									class="inline-flex"
									class:animate-heart-pop={justLiked}
									onanimationend={() => (justLiked = false)}
								>
									<HugeiconsIcon
										icon={FavouriteIcon}
										class="h-4.5 w-4.5 {playback.rating === 'like'
											? 'fill-current text-primary'
											: 'text-muted-foreground'}"
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
							>
								<HugeiconsIcon icon={Add01Icon} class="h-4.5 w-4.5 text-muted-foreground" />
							</Button>
						{/if}
						{#if currentSong}
							<TrackMenu
								song={currentSong}
								onAdd={() => openAddToPlaylist(currentSong!)}
								triggerClass="inline-flex size-8 cursor-pointer items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-foreground"
							/>
						{/if}
					</div>
				{/if}
			</div>
		</div>

		<!-- Spotify-style Synced Lyrics Card with Glassy Turbo Fluid Styling -->
		<div class="overflow-hidden rounded-xl border border-border/80 bg-muted/40 p-4 transition-all shadow-sm">
			<div class="flex items-center justify-between pb-3">
				<div class="flex items-center gap-1.5">
					<HugeiconsIcon icon={Mic01Icon} class="h-4 w-4 text-primary" />
					<span class="text-xs font-bold uppercase tracking-wider text-foreground">Lyrics</span>
				</div>
				<div class="flex items-center gap-1">
					<button
						onclick={() => (expandedLyrics = !expandedLyrics)}
						class="flex items-center gap-1 rounded px-1.5 py-0.5 text-xs font-semibold text-primary hover:bg-primary/10 cursor-pointer"
					>
						{expandedLyrics ? 'Collapse' : 'Expand'}
						<HugeiconsIcon icon={expandedLyrics ? Minimize01Icon : Maximize01Icon} class="h-3 w-3" />
					</button>
				</div>
			</div>

			<!-- Lyrics Container -->
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				bind:this={lyricsScroller}
				onwheel={onUserLyricsScroll}
				ontouchmove={onUserLyricsScroll}
				onpointerdown={onUserLyricsScroll}
				class="lyrics-scroller overflow-y-auto overflow-x-hidden transition-all duration-300 {expandedLyrics ? 'max-h-96' : 'max-h-48'}"
			>
				{#if loadingLyrics}
					<div class="space-y-2.5 py-2">
						{#each { length: 4 } as _, i (i)}
							<div class="h-4 animate-pulse rounded bg-muted-foreground/20" style="width:{60 + ((i * 15) % 35)}%"></div>
						{/each}
					</div>
				{:else if lyrics?.instrumental}
					<div class="py-6 text-center text-sm font-medium text-muted-foreground">
						Instrumental ♪
					</div>
				{:else if lyrics && lyrics.synced}
					<div class="space-y-3 py-4">
						{#each lyrics.lines as line, i (i)}
							{@const isActive = i === activeIndex}
							{@const isPast = i < activeIndex}
							<button
								data-line={i}
								onclick={() => seekTo(line)}
								class="block w-full origin-left cursor-pointer text-left font-heading text-sm font-bold leading-snug transition-all duration-300 hover:text-foreground
									{isActive
									? 'scale-[1.03] text-foreground opacity-100 drop-shadow-[0_0_18px_var(--primary)]'
									: isPast
										? 'text-muted-foreground/45 opacity-60 blur-[0.2px] hover:blur-none hover:opacity-90'
										: 'text-muted-foreground/75 opacity-75 blur-[0.15px] hover:blur-none hover:opacity-100'}"
							>
								{#if line.words && line.words.length > 0}
									<span class="inline-flex flex-wrap items-baseline">
										{#each line.words as word, wIdx (wIdx)}
											{@const isWordEnd = word.text.endsWith(' ')}
											{@const cleanText = word.text.trimEnd()}
											{#if isActive}
												{@const progress = getWordProgress(word, posMs)}
												{@const pct = Math.round(Math.min(1, Math.max(0, progress)) * 100)}
												{@const isCurrentWord = progress > 0 && progress < 1}
												<span
													class="inline-block bg-clip-text text-transparent [-webkit-text-fill-color:transparent] transition-transform duration-100 ease-out {isWordEnd ? 'mr-[0.22em]' : ''} {isCurrentWord ? 'scale-[1.04] drop-shadow-[0_0_10px_var(--primary)]' : ''}"
													style="background-image: linear-gradient(90deg, var(--foreground) {pct}%, var(--muted-foreground) {pct}%)"
												>
													{cleanText}
												</span>
											{:else}
												<span class="inline-block {isWordEnd ? 'mr-[0.22em]' : ''} {isPast ? 'text-muted-foreground/40' : 'text-muted-foreground/70'}">
													{cleanText}
												</span>
											{/if}
										{/each}
									</span>
								{:else}
									<span class="{isActive ? 'bg-gradient-to-r from-foreground to-primary/80 bg-clip-text' : ''}">{line.text || '♪'}</span>
								{/if}

								{#if line.translation}
									<p class="mt-0.5 text-xs font-normal italic opacity-75">
										{line.translation}
									</p>
								{/if}
							</button>
						{/each}
					</div>
				{:else if lyrics}
					<div class="space-y-1.5 py-2 text-xs leading-relaxed text-foreground/90">
						{#each lyrics.lines as line, i (i)}
							{#if line.text}
								<div>
									<p>{line.text}</p>
									{#if line.translation}
										<p class="text-[11px] italic text-muted-foreground">{line.translation}</p>
									{/if}
								</div>
							{/if}
						{/each}
					</div>
				{:else}
					<div class="py-6 text-center text-xs text-muted-foreground">
						No lyrics available for this song.
					</div>
				{/if}
			</div>

			{#if lyrics && !loadingLyrics}
				<div class="mt-3 flex items-center justify-between border-t border-border/40 pt-2 text-[10px] text-muted-foreground">
					<span>{lyrics.source.startsWith('Source:') ? lyrics.source : `Lyrics from ${lyrics.source}`}</span>
					<LyricsSyncDock />
				</div>
			{/if}
		</div>

		<!-- About the Artist Card (Spotify-Style) -->
		{#if playback.now?.artistId || playback.now?.artists}
			<div class="overflow-hidden rounded-xl border bg-muted/40 p-4 transition-colors hover:bg-muted/60">
				<div class="flex items-center justify-between pb-3">
					<span class="text-xs font-bold uppercase tracking-wider text-muted-foreground">About the artist</span>
					{#if playback.now?.artistId}
						<button
							onclick={() => goto(`/artist/${encodeURIComponent(playback.now!.artistId!)}`)}
							class="flex items-center gap-1 text-xs font-semibold text-primary hover:underline"
						>
							View profile
							<HugeiconsIcon icon={ArrowRight01Icon} class="h-3 w-3" />
						</button>
					{/if}
				</div>

				<div class="flex items-center gap-3">
					<div class="relative h-14 w-14 shrink-0 overflow-hidden rounded-full bg-muted shadow">
						{#if artistInfo?.thumbnail}
							<img src={thumb(artistInfo.thumbnail, 120)} alt="" class="h-full w-full object-cover" />
						{:else if playback.now?.thumbnail}
							<img src={thumb(playback.now.thumbnail, 120)} alt="" class="h-full w-full object-cover" />
						{:else}
							<div class="flex h-full w-full items-center justify-center bg-muted text-muted-foreground">
								<HugeiconsIcon icon={UserIcon} class="h-6 w-6" />
							</div>
						{/if}
					</div>

					<div class="min-w-0 flex-1">
						<div class="truncate text-sm font-bold text-foreground">
							{artistInfo?.name || playback.now?.artists || 'Artist'}
						</div>
						{#if artistInfo?.subscribers}
							<div class="text-xs text-muted-foreground">
								{artistInfo.subscribers} subscribers
							</div>
						{:else if artistInfo?.monthlyListeners}
							<div class="text-xs text-muted-foreground">
								{artistInfo.monthlyListeners} monthly listeners
							</div>
						{/if}
					</div>
				</div>

				{#if artistInfo?.description}
					<p class="mt-3 line-clamp-3 text-xs leading-relaxed text-muted-foreground">
						{artistInfo.description}
					</p>
				{/if}
			</div>
		{/if}

		<!-- Next in Queue Card -->
		{#if nextSong}
			<div class="rounded-xl border bg-muted/40 p-4">
				<div class="flex items-center justify-between pb-2.5">
					<span class="text-xs font-bold uppercase tracking-wider text-muted-foreground">Next in queue</span>
					{#if onOpenQueue}
						<button
							onclick={onOpenQueue}
							class="text-xs font-semibold text-primary hover:underline"
						>
							Open queue
						</button>
					{/if}
				</div>

				<div class="flex items-center gap-3">
					<div class="h-10 w-10 shrink-0 overflow-hidden rounded-lg bg-muted">
						{#if nextSong.thumbnail}
							<img src={thumb(nextSong.thumbnail, 96)} alt="" class="h-full w-full object-cover" />
						{:else}
							<div class="flex h-full w-full items-center justify-center text-muted-foreground/50">
								<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4" />
							</div>
						{/if}
					</div>
					<div class="min-w-0 flex-1">
						<div class="truncate text-xs font-semibold text-foreground">{nextSong.title}</div>
						<div class="truncate text-[11px] text-muted-foreground">{nextSong.artists}</div>
					</div>
				</div>
			</div>
		{/if}

		<!-- Quick Lyrics / Queue Switcher Buttons -->
		<div class="flex items-center gap-2 pt-1">
			{#if onOpenLyrics}
				<Button
					variant="outline"
					size="sm"
					class="flex-1 gap-2 text-xs"
					onclick={onOpenLyrics}
				>
					<HugeiconsIcon icon={Mic01Icon} class="h-3.5 w-3.5" />
					Full Lyrics
				</Button>
			{/if}
			{#if onOpenQueue}
				<Button
					variant="outline"
					size="sm"
					class="flex-1 gap-2 text-xs"
					onclick={onOpenQueue}
				>
					<HugeiconsIcon icon={Queue01Icon} class="h-3.5 w-3.5" />
					Full Queue
				</Button>
			{/if}
		</div>
	</div>
</aside>
