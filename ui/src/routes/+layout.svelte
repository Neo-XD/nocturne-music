<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/icon.png';
	import { ModeWatcher } from 'mode-watcher';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		CheckmarkCircle02Icon,
		AlertCircleIcon,
		InformationCircleIcon
	} from '@hugeicons/core-free-icons';
	import { browser } from '$app/environment';
	import { onMount } from 'svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { fly, fade, slide } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { theme, appearance, applyArtworkAccent, prewarmArtworkAccent, initTheme } from '$lib/theme.svelte';
	import { thumb } from '$lib/thumb';
	import { blockForeignDrag, dragScroll } from '$lib/dnd';
	import { suppressNative } from '$lib/menu';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import Titlebar from '$lib/components/Titlebar.svelte';
	import ResizeBorders from '$lib/components/ResizeBorders.svelte';
	import PlayerBar from '$lib/components/PlayerBar.svelte';
	import QueuePanel from '$lib/components/QueuePanel.svelte';
	import LyricsPanel from '$lib/components/LyricsPanel.svelte';
	import AddToPlaylist from '$lib/components/AddToPlaylist.svelte';
	import SettingsDialog from '$lib/components/SettingsDialog.svelte';
	import ShareDialog from '$lib/components/ShareDialog.svelte';
	import ChannelPicker from '$lib/components/ChannelPicker.svelte';
	import ListenTogether from '$lib/components/ListenTogether.svelte';
	import LinkDialog from '$lib/components/LinkDialog.svelte';
	import MiniPlayer from '$lib/components/MiniPlayer.svelte';
	import NowPlaying from '$lib/components/NowPlaying.svelte';
	import NowPlayingSidebar from '$lib/components/NowPlayingSidebar.svelte';
	import DevicesSidebar from '$lib/components/DevicesSidebar.svelte';
	import FullscreenPlayer from '$lib/components/FullscreenPlayer.svelte';
	import AnimatedArtwork from '$lib/components/AnimatedArtwork.svelte';
	import VideoSurface from '$lib/components/VideoSurface.svelte';
	import CommandPalette from '$lib/components/CommandPalette.svelte';
	import KeyboardShortcuts from '$lib/components/KeyboardShortcuts.svelte';
	import { Button } from '$lib/components/ui/button';
	import { auth, initApp, np, playback, prefs, ui } from '$lib/player.svelte';
	import { win, initWin, setWindowFullscreen, onFullscreenExit } from '$lib/win.svelte';
	import { initZoom } from '$lib/zoom';
	import { initShortcuts } from '$lib/shortcuts.svelte';
	import {
		updateState,
		installUpdate,
		openDownloadPage,
		checkForUpdatesQuiet
	} from '$lib/updater.svelte';

	let { children } = $props();
	const tabbed = $derived(np.open && appearance.tabbedPlayer);

	// Right sidebar: only 1 right sidebar can be active at a time.
	// Closing a sidebar cleanly closes it without resurrecting any previous sidebar.
	type RightSidebarId = 'np' | 'queue' | 'lyrics' | 'devices';
	let activeRightSidebar = $state<RightSidebarId | null>(np.sidebarOpen ? 'np' : null);

	function toggleRightSidebar(id: RightSidebarId) {
		if (tabbed && (id === 'queue' || id === 'lyrics')) {
			np.tab = id;
			return;
		}
		if (id === 'np') {
			np.open = false;
		}
		if (activeRightSidebar === id) {
			if (id === 'np') np.sidebarOpen = false;
			activeRightSidebar = null;
		} else {
			if (id === 'np') np.sidebarOpen = true;
			else np.sidebarOpen = false;
			activeRightSidebar = id;
		}
	}

	function closeRightSidebar(id?: RightSidebarId) {
		if (!id || activeRightSidebar === id) {
			if (activeRightSidebar === 'np') {
				np.sidebarOpen = false;
			}
			activeRightSidebar = null;
		}
	}

	let npSidebarWidth = $state(
		browser
			? Math.min(650, Math.max(280, parseInt(localStorage.getItem('nowPlayingSidebarWidth') || '384', 10) || 384))
			: 384
	);
	let isResizingSidebar = $state(false);

	const currentSidebarWidth = $derived.by(() => {
		if (!activeRightSidebar) return 0;
		if (activeRightSidebar === 'np') return npSidebarWidth;
		if (activeRightSidebar === 'devices') return 352;
		return 320; // queue & lyrics
	});

	const queueOpen = $derived(!tabbed && activeRightSidebar === 'queue');
	const lyricsOpen = $derived(!tabbed && activeRightSidebar === 'lyrics');
	const devicesOpen = $derived(activeRightSidebar === 'devices');
	const npSidebarOpen = $derived(activeRightSidebar === 'np' && !np.open);

	$effect(() => {
		if (np.sidebarOpen && activeRightSidebar === null && !np.open) {
			activeRightSidebar = 'np';
		} else {
			np.sidebarOpen = npSidebarOpen;
		}
	});
	$effect(() => {
		np.devicesOpen = devicesOpen;
	});

	// "Adapt colors to artwork": re-run on every track change and on the toggle itself. The 120px
	// cover is the one the player bar has already loaded, so this costs no extra request.
	$effect(() => {
		applyArtworkAccent(
			appearance.artworkAccent ? thumb(playback.now?.thumbnail, 120) : null
		);
	});
	// Same colour, one track early. Reading it off the queue instead of the track change means the
	// palette starts moving on the frame the artwork swaps, not after a fetch and a decode.
	$effect(() => {
		if (!appearance.artworkAccent) return;
		const q = playback.queue;
		prewarmArtworkAccent(thumb(q.items[q.currentIndex + 1]?.thumbnail, 120));
	});

	// The mini player runs this same SPA in a second window (Rust `mini.rs`), so the window label is
	// what tells the two apart: `mini` gets the widget instead of the app chrome, and none of the
	// routes below it are ever rendered. Constant for the window's lifetime.
	const isMini =
		browser &&
		(() => {
			try {
				return getCurrentWindow().label === 'mini';
			} catch {
				return false;
			}
		})();

	// Apply the saved accent color before the first paint (ssr=false → nothing renders until now).
	if (browser) initTheme();

	// Mirrors the condition that renders FullscreenPlayer, so a track ending cannot leave the window fullscreen with no overlay.
	$effect(() => {
		void setWindowFullscreen(np.fullscreenOpen && !!playback.now);
	});

	// Wire the Tauri event bridge once for the whole app; teardown on destroy. Check for an update
	// on every app open (silent unless one exists).
	onMount(() => {
		if (isMini) return initApp(true);
		// First: it reveals the window (see initWin).
		const teardownWin = initWin();
		onFullscreenExit(() => (np.fullscreenOpen = false));
		checkForUpdatesQuiet();
		const teardownApp = initApp();
		const teardownZoom = initZoom();
		const teardownShortcuts = initShortcuts();
		return () => {
			teardownApp();
			teardownWin();
			teardownZoom();
			teardownShortcuts();
		};
	});
