<script lang="ts">
	import { fly, scale } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { beforeNavigate } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Maximize01Icon,
		Minimize01Icon,
		Mic01Icon,
		MusicNote01Icon,
		PlayIcon,
		PauseIcon,
		Queue01Icon,
		Video01Icon,
		VideoOffIcon
	} from '@hugeicons/core-free-icons';
	import * as Tabs from '$lib/components/ui/tabs';
	import * as api from '$lib/api';
	import { np, playback, prefs, ui } from '$lib/player.svelte';
	import { appearance } from '$lib/theme.svelte';
	import { thumb } from '$lib/thumb';
	import QueueList from './QueueList.svelte';
	import LyricsView from './LyricsView.svelte';

	// Off in settings, this view drops its tabs and the queue/lyrics panels stay in charge of both
	// (see +layout): they paint above this (z-30 over z-20), so all this needs is to hand back the
	// width they take at lg+ instead of letting them cover a third of the artwork. Below lg they're
	// a scrimmed overlay and there's nothing to shrink into. In tabbed mode both are always closed.
	let { queueOpen, lyricsOpen }: { queueOpen: boolean; lyricsOpen: boolean } = $props();
	const tabbed = $derived(appearance.tabbedPlayer);
	// ponytail: mirrors QueuePanel / LyricsPanel's w-80, keep in sync if those change.
	const panels = $derived(Number(queueOpen) + Number(lyricsOpen));
	const inset = $derived(['', 'lg:right-80', 'lg:right-[40rem]'][panels]);

	// Going somewhere means the user wants that page, not this one: minimise. The player bar brings
	// it back. beforeNavigate (not a pathname effect) so clicking the tab you're already on counts.
	beforeNavigate(() => (np.open = false));

	// Enlarged lyrics take the whole view, artwork column and tab strip included. A class swap
	// rather than unmounting the tabs: LyricsView must survive it or it refetches and loses its
	// scroll position.
	let big = $state(false);
	$effect(() => {
		if (np.tab !== 'lyrics') big = false; // nothing to enlarge on the queue tab
	});

	// Google's CDN doesn't serve every rewritten size for every image (see MediaCard), and at this
	// size a broken-image glyph *is* the page. So step down until one loads: crisp, then the size
	// proven everywhere else in the app, then the 120 the player bar is already showing for this
	// very track, and only then a music note.
	let attempt = $state(0);
	let bgFailed = $state(false);
	$effect(() => {
		playback.now?.thumbnail; // re-arm on every track change
		attempt = 0;
		bgFailed = false;
	});
	const srcs = $derived([720, 400, 120].map((px) => thumb(playback.now?.thumbnail, px)));
	const src = $derived(srcs[attempt]);
	const imgFailed = () => attempt++;

	// Clicking the artwork toggles playback, and flashes the action just taken over it so the click
	// visibly did something. Read `paused` before the toggle: the backend event that flips it is a
	// round trip away, and the icon has to be right on the frame the user clicked.
	let flash: 'play' | 'pause' | null = $state(null);
	let flashTimer: ReturnType<typeof setTimeout>;
	function toggle() {
		flash = playback.paused ? 'play' : 'pause';
		clearTimeout(flashTimer);
		flashTimer = setTimeout(() => (flash = null), 220);
		api.togglePause();
	}

	// --- Music videos (plan 031) -------------------------------------------------------------
	// When the track *is* a music video, this box draws the video instead of the artwork. mpv stays
	// the audio master: the element is muted and its clock is stapled to mpv's position. Bytes come
	// from Rust over a loopback proxy, so nothing here ever sees a googlevideo URL.
	//
	// `wantVideo` is session-sticky on purpose: someone who hits "show artwork" wants artwork now
	// and almost certainly on the next video too, but a permanent no is what the setting is for.
	let wantVideo = $state(true);
	let videoUrl = $state<string | null>(null);
	let videoEl = $state<HTMLVideoElement | null>(null);
	const canVideo = $derived(prefs.musicVideos && !!playback.now?.isVideo);
	const showVideo = $derived(canVideo && wantVideo && !!videoUrl);

	const DRIFT_LIMIT = 1;
	const RESYNC_GAP = 2000;
	let lastResync = 0;

	$effect(() => {
		const id = playback.now?.videoId;
		videoUrl = null;
		lastResync = 0; // a new element gets its first sync immediately, not after the gap
		if (!id || !canVideo || !wantVideo) return;
		let cancelled = false;
		// Silent on failure: a null answer is the ordinary case (no video stream), and the artwork
		// staying put is already the right thing to show.
		api.videoStream(id).then((u) => !cancelled && (videoUrl = u)).catch(() => {});
		return () => (cancelled = true);
	});

	// mpv owns the clock and this element is a picture stapled to it, but the staples have to be
	// rare. Position ticks arrive at ~4 Hz (src-tauri/src/lib.rs), so `playback.position` is a
	// sample up to 250 ms stale and the measured drift swings by that much with nothing actually
	// wrong. Correcting for that swing is what made the picture stutter: a `playbackRate` nudge
	// re-times the GStreamer pipeline and a seek flushes it, and the old code did one or the other
	// four times a second, forever. Both clocks run on the same machine at the same speed, so once
	// they agree they stay agreeing: leave the element alone until it is visibly wrong, then seek
	// once and give the seek time to land before judging it again.
	function resync(tolerance: number) {
		const el = videoEl;
		if (!el || el.readyState === 0 || el.seeking) return;
		const now = performance.now();
		if (now - lastResync < RESYNC_GAP) return;
		if (Math.abs(playback.position - el.currentTime) < tolerance) return;
		lastResync = now;
		el.currentTime = playback.position;
	}

	$effect(() => {
		playback.position; // the tick this effect exists to answer
		if (showVideo) resync(DRIFT_LIMIT);
	});

	// Follow the tempo control, or mpv's clock outruns the element at any speed but 1x and the
	// resync above turns into a seek every few seconds. This is a stable value the user picked and
	// it is written only when it changes, which is what makes it safe where the old nudge was not.
	$effect(() => {
		const rate = playback.speed;
		const el = videoEl;
		if (el && showVideo && el.playbackRate !== rate) el.playbackRate = rate;
	});

	$effect(() => {
		const paused = playback.paused;
		const el = videoEl;
		if (!el || !showVideo) return;
		if (paused) el.pause();
		else el.play().catch(() => {});
	});

	// At `loadedmetadata` the element is at 0 and cannot play yet, so seeking here is only half the
	// job: by the time it has buffered enough to start, mpv has moved on by however long that took,
	// and that offset would sit there under DRIFT_LIMIT forever. `playing` is the moment its clock
	// starts meaning something, so snap it tight there too. That is exactly what pausing and
	// unpausing by hand used to do. It fires after every seek as well, which is why `resync` keeps
	// its own gap: the follow-up call is a no-op instead of a loop.
	const videoStart = () => {
		resync(0.3);
		if (!playback.paused) videoEl?.play().catch(() => {});
	};

	// Minimised to the tray, the window still decodes video unless we stop it. Nothing here touches
	// mpv, so the audio carries on.
	$effect(() => {
		const onVisibility = () => {
			if (!videoEl) return;
			if (document.hidden) videoEl.pause();
			else videoStart();
		};
		document.addEventListener('visibilitychange', onVisibility);
		return () => document.removeEventListener('visibilitychange', onVisibility);
	});
