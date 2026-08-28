<script lang="ts">
	import { untrack, onMount, type Snippet } from 'svelte';
	import { open } from '@tauri-apps/plugin-dialog';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Cancel01Icon,
		Settings02Icon,
		PaintBoardIcon,
		PlayCircleIcon,
		Database02Icon,
		InformationCircleIcon,
		ViewIcon,
		ViewOffSlashIcon,
		Link04Icon,
		KeyboardIcon,
		ArrowUp01Icon,
		ArrowDown01Icon,
		Mic01Icon,
		RotateLeft01Icon,
		FlashIcon,
		CpuIcon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Switch } from '$lib/components/ui/switch';
	import { Slider } from '$lib/components/ui/slider';
	import { Alert, AlertDescription } from '$lib/components/ui/alert';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as api from '$lib/api';
	import { prefs, ui, toast } from '$lib/player.svelte';
	import {
		formatKey,
		keybindings,
		setKeybinding,
		resetKeybindings,
		normalizeEvent,
		SHORTCUT_DEFINITIONS,
		type ShortcutAction,
		MOD
	} from '$lib/shortcuts.svelte';
	import ColorPicker from '$lib/components/ColorPicker.svelte';
	import Changelog from '$lib/components/Changelog.svelte';
	import {
		THEMES,
		FONTS,
		theme,
		appearance,
		setAppearance,
		custom,
		effective,
		applyTheme,
		toggleGlassyTheme,
		setCustom,
		resetCustom,
		isDefaultCustom,
		readBack,
		familyName,
		fontAvailable,
		fileFonts,
		fileFamily,
		addFontFile,
		removeFontFile,
		registerFontFiles,
		type Custom,
		type ThemeId
	} from '$lib/theme.svelte';
	import {
		updateState,
		checkForUpdatesInteractive,
		installUpdate,
		openDownloadPage
	} from '$lib/updater.svelte';
	import { getVersion } from '@tauri-apps/api/app';

	type TabId = 'general' | 'themes' | 'playback' | 'performance' | 'lyrics' | 'keybindings' | 'data' | 'about';
	const TABS: { id: TabId; label: string; hint: string; icon: typeof Settings02Icon }[] = [
		{ id: 'general', label: 'General', hint: 'History, integrations and how the app starts.', icon: Settings02Icon },
		{ id: 'themes', label: 'Appearance', hint: 'Colors, fonts and the player view.', icon: PaintBoardIcon },
		{ id: 'playback', label: 'Playback', hint: 'Quality, queue behaviour and stream clients.', icon: PlayCircleIcon },
		{ id: 'performance', label: 'Performance', hint: 'Graphics, animation speed and resource optimizations.', icon: FlashIcon },
		{ id: 'lyrics', label: 'Lyrics', hint: 'Provider priority, sources and synchronization.', icon: Mic01Icon },
		{ id: 'keybindings', label: 'Keybindings', hint: 'Keyboard shortcuts and custom key mappings.', icon: KeyboardIcon },
		{ id: 'data', label: 'Data & storage', hint: 'Network and cached files.', icon: Database02Icon },
		{ id: 'about', label: 'About', hint: 'Version, updates and what changed.', icon: InformationCircleIcon }
	];

	// Shared shapes for the settings rows. Kept as strings so the markup below stays readable and
	// every group looks identical without a wrapper component per row.
	const GROUP = 'mb-7 last:mb-1';
	const LABEL =
		'mb-2 px-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground';
	const CARD = 'divide-y divide-border/60 overflow-hidden rounded-xl border bg-card';

	const ACCENT_THEMES = THEMES.filter((t) => t.kind === 'accent');
	const PALETTE_THEMES = THEMES.filter((t) => t.kind === 'palette');
	const currentTheme = $derived(THEMES.find((t) => t.id === theme.id) ?? THEMES[0]);

	// --- Themes tab ---
	type FontKey = 'fontSans' | 'fontHeading' | 'fontLyrics';
	const FONT_ROWS: { key: FontKey; label: string; hint: string }[] = [
		{ key: 'fontSans', label: 'Interface font', hint: 'Everything except headings and lyrics.' },
		{ key: 'fontHeading', label: 'Heading font', hint: 'Page and section titles.' },
		{ key: 'fontLyrics', label: 'Lyrics font', hint: 'Synchronized and static lyrics across the player.' }
	];
	let pickerOpen = $state(false);
	// Whether each font row is on "Custom", and the family name typed into it. Kept locally because
	// the select can sit on Custom before anything has been typed.
	let isCustomFont = $state<Record<FontKey, boolean>>({ fontSans: false, fontHeading: false, fontLyrics: false });
	let fontName = $state<Record<FontKey, string>>({ fontSans: '', fontHeading: '', fontLyrics: '' });

	/** Which entry in the font dropdown a resolved stack corresponds to. */
	const fontOptions = $derived([...FONTS, ...fileFonts()]);
	const matchFont = (stack: string) =>
		fontOptions.find((f) => familyName(f.value) === familyName(stack))?.value ?? 'custom';

	async function pickFontFiles() {
		const picked = await open({
			multiple: true,
			title: 'Load a font',
			filters: [{ name: 'Fonts', extensions: ['ttf', 'otf', 'woff', 'woff2'] }]
		});
		for (const path of picked ?? []) {
			try {
				toast.success(`${await addFontFile(path)} loaded — pick it above`);
			} catch (e) {
				toast.error(String(e));
			}
		}
	}

	function chooseFont(key: FontKey, value: string) {
		isCustomFont[key] = value === 'custom';
		if (value === 'custom') fontName[key] = familyName(effective[key]);
		else setCustom({ [key]: value } as Partial<Custom>);
	}

	// Applying a font family rewrites --font-sans/--font-heading on <html>, which restyles and
	// reflows the whole app (and `apply` then re-reads the computed tokens). Doing that per
	// keystroke is what made typing a font name lag (#97), so the input updates immediately and the
	// theme follows once typing pauses. Half-typed names are meaningless anyway.
	const fontTimers: Record<FontKey, ReturnType<typeof setTimeout> | undefined> = {
		fontSans: undefined,
		fontHeading: undefined,
		fontLyrics: undefined
	};

	function typeFont(key: FontKey, name: string) {
		fontName[key] = name;
		clearTimeout(fontTimers[key]);
		fontTimers[key] = setTimeout(() => {
			// Blank clears the override, so the preset's font comes back.
			setCustom({ [key]: name.trim() ? `'${name.trim()}', sans-serif` : null } as Partial<Custom>);
		}, 300);
	}

	let tab = $state<TabId>('general');
	const currentTab = $derived(TABS.find((t) => t.id === tab) ?? TABS[0]);
	let settings = $state<Record<string, string>>({});
	let clients = $state<string[]>([]);
	let proxyInput = $state('');
	let loaded = $state(false);
	let clearing = $state(false);
	let version = $state('');
	getVersion().then((v) => (version = v));
	// Result of the last "Check for updates" click — shown inline (a toast renders behind the modal).
	let updateResult = $state<{ message: string; error: boolean } | null>(null);

	// (Re)load whenever the modal opens, so it reflects the current persisted values. Also clear the
	// stale update-check result so re-opening the modal doesn't show it until pressed again.
	// untrack: this reads and writes theme state, and `registerFontFiles` can rewrite it again when
	// it prunes a deleted font. Opening the modal is the only thing that should run it.
	$effect(() => {
		if (!ui.settingsOpen) return;
		untrack(() => {
			load();
			updateResult = null;
			pickerOpen = false;
			readBack();
			// Catches a font deleted while the app was running, not just between launches.
			registerFontFiles();
			for (const key of ['fontSans', 'fontHeading', 'fontLyrics'] as FontKey[]) {
				isCustomFont[key] = matchFont(effective[key]) === 'custom';
				fontName[key] = isCustomFont[key] ? familyName(effective[key]) : '';
			}
		});
	});

	async function checkUpdates() {
		updateResult = await checkForUpdatesInteractive();
	}

	let lastfmConnected = $state(false);
	let lastfmUser = $state<string | null>(null);
	let lastfmConnecting = $state(false);
	let lastfmKeyInput = $state('');
	let lastfmSecretInput = $state('');
	let showSecret = $state(false);

	async function loadLastfm() {
		try {
			const s = await api.lastfmStatus();
			lastfmConnected = s.connected;
			lastfmUser = s.username ?? null;
		} catch {}
	}

	async function updateLastfmKey(val: string) {
		lastfmKeyInput = val;
		settings.lastfm_api_key = val;
		await api.setSetting('lastfm_api_key', val);
	}

	async function updateLastfmSecret(val: string) {
		lastfmSecretInput = val;
		settings.lastfm_api_secret = val;
		await api.setSetting('lastfm_api_secret', val);
	}

	async function connectLastfm() {
		const key = (lastfmKeyInput || settings.lastfm_api_key || '').trim();
		const secret = (lastfmSecretInput || settings.lastfm_api_secret || '').trim();
		if (!key || !secret) {
			toast.error('Please enter both your Last.fm API Key and Shared Secret below.');
			return;
		}
		lastfmConnecting = true;
		try {
			await updateLastfmKey(key);
			await updateLastfmSecret(secret);
			await api.lastfmConnect();
			toast('Approve Nocturne in your browser');
		} catch (e) {
			lastfmConnecting = false;
			toast.error(String(e));
		}
	}

	async function disconnectLastfm() {
		try {
			await api.lastfmDisconnect();
			lastfmConnected = false;
			lastfmUser = null;
			toast.success('Last.fm disconnected');
		} catch (e) {
			toast.error(String(e));
		}
	}

	onMount(() => {
		const sub = api.onLastfmState((s) => {
			lastfmConnecting = false;
			lastfmConnected = s.connected;
			lastfmUser = s.username ?? null;
		});
		return () => {
			sub.then((u) => u());
		};
	});

	// --- Keybindings tab ---
	let recordingAction = $state<ShortcutAction | null>(null);

	function startRecording(action: ShortcutAction) {
		recordingAction = action;
	}

	function stopRecording() {
		recordingAction = null;
	}

	function onKeyRecord(e: KeyboardEvent) {
		if (!recordingAction) return;
		e.preventDefault();
		e.stopPropagation();

		if (e.key === 'Escape') {
			stopRecording();
			return;
		}

		const combo = normalizeEvent(e);
		if (combo) {
			const targetAction = recordingAction;
			setKeybinding(targetAction, combo);
			const def = SHORTCUT_DEFINITIONS.find((d) => d.id === targetAction);
			toast.success(`Updated shortcut for ${def?.label ?? targetAction}`);
			stopRecording();
		}
	}

	async function load() {
		try {
			const [s, c] = await Promise.all([
				api.getSettings(),
				api.getStreamClients(),
				loadLastfm()
			]);
			settings = s;
			clients = c;
			proxyInput = s.proxy ?? '';
			lastfmKeyInput = s.lastfm_api_key ?? '';
			lastfmSecretInput = s.lastfm_api_secret ?? '';
			initLyricsProviders(s.lyrics_providers ?? s.lyrics_priority);
		} catch (e) {
			toast.error(String(e));
		}
		loaded = true;
	}

	interface LyricsProviderInfo {
		id: string;
		name: string;
		description: string;
		badge?: string;
		enabled: boolean;
	}

	const ALL_PROVIDERS: Omit<LyricsProviderInfo, 'enabled'>[] = [
		{
			id: 'betterlyrics',
			name: 'Better Lyrics',
			description: 'High-precision syllable-by-syllable & word-level synchronized lyrics (TTML/eLRC).',
			badge: 'Word Sync'
		},
		{
			id: 'lrclib',
			name: 'LRCLIB',
			description: 'Open community database of synchronized and plain lyrics with wide global coverage.',
			badge: 'Line Sync'
		},
		{
			id: 'ytm',
			name: 'YouTube Music',
			description: 'Official real-time timed lyrics extracted directly from YouTube Music.',
			badge: 'Official'
		},
		{
			id: 'qq',
			name: 'QQ Music',
			description: 'Synchronized lyrics database with extensive Asian and international catalogue coverage.',
			badge: 'LRC'
		},
		{
			id: 'kugou',
			name: 'Kugou',
			description: 'High-coverage LRC lyrics archive matched by exact audio duration.',
			badge: 'LRC'
		}
	];

	let lyricsProviders = $state<LyricsProviderInfo[]>([]);

	function initLyricsProviders(saved?: string) {
		const defaultIds = ['betterlyrics', 'lrclib', 'ytm', 'qq', 'kugou'];
		let enabledIds: string[] = defaultIds;
		let savedOrder: string[] = [];
		if (saved && saved.trim()) {
			try {
				if (saved.trim().startsWith('[')) {
					savedOrder = JSON.parse(saved);
				} else {
					savedOrder = saved.split(',').map((s) => s.trim().toLowerCase()).filter(Boolean);
				}
			} catch {}
		}
		if (savedOrder.length > 0) {
			enabledIds = savedOrder;
		}

		const map = new Map(ALL_PROVIDERS.map((p) => [p.id, p]));
		const result: LyricsProviderInfo[] = [];

		for (const id of enabledIds) {
			const item = map.get(id);
			if (item) {
				result.push({ ...item, enabled: true });
				map.delete(id);
			}
		}
		for (const item of map.values()) {
			result.push({ ...item, enabled: false });
		}
		lyricsProviders = result;
	}

	async function saveLyricsProviders() {
		const enabledIds = lyricsProviders.filter((p) => p.enabled).map((p) => p.id);
		const value = enabledIds.join(',');
		settings.lyrics_providers = value;
		await api.setSetting('lyrics_providers', value);
		toast.success('Lyrics priority updated');
	}

	function moveProvider(index: number, direction: -1 | 1) {
		const target = index + direction;
		if (target < 0 || target >= lyricsProviders.length) return;
		const item = lyricsProviders[index];
		lyricsProviders.splice(index, 1);
		lyricsProviders.splice(target, 0, item);
		saveLyricsProviders();
	}

	function toggleProvider(id: string, on: boolean) {
		const p = lyricsProviders.find((x) => x.id === id);
		if (p) {
			p.enabled = on;
			saveLyricsProviders();
		}
	}

	function resetLyricsProviders() {
		initLyricsProviders();
		saveLyricsProviders();
	}

	const quality = $derived(settings.quality ?? 'HIGH');
	const historyOn = $derived(settings.enable_history !== 'false');
	const autoplayOn = $derived(settings.autoplay !== 'false');
	const hideVideosOn = $derived(settings.hide_videos === 'true');
	// Off until the setting is turned on: still experimental, so nobody gets video they didn't ask
	// for. Same test in `player.svelte.ts`, which hydrates `prefs` at launch.
	const musicVideosOn = $derived(settings.music_videos === 'true');
	const boiduOn = $derived(settings.lyrics_boidu !== 'false');
	const filterExplicitOn = $derived(settings.filter_explicit === 'true');
	const animatedArtworkOn = $derived(settings.animated_artwork !== 'false');
	const preventDuplicatesOn = $derived(settings.prevent_duplicates === 'true');
	const updateBannerOn = $derived(settings.update_banner !== 'false');
	const discordOn = $derived(settings.discord_rpc === 'true');
	const trayOn = $derived(settings.close_to_tray !== 'false');
	const autostartOn = $derived(settings.autostart === 'true');
	const disabled = $derived(
		new Set(
			(settings.disabled_stream_clients ?? '')
				.split(',')
				.map((s) => s.trim())
				.filter(Boolean)
		)
	);

	const QUALITIES = [
		{ id: 'LOW', label: 'Low' },
		{ id: 'AUTO', label: 'Auto' },
		{ id: 'HIGH', label: 'High' }
	];

	async function setQuality(q: string) {
		settings.quality = q;
		await api.setSetting('quality', q);
		// Cached URLs are keyed by video only, so clear them to apply the new quality everywhere.
		await api.clearCaches();
		toast.success('Audio quality updated');
	}

	async function setHistory(on: boolean) {
		settings.enable_history = on ? 'true' : 'false';
		await api.setSetting('enable_history', settings.enable_history);
	}

	async function setAutoplay(on: boolean) {
		settings.autoplay = on ? 'true' : 'false';
		await api.setSetting('autoplay', settings.autoplay);
	}

	async function setFilterExplicit(on: boolean) {
		settings.filter_explicit = on ? 'true' : 'false';
		prefs.filterExplicit = on;
		await api.setSetting('filter_explicit', settings.filter_explicit);
	}

	async function setAnimatedArtwork(on: boolean) {
		settings.animated_artwork = on ? 'true' : 'false';
		prefs.animatedArtwork = on;
		await api.setSetting('animated_artwork', settings.animated_artwork);
	}

	// Also lands in `prefs`, which is where the player view reads it: the switch has to take effect
	// on the track that's already playing, not on the next launch.
	async function setMusicVideos(on: boolean) {
		settings.music_videos = on ? 'true' : 'false';
		prefs.musicVideos = on;
		await api.setSetting('music_videos', settings.music_videos);
	}

	async function setHideVideos(on: boolean) {
		settings.hide_videos = on ? 'true' : 'false';
		await api.setSetting('hide_videos', settings.hide_videos);
	}

	async function setBoidu(on: boolean) {
		settings.lyrics_boidu = on ? 'true' : 'false';
		await api.setSetting('lyrics_boidu', settings.lyrics_boidu);
	}

	async function setPreventDuplicates(on: boolean) {
		settings.prevent_duplicates = on ? 'true' : 'false';
		await api.setSetting('prevent_duplicates', settings.prevent_duplicates);
	}

	async function setUpdateBanner(on: boolean) {
		settings.update_banner = on ? 'true' : 'false';
		await api.setSetting('update_banner', settings.update_banner);
	}

	async function setDiscord(on: boolean) {
		settings.discord_rpc = on ? 'true' : 'false';
		await api.setSetting('discord_rpc', settings.discord_rpc);
	}

	async function setTray(on: boolean) {
		settings.close_to_tray = on ? 'true' : 'false';
		await api.setSetting('close_to_tray', settings.close_to_tray);
	}

	async function setAutostart(on: boolean) {
		settings.autostart = on ? 'true' : 'false';
		try {
			await api.setSetting('autostart', settings.autostart);
		} catch (e) {
			settings.autostart = on ? 'false' : 'true'; // registration failed — revert the switch
			toast.error(String(e));
		}
	}

	async function toggleClient(name: string) {
		const set = new Set(disabled);
		if (set.has(name)) set.delete(name);
		else set.add(name);
		settings.disabled_stream_clients = [...set].join(',');
		await api.setSetting('disabled_stream_clients', settings.disabled_stream_clients);
	}

	async function saveProxy() {
		settings.proxy = proxyInput.trim();
		await api.setSetting('proxy', settings.proxy);
		toast.success('Proxy saved — restart to apply');
	}

	async function doClearCaches() {
		clearing = true;
		try {
			await api.clearCaches();
			toast.success('Caches cleared');
		} finally {
			clearing = false;
		}
	}

	function applyMaxPerformance() {
		if (theme.id === 'glassy') {
			toggleGlassyTheme(false);
		}
		setAppearance({
			reduceTransparency: true,
			reduceMotion: true,
			artworkBackground: false,
			artworkAccent: false
		});
		setAnimatedArtwork(false);
		setMusicVideos(false);
		toast.success('Applied Maximum Performance profile');
	}

	function restoreHighQuality() {
		setAppearance({
			reduceTransparency: false,
			reduceMotion: false,
			artworkBackground: true
		});
		setAnimatedArtwork(true);
		toast.success('Restored High Quality Visuals');
	}
	function resetGlassyVisuals() {
		setAppearance({
			glassyWarp: 1.5,
			glassyLightness: 0.45,
			glassyBlur: 64,
			glassySaturation: 1.0
		});
		toast.success('Reset Glassy theme background settings');
	}

	function resetFullscreenVisuals() {
		setAppearance({
			fullscreenWarp: 1.6,
			fullscreenLightness: 0.45,
			fullscreenBlur: 64,
			fullscreenSaturation: 1.0
		});
		toast.success('Reset Fullscreen player background settings');
	}
