<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
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
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { initTheme } from '$lib/theme.svelte';
	import { dragScroll } from '$lib/dnd';
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
	import MiniPlayer from '$lib/components/MiniPlayer.svelte';
	import NowPlaying from '$lib/components/NowPlaying.svelte';
	import { Button } from '$lib/components/ui/button';
	import { auth, initApp, np, playback, ui } from '$lib/player.svelte';
	import { win, initWin } from '$lib/win.svelte';
	import { initZoom } from '$lib/zoom';
	import { updateState, installUpdate, checkForUpdatesQuiet } from '$lib/updater.svelte';

	let { children } = $props();
	// Queue and lyrics toggle independently and both float over the page rather than docking into
	// it — two docked columns squeezed the content down to an unusable strip. At lg+ they sit side
	// by side over the content; narrower, they stack (see QueuePanel / LyricsPanel).
	let queueOpen = $state(false);
	let lyricsOpen = $state(false);
	// The now-playing view carries its own queue and lyrics, so the side panels step aside for it
	// and the bar's two buttons switch its tabs instead of opening a panel on top of it.
	$effect(() => {
		if (np.open) queueOpen = lyricsOpen = false;
	});

	// The mini player runs this same SPA in a second window (Rust `mini.rs`), so the window label is
	// what tells the two apart: `mini` gets the widget instead of the app chrome, and none of the
	// routes below it are ever rendered. Constant for the window's lifetime.
	const isMini = browser && getCurrentWindow().label === 'mini';

	// Apply the saved accent color before the first paint (ssr=false → nothing renders until now).
	if (browser) initTheme();

	// Wire the Tauri event bridge once for the whole app; teardown on destroy. Check for an update
	// on every app open (silent unless one exists).
	onMount(() => {
		if (isMini) return initApp(true);
		checkForUpdatesQuiet();
		const teardownApp = initApp();
		const teardownWin = initWin();
		const teardownZoom = initZoom();
		return () => {
			teardownApp();
			teardownWin();
			teardownZoom();
		};
	});
</script>

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
		class="flex h-screen flex-col overflow-hidden bg-background text-foreground {win.maximized
			? ''
			: 'rounded-lg'}"
	>
		<ResizeBorders />
		<Titlebar />
		<!-- relative: the queue and lyrics panels are absolute overlays inside it (see QueuePanel). -->
		<div class="relative flex min-h-0 flex-1">
			<Sidebar />
			<!-- dragScroll: dragging a card up to home's Shortcuts grid has to be possible from anywhere in
			     the feed, so aiming at the top edge scrolls this container while the drag is in flight. -->
			<main class="min-w-0 flex-1 overflow-y-auto" {@attach dragScroll}>
				<!-- Remount the current page on sign-in/out so it refetches with the new account. -->
				{#key auth.epoch}
					{@render children()}
				{/key}
			</main>
			{#if np.open && playback.now}<NowPlaying />{/if}
			<!-- Lyrics before queue: side by side over the page, lyrics on the left, queue on the right. -->
			{#if lyricsOpen}<LyricsPanel onClose={() => (lyricsOpen = false)} {queueOpen} />{/if}
			{#if queueOpen}<QueuePanel onClose={() => (queueOpen = false)} />{/if}
		</div>
		{#if playback.now}
			<!-- Slides up from its own height on first play; leaves instantly (bar removal is rare).
			     z-20 on the wrapper, not the bar: the intro's transform makes this a stacking context,
			     so a z on the footer inside would be trapped under it. The now-playing view is z-20 and
			     earlier in the DOM, which is what puts it behind the bar as it slides in and out. -->
			<div class="relative z-20" in:fly={{ y: 64, duration: 250, easing: cubicOut }}>
				<PlayerBar
					onToggleQueue={() => (np.open ? (np.tab = 'queue') : (queueOpen = !queueOpen))}
					queueOpen={np.open ? np.tab === 'queue' : queueOpen}
					onToggleLyrics={() => (np.open ? (np.tab = 'lyrics') : (lyricsOpen = !lyricsOpen))}
					lyricsOpen={np.open ? np.tab === 'lyrics' : lyricsOpen}
				/>
			</div>
		{/if}
	</div>

	<AddToPlaylist />
	<ShareDialog />
	<SettingsDialog />
	<ChannelPicker />
	<ListenTogether />

	<!-- The two notification banners below run at z-[100]. Dialogs and menus sit at z-50 and portal to
	     <body>, so a z-50 banner loses the tie on DOM order and hides behind an open modal. -->
	{#if updateState.available}
		<div
			transition:fly={{ y: 16, duration: 220, easing: cubicOut }}
			class="fixed bottom-24 left-1/2 z-[100] flex -translate-x-1/2 items-center gap-3 rounded-lg border bg-card px-4 py-2 text-sm shadow-lg"
		>
			<span>Update available — v{updateState.available.version}</span>
			<Button size="sm" onclick={installUpdate} disabled={updateState.installing}>
				{updateState.installing ? 'Updating…' : 'Update now'}
			</Button>
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