</script>

<!-- Covers the page but not the sidebar (you navigate away to minimise) and not the player bar,
     which stays in charge of transport and paints above this on the way in and out.
     z-20 matches the highest a page uses for its own chrome (home's sticky mood chips) and wins the
     tie on DOM order, since <main> is static and its z-indexes land in the same stacking context.
     The player bar and the queue/lyrics panels come later/higher, so they still paint above.
     ponytail: left offsets mirror Sidebar's w-16/lg:w-60 (and its manual collapse) — keep in sync
     if those change. -->
<div
	transition:fly={{ y: '100%', duration: 320, easing: cubicOut }}
	class="absolute inset-y-0 left-16 right-0 z-20 flex justify-center overflow-hidden bg-background px-4 py-4 sm:px-6 sm:py-6 lg:px-10 {ui.sidebarCollapsed
		? ''
		: 'lg:left-60'} {inset}"
>
	<!-- The artwork itself, blurred to a wash, is the background: same trick as HomeHero, and it
	     needs no colour extraction (which a remote image would taint the canvas for anyway). The
	     120px variant is the one the player bar has already loaded for this track, so this costs
	     no request and nothing new to decode.
	     Two opacities because the wash sits on opposite grounds: over white it has to stay pale
	     enough for dark text, over near-black it can carry more colour before muted-foreground
	     stops reading. Turn them up together if it's too subtle. -->
	{#if appearance.artworkBackground && srcs[2] && !bgFailed}
		<img
			src={srcs[2]}
			alt=""
			onerror={() => (bgFailed = true)}
			class="pointer-events-none absolute inset-0 h-full w-full scale-110 object-cover opacity-30 blur-2xl dark:opacity-40"
		/>
	{/if}

	<!-- Capped and centred, so a wide window doesn't park the artwork in the middle of an empty half
	     with the tabs glued to the right edge. --art is the artwork's side: whichever is smaller of
	     the column's width and the height left over once the titlebar, the player bar and this
	     padding have had theirs, at 75% so the square doesn't dominate the view.
	     ponytail: 11rem is those three measured, not computed. The 0.75 leaves it plenty of slack
	     now, so only a much taller player bar would need it raised. -->
	<div
		class="relative flex w-full max-w-[80rem] gap-6 xl:gap-10"
		style="--art:calc(min(100%,100vh - 11rem) * 0.75)"
	>
		{#if !big}
			<!-- Centred against the full height of the column on the right. Below md there isn't room
			     for both columns, and the queue wins. Untabbed there is no second column, so the
			     artwork is the whole view at every width. -->
			<div
				class="min-w-0 flex-1 items-center justify-center {tabbed ? 'hidden md:flex' : 'flex'}"
			>
				<!-- A div, not a button: the video-mode toggle has to be a sibling of the play/pause
				     button rather than nested inside it (nested buttons are invalid HTML and the
				     inner one never reliably gets the click). -->
				<div class="relative w-full max-w-[var(--art)]">
					<button
						type="button"
						onclick={toggle}
						aria-label="Play/pause"
						class="block w-full cursor-pointer"
					>
						{#if flash}
							<!-- No backdrop-blur: re-blurring the plate on every frame of the scale is what made
							     this stutter on WebKitGTK. Transform and opacity only. -->
							<div
								in:scale={{ start: 0.7, duration: 150, easing: cubicOut }}
								out:scale={{ start: 1.3, duration: 320, easing: cubicOut }}
								class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center"
							>
								<div class="rounded-full bg-black/55 p-3.5 text-white">
									<!-- icon is frozen at mount, so swap via showAlt, not a ternary. -->
									<HugeiconsIcon
										icon={PauseIcon}
										altIcon={PlayIcon}
										showAlt={flash === 'play'}
										class="h-7 w-7"
									/>
								</div>
							</div>
						{/if}
						{#if showVideo}
							<!-- Muted and never seeked by the user: mpv is the clock (see the sync effects).
							     16:9 in the same --art width, so the column keeps its layout. -->
							<!-- svelte-ignore a11y_media_has_caption -->
							<video
								bind:this={videoEl}
								src={videoUrl}
								muted
								playsinline
								preload="auto"
								onloadedmetadata={videoStart}
								onplaying={videoStart}
								onerror={() => (videoUrl = null)}
								class="aspect-video w-full rounded-2xl bg-black object-contain shadow-2xl"
							></video>
						{:else if src && attempt < srcs.length}
							<img
								{src}
								alt=""
								onerror={imgFailed}
								class="aspect-square w-full rounded-2xl object-cover shadow-2xl"
							/>
						{:else}
							<div
								class="flex aspect-square w-full items-center justify-center rounded-2xl bg-muted text-muted-foreground/40"
							>
								<HugeiconsIcon icon={MusicNote01Icon} class="h-16 w-16" />
							</div>
						{/if}
					</button>
					{#if canVideo}
						<!-- Both directions, or there is no way back to the video. On a plate, since it sits
						     over whatever frame happens to be showing. -->
						<button
							type="button"
							onclick={() => (wantVideo = !wantVideo)}
							aria-label={showVideo ? 'Show artwork' : 'Show video'}
							class="absolute right-3 top-3 z-10 cursor-pointer rounded-md bg-black/40 p-1.5 text-white/70 transition-colors hover:text-white"
						>
							<!-- icon swap via altIcon/showAlt: `icon` is frozen at mount -->
							<HugeiconsIcon
								icon={Video01Icon}
								altIcon={VideoOffIcon}
								showAlt={showVideo}
								class="h-4 w-4"
							/>
						</button>
					{/if}
				</div>
			</div>
		{/if}

		{#if tabbed}
			<div class="flex min-h-0 flex-col {big ? 'flex-1' : 'w-full md:w-[22rem] xl:w-[26rem]'}">
				<Tabs.Root
					value={np.tab}
					onValueChange={(v) => (np.tab = v as typeof np.tab)}
					class="min-h-0 flex-1"
				>
					<div class="flex items-center gap-2 {big ? 'justify-end' : ''}">
						<!-- Same two glyphs the player bar uses for the queue and lyrics buttons. -->
						<Tabs.List class={big ? 'hidden' : 'flex-1'}>
							<Tabs.Trigger value="queue" class="gap-2.5">
								<HugeiconsIcon icon={Queue01Icon} class="h-4 w-4" /> Queue
							</Tabs.Trigger>
							<Tabs.Trigger value="lyrics" class="gap-2.5">
								<HugeiconsIcon icon={Mic01Icon} class="h-4 w-4" /> Lyrics
							</Tabs.Trigger>
						</Tabs.List>
						{#if np.tab === 'lyrics'}
							<button
								onclick={() => (big = !big)}
								class="cursor-pointer rounded-md p-1.5 text-muted-foreground transition-colors hover:text-foreground"
								aria-label={big ? 'Shrink lyrics' : 'Enlarge lyrics'}
							>
								<!-- icon swap via altIcon/showAlt: `icon` is frozen at mount -->
								<HugeiconsIcon
									icon={Maximize01Icon}
									altIcon={Minimize01Icon}
									showAlt={big}
									class="h-4 w-4"
								/>
							</button>
						{/if}
					</div>
					<!-- Only the open tab is mounted: bits-ui keeps inactive content in the DOM, which would
					     leave LyricsView fetching lyrics for every track you never asked to see. -->
					{#if np.tab === 'queue'}
						<Tabs.Content value="queue" class="flex min-h-0 flex-col">
							<QueueList />
						</Tabs.Content>
					{:else}
						<Tabs.Content value="lyrics" class="flex min-h-0 flex-col">
							<LyricsView expanded={big} />
						</Tabs.Content>
					{/if}
				</Tabs.Root>
			</div>
		{/if}
	</div>
</div>