</script>

<!-- One row shape for the whole modal: label and description on the left, the control on the right,
     and an optional block underneath for the things that expand (color picker, font input, lists). -->
{#snippet row(o: {
	title: string;
	desc?: string;
	badge?: string;
	badgeVariant?: 'default' | 'performance' | 'warning' | 'saving' | 'info';
	control?: Snippet;
	below?: Snippet;
	tall?: boolean;
})}
	<div class="px-4 py-3.5">
		<div class="flex {o.tall ? 'items-start' : 'items-center'} justify-between gap-6">
			<div class="min-w-0">
				<div class="flex items-center gap-2 flex-wrap">
					<span class="text-sm font-medium">{o.title}</span>
					{#if o.badge}
						<span
							class="rounded-full px-2 py-0.5 text-[10px] font-semibold tracking-wide {o.badgeVariant === 'performance'
								? 'bg-amber-500/15 text-amber-600 dark:text-amber-400 border border-amber-500/25'
								: o.badgeVariant === 'warning'
									? 'bg-rose-500/15 text-rose-600 dark:text-rose-400 border border-rose-500/25'
									: o.badgeVariant === 'saving'
										? 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 border border-emerald-500/25'
										: o.badgeVariant === 'info'
											? 'bg-sky-500/15 text-sky-600 dark:text-sky-400 border border-sky-500/25'
											: 'bg-primary/12 text-primary'}"
						>
							{o.badge}
						</span>
					{/if}
				</div>
				{#if o.desc}
					<p class="mt-1 max-w-prose text-xs leading-relaxed text-muted-foreground">{o.desc}</p>
				{/if}
			</div>
			{#if o.control}
				<div class="shrink-0">{@render o.control()}</div>
			{/if}
		</div>
		{#if o.below}
			<div class="mt-3">{@render o.below()}</div>
		{/if}
	</div>
{/snippet}

<svelte:window onkeydown={recordingAction ? onKeyRecord : undefined} />

<Dialog.Root bind:open={ui.settingsOpen}>
	<Dialog.Content class="gap-0 overflow-hidden p-0 sm:max-w-3xl lg:max-w-4xl">
		<Dialog.Description class="sr-only">Application settings</Dialog.Description>

		<div class="flex h-[min(38rem,80vh)]">
			<!-- Tab rail -->
			<nav class="flex w-52 shrink-0 flex-col border-r bg-muted/40 p-3">
				<Dialog.Title class="px-3 pt-1 pb-4 font-heading text-base font-semibold">
					Settings
				</Dialog.Title>
				<div class="flex flex-col gap-0.5">
					{#each TABS as t (t.id)}
						<button
							onclick={() => (tab = t.id)}
							aria-current={tab === t.id}
							class="flex w-full cursor-pointer items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm font-medium transition-colors {tab ===
							t.id
								? 'bg-background text-foreground shadow-sm ring-1 ring-border/70'
								: 'text-muted-foreground hover:bg-foreground/5 hover:text-foreground'}"
						>
							<HugeiconsIcon
								icon={t.icon}
								size={17}
								strokeWidth={2}
								class={tab === t.id ? 'text-primary' : ''}
							/>
							<span class="truncate">{t.label}</span>
						</button>
					{/each}
				</div>
				{#if version}
					<span class="mt-auto px-3 pb-1 text-[11px] text-muted-foreground">v{version}</span>
				{/if}
			</nav>

			<!-- Content pane. min-w-0: a flex child's min-width is auto, so without it one wide row
			     (a long font name, a long path) widens the pane and pushes every tab off the modal. -->
			<div class="flex min-w-0 flex-1 flex-col">
				<!-- h-14 also keeps the dialog's close button clear of the first row. -->
				<header class="flex h-14 shrink-0 flex-col justify-center border-b px-6 pr-14">
					<h2 class="text-sm font-semibold">{currentTab.label}</h2>
					<p class="truncate text-xs text-muted-foreground">{currentTab.hint}</p>
				</header>

				<div class="min-w-0 flex-1 overflow-y-auto px-6 py-5 pb-10">
					{#if !loaded}
						<p class="text-sm text-muted-foreground">Loading…</p>
					{:else if tab === 'general'}
						<!-- The shortcuts list has no other entry point in the chrome. Closing settings
						     first: two stacked dialogs would trap focus in the wrong one. -->
						<button
							type="button"
							class="mb-5 inline-flex items-center gap-2 rounded-full border bg-muted/50 px-3 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
							onclick={() => {
								ui.settingsOpen = false;
								ui.shortcutsOpen = true;
							}}
						>
							<HugeiconsIcon icon={KeyboardIcon} class="h-3.5 w-3.5" />
							Open the keyboard shortcuts with <kbd class="font-mono font-medium">{MOD}H</kbd>
						</button>
						<section class={GROUP}>
							<h3 class={LABEL}>Activity</h3>
							<div class={CARD}>
								{@render row({
									title: 'Watch history',
									desc: 'Register plays in your YouTube Music history. Needs sign-in.',
									control: historySwitch
								})}
								{@render row({
									title: 'Discord rich presence',
									desc: "Show what you're listening to on your Discord profile. Needs the Discord desktop app running, no login here.",
									control: discordSwitch
								})}
								{@render row({
									title: 'Last.fm scrobbling',
									desc: lastfmConnected
										? `Connected and scrobbling as ${lastfmUser}.`
										: 'Connect your Last.fm account to scrobble songs and update now playing.',
									control: lastfmButton,
									below: lastfmConfig
								})}
							</div>
						</section>
						<section class={GROUP}>
							<h3 class={LABEL}>System</h3>
							<div class={CARD}>
								{@render row({
									title: 'Close to tray',
									desc: 'Closing the window keeps music playing in the background. Restore or quit from the tray icon.',
									control: traySwitch
								})}
								{@render row({
									title: 'Start on login',
									desc: 'Launch Nocturne automatically when you log in.',
									control: autostartSwitch
								})}
							</div>
						</section>
					{:else if tab === 'themes'}
						<section class={GROUP}>
							<h3 class={LABEL}>Theme</h3>
							<div class={CARD}>
								{@render row({
									title: 'Preset',
									desc: 'Accent colors tint the default look; palettes swap every color.',
									control: presetSelect
								})}
								{@render row({
									title: 'Accent color',
									desc: 'Buttons, highlights and the progress bar. Applies over any preset.',
									control: accentSwatch,
									below: pickerOpen ? accentPicker : undefined
								})}
								{@render row({
									title: 'Background tint',
									desc:
										currentTheme.kind === 'palette'
											? `Only shades the default palette, ${currentTheme.label} brings its own colors.`
											: 'Shades the greys: surfaces, borders and secondary text.',
									control: tintSlider
								})}
								{@render row({
									title: 'Roundness',
									desc: 'Corner radius of cards, buttons and artwork.',
									control: radiusSlider
								})}
							</div>
						</section>

						<section class={GROUP}>
							<h3 class={LABEL}>Typography</h3>
							<div class={CARD}>
								{#each FONT_ROWS as fr (fr.key)}
									<!-- Zero-arg wrappers: a snippet passed as a value can't carry arguments. -->
									{#snippet pick()}{@render fontSelect(fr.key, fr.label)}{/snippet}
									{#snippet type()}{@render fontInput(fr.key, fr.label)}{/snippet}
									{@render row({
										title: fr.label,
										desc: fr.hint,
										control: pick,
										below: isCustomFont[fr.key] ? type : undefined
									})}
								{/each}
								{@render row({
									title: 'Font files',
									desc: 'Load a .ttf, .otf or .woff from anywhere on this computer. It joins both dropdowns above.',
									control: addFontButton,
									below: custom.fontFiles.length ? fontFileList : undefined
								})}
							</div>
						</section>

						<section class={GROUP}>
							<h3 class={LABEL}>Player view & effects</h3>
							<div class={CARD}>
								{@render row({
									title: 'Open the player when you press play',
									desc: 'On, playing a song, album or playlist opens the now playing sidebar beside the main content. Off, it starts playing and you stay on the current page.',
									control: openPlayerSwitch,
									tall: true
								})}
								{@render row({
									title: 'Queue and lyrics in the player view',
									desc: "On, the player view carries them as tabs and the bar's two buttons switch between them. Off, those buttons only ever open the side panels, which stay open over the player view so you can see both at once.",
									control: tabbedSwitch,
									tall: true
								})}
								{@render row({
									title: 'Artwork background',
									badge: 'Moderate GPU',
									badgeVariant: 'performance',
									desc: "Tint the player view with the playing track's cover, blurred. Off leaves it plain.",
									control: artworkBgSwitch,
									tall: true
								})}
								{@render row({
									title: 'Animated Fullscreen Background',
									badge: 'High GPU',
									badgeVariant: 'performance',
									desc: 'Display real-time fluid GPU shaders and domain-warped blur behind the fullscreen player.',
									control: animatedArtworkSwitch,
									tall: true
								})}
								{@render row({
									title: 'Adapt colors to artwork',
									badge: 'CPU Sampling',
									badgeVariant: 'warning',
									desc: "Recolor the app from the playing track's cover: accent, surfaces and borders, fading between tracks. Off keeps the selected theme's own colors.",
									control: artworkAccentSwitch,
									tall: true
								})}
								{@render row({
									title: 'Reduce transparency & blur',
									badge: 'Saves GPU & Battery',
									badgeVariant: 'saving',
									desc: 'Disables full-window backdrop-filter blurs and translucent glass surfaces across the app for significant rendering performance gains.',
									control: reduceTransparencySwitch,
									tall: true
								})}
								{@render row({
									title: 'Reduce animations & motion',
									badge: 'Saves CPU',
									badgeVariant: 'saving',
									desc: 'Disables UI transitions, marquee auto-scroll tickers, and spring animations for instant, lightweight response.',
									control: reduceMotionSwitch,
									tall: true
								})}
								{@render row({
									title: 'Reset customization',
									desc: 'Drop the color, roundness and font overrides. Keeps the preset.',
									control: resetButton
								})}
							</div>
						</section>

						<section class={GROUP}>
							<h3 class={LABEL}>Glassy Theme Background (Ambient App Wash)</h3>
							<div class={CARD}>
								{@render row({
									title: 'Warping intensity',
									desc: 'Controls the fluid wave distortion and liquid displacement in the ambient app background.',
									control: glassyWarpSlider,
									tall: true
								})}
								{@render row({
									title: 'Brightness & opacity',
									desc: 'Adjusts how brightly the ambient album art shines through behind the UI.',
									control: glassyLightnessSlider,
									tall: true
								})}
								{@render row({
									title: 'Blur radius',
									desc: 'Sets the gaussian blur radius applied over the ambient background art.',
									control: glassyBlurSlider,
									tall: true
								})}
								{@render row({
									title: 'Saturation',
									desc: 'Controls color vibrancy in the background wash.',
									control: glassySaturationSlider,
									tall: true
								})}
								{@render row({
									title: 'Reset Glassy background',
									desc: 'Restore default warp intensity, brightness, blur radius and saturation.',
									control: resetGlassyButton
								})}
							</div>
						</section>

						<section class={GROUP}>
							<h3 class={LABEL}>Fullscreen Player Background</h3>
							<div class={CARD}>
								{@render row({
									title: 'Warping intensity',
									desc: 'Controls the fluid wave distortion behind fullscreen lyrics and player.',
									control: fullscreenWarpSlider,
									tall: true
								})}
								{@render row({
									title: 'Brightness & opacity',
									desc: 'Adjusts background brightness and opacity in fullscreen mode.',
									control: fullscreenLightnessSlider,
									tall: true
								})}
								{@render row({
									title: 'Blur radius',
									desc: 'Sets the gaussian blur radius applied over fullscreen background artwork.',
									control: fullscreenBlurSlider,
									tall: true
								})}
								{@render row({
									title: 'Saturation',
									desc: 'Controls color vibrancy behind fullscreen playback.',
									control: fullscreenSaturationSlider,
									tall: true
								})}
								{@render row({
									title: 'Reset Fullscreen background',
									desc: 'Restore default warp intensity, brightness, blur radius and saturation.',
									control: resetFullscreenButton
								})}
							</div>
						</section>
					{:else if tab === 'playback'}
						<section class={GROUP}>
							<h3 class={LABEL}>Audio</h3>
							<div class={CARD}>
								{@render row({
									title: 'Audio quality',
									badge: 'Network & CPU',
									badgeVariant: 'info',
									desc: 'Preferred stream quality when resolving a track. Lower qualities use less data and CPU.',
									control: qualityPicker
								})}
								{@render row({
									title: 'Autoplay',
									desc: 'Keep the music going with similar songs when your queue ends.',
									control: autoplaySwitch
								})}
								{@render row({
									title: 'Prevent duplicate tracks in queue',
									desc: "Adding a track that's already in the queue moves it from its old position instead of adding a second copy.",
									control: dupSwitch,
									tall: true
								})}
								{@render row({
									title: 'Explicit content filter',
									desc: 'Filter and automatically skip tracks containing explicit lyrics or themes.',
									control: filterExplicitSwitch,
									tall: true
								})}
							</div>
						</section>
						<section class={GROUP}>
							<h3 class={LABEL}>Video</h3>
							<div class={CARD}>
								{@render row({
									title: 'Play music videos',
									badge: 'High GPU & Data',
									badgeVariant: 'performance',
									desc: 'When a track is a music video, the player shows the video instead of the artwork. Uses noticeably more data, hardware video decoding, and battery than audio alone.',
									control: musicVideoSwitch,
									tall: true
								})}
								{@render row({
									title: 'Animated Fullscreen Background',
									badge: 'High GPU',
									badgeVariant: 'performance',
									desc: 'Display real-time fluid GPU shaders and domain-warped blur behind the fullscreen player.',
									control: animatedArtworkSwitch,
									tall: true
								})}
								{@render row({
									title: 'Hide music videos',
									desc: "Keep only the audio version of a track, so the official video doesn't turn up beside it. Applies to newly loaded content.",
									control: hideVideoSwitch,
									tall: true
								})}
							</div>
						</section>
						<section class={GROUP}>
							<h3 class={LABEL}>Advanced</h3>
							<div class={CARD}>
								{@render row({ title: 'Stream clients', below: clientList })}
							</div>
						</section>
					{:else if tab === 'performance'}
						<section class={GROUP}>
							<h3 class={LABEL}>Quick Presets</h3>
							<div class={CARD}>
								{@render row({
									title: 'Performance profiles',
									desc: 'One-click configurations to optimize Nocturne Music for your hardware.',
									below: performancePresets
								})}
							</div>
						</section>

						<section class={GROUP}>
							<h3 class={LABEL}>Optimization & Battery Saving</h3>
							<div class={CARD}>
								{@render row({
									title: 'Reduce transparency & frosted blur',
									badge: 'Saves GPU & Battery',
									badgeVariant: 'saving',
									desc: 'Replaces expensive CSS backdrop-filter blurs and translucent panels with solid surfaces. Highly recommended for laptops on battery or integrated GPUs.',
									control: reduceTransparencySwitch,
									tall: true
								})}
								{@render row({
									title: 'Reduce motion & animations',
									badge: 'Saves CPU',
									badgeVariant: 'saving',
									desc: 'Disables UI transition animations, spring fly-ins, and marquee ticker scrolls for maximum responsiveness on lower-end CPUs.',
									control: reduceMotionSwitch,
									tall: true
								})}
							</div>
						</section>

						<section class={GROUP}>
							<h3 class={LABEL}>Resource Impact Settings</h3>
							<div class={CARD}>
								{@render row({
									title: 'Animated Fullscreen Background',
									badge: 'High GPU Impact',
									badgeVariant: 'performance',
									desc: 'Runs continuous WebGL domain-warping fluid shaders behind the fullscreen player. Automatically halts when paused or obscured.',
									control: animatedArtworkSwitch,
									tall: true
								})}
								{@render row({
									title: 'Play music videos',
									badge: 'High GPU & Data',
									badgeVariant: 'performance',
									desc: 'Streams and hardware-decodes high-definition video when available instead of static cover artwork.',
									control: musicVideoSwitch,
									tall: true
								})}
								{@render row({
									title: 'Glassy theme (Album art background)',
									badge: 'Moderate GPU',
									badgeVariant: 'performance',
									desc: 'Sets the entire app background to a dimmed and blurred version of the playing album art with frosted glass panels.',
									control: glassyThemeSwitch,
									tall: true
								})}
								{@render row({
									title: 'Artwork background wash',
									badge: 'Moderate GPU',
									badgeVariant: 'performance',
									desc: 'Full-window blurred cover wash in the Now Playing player view.',
									control: artworkBgSwitch,
									tall: true
								})}
								{@render row({
									title: 'Adapt colors to artwork',
									badge: 'CPU Sampling',
									badgeVariant: 'warning',
									desc: 'Reads cover image pixels on a 2D canvas to calculate dynamic palette colors on every song change.',
									control: artworkAccentSwitch,
									tall: true
								})}
							</div>
						</section>
					{:else if tab === 'lyrics'}
						<section class={GROUP}>
							<div class="flex items-center justify-between px-1 mb-2">
								<h3 class="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
									Lyrics Providers & Priority
								</h3>
								<button
									type="button"
									onclick={resetLyricsProviders}
									class="text-[11px] font-medium text-primary hover:underline cursor-pointer"
								>
									Reset priority
								</button>
							</div>
							<div class={CARD}>
								<div class="p-3 bg-muted/20 border-b border-border/40 text-xs text-muted-foreground">
									Nocturne queries lyrics providers in order from top to bottom. Use the arrows to set your preferred priority, and toggle any source on or off.
								</div>
								{#each lyricsProviders as p, idx (p.id)}
									<div
										class="flex items-center justify-between gap-3 px-4 py-3 transition-colors hover:bg-muted/10 {p.enabled
											? ''
											: 'opacity-50'}"
									>
										<div class="flex items-center gap-3 min-w-0 flex-1">
											<span
												class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-muted text-[10px] font-bold text-muted-foreground"
											>
												{idx + 1}
											</span>
											<div class="min-w-0 flex-1">
												<div class="flex items-center gap-2">
													<span class="text-xs font-semibold text-foreground">{p.name}</span>
													{#if p.badge}
														<span
															class="rounded bg-primary/10 px-1.5 py-0.5 text-[9px] font-medium text-primary"
														>
															{p.badge}
														</span>
													{/if}
												</div>
												<p class="text-[11px] text-muted-foreground truncate">{p.description}</p>
											</div>
										</div>

										<div class="flex items-center gap-2 shrink-0">
											<!-- Move Up / Down controls -->
											<div class="flex items-center gap-0.5 mr-1">
												<button
													type="button"
													disabled={idx === 0}
													onclick={() => moveProvider(idx, -1)}
													aria-label="Move {p.name} up"
													title="Move up"
													class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30 disabled:pointer-events-none cursor-pointer transition-colors"
												>
													<HugeiconsIcon icon={ArrowUp01Icon} class="h-3.5 w-3.5" />
												</button>
												<button
													type="button"
													disabled={idx === lyricsProviders.length - 1}
													onclick={() => moveProvider(idx, 1)}
													aria-label="Move {p.name} down"
													title="Move down"
													class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30 disabled:pointer-events-none cursor-pointer transition-colors"
												>
													<HugeiconsIcon icon={ArrowDown01Icon} class="h-3.5 w-3.5" />
												</button>
											</div>

											<!-- Enable / Disable Switch -->
											<Switch
												checked={p.enabled}
												onCheckedChange={(on) => toggleProvider(p.id, on)}
											/>
										</div>
									</div>
								{/each}
							</div>
						</section>
					{:else if tab === 'keybindings'}
						<div class="mb-5 flex items-center justify-between px-1">
							<div>
								<h3 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
									Keyboard Shortcuts & Keybindings
								</h3>
								<p class="text-xs text-muted-foreground mt-0.5">
									Click any keybind badge to record a new key combination.
								</p>
							</div>
							<Button
								variant="outline"
								size="sm"
								class="flex items-center gap-1.5 text-xs cursor-pointer text-muted-foreground hover:text-foreground"
								onclick={resetKeybindings}
							>
								<HugeiconsIcon icon={RotateLeft01Icon} class="h-3.5 w-3.5" />
								<span>Reset to defaults</span>
							</Button>
						</div>

						{#if recordingAction}
							<div class="mb-4 rounded-xl border border-primary/40 bg-primary/10 p-3.5 animate-in fade-in-0 duration-150">
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-2">
										<span class="relative flex h-2.5 w-2.5">
											<span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-primary opacity-75"></span>
											<span class="relative inline-flex rounded-full h-2.5 w-2.5 bg-primary"></span>
										</span>
										<span class="text-xs font-semibold text-primary">
											Recording keybind for: {SHORTCUT_DEFINITIONS.find((d) => d.id === recordingAction)?.label}
										</span>
									</div>
									<button
										type="button"
										onclick={stopRecording}
										class="text-xs text-muted-foreground hover:text-foreground underline cursor-pointer"
									>
										Cancel (Esc)
									</button>
								</div>
								<p class="mt-1 text-[11px] text-muted-foreground">
									Press the desired key or combination (e.g. <kbd class="font-mono px-1 py-0.5 rounded border bg-background/50">Ctrl+Space</kbd>, <kbd class="font-mono px-1 py-0.5 rounded border bg-background/50">Ctrl+Alt+N</kbd>, <kbd class="font-mono px-1 py-0.5 rounded border bg-background/50">F9</kbd>).
								</p>
							</div>
						{/if}

						{#each (['Playback', 'Navigation', 'General'] as const) as groupName}
							{@const groupDefs = SHORTCUT_DEFINITIONS.filter((d) => d.group === groupName)}
							{#if groupDefs.length}
								<section class={GROUP}>
									<h3 class={LABEL}>{groupName}</h3>
									<div class={CARD}>
										{#each groupDefs as def (def.id)}
											{@const currentKey = keybindings[def.id]}
											{@const isDefault = currentKey === def.defaultKey}
											{@const isRecordingThis = recordingAction === def.id}
											<div class="flex items-center justify-between gap-4 px-4 py-3 transition-colors hover:bg-muted/10">
												<div class="min-w-0 flex-1">
													<div class="flex items-center gap-2">
														<span class="text-xs font-semibold text-foreground">{def.label}</span>
														{#if !isDefault}
															<span class="rounded bg-primary/10 px-1.5 py-0.5 text-[9px] font-medium text-primary">
																Custom
															</span>
														{/if}
													</div>
													<p class="text-[11px] text-muted-foreground truncate">{def.description}</p>
												</div>

												<div class="flex items-center gap-2 shrink-0">
													<button
														type="button"
														onclick={() => (isRecordingThis ? stopRecording() : startRecording(def.id))}
														class="group/key min-w-[5.5rem] px-2.5 py-1.5 rounded-lg border text-center font-mono text-xs font-medium transition-all cursor-pointer {isRecordingThis
															? 'border-primary bg-primary text-primary-foreground shadow-md ring-2 ring-primary/30'
															: 'border-border/80 bg-muted/40 text-foreground hover:border-primary/50 hover:bg-accent/40'}"
													>
														{#if isRecordingThis}
															<span class="animate-pulse">Press keys...</span>
														{:else}
															<span>{formatKey(currentKey)}</span>
														{/if}
													</button>
													{#if !isDefault}
														<button
															type="button"
															title="Reset to default ({formatKey(def.defaultKey)})"
															onclick={() => setKeybinding(def.id, def.defaultKey)}
															class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground cursor-pointer transition-colors"
														>
															<HugeiconsIcon icon={RotateLeft01Icon} class="h-3.5 w-3.5" />
														</button>
													{/if}
												</div>
											</div>
										{/each}
									</div>
								</section>
							{/if}
						{/each}
					{:else if tab === 'data'}
						<section class={GROUP}>
							<h3 class={LABEL}>Network</h3>
							<div class={CARD}>
								{@render row({
									title: 'Proxy',
									desc: 'HTTP/SOCKS proxy for all YouTube traffic. Takes effect on restart.',
									below: proxyForm
								})}
							</div>
						</section>
						<section class={GROUP}>
							<h3 class={LABEL}>Storage</h3>
							<div class={CARD}>
								{@render row({
									title: 'Cache',
									desc: 'Clear cached stream URLs and downloaded audio bytes.',
									control: clearButton
								})}
							</div>
						</section>
					{:else if tab === 'about'}
						<div
							class="mb-7 rounded-xl border bg-gradient-to-br from-primary/8 to-transparent px-4 py-4"
						>
							<div class="flex items-center gap-2">
								<span class="font-heading text-lg font-bold">Nocturne Music</span>
								{#if version}
									<span
										class="rounded-full bg-primary/12 px-2 py-0.5 text-[11px] font-semibold text-primary"
									>
										v{version}
									</span>
								{/if}
							</div>
							<p class="mt-1.5 max-w-prose text-xs leading-relaxed text-muted-foreground">
								A cross-platform desktop YouTube Music client. Ad-free playback straight from
								YouTube's private API, with your real library and OS media keys.
							</p>
						</div>

						<section class={GROUP}>
							<h3 class={LABEL}>Updates</h3>
							<div class={CARD}>
								{@render row({
									title: 'Updates',
									desc: updateState.available && !updateState.canInstall
										? `Version ${updateState.available.version} is available. This build was installed by a package manager, so update it the same way.`
										: updateState.available
											? `Version ${updateState.available.version} is available.`
											: 'Check GitHub for a newer release.',
									control: updateButton,
									below: updateResult && !updateState.available ? updateAlert : undefined
								})}
								{@render row({
									title: 'Tell me about new versions',
									desc: 'Check on launch and show a banner when a newer version is out. Off means no check and no banner, so use the button above to look.',
									control: bannerSwitch,
									tall: true
								})}
							</div>
						</section>

						<section class={GROUP}>
							<h3 class={LABEL}>What's new</h3>
							<div class={CARD}>
								{@render row({
									title: 'Release notes',
									desc: 'What changed in this version and the ones before it.',
									below: changelog
								})}
							</div>
						</section>
					{/if}
				</div>
			</div>
		</div>
	</Dialog.Content>
</Dialog.Root>

<!-- Controls. Split out so the rows above read as a list of settings rather than a wall of markup. -->
{#snippet historySwitch()}<Switch checked={historyOn} onCheckedChange={setHistory} />{/snippet}
{#snippet discordSwitch()}<Switch checked={discordOn} onCheckedChange={setDiscord} />{/snippet}
{#snippet lastfmButton()}
	{#if lastfmConnected}
		<Button size="sm" variant="outline" onclick={disconnectLastfm}>Disconnect</Button>
	{:else}
		<Button size="sm" onclick={connectLastfm} disabled={lastfmConnecting}>
			{lastfmConnecting ? 'Connecting…' : 'Connect Last.fm'}
		</Button>
	{/if}
{/snippet}
{#snippet lastfmConfig()}
	<div class="mt-2.5 space-y-2.5 rounded-xl border border-border/60 bg-muted/30 p-3">
		<div class="flex items-center justify-between gap-2">
			<div>
				<span class="text-xs font-semibold text-foreground">API Credentials</span>
				<p class="text-[11px] text-muted-foreground">
					Type your Last.fm API Key and Shared Secret to authorize scrobbling directly in the app.
				</p>
			</div>
			<button
				type="button"
				onclick={() => api.openExternal('https://www.last.fm/api/account/create')}
				class="inline-flex items-center gap-1 text-[11px] font-medium text-primary hover:underline cursor-pointer"
			>
				<span>Get keys</span>
				<HugeiconsIcon icon={Link04Icon} class="h-3 w-3" />
			</button>
		</div>

		<div class="grid gap-2.5 sm:grid-cols-2">
			<div class="space-y-1">
				<label for="lastfm-api-key" class="text-[11px] font-medium text-muted-foreground">API Key</label>
				<input
					id="lastfm-api-key"
					type="text"
					bind:value={lastfmKeyInput}
					oninput={(e) => updateLastfmKey(e.currentTarget.value)}
					placeholder="Paste Last.fm API key"
					class="w-full rounded-md border border-border bg-background px-3 py-1.5 font-mono text-xs text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-1 focus:ring-primary shadow-xs"
				/>
			</div>
			<div class="space-y-1">
				<div class="flex items-center justify-between">
					<label for="lastfm-api-secret" class="text-[11px] font-medium text-muted-foreground">Shared Secret</label>
					<button
						type="button"
						onclick={() => (showSecret = !showSecret)}
						class="text-[10px] text-muted-foreground hover:text-foreground cursor-pointer flex items-center gap-1"
					>
						<HugeiconsIcon icon={showSecret ? ViewOffSlashIcon : ViewIcon} class="h-3 w-3" />
						<span>{showSecret ? 'Hide' : 'Show'}</span>
					</button>
				</div>
				<input
					id="lastfm-api-secret"
					type={showSecret ? 'text' : 'password'}
					bind:value={lastfmSecretInput}
					oninput={(e) => updateLastfmSecret(e.currentTarget.value)}
					placeholder="Paste Shared Secret"
					class="w-full rounded-md border border-border bg-background px-3 py-1.5 font-mono text-xs text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-1 focus:ring-primary shadow-xs"
				/>
			</div>
		</div>
	</div>
{/snippet}
{#snippet traySwitch()}<Switch checked={trayOn} onCheckedChange={setTray} />{/snippet}
{#snippet autostartSwitch()}<Switch checked={autostartOn} onCheckedChange={setAutostart} />{/snippet}
{#snippet autoplaySwitch()}<Switch checked={autoplayOn} onCheckedChange={setAutoplay} />{/snippet}
{#snippet dupSwitch()}<Switch
		checked={preventDuplicatesOn}
		onCheckedChange={setPreventDuplicates}
	/>{/snippet}
{#snippet filterExplicitSwitch()}<Switch
		checked={filterExplicitOn}
		onCheckedChange={setFilterExplicit}
	/>{/snippet}
{#snippet musicVideoSwitch()}<Switch checked={musicVideosOn} onCheckedChange={setMusicVideos} />{/snippet}
{#snippet animatedArtworkSwitch()}<Switch checked={animatedArtworkOn} onCheckedChange={setAnimatedArtwork} />{/snippet}
{#snippet hideVideoSwitch()}<Switch checked={hideVideosOn} onCheckedChange={setHideVideos} />{/snippet}
{#snippet boiduSwitch()}<Switch checked={boiduOn} onCheckedChange={setBoidu} />{/snippet}
{#snippet bannerSwitch()}<Switch checked={updateBannerOn} onCheckedChange={setUpdateBanner} />{/snippet}
{#snippet openPlayerSwitch()}<Switch
		checked={appearance.openPlayerOnPlay}
		onCheckedChange={(on) => setAppearance({ openPlayerOnPlay: on })}
	/>{/snippet}
{#snippet tabbedSwitch()}<Switch
		checked={appearance.tabbedPlayer}
		onCheckedChange={(on) => setAppearance({ tabbedPlayer: on })}
	/>{/snippet}
{#snippet artworkBgSwitch()}<Switch
		checked={appearance.artworkBackground}
		onCheckedChange={(on) => setAppearance({ artworkBackground: on })}
	/>{/snippet}
{#snippet artworkAccentSwitch()}<Switch
		checked={appearance.artworkAccent}
		onCheckedChange={(on) => setAppearance({ artworkAccent: on })}
	/>{/snippet}
{#snippet reduceTransparencySwitch()}<Switch
		checked={appearance.reduceTransparency}
		onCheckedChange={(on) => setAppearance({ reduceTransparency: on })}
	/>{/snippet}
{#snippet reduceMotionSwitch()}<Switch
		checked={appearance.reduceMotion}
		onCheckedChange={(on) => setAppearance({ reduceMotion: on })}
	/>{/snippet}
{#snippet performancePresets()}
	<div class="flex flex-wrap gap-2 pt-1">
		<Button variant="outline" size="sm" onclick={applyMaxPerformance} class="text-xs cursor-pointer">
			<HugeiconsIcon icon={FlashIcon} class="mr-1.5 h-3.5 w-3.5 text-amber-500" />
			Maximum Performance
		</Button>
		<Button variant="outline" size="sm" onclick={restoreHighQuality} class="text-xs cursor-pointer">
			<HugeiconsIcon icon={PaintBoardIcon} class="mr-1.5 h-3.5 w-3.5 text-primary" />
			High Quality Visuals
		</Button>
	</div>
{/snippet}

{#snippet glassyThemeSwitch()}<Switch
		checked={theme.id === 'glassy'}
		onCheckedChange={(on) => toggleGlassyTheme(on)}
	/>{/snippet}

{#snippet glassyWarpSlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{((appearance.glassyWarp) / 3.0) * 100}%"
			min="0"
			max="3.0"
			step="0.1"
			value={appearance.glassyWarp}
			oninput={(e) => setAppearance({ glassyWarp: parseFloat(e.currentTarget.value) })}
			aria-label="Glassy warping intensity"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{appearance.glassyWarp.toFixed(1)}x</span>
	</div>
{/snippet}

{#snippet glassyLightnessSlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{((appearance.glassyLightness - 0.1) / 0.9) * 100}%"
			min="0.1"
			max="1.0"
			step="0.05"
			value={appearance.glassyLightness}
			oninput={(e) => setAppearance({ glassyLightness: parseFloat(e.currentTarget.value) })}
			aria-label="Glassy brightness and opacity"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{Math.round(appearance.glassyLightness * 100)}%</span>
	</div>
{/snippet}

{#snippet glassyBlurSlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{(appearance.glassyBlur / 100) * 100}%"
			min="0"
			max="100"
			step="2"
			value={appearance.glassyBlur}
			oninput={(e) => setAppearance({ glassyBlur: parseInt(e.currentTarget.value, 10) })}
			aria-label="Glassy blur radius"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{appearance.glassyBlur}px</span>
	</div>
{/snippet}

{#snippet glassySaturationSlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{(appearance.glassySaturation / 2.0) * 100}%"
			min="0.0"
			max="2.0"
			step="0.05"
			value={appearance.glassySaturation}
			oninput={(e) => setAppearance({ glassySaturation: parseFloat(e.currentTarget.value) })}
			aria-label="Glassy saturation"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{Math.round(appearance.glassySaturation * 100)}%</span>
	</div>
{/snippet}

{#snippet fullscreenWarpSlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{((appearance.fullscreenWarp) / 3.0) * 100}%"
			min="0"
			max="3.0"
			step="0.1"
			value={appearance.fullscreenWarp}
			oninput={(e) => setAppearance({ fullscreenWarp: parseFloat(e.currentTarget.value) })}
			aria-label="Fullscreen warping intensity"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{appearance.fullscreenWarp.toFixed(1)}x</span>
	</div>
{/snippet}

{#snippet fullscreenLightnessSlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{((appearance.fullscreenLightness - 0.1) / 0.9) * 100}%"
			min="0.1"
			max="1.0"
			step="0.05"
			value={appearance.fullscreenLightness}
			oninput={(e) => setAppearance({ fullscreenLightness: parseFloat(e.currentTarget.value) })}
			aria-label="Fullscreen brightness and opacity"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{Math.round(appearance.fullscreenLightness * 100)}%</span>
	</div>
{/snippet}

{#snippet fullscreenBlurSlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{(appearance.fullscreenBlur / 100) * 100}%"
			min="0"
			max="100"
			step="2"
			value={appearance.fullscreenBlur}
			oninput={(e) => setAppearance({ fullscreenBlur: parseInt(e.currentTarget.value, 10) })}
			aria-label="Fullscreen blur radius"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{appearance.fullscreenBlur}px</span>
	</div>
{/snippet}

{#snippet fullscreenSaturationSlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{(appearance.fullscreenSaturation / 2.0) * 100}%"
			min="0.0"
			max="2.0"
			step="0.05"
			value={appearance.fullscreenSaturation}
			oninput={(e) => setAppearance({ fullscreenSaturation: parseFloat(e.currentTarget.value) })}
			aria-label="Fullscreen saturation"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{Math.round(appearance.fullscreenSaturation * 100)}%</span>
	</div>
{/snippet}

{#snippet resetGlassyButton()}
	<Button variant="outline" size="sm" onclick={resetGlassyVisuals}>Reset Glassy visuals</Button>
{/snippet}

{#snippet resetFullscreenButton()}
	<Button variant="outline" size="sm" onclick={resetFullscreenVisuals}>Reset Fullscreen visuals</Button>
{/snippet}

{#snippet presetSelect()}
	<Select.Root type="single" value={theme.id} onValueChange={(v) => applyTheme(v as ThemeId)}>
		<Select.Trigger class="w-44 shrink-0" aria-label="Theme">
			<span
				class="size-4 shrink-0 rounded-full ring-1 ring-black/10"
				style="background:{currentTheme.color}"
			></span>
			<span class="flex-1 truncate text-left">{currentTheme.label}</span>
		</Select.Trigger>
		<Select.Content>
			<Select.Group>
				<Select.GroupHeading>Accent colors</Select.GroupHeading>
				{#each ACCENT_THEMES as t (t.id)}
					<Select.Item value={t.id} label={t.label}>
						<span
							class="size-4 shrink-0 rounded-full ring-1 ring-black/10"
							style="background:{t.color}"
						></span>
						{t.label}
					</Select.Item>
				{/each}
			</Select.Group>
			<Select.Group>
				<Select.GroupHeading>Palettes</Select.GroupHeading>
				{#each PALETTE_THEMES as t (t.id)}
					<Select.Item value={t.id} label={t.label}>
						<span
							class="size-4 shrink-0 rounded-full ring-1 ring-black/10"
							style="background:{t.color}"
						></span>
						{t.label}
					</Select.Item>
				{/each}
			</Select.Group>
		</Select.Content>
	</Select.Root>
{/snippet}

{#snippet accentSwatch()}
	<button
		type="button"
		onclick={() => (pickerOpen = !pickerOpen)}
		aria-label="Choose accent color"
		aria-expanded={pickerOpen}
		class="size-8 cursor-pointer rounded-lg ring-1 ring-black/10 transition-transform hover:scale-105 {pickerOpen
			? 'ring-2 ring-primary/60'
			: ''}"
		style="background:{effective.accent}"
	></button>
{/snippet}

{#snippet accentPicker()}
	<ColorPicker value={effective.accent} onchange={(hex) => setCustom({ accent: hex })} />
{/snippet}

{#snippet tintSlider()}
	<Slider
		type="single"
		aria-label="Background tint"
		max={360}
		step={1}
		disabled={currentTheme.kind === 'palette'}
		value={effective.hue}
		onValueChange={(hue) => setCustom({ hue })}
		class="w-44 shrink-0 [&_[data-slot=slider-range]]:bg-transparent [&_[data-slot=slider-track]]:bg-[linear-gradient(to_right,#f00,#ff0,#0f0,#0ff,#00f,#f0f,#f00)]"
	/>
{/snippet}

{#snippet radiusSlider()}
	<div class="flex w-44 shrink-0 items-center gap-3">
		<Slider
			type="single"
			aria-label="Roundness"
			max={1.5}
			step={0.05}
			value={effective.radius}
			onValueChange={(radius) => setCustom({ radius })}
		/>
		<span class="w-10 shrink-0 text-right font-mono text-xs text-muted-foreground">
			{effective.radius.toFixed(2)}
		</span>
	</div>
{/snippet}

{#snippet fontSelect(key: FontKey, label: string)}
	<Select.Root
		type="single"
		value={isCustomFont[key] ? 'custom' : matchFont(effective[key])}
		onValueChange={(v) => chooseFont(key, v)}
	>
		<Select.Trigger class="w-44 shrink-0" aria-label={label}>
			<span class="min-w-0 flex-1 truncate text-left" style="font-family:{effective[key]}">
				{isCustomFont[key] ? 'Custom' : familyName(effective[key])}
			</span>
		</Select.Trigger>
		<!-- max-w: a loaded font's name is whatever the file was called, and the dropdown grows to
		     its widest item. -->
		<Select.Content class="max-w-64">
			{#each FONTS as f (f.value)}
				<Select.Item value={f.value} label={f.label}>
					<span class="block truncate" style="font-family:{f.value}">{f.label}</span>
				</Select.Item>
			{/each}
			{#if custom.fontFiles.length}
				<Select.Group>
					<Select.GroupHeading>Your fonts</Select.GroupHeading>
					{#each fileFonts() as f (f.value)}
						<Select.Item value={f.value} label={f.label}>
							<span class="block truncate" style="font-family:{f.value}">{f.label}</span>
						</Select.Item>
					{/each}
				</Select.Group>
			{/if}
			<Select.Item value="custom" label="Custom">Custom…</Select.Item>
		</Select.Content>
	</Select.Root>
{/snippet}

{#snippet fontInput(key: FontKey, label: string)}
	<Input
		value={fontName[key]}
		oninput={(e) => typeFont(key, e.currentTarget.value)}
		placeholder="Font installed on this computer, e.g. Inter"
		aria-label="{label} family name"
		spellcheck={false}
		style="font-family:{effective[key]}"
	/>
	<!-- Probes the *applied* family, not the half-typed one: measuring a font on every keystroke is
	     the other half of #97, and a name mid-typing is never installed anyway. -->
	{#if fontName[key].trim() && !fontAvailable(familyName(effective[key]))}
		<p class="mt-1.5 text-xs text-muted-foreground">
			Not installed — install the font, then reopen settings.
		</p>
	{/if}
{/snippet}

{#snippet addFontButton()}
	<Button variant="outline" size="sm" class="shrink-0" onclick={pickFontFiles}>Add font…</Button>
{/snippet}

{#snippet fontFileList()}
	<div class="flex flex-col gap-1.5">
		{#each custom.fontFiles as path (path)}
			<div class="flex items-center gap-3 rounded-lg bg-secondary/60 py-1.5 pr-1.5 pl-3 text-sm">
				<!-- The name is the identity; the path only earns a tooltip. A font called
				     BigBlueTerm437NerdFontMono-Regular is wider than the modal. -->
				<span class="min-w-0 flex-1 truncate" style="font-family:'{fileFamily(path)}'" title={path}>
					{fileFamily(path)}
				</span>
				<button
					type="button"
					onclick={() => removeFontFile(path)}
					aria-label="Remove {fileFamily(path)}"
					class="flex size-6 shrink-0 cursor-pointer items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
				>
					<HugeiconsIcon icon={Cancel01Icon} size={14} />
				</button>
			</div>
		{/each}
	</div>
{/snippet}

{#snippet resetButton()}
	<Button
		variant="outline"
		size="sm"
		disabled={isDefaultCustom()}
		onclick={() => {
			resetCustom();
			isCustomFont = { fontSans: false, fontHeading: false, fontLyrics: false };
			fontName = { fontSans: '', fontHeading: '', fontLyrics: '' };
		}}
	>
		Reset
	</Button>
{/snippet}

<!-- Segmented, not three buttons: the options are one exclusive choice and should look like it. -->
{#snippet qualityPicker()}
	<div class="flex rounded-lg bg-muted p-0.5">
		{#each QUALITIES as q (q.id)}
			<button
				type="button"
				onclick={() => setQuality(q.id)}
				aria-pressed={quality === q.id}
				class="cursor-pointer rounded-md px-3.5 py-1.5 text-xs font-medium transition-colors {quality ===
				q.id
					? 'bg-background text-foreground shadow-sm'
					: 'text-muted-foreground hover:text-foreground'}"
			>
				{q.label}
			</button>
		{/each}
	</div>
{/snippet}

{#snippet clientList()}
	<p class="mb-3 max-w-prose text-xs leading-relaxed text-muted-foreground">
		Turn a client off to skip it when resolving streams. Overridden by the
		<span class="font-mono">NOCTURNE_DISABLED_CLIENTS</span> env var.
	</p>
	<div class="flex flex-col gap-2">
		{#each clients as name (name)}
			<div class="flex items-center justify-between rounded-lg bg-muted/60 py-1.5 pr-2 pl-3">
				<span class="font-mono text-xs">{name}</span>
				<Switch checked={!disabled.has(name)} onCheckedChange={() => toggleClient(name)} />
			</div>
		{/each}
	</div>
{/snippet}

{#snippet proxyForm()}
	<form
		class="flex gap-2"
		onsubmit={(e) => {
			e.preventDefault();
			saveProxy();
		}}
	>
		<Input bind:value={proxyInput} placeholder="http://host:port (blank = none)" />
		<Button type="submit" variant="outline">Save</Button>
	</form>
{/snippet}

{#snippet clearButton()}
	<Button variant="destructive" size="sm" onclick={doClearCaches} disabled={clearing}>
		{clearing ? 'Clearing…' : 'Clear caches'}
	</Button>
{/snippet}

{#snippet updateButton()}
	{#if updateState.available && !updateState.canInstall}
		<Button size="sm" onclick={openDownloadPage}>Download</Button>
	{:else if updateState.available}
		<Button size="sm" onclick={installUpdate} disabled={updateState.installing}>
			{updateState.installing ? 'Updating…' : 'Update now'}
		</Button>
	{:else}
		<Button variant="outline" size="sm" onclick={checkUpdates} disabled={updateState.checking}>
			{updateState.checking ? 'Checking…' : 'Check for updates'}
		</Button>
	{/if}
{/snippet}

{#snippet updateAlert()}
	<Alert variant={updateResult?.error ? 'destructive' : 'default'}>
		<AlertDescription>{updateResult?.message}</AlertDescription>
	</Alert>
{/snippet}

{#snippet changelog()}
	<Changelog current={version} />
{/snippet}