</script>

<!-- oncontextmenu: the app's own menus handle their right-click and stop the event, so anything
     that reaches the window is a place where WebKit would have offered back / reload / inspect.
     Text fields and selections keep the native menu (see `suppressNative`). -->
<svelte:window
	ondragover={blockForeignDrag}
	ondrop={blockForeignDrag}
	oncontextmenu={suppressNative}
/>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>
<ModeWatcher />

<!-- The mini player is the whole window when it is the window: no titlebar, no sidebar, no routes,
     and no toasts (a banner would cover most of a 560x180 widget). -->
{#if isMini}
	<MiniPlayer />
{:else}
	<!-- The window itself is transparent; this root paints the background and, when not maximized,
	     rounds the corners (the compositor can't round an undecorated window for us). -->
	<div
		class="relative flex h-screen flex-col overflow-hidden bg-background text-foreground {win.maximized ||
		win.fullscreen
			? ''
			: 'rounded-lg'}"
	>
		<!-- Glassy Theme: Solid Base Underlayer + Warped/Blurred Animated Album Art Background -->
		{#if theme.id === 'glassy'}
			<div class="pointer-events-none absolute inset-0 z-0 overflow-hidden select-none bg-background">
				{#if playback.now?.thumbnail}
					{#if prefs.animatedArtwork}
						<AnimatedArtwork
							src={thumb(playback.now.thumbnail, 720)}
							class="absolute inset-0 h-full w-full scale-125 object-cover transition-all duration-700"
							style="opacity: {appearance.glassyLightness}; filter: blur({Math.min(appearance.glassyBlur, 18)}px) saturate({Math.round(appearance.glassySaturation * 100)}%);"
							intensity={appearance.glassyWarp}
							speed={appearance.glassySpeed}
						/>
					{:else}
						<img
							src={thumb(playback.now.thumbnail, 720)}
							alt=""
							class="absolute inset-0 h-full w-full object-cover scale-125 transition-all duration-700"
							style="opacity: {appearance.glassyLightness}; filter: blur({appearance.glassyBlur}px) saturate({Math.round(appearance.glassySaturation * 100)}%);"
						/>
					{/if}
				{:else}
					<div
						class="absolute inset-0 bg-gradient-to-br from-primary/20 via-background to-secondary/30"
					></div>
				{/if}
				<div class="absolute inset-0 bg-background/45 dark:bg-black/40 {prefs.animatedArtwork ? 'backdrop-blur-sm' : 'backdrop-blur-2xl'}"></div>
			</div>
		{/if}

		<ResizeBorders />
		<Titlebar />
		<!-- relative: the queue and lyrics panels are absolute overlays inside it (see QueuePanel). -->
		<div class="relative z-10 flex min-h-0 flex-1">
			<Sidebar />
			<!-- dragScroll: dragging a card up to home's Shortcuts grid has to be possible from anywhere in
			     the feed, so aiming at the top edge scrolls this container while the drag is in flight. -->
			<main class="min-w-0 flex-1 overflow-y-auto" {@attach dragScroll}>
				<!-- Remount the current page on sign-in/out so it refetches with the new account. -->
				{#key auth.epoch}
					{@render children()}
				{/key}
			</main>
			<!-- Always mounted, unlike the player view below it: it owns the one <video> element, which
			     has to keep playing while the view is closed. It renders nothing but a zero-sized
			     parking container until the view borrows the picture. -->
			<VideoSurface />
			{#if np.open && playback.now}<NowPlaying {queueOpen} {lyricsOpen} />{/if}
			{#if activeRightSidebar && (activeRightSidebar !== 'np' || (playback.now && !np.open))}
				<div
					class="relative z-20 flex h-full shrink-0 flex-col overflow-hidden {isResizingSidebar
						? 'transition-none'
						: 'transition-[width] duration-250 ease-out'}"
					style="width: {currentSidebarWidth}px;"
					transition:slide={{ axis: 'x', duration: 250, easing: cubicOut }}
				>
					<div class="relative h-full w-full overflow-hidden">
						{#key activeRightSidebar}
							<div
								class="h-full w-full"
								in:fly={{ x: 28, duration: 220, easing: cubicOut }}
							>
								{#if activeRightSidebar === 'lyrics'}
									<LyricsPanel onClose={() => closeRightSidebar('lyrics')} />
								{:else if activeRightSidebar === 'queue'}
									<QueuePanel onClose={() => closeRightSidebar('queue')} />
								{:else if activeRightSidebar === 'devices'}
									<DevicesSidebar onClose={() => closeRightSidebar('devices')} />
								{:else if activeRightSidebar === 'np'}
									<NowPlayingSidebar
										onClose={() => closeRightSidebar('np')}
										onOpenQueue={() => toggleRightSidebar('queue')}
										onOpenLyrics={() => toggleRightSidebar('lyrics')}
										onWidthChange={(w) => (npSidebarWidth = w)}
										onResizingChange={(r) => (isResizingSidebar = r)}
									/>
								{/if}
							</div>
						{/key}
					</div>
				</div>
			{/if}
		</div>
		{#if playback.now}
			<div class="relative z-20" transition:fly={{ y: 64, duration: 200, easing: cubicOut }}>
				<PlayerBar
					onToggleQueue={() => toggleRightSidebar('queue')}
					queueOpen={tabbed ? np.tab === 'queue' : queueOpen}
					onToggleLyrics={() => toggleRightSidebar('lyrics')}
					lyricsOpen={tabbed ? np.tab === 'lyrics' : lyricsOpen}
					onToggleDevices={() => toggleRightSidebar('devices')}
					devicesOpen={devicesOpen}
					onToggleNowPlayingSidebar={() => toggleRightSidebar('np')}
				/>
			</div>
		{/if}
	</div>

	<CommandPalette />
	<KeyboardShortcuts />
	<AddToPlaylist />
	<ShareDialog />
	<SettingsDialog />
	<ChannelPicker />
	<ListenTogether />
	<LinkDialog />

	{#if np.fullscreenOpen && playback.now}
		<FullscreenPlayer />
	{/if}

	<!-- The two notification banners below run at z-[100]. Dialogs and menus sit at z-50 and portal to
	     <body>, so a z-50 banner loses the tie on DOM order and hides behind an open modal. -->
	{#if updateState.available}
		<div
			transition:fly={{ y: 16, duration: 220, easing: cubicOut }}
			class="fixed bottom-24 left-1/2 z-[100] flex -translate-x-1/2 items-center gap-3 rounded-lg border bg-card px-4 py-2 text-sm shadow-lg"
		>
			<span>Update available — v{updateState.available.version}</span>
			{#if updateState.canInstall}
				<Button size="sm" onclick={installUpdate} disabled={updateState.installing}>
					{updateState.installing ? 'Updating…' : 'Update now'}
				</Button>
			{:else}
				<!-- Packaged build (.rpm, AUR): the updater can only rewrite an AppImage, so send them
				     to the releases page and let their package manager do it. -->
				<Button size="sm" onclick={openDownloadPage}>Download</Button>
			{/if}
			{#if !updateState.installing}
				<button
					class="text-muted-foreground hover:text-foreground"
					aria-label="Dismiss"
					onclick={() => (updateState.available = null)}>✕</button
				>
			{/if}
		</div>
	{/if}

	{#if ui.toast}
		{@const t = ui.toast}
		<div
			transition:fly={{ y: 16, duration: 220, easing: cubicOut }}
			class="fixed bottom-40 left-1/2 z-[100] flex -translate-x-1/2 items-center gap-2 rounded-lg border bg-card px-4 py-2 text-sm shadow-lg"
		>
			<!-- Three branches instead of a ternary on `icon`: HugeiconsIcon freezes `icon` at mount, so a
			     new toast replacing a visible one would keep the old glyph. -->
			{#if t.kind === 'success'}
				<HugeiconsIcon icon={CheckmarkCircle02Icon} class="h-4 w-4 shrink-0 text-primary" />
			{:else if t.kind === 'error'}
				<HugeiconsIcon icon={AlertCircleIcon} class="h-4 w-4 shrink-0 text-destructive" />
			{:else}
				<HugeiconsIcon
					icon={InformationCircleIcon}
					class="h-4 w-4 shrink-0 text-muted-foreground"
				/>
			{/if}
			{t.msg}
		</div>
	{/if}
{/if}
