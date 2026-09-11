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
		Refresh01Icon,
		Link04Icon,
		KeyboardIcon,
		ArrowUp01Icon,
		ArrowDown01Icon,
		ArrowRight01Icon,
		Mic01Icon,
		RotateLeft01Icon,
		FlashIcon,
		CpuIcon,
		ComputerIcon,
		SmartPhone01Icon,
		CheckmarkCircle02Icon,
		Wifi01Icon,
		FavouriteIcon,
		Edit02Icon,
		Delete02Icon,
		Tick02Icon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Switch } from '$lib/components/ui/switch';
	import { Slider } from '$lib/components/ui/slider';
	import { Alert, AlertDescription } from '$lib/components/ui/alert';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as api from '$lib/api';
	import {
		prefs,
		ui,
		toast,
		setFloatingTopBar,
		setFloatingSidebarLeft,
		setFloatingSidebarRight,
		setFloatingPlayerBar,
		setVisibleIcon
	} from '$lib/player.svelte';
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
	const CARD =
		'divide-y divide-border/60 overflow-hidden rounded-xl border border-border/60 bg-card/80 dark:bg-card/65 backdrop-blur-md shadow-xs';

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
	let clientStats = $state<Record<string, api.ClientStats>>({});
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

	async function loadLastfm() {
		try {
			const s = await api.lastfmStatus();
			lastfmConnected = s.connected;
			lastfmUser = s.username ?? null;
		} catch {}
	}

	async function connectLastfm() {
		lastfmConnecting = true;
		try {
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
		// Without this syncInfo stays null and the pairing PIN row cannot show the server's PIN.
		void fetchSyncStatus();
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
			const [s, c, stats] = await Promise.all([
				api.getSettings(),
				api.getStreamClients(),
				api.getClientLatencies().catch(() => []),
				loadLastfm()
			]);
			settings = s;
			let clientOrder = c;
			if (s.stream_client_priority) {
				try {
					const parsed = JSON.parse(s.stream_client_priority);
					if (Array.isArray(parsed) && parsed.length) {
						const set = new Set(parsed);
						clientOrder = [...parsed, ...c.filter((x) => !set.has(x))];
					}
				} catch {}
			}
			clients = clientOrder;
			autoRankClients = s.stream_client_auto_rank !== 'false';
			const smap: Record<string, api.ClientStats> = {};
			for (const st of stats as api.ClientStats[]) {
				smap[st.key] = st;
			}
			clientStats = smap;
			proxyInput = s.proxy ?? '';
			remoteSyncPort = s.remote_sync_port ?? '8080';
			initLyricsProviders(s.lyrics_providers ?? s.lyrics_priority);
			await Promise.all([loadCustomProviders(), loadCacheStats()]);
		} catch (e) {
			toast.error(String(e));
		}
		loaded = true;
	}

	// Custom lyric providers state
	let customProviders = $state<api.CustomLyricProvider[]>([]);
	let newCustomName = $state('');
	let newCustomUrl = $state('');
	let newCustomFormat = $state<'lrclib' | 'ttml' | 'lrc' | 'json'>('lrclib');
	let showAddCustom = $state(false);

	async function loadCustomProviders() {
		try {
			customProviders = await api.getCustomLyricProviders();
		} catch (e) {
			console.error('Failed to load custom lyric providers', e);
		}
	}

	async function addCustomProvider() {
		if (!newCustomName.trim() || !newCustomUrl.trim()) {
			toast.error('Provider name and URL are required');
			return;
		}
		const id = 'custom_' + Date.now();
		const updated = [
			...customProviders,
			{
				id,
				name: newCustomName.trim(),
				url: newCustomUrl.trim(),
				format: newCustomFormat,
				enabled: true
			}
		];
		try {
			await api.saveCustomLyricProviders(updated);
			customProviders = updated;
			newCustomName = '';
			newCustomUrl = '';
			showAddCustom = false;
			toast.success('Custom lyric provider added');
		} catch (e) {
			toast.error(String(e));
		}
	}

	async function toggleCustomProvider(id: string, on: boolean) {
		const updated = customProviders.map((p) => (p.id === id ? { ...p, enabled: on } : p));
		try {
			await api.saveCustomLyricProviders(updated);
			customProviders = updated;
		} catch (e) {
			toast.error(String(e));
		}
	}

	async function removeCustomProvider(id: string) {
		const updated = customProviders.filter((p) => p.id !== id);
		try {
			await api.saveCustomLyricProviders(updated);
			customProviders = updated;
			toast.success('Custom lyric provider removed');
		} catch (e) {
			toast.error(String(e));
		}
	}

	// Cache stats & limit state
	let cacheStats = $state<api.CacheStats | null>(null);
	let cacheLimitMb = $state<number | null>(null);

	function formatBytes(bytes: number): string {
		if (!bytes || bytes === 0) return '0 B';
		const k = 1024;
		const sizes = ['B', 'KB', 'MB', 'GB'];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
	}

	async function loadCacheStats() {
		try {
			const stats = await api.getCacheStats();
			cacheStats = stats;
			cacheLimitMb = stats.limit_mb;
		} catch (e) {
			console.error('Failed to load cache stats', e);
		}
	}

	async function updateCacheLimit(mb: number | null) {
		cacheLimitMb = mb;
		try {
			await api.setCacheLimit(mb);
			toast.success(
				mb
					? `Cache limit set to ${mb >= 1024 ? mb / 1024 + ' GB' : mb + ' MB'}`
					: 'Cache limit set to Unlimited'
			);
			await loadCacheStats();
		} catch (e) {
			toast.error(String(e));
		}
	}

	async function clearCacheKind(which: 'audio' | 'cipher' | 'covers' | 'all') {
		clearing = true;
		try {
			await api.clearCacheData(which);
			toast.success(`${which === 'all' ? 'All caches' : which + ' cache'} cleared`);
			await loadCacheStats();
		} catch (e) {
			toast.error(String(e));
		} finally {
			clearing = false;
		}
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
		},
		{
			id: 'youlyplus',
			name: 'YouLyPlus',
			description: 'High-accuracy synchronized lyrics with word-level annotations and community sync.',
			badge: 'Enhanced'
		},
		{
			id: 'paxsenix',
			name: 'Paxsenix',
			description: 'Aggregated synchronized and romanized lyrics provider covering broad streaming catalogues.',
			badge: 'Universal'
		}
	];

	let lyricsProviders = $state<LyricsProviderInfo[]>([]);

	function initLyricsProviders(saved?: string) {
		const defaultIds = ['betterlyrics', 'lrclib', 'ytm', 'qq', 'kugou', 'youlyplus', 'paxsenix'];
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
	const crossfadeSecs = $derived(parseInt(settings.crossfade_seconds || '0', 10) || 0);
	const discordShowTime = $derived(settings.discord_rpc_show_time !== 'false');
	const discordShowPause = $derived(settings.discord_rpc_show_pause !== 'false');
	const discordShowButton = $derived(settings.discord_rpc_show_button !== 'false');
	const discordButtonLabel = $derived(settings.discord_rpc_button_label ?? 'Listen on Nocturne');
	const discordDetails = $derived(settings.discord_rpc_details ?? '{title}');
	const discordState = $derived(settings.discord_rpc_state ?? '{artist}');
	const discordAppId = $derived(settings.discord_rpc_app_id ?? '');

	async function setCrossfade(secs: number) {
		settings.crossfade_seconds = secs.toString();
		await api.setSetting('crossfade_seconds', secs.toString());
	}

	async function setDiscordShowTime(on: boolean) {
		settings.discord_rpc_show_time = on ? 'true' : 'false';
		await api.setSetting('discord_rpc_show_time', settings.discord_rpc_show_time);
	}

	async function setDiscordShowPause(on: boolean) {
		settings.discord_rpc_show_pause = on ? 'true' : 'false';
		await api.setSetting('discord_rpc_show_pause', settings.discord_rpc_show_pause);
	}

	async function setDiscordShowButton(on: boolean) {
		settings.discord_rpc_show_button = on ? 'true' : 'false';
		await api.setSetting('discord_rpc_show_button', settings.discord_rpc_show_button);
	}

	async function setDiscordButtonLabel(label: string) {
		settings.discord_rpc_button_label = label;
		await api.setSetting('discord_rpc_button_label', label);
	}

	async function setDiscordDetails(val: string) {
		settings.discord_rpc_details = val;
		await api.setSetting('discord_rpc_details', val);
	}

	async function setDiscordState(val: string) {
		settings.discord_rpc_state = val;
		await api.setSetting('discord_rpc_state', val);
	}

	async function setDiscordAppId(val: string) {
		settings.discord_rpc_app_id = val;
		await api.setSetting('discord_rpc_app_id', val);
	}
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

	const remoteSyncOn = $derived(settings.remote_sync_enabled === 'true');
	let remoteSyncPort = $state('8080');
	let syncInfo = $state<api.RemoteSyncInfo | null>(null);
	// The server is the only source for this; empty means it holds no PIN a phone could use.
	const pairingPin = $derived(syncInfo?.pairing_pin ?? '');
	let showHostIp = $state(false);

	function maskIp(ip: string) {
		if (!ip) return '•••.•••.•••.•••';
		const parts = ip.split('.');
		if (parts.length === 4) {
			return `${parts[0]}.${parts[1]}.•••.•••`;
		}
		return '••••••••••••';
	}

	async function fetchSyncStatus() {
		try {
			syncInfo = await api.getRemoteSyncStatus();
		} catch (e) {
			console.error('Failed to get remote sync status', e);
		}
	}

	async function setRemoteSync(on: boolean) {
		settings.remote_sync_enabled = on ? 'true' : 'false';
		await api.setSetting('remote_sync_enabled', settings.remote_sync_enabled);
		// Enabling generates a PIN on the Rust side, so the panel must re-read it rather than show the old value.
		await fetchSyncStatus();
		if (on) toast.success(`Remote Sync Server listening on port ${remoteSyncPort || '8080'}`);
		else toast('Remote Sync Server stopped');
	}

	async function updateRemoteSyncPort(val: string) {
		remoteSyncPort = val;
		settings.remote_sync_port = val;
		await api.setSetting('remote_sync_port', val);
	}

	let showPairingPin = $state(false);

	// Minted in Rust from a CSPRNG; Math.random is not suitable for a pairing credential.
	async function regenerateRemoteSyncPin() {
		try {
			await api.regenerateRemoteSyncPin();
		} catch (e) {
			console.error('Failed to regenerate pairing PIN', e);
		}
		await fetchSyncStatus();
	}

	let editingDeviceId = $state<string | null>(null);
	let editingDeviceName = $state('');

	function startRename(device: api.PairedDevice) {
		editingDeviceId = device.id;
		editingDeviceName = device.name;
	}

	async function saveRename(id: string) {
		const trimmed = editingDeviceName.trim();
		if (!trimmed) return;
		try {
			await api.renameRemoteSyncDevice(id, trimmed);
			toast.success('Device renamed');
			await fetchSyncStatus();
		} catch (e) {
			toast.error(String(e));
		} finally {
			editingDeviceId = null;
		}
	}

	async function toggleFavorite(device: api.PairedDevice) {
		try {
			await api.setRemoteSyncDeviceFavorite(device.id, !device.is_favorite);
			toast.success(!device.is_favorite ? 'Device added to trusted favorites (bypasses PIN)' : 'Device removed from favorites');
			await fetchSyncStatus();
		} catch (e) {
			toast.error(String(e));
		}
	}

	async function removeDevice(id: string) {
		try {
			await api.removeRemoteSyncDevice(id);
			toast.success('Device unpaired');
			await fetchSyncStatus();
		} catch (e) {
			toast.error(String(e));
		}
	}

	let autoRankClients = $state(true);
	let benchmarkingClients = $state(false);

	async function runClientBenchmark() {
		benchmarkingClients = true;
		try {
			const stats = await api.benchmarkStreamClients();
			const smap: Record<string, api.ClientStats> = {};
			for (const st of stats) {
				smap[st.key] = st;
			}
			clientStats = smap;
			toast.success('Stream client latencies updated');
		} catch (e) {
			toast.error(String(e));
		} finally {
			benchmarkingClients = false;
		}
	}

	async function toggleAutoRank(on: boolean) {
		autoRankClients = on;
		settings.stream_client_auto_rank = on ? 'true' : 'false';
		await api.setSetting('stream_client_auto_rank', settings.stream_client_auto_rank);
		toast.success(on ? 'Auto-ranking enabled' : 'Custom stream client priority active');
	}

	function moveClientUp(index: number) {
		if (index <= 0) return;
		const next = [...clients];
		const temp = next[index];
		next[index] = next[index - 1];
		next[index - 1] = temp;
		clients = next;
		saveStreamClientPriority();
	}

	function moveClientDown(index: number) {
		if (index >= clients.length - 1) return;
		const next = [...clients];
		const temp = next[index];
		next[index] = next[index + 1];
		next[index + 1] = temp;
		clients = next;
		saveStreamClientPriority();
	}

	async function saveStreamClientPriority() {
		settings.stream_client_priority = JSON.stringify(clients);
		await api.setSetting('stream_client_priority', settings.stream_client_priority);
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

	function applyBalanced() {
		if (theme.id === 'glassy') {
			toggleGlassyTheme(false);
		}
		setAppearance({
			reduceTransparency: false,
			reduceMotion: false,
			artworkBackground: false
		});
		setAnimatedArtwork(false);
		toast.success('Applied Balanced profile (Default)');
	}

	function applyHighQuality() {
		toggleGlassyTheme(true);
		setAnimatedArtwork(true);
		setAppearance({
			reduceTransparency: false,
			reduceMotion: false,
			artworkBackground: true
		});
		toast.success('Applied High Quality profile (Glassy theme, Animated BG, Artwork Wash)');
	}
	function resetGlassyVisuals() {
		setAppearance({
			glassyWarp: 1.5,
			glassySpeed: 0.5,
			glassyLightness: 0.45,
			glassyBlur: 64,
			glassySaturation: 1.0
		});
		toast.success('Reset Glassy theme background settings');
	}

	function resetFullscreenVisuals() {
		setAppearance({
			fullscreenWarp: 1.6,
			fullscreenSpeed: 0.6,
			fullscreenLightness: 0.45,
			fullscreenBlur: 64,
			fullscreenSaturation: 1.0
		});
		toast.success('Reset Fullscreen player background settings');
	}

	function resetTranslucencyVisuals() {
		setAppearance({
			dialogOpacity: 0.80,
			sidebarOpacity: 0.60,
			cardOpacity: 0.80,
			overlayDimming: 0.25
		});
		toast.success('Reset translucency & opacity settings');
	}

	const APPEARANCE_SECTIONS = [
		{ id: 'sec-theme', label: 'Theme & Accent' },
		{ id: 'sec-translucency', label: 'Translucency' },
		{ id: 'sec-typography', label: 'Typography' },
		{ id: 'sec-player', label: 'Player & Visuals' },
		{ id: 'sec-glassy', label: 'Glassy Theme' },
		{ id: 'sec-fullscreen', label: 'Fullscreen Player' },
		{ id: 'sec-layout', label: 'Layout & Icons' }
	];
	let activeAppearanceSection = $state('sec-theme');
	let collapsedCategories = $state<Record<string, boolean>>({});

	function scrollToAppearanceSection(id: string) {
		activeAppearanceSection = id;
		const el = document.getElementById(id);
		el?.scrollIntoView({ behavior: 'smooth', block: 'start' });
	}

	function toggleAllCategories(collapse: boolean) {
		for (const s of APPEARANCE_SECTIONS) {
			collapsedCategories[s.id] = collapse;
		}
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
	<Dialog.Content class="settings-dialog gap-0 overflow-hidden p-0 sm:max-w-3xl lg:max-w-4xl border border-border/80 shadow-2xl rounded-2xl">
		<Dialog.Description class="sr-only">Application settings</Dialog.Description>

		<div class="flex h-[min(38rem,80vh)]">
			<!-- Tab rail -->
			<nav class="settings-nav-rail flex w-52 shrink-0 flex-col border-r border-border/30 p-3 backdrop-blur-3xl">
				<Dialog.Title class="px-3 pt-3 pb-4 font-heading text-base font-semibold text-foreground">
					Settings
				</Dialog.Title>
				<div class="flex flex-col gap-0.5">
					{#each TABS as t (t.id)}
						<button
							onclick={() => (tab = t.id)}
							aria-current={tab === t.id}
							class="flex w-full cursor-pointer items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm font-medium transition-colors {tab ===
							t.id
								? 'bg-white/15 dark:bg-white/10 text-foreground shadow-xs ring-1 ring-white/10 font-semibold'
								: 'text-muted-foreground hover:bg-white/5 hover:text-foreground'}"
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
			<div class="settings-content-pane relative flex min-w-0 flex-1 flex-col overflow-hidden">
				<!-- Header with generous top padding matching desired.png and no dividing border -->
				<header class="shrink-0 px-8 pt-8 pb-3 pr-16">
					<h2 class="text-base font-bold tracking-tight text-foreground">{currentTab.label}</h2>
					<p class="mt-0.5 truncate text-xs text-muted-foreground">{currentTab.hint}</p>
				</header>

				<div
					class="relative min-w-0 flex-1 overflow-y-auto px-8 {tab === 'themes' ? 'pr-14' : ''} pb-12"
					onscroll={(e) => {
						if (tab !== 'themes') return;
						const target = e.currentTarget;
						const containerRect = target.getBoundingClientRect();
						for (const s of APPEARANCE_SECTIONS) {
							const el = document.getElementById(s.id);
							if (el) {
								const rect = el.getBoundingClientRect();
								if (rect.top - containerRect.top <= 140 && rect.bottom - containerRect.top > 40) {
									activeAppearanceSection = s.id;
								}
							}
						}
					}}
				>
					{#if !loaded}
						<p class="text-sm text-muted-foreground">Loading…</p>
					{:else if tab === 'general'}
						<!-- The shortcuts list has no other entry point in the chrome. Closing settings
						     first: two stacked dialogs would trap focus in the wrong one. -->
						<button
							type="button"
							class="mb-5 inline-flex items-center gap-2 rounded-full border border-border/60 bg-muted/40 dark:bg-muted/20 backdrop-blur-xs px-3 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted/70 hover:text-foreground"
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
									control: discordSwitch,
									below: discordOn ? discordConfig : undefined
								})}
								{@render row({
									title: 'Last.fm scrobbling',
									desc: lastfmConnected
										? `Connected and scrobbling as ${lastfmUser}.`
										: 'Connect your Last.fm account to scrobble songs and update now playing.',
									control: lastfmButton
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
						<section class={GROUP}>
							<h3 class={LABEL}>Remote Device Sync (Tailscale / LAN)</h3>
							<div class={CARD}>
								{@render row({
									title: 'Mobile Sync Server',
									desc: 'Allow Nocturne Mobile to connect directly over Tailscale or Local Wi-Fi to sync and control playback.',
									control: remoteSyncSwitch,
									below: remoteSyncConfig
								})}
							</div>
						</section>
					{:else if tab === 'themes'}
						<!-- Expand / Collapse all controls header -->
						<div class="mb-4 flex items-center justify-between px-1">
							<span class="text-xs text-muted-foreground">Customize colors, fonts, translucency, and visuals.</span>
							<div class="flex items-center gap-2">
								<button
									type="button"
									onclick={() => toggleAllCategories(false)}
									class="text-[11px] font-medium text-muted-foreground hover:text-foreground cursor-pointer transition-colors"
								>
									Expand all
								</button>
								<span class="text-border text-xs">•</span>
								<button
									type="button"
									onclick={() => toggleAllCategories(true)}
									class="text-[11px] font-medium text-muted-foreground hover:text-foreground cursor-pointer transition-colors"
								>
									Collapse all
								</button>
							</div>
						</div>

						<div class="space-y-6">
							<!-- 1. Theme & Accent Color -->
							<section id="sec-theme" class="{GROUP} scroll-mt-3">
							<button
								type="button"
								onclick={() => (collapsedCategories['sec-theme'] = !collapsedCategories['sec-theme'])}
								class="group/cat mb-2 flex w-full cursor-pointer items-center justify-between px-1 text-left text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground transition-colors hover:text-foreground"
							>
								<div class="flex items-center gap-1.5">
									<HugeiconsIcon
										icon={collapsedCategories['sec-theme'] ? ArrowRight01Icon : ArrowDown01Icon}
										class="h-3.5 w-3.5 text-muted-foreground transition-transform group-hover/cat:text-foreground"
									/>
									<span>Theme & Accent</span>
								</div>
								<span class="text-[10px] font-normal lowercase tracking-normal text-muted-foreground/60 group-hover/cat:text-primary">
									{collapsedCategories['sec-theme'] ? 'expand' : 'collapse'}
								</span>
							</button>
							{#if !collapsedCategories['sec-theme']}
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
							{/if}
						</section>

						<!-- 2. Translucency & Opacity -->
						<section id="sec-translucency" class="{GROUP} scroll-mt-3">
							<button
								type="button"
								onclick={() => (collapsedCategories['sec-translucency'] = !collapsedCategories['sec-translucency'])}
								class="group/cat mb-2 flex w-full cursor-pointer items-center justify-between px-1 text-left text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground transition-colors hover:text-foreground"
							>
								<div class="flex items-center gap-1.5">
									<HugeiconsIcon
										icon={collapsedCategories['sec-translucency'] ? ArrowRight01Icon : ArrowDown01Icon}
										class="h-3.5 w-3.5 text-muted-foreground transition-transform group-hover/cat:text-foreground"
									/>
									<span>Translucency & Opacity</span>
								</div>
								<span class="text-[10px] font-normal lowercase tracking-normal text-muted-foreground/60 group-hover/cat:text-primary">
									{collapsedCategories['sec-translucency'] ? 'expand' : 'collapse'}
								</span>
							</button>
							{#if !collapsedCategories['sec-translucency']}
								<div class={CARD}>
									{@render row({
										title: 'Settings content opacity',
										desc: 'Controls how translucent or opaque the settings content panel is (macOS vibrant style).',
										control: dialogOpacitySlider,
										tall: true
									})}
									{@render row({
										title: 'Navigation sidebar rail opacity',
										desc: 'Controls the transparency level of the left navigation sidebar column.',
										control: sidebarOpacitySlider,
										tall: true
									})}
									{@render row({
										title: 'Card & surface opacity',
										desc: 'Adjusts background opacity for settings cards and elevated rows.',
										control: cardOpacitySlider,
										tall: true
									})}
									{@render row({
										title: 'Dialog backdrop dimming',
										desc: 'Dimming intensity applied behind open modals and dialogs.',
										control: overlayDimmingSlider,
										tall: true
									})}
									{@render row({
										title: 'Reset translucency',
										desc: 'Restore default settings window opacity (80% content, 60% sidebar, 25% scrim).',
										control: resetTranslucencyButton
									})}
								</div>
							{/if}
						</section>

						<!-- 3. Typography -->
						<section id="sec-typography" class="{GROUP} scroll-mt-3">
							<button
								type="button"
								onclick={() => (collapsedCategories['sec-typography'] = !collapsedCategories['sec-typography'])}
								class="group/cat mb-2 flex w-full cursor-pointer items-center justify-between px-1 text-left text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground transition-colors hover:text-foreground"
							>
								<div class="flex items-center gap-1.5">
									<HugeiconsIcon
										icon={collapsedCategories['sec-typography'] ? ArrowRight01Icon : ArrowDown01Icon}
										class="h-3.5 w-3.5 text-muted-foreground transition-transform group-hover/cat:text-foreground"
									/>
									<span>Typography</span>
								</div>
								<span class="text-[10px] font-normal lowercase tracking-normal text-muted-foreground/60 group-hover/cat:text-primary">
									{collapsedCategories['sec-typography'] ? 'expand' : 'collapse'}
								</span>
							</button>
							{#if !collapsedCategories['sec-typography']}
								<div class={CARD}>
									{#each FONT_ROWS as fr (fr.key)}
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
							{/if}
						</section>

						<!-- 4. Player View & Effects -->
						<section id="sec-player" class="{GROUP} scroll-mt-3">
							<button
								type="button"
								onclick={() => (collapsedCategories['sec-player'] = !collapsedCategories['sec-player'])}
								class="group/cat mb-2 flex w-full cursor-pointer items-center justify-between px-1 text-left text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground transition-colors hover:text-foreground"
							>
								<div class="flex items-center gap-1.5">
									<HugeiconsIcon
										icon={collapsedCategories['sec-player'] ? ArrowRight01Icon : ArrowDown01Icon}
										class="h-3.5 w-3.5 text-muted-foreground transition-transform group-hover/cat:text-foreground"
									/>
									<span>Player View & Effects</span>
								</div>
								<span class="text-[10px] font-normal lowercase tracking-normal text-muted-foreground/60 group-hover/cat:text-primary">
									{collapsedCategories['sec-player'] ? 'expand' : 'collapse'}
								</span>
							</button>
							{#if !collapsedCategories['sec-player']}
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
							{/if}
						</section>

						<!-- 5. Glassy Theme Background -->
						<section id="sec-glassy" class="{GROUP} scroll-mt-3">
							<button
								type="button"
								onclick={() => (collapsedCategories['sec-glassy'] = !collapsedCategories['sec-glassy'])}
								class="group/cat mb-2 flex w-full cursor-pointer items-center justify-between px-1 text-left text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground transition-colors hover:text-foreground"
							>
								<div class="flex items-center gap-1.5">
									<HugeiconsIcon
										icon={collapsedCategories['sec-glassy'] ? ArrowRight01Icon : ArrowDown01Icon}
										class="h-3.5 w-3.5 text-muted-foreground transition-transform group-hover/cat:text-foreground"
									/>
									<span>Glassy Theme Background (Ambient App Wash)</span>
								</div>
								<span class="text-[10px] font-normal lowercase tracking-normal text-muted-foreground/60 group-hover/cat:text-primary">
									{collapsedCategories['sec-glassy'] ? 'expand' : 'collapse'}
								</span>
							</button>
							{#if !collapsedCategories['sec-glassy']}
								<div class={CARD}>
									{@render row({
										title: 'Warping intensity',
										desc: 'Controls the fluid wave distortion and liquid displacement in the ambient app background.',
										control: glassyWarpSlider,
										tall: true
									})}
									{@render row({
										title: 'Animation speed',
										desc: 'Sets the speed of fluid wave motion and liquid warping in the ambient background.',
										control: glassySpeedSlider,
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
										desc: 'Restore default warp intensity, speed, brightness, blur radius and saturation.',
										control: resetGlassyButton
									})}
								</div>
							{/if}
						</section>

						<!-- 6. Fullscreen Player Background -->
						<section id="sec-fullscreen" class="{GROUP} scroll-mt-3">
							<button
								type="button"
								onclick={() => (collapsedCategories['sec-fullscreen'] = !collapsedCategories['sec-fullscreen'])}
								class="group/cat mb-2 flex w-full cursor-pointer items-center justify-between px-1 text-left text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground transition-colors hover:text-foreground"
							>
								<div class="flex items-center gap-1.5">
									<HugeiconsIcon
										icon={collapsedCategories['sec-fullscreen'] ? ArrowRight01Icon : ArrowDown01Icon}
										class="h-3.5 w-3.5 text-muted-foreground transition-transform group-hover/cat:text-foreground"
									/>
									<span>Fullscreen Player Background</span>
								</div>
								<span class="text-[10px] font-normal lowercase tracking-normal text-muted-foreground/60 group-hover/cat:text-primary">
									{collapsedCategories['sec-fullscreen'] ? 'expand' : 'collapse'}
								</span>
							</button>
							{#if !collapsedCategories['sec-fullscreen']}
								<div class={CARD}>
									{@render row({
										title: 'Warping intensity',
										desc: 'Controls the fluid wave distortion behind fullscreen lyrics and player.',
										control: fullscreenWarpSlider,
										tall: true
									})}
									{@render row({
										title: 'Animation speed',
										desc: 'Sets the speed of fluid wave motion behind the fullscreen player.',
										control: fullscreenSpeedSlider,
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
										desc: 'Restore default warp intensity, speed, brightness, blur radius and saturation.',
										control: resetFullscreenButton
									})}
								</div>
							{/if}
						</section>

						<!-- 7. Layout & Icons -->
						<section id="sec-layout" class="{GROUP} scroll-mt-3">
							<button
								type="button"
								onclick={() => (collapsedCategories['sec-layout'] = !collapsedCategories['sec-layout'])}
								class="group/cat mb-2 flex w-full cursor-pointer items-center justify-between px-1 text-left text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground transition-colors hover:text-foreground"
							>
								<div class="flex items-center gap-1.5">
									<HugeiconsIcon
										icon={collapsedCategories['sec-layout'] ? ArrowRight01Icon : ArrowDown01Icon}
										class="h-3.5 w-3.5 text-muted-foreground transition-transform group-hover/cat:text-foreground"
									/>
									<span>Layout & Icons</span>
								</div>
								<span class="text-[10px] font-normal lowercase tracking-normal text-muted-foreground/60 group-hover/cat:text-primary">
									{collapsedCategories['sec-layout'] ? 'expand' : 'collapse'}
								</span>
							</button>
							{#if !collapsedCategories['sec-layout']}
								<div class={CARD}>
									{@render row({
										title: 'Floating panels',
										desc: 'Choose which surfaces float with rounded corners, margins, and translucent blur.',
										below: floatingPanelsConfig
									})}
									{@render row({
										title: 'Top titlebar icons',
										desc: 'Choose which action icons appear on the top bar.',
										below: titlebarIconsConfig
									})}
									{@render row({
										title: 'Bottom player bar icons',
										desc: 'Choose which controls appear on the bottom player bar.',
										below: playerbarIconsConfig
									})}
								</div>
							{/if}
						</section>
					</div>
					{:else if tab === 'playback'}
						<section class={GROUP}>
							<h3 class={LABEL}>Audio</h3>
							<div class={CARD}>
								{@render row({
									title: 'Crossfade',
									desc: crossfadeSecs === 0 ? 'Off (gapless playback)' : `Smoothly crossfade between songs (${crossfadeSecs}s).`,
									control: crossfadeSlider,
									tall: true
								})}
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
									title: 'Aggressive Memory Trimming (Experimental - Linux)',
									badge: 'Linux glibc',
									badgeVariant: 'warning',
									desc: 'Periodically calls malloc_trim to release unused heap pages back to the OS. Default disabled to prevent idle audio/webview thread crashes.',
									control: aggressiveTrimSwitch,
									tall: true
								})}
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

						<section class={GROUP}>
							<div class="flex items-center justify-between px-1 mb-2">
								<div>
									<h3 class="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
										Custom Lyric Providers
									</h3>
									<p class="text-xs text-muted-foreground mt-0.5">
										Add external APIs with URL templates like <code>{'{title}'}</code>, <code>{'{artist}'}</code>, <code>{'{duration}'}</code>, <code>{'{videoId}'}</code>.
									</p>
								</div>
								<Button
									variant="outline"
									size="sm"
									onclick={() => (showAddCustom = !showAddCustom)}
									class="text-xs cursor-pointer"
								>
									{showAddCustom ? 'Cancel' : 'Add provider'}
								</Button>
							</div>

							{#if showAddCustom}
								<div class="mb-4 rounded-xl border border-primary/40 bg-muted/30 p-4 space-y-3">
									<h4 class="text-xs font-semibold text-foreground">New Custom Lyric Provider</h4>
									<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
										<div>
											<label class="text-[11px] text-muted-foreground block mb-1">Provider Name</label>
											<Input
												placeholder="e.g. My Lyrics API"
												bind:value={newCustomName}
												class="text-xs"
											/>
										</div>
										<div>
											<label class="text-[11px] text-muted-foreground block mb-1">Response Format</label>
											<select
												bind:value={newCustomFormat}
												class="w-full h-9 rounded-md border border-border bg-background px-3 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
											>
												<option value="lrclib">LRCLIB JSON (syncedLyrics/plainLyrics)</option>
												<option value="ttml">TTML / XML (Timed Text)</option>
												<option value="lrc">Plain LRC ([00:12.34] text)</option>
												<option value="json">Generic JSON (lyrics / lines)</option>
											</select>
										</div>
									</div>
									<div>
										<label class="text-[11px] text-muted-foreground block mb-1">
											URL Template (supports <code>{'{title}'}</code>, <code>{'{artist}'}</code>, <code>{'{duration}'}</code>, <code>{'{videoId}'}</code>)
										</label>
										<Input
											placeholder={'https://example.com/api/lyrics?title={title}&artist={artist}&duration={duration}'}
											bind:value={newCustomUrl}
											class="text-xs font-mono"
										/>
									</div>
									<div class="flex justify-end gap-2 pt-1">
										<Button variant="ghost" size="sm" onclick={() => (showAddCustom = false)}>
											Cancel
										</Button>
										<Button size="sm" onclick={addCustomProvider}>
											Save Provider
										</Button>
									</div>
								</div>
							{/if}

							<div class={CARD}>
								{#if customProviders.length === 0}
									<div class="p-4 text-center text-xs text-muted-foreground">
										No custom lyric providers configured. Click "Add provider" above to connect one.
									</div>
								{:else}
									{#each customProviders as cp (cp.id)}
										<div class="flex items-center justify-between gap-3 px-4 py-3 transition-colors hover:bg-muted/10">
											<div class="min-w-0 flex-1">
												<div class="flex items-center gap-2">
													<span class="text-xs font-semibold text-foreground">{cp.name}</span>
													<span class="rounded bg-muted px-1.5 py-0.5 text-[9px] font-mono text-muted-foreground uppercase">
														{cp.format}
													</span>
												</div>
												<p class="text-[11px] font-mono text-muted-foreground truncate mt-0.5">{cp.url}</p>
											</div>
											<div class="flex items-center gap-3 shrink-0">
												<button
													type="button"
													onclick={() => removeCustomProvider(cp.id)}
													title="Delete provider"
													class="text-muted-foreground hover:text-destructive text-xs cursor-pointer p-1 transition-colors"
												>
													<HugeiconsIcon icon={Cancel01Icon} class="h-3.5 w-3.5" />
												</button>
												<Switch
													checked={cp.enabled}
													onCheckedChange={(on) => toggleCustomProvider(cp.id, on)}
												/>
											</div>
										</div>
									{/each}
								{/if}
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
							<h3 class={LABEL}>Storage & Cache Management</h3>
							<div class={CARD}>
								{@render row({
									title: 'Cache Usage',
									desc: cacheStats
										? `Currently using ${formatBytes(cacheStats.total_bytes)} of storage.`
										: 'Calculating storage usage...',
									control: clearButton,
									below: cacheStatsBreakdown
								})}
								{@render row({
									title: 'Cache storage limit',
									desc: 'Nocturne automatically prunes oldest audio chunks when the cache exceeds this limit.',
									control: cacheLimitSelector
								})}
							</div>
						</section>
					{:else if tab === 'about'}
						<div
							class="mb-7 rounded-xl border border-border/60 bg-gradient-to-br from-primary/10 via-card/75 to-card/45 backdrop-blur-md px-4 py-4"
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

				<!-- Quick Section Dots Navigation (Sticky & pinned on the right of Appearance tab) -->
				{#if tab === 'themes'}
					<nav class="absolute right-3.5 top-24 z-30 hidden sm:flex flex-col items-center gap-2 rounded-full border border-white/15 dark:border-white/10 bg-card/50 dark:bg-black/50 py-2.5 px-1.5 shadow-lg backdrop-blur-2xl" aria-label="Appearance section dots">
						{#each APPEARANCE_SECTIONS as sec}
							<button
								type="button"
								onclick={() => scrollToAppearanceSection(sec.id)}
								class="group relative flex size-5 items-center justify-center rounded-full cursor-pointer transition-transform hover:scale-110 active:scale-95"
								title={sec.label}
								aria-label="Jump to {sec.label}"
							>
								<span class="rounded-full transition-all duration-200 {activeAppearanceSection === sec.id ? 'size-2.5 bg-white dark:bg-white shadow-[0_0_8px_rgba(255,255,255,0.8)]' : 'size-1.5 bg-white/25 dark:bg-white/25 group-hover:bg-white/60'}"></span>
								<!-- Tooltip on hover -->
								<span class="pointer-events-none absolute right-8 whitespace-nowrap rounded-[--radius] border border-border/70 bg-popover/95 px-2 py-0.5 text-[10px] font-semibold text-popover-foreground opacity-0 shadow-sm backdrop-blur-md transition-opacity duration-150 group-hover:opacity-100">
									{sec.label}
								</span>
							</button>
						{/each}
					</nav>
				{/if}
			</div>
		</div>
	</Dialog.Content>
</Dialog.Root>

<!-- Controls. Split out so the rows above read as a list of settings rather than a wall of markup. -->
{#snippet historySwitch()}<Switch checked={historyOn} onCheckedChange={setHistory} />{/snippet}
{#snippet discordSwitch()}<Switch checked={discordOn} onCheckedChange={setDiscord} />{/snippet}
{#snippet discordConfig()}
	<div class="mt-2.5 space-y-3 rounded-xl border border-border/60 bg-muted/35 dark:bg-muted/20 p-3.5 backdrop-blur-md">
		<div class="text-xs font-semibold text-foreground">Discord RPC Customization</div>

		<!-- Details & State Templates -->
		<div class="grid grid-cols-1 md:grid-cols-2 gap-3 text-xs">
			<div>
				<label for="discord-details-input" class="mb-1 block font-medium text-muted-foreground">Top line (Details)</label>
				<Input
					id="discord-details-input"
					type="text"
					value={discordDetails}
					oninput={(e) => setDiscordDetails(e.currentTarget.value)}
					placeholder={'{title}'}
					class="h-8 text-xs font-mono"
				/>
				<div class="mt-1 flex items-center gap-1.5 text-[10px] text-muted-foreground">
					<span>Tokens:</span>
					<button type="button" class="hover:text-primary underline cursor-pointer" onclick={() => setDiscordDetails((discordDetails ? discordDetails + ' ' : '') + '{title}')}>{"{title}"}</button>
					<button type="button" class="hover:text-primary underline cursor-pointer" onclick={() => setDiscordDetails((discordDetails ? discordDetails + ' ' : '') + '{artist}')}>{"{artist}"}</button>
					<button type="button" class="hover:text-primary underline cursor-pointer" onclick={() => setDiscordDetails((discordDetails ? discordDetails + ' ' : '') + '{album}')}>{"{album}"}</button>
				</div>
			</div>
			<div>
				<label for="discord-state-input" class="mb-1 block font-medium text-muted-foreground">Bottom line (State)</label>
				<Input
					id="discord-state-input"
					type="text"
					value={discordState}
					oninput={(e) => setDiscordState(e.currentTarget.value)}
					placeholder={'{artist}'}
					class="h-8 text-xs font-mono"
				/>
				<div class="mt-1 flex items-center gap-1.5 text-[10px] text-muted-foreground">
					<span>Tokens:</span>
					<button type="button" class="hover:text-primary underline cursor-pointer" onclick={() => setDiscordState((discordState ? discordState + ' ' : '') + '{title}')}>{"{title}"}</button>
					<button type="button" class="hover:text-primary underline cursor-pointer" onclick={() => setDiscordState((discordState ? discordState + ' ' : '') + '{artist}')}>{"{artist}"}</button>
					<button type="button" class="hover:text-primary underline cursor-pointer" onclick={() => setDiscordState((discordState ? discordState + ' ' : '') + '{album}')}>{"{album}"}</button>
				</div>
			</div>
		</div>

		<!-- Toggles -->
		<div class="space-y-2 rounded-lg border border-border/60 bg-card/75 dark:bg-card/60 backdrop-blur-sm p-3 text-xs">
			<div class="flex items-center justify-between">
				<div>
					<div class="font-medium text-foreground">Show track time</div>
					<div class="text-[11px] text-muted-foreground">Display elapsed time and progress bar in Discord</div>
				</div>
				<Switch checked={discordShowTime} onCheckedChange={setDiscordShowTime} />
			</div>
			<div class="flex items-center justify-between border-t border-border/40 pt-2">
				<div>
					<div class="font-medium text-foreground">Show presence when paused</div>
					<div class="text-[11px] text-muted-foreground">Keep profile status active with (Paused) badge</div>
				</div>
				<Switch checked={discordShowPause} onCheckedChange={setDiscordShowPause} />
			</div>
			<div class="flex items-center justify-between border-t border-border/40 pt-2">
				<div>
					<div class="font-medium text-foreground">Show button on Discord</div>
					<div class="text-[11px] text-muted-foreground">Adds clickable link button to your Discord presence</div>
				</div>
				<Switch checked={discordShowButton} onCheckedChange={setDiscordShowButton} />
			</div>
		</div>

		<!-- Button Label and App ID -->
		<div class="grid grid-cols-1 md:grid-cols-2 gap-3 text-xs">
			{#if discordShowButton}
				<div>
					<label for="discord-btn-label" class="mb-1 block font-medium text-muted-foreground">Button label</label>
					<Input
						id="discord-btn-label"
						type="text"
						value={discordButtonLabel}
						oninput={(e) => setDiscordButtonLabel(e.currentTarget.value)}
						placeholder="Listen on Nocturne"
						class="h-8 text-xs"
					/>
				</div>
			{/if}
			<div>
				<label for="discord-app-id" class="mb-1 block font-medium text-muted-foreground">Custom Application ID (optional)</label>
				<Input
					id="discord-app-id"
					type="text"
					value={discordAppId}
					oninput={(e) => setDiscordAppId(e.currentTarget.value)}
					placeholder="Default: Nocturne Music"
					class="h-8 text-xs font-mono"
				/>
			</div>
		</div>
	</div>
{/snippet}
{#snippet crossfadeSlider()}
	<div class="flex items-center gap-3 w-48">
		<Slider
			type="single"
			min={0}
			max={12}
			step={1}
			value={crossfadeSecs}
			onValueChange={(val) => setCrossfade(val)}
			class="flex-1"
		/>
		<span class="w-8 text-right font-mono text-xs text-muted-foreground">{crossfadeSecs === 0 ? 'Off' : `${crossfadeSecs}s`}</span>
	</div>
{/snippet}
{#snippet floatingPanelsConfig()}
	<div class="mt-2 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-2 rounded-xl border border-border/60 bg-muted/35 dark:bg-muted/20 p-3 backdrop-blur-md text-xs">
		<label class="flex items-center justify-between gap-2 p-2.5 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<div>
				<span class="font-medium text-foreground/90 block">Top bar</span>
				<span class="text-[10px] text-muted-foreground block">Window titlebar</span>
			</div>
			<input
				type="checkbox"
				checked={prefs.floatingTopBar}
				onchange={(e) => setFloatingTopBar(e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2.5 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<div>
				<span class="font-medium text-foreground/90 block">Left sidebar</span>
				<span class="text-[10px] text-muted-foreground block">Navigation rail</span>
			</div>
			<input
				type="checkbox"
				checked={prefs.floatingSidebarLeft}
				onchange={(e) => setFloatingSidebarLeft(e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2.5 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<div>
				<span class="font-medium text-foreground/90 block">Right sidebar</span>
				<span class="text-[10px] text-muted-foreground block">Info, queue & lyrics</span>
			</div>
			<input
				type="checkbox"
				checked={prefs.floatingSidebarRight}
				onchange={(e) => setFloatingSidebarRight(e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2.5 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<div>
				<span class="font-medium text-foreground/90 block">Bottom player bar</span>
				<span class="text-[10px] text-muted-foreground block">Playback controls</span>
			</div>
			<input
				type="checkbox"
				checked={prefs.floatingPlayerBar}
				onchange={(e) => setFloatingPlayerBar(e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
	</div>
{/snippet}
{#snippet titlebarIconsConfig()}
	<div class="mt-2 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-2 rounded-xl border border-border/60 bg-muted/35 dark:bg-muted/20 p-3 backdrop-blur-md text-xs">
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Navigation (Back/Fwd)</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.titlebar.navigation}
				onchange={(e) => setVisibleIcon('titlebar', 'navigation', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Search bar</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.titlebar.search}
				onchange={(e) => setVisibleIcon('titlebar', 'search', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Open link</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.titlebar.openLink}
				onchange={(e) => setVisibleIcon('titlebar', 'openLink', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Listen Together</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.titlebar.listenTogether}
				onchange={(e) => setVisibleIcon('titlebar', 'listenTogether', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Discord presence</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.titlebar.discord}
				onchange={(e) => setVisibleIcon('titlebar', 'discord', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Last.fm scrobbler</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.titlebar.lastfm}
				onchange={(e) => setVisibleIcon('titlebar', 'lastfm', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Mini player</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.titlebar.miniPlayer}
				onchange={(e) => setVisibleIcon('titlebar', 'miniPlayer', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Fullscreen</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.titlebar.fullscreen}
				onchange={(e) => setVisibleIcon('titlebar', 'fullscreen', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
	</div>
{/snippet}
{#snippet playerbarIconsConfig()}
	<div class="mt-2 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-2 rounded-xl border border-border/60 bg-muted/35 dark:bg-muted/20 p-3 backdrop-blur-md text-xs">
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Favourite / Like</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.playerbar.like}
				onchange={(e) => setVisibleIcon('playerbar', 'like', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Shuffle</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.playerbar.shuffle}
				onchange={(e) => setVisibleIcon('playerbar', 'shuffle', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Repeat</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.playerbar.repeat}
				onchange={(e) => setVisibleIcon('playerbar', 'repeat', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Volume slider</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.playerbar.volume}
				onchange={(e) => setVisibleIcon('playerbar', 'volume', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Now Playing info</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.playerbar.info}
				onchange={(e) => setVisibleIcon('playerbar', 'info', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Mini player</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.playerbar.miniPlayer}
				onchange={(e) => setVisibleIcon('playerbar', 'miniPlayer', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Lyrics view</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.playerbar.lyrics}
				onchange={(e) => setVisibleIcon('playerbar', 'lyrics', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Queue panel</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.playerbar.queue}
				onchange={(e) => setVisibleIcon('playerbar', 'queue', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Devices / Cast</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.playerbar.devices}
				onchange={(e) => setVisibleIcon('playerbar', 'devices', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
		<label class="flex items-center justify-between gap-2 p-2 rounded-lg bg-card/60 dark:bg-card/40 border border-border/40 cursor-pointer hover:bg-card/80 transition-colors">
			<span class="font-medium text-foreground/90">Open player</span>
			<input
				type="checkbox"
				checked={prefs.visibleIcons.playerbar.fullscreen}
				onchange={(e) => setVisibleIcon('playerbar', 'fullscreen', e.currentTarget.checked)}
				class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
			/>
		</label>
	</div>
{/snippet}
{#snippet lastfmButton()}
	{#if lastfmConnected}
		<Button size="sm" variant="outline" onclick={disconnectLastfm}>Disconnect</Button>
	{:else}
		<Button size="sm" onclick={connectLastfm} disabled={lastfmConnecting}>
			{lastfmConnecting ? 'Connecting…' : 'Connect Last.fm'}
		</Button>
	{/if}
{/snippet}
{#snippet remoteSyncSwitch()}<Switch checked={remoteSyncOn} onCheckedChange={setRemoteSync} />{/snippet}
{#snippet remoteSyncConfig()}
	<div class="mt-2.5 space-y-3 rounded-xl border border-border/60 bg-muted/35 dark:bg-muted/20 p-3.5 backdrop-blur-md">
		<div class="flex items-center justify-between gap-2">
			<div>
				<span class="text-xs font-semibold text-foreground">Host Connection Info</span>
				<p class="text-[11px] text-muted-foreground">
					Nocturne Mobile auto-discovers this PC over Wi-Fi. For manual connection, use the IP below.
				</p>
			</div>
			{#if syncInfo?.is_running}
				<span class="flex items-center gap-1 text-[11px] font-medium text-emerald-500">
					<HugeiconsIcon icon={Wifi01Icon} class="h-3.5 w-3.5" />
					LAN Sync Ready
				</span>
			{:else}
				<span class="flex items-center gap-1 text-[11px] font-medium text-muted-foreground">
					<HugeiconsIcon icon={Wifi01Icon} class="h-3.5 w-3.5" />
					Sync Server Stopped
				</span>
			{/if}
		</div>

		<div class="space-y-2 rounded-lg border border-border/60 bg-card/75 dark:bg-card/60 backdrop-blur-sm p-3 text-xs">
			<div class="flex items-center justify-between">
				<span class="text-muted-foreground">Device Name:</span>
				<span class="font-medium text-foreground">{syncInfo?.device_name || 'Nocturne PC'}</span>
			</div>

			<div class="flex items-center justify-between">
				<span class="text-muted-foreground">Local IP Address:</span>
				<div class="flex items-center gap-2 font-mono text-foreground">
					<span>{showHostIp ? (syncInfo?.local_ip || '127.0.0.1') : maskIp(syncInfo?.local_ip || '')}</span>
					<button
						type="button"
						class="cursor-pointer text-muted-foreground hover:text-foreground transition-colors"
						onclick={() => (showHostIp = !showHostIp)}
						title={showHostIp ? 'Hide IP' : 'Reveal IP'}
					>
						<HugeiconsIcon icon={showHostIp ? ViewOffSlashIcon : ViewIcon} class="h-3.5 w-3.5" />
					</button>
				</div>
			</div>

			<div class="flex items-center justify-between">
				<span class="text-muted-foreground">Pairing PIN:</span>
				<div class="flex items-center gap-2 font-mono text-foreground">
					{#if pairingPin}
						<span>{showPairingPin ? pairingPin : '•'.repeat(pairingPin.length)}</span>
						<button
							type="button"
							class="cursor-pointer text-muted-foreground hover:text-foreground transition-colors"
							onclick={() => (showPairingPin = !showPairingPin)}
							title={showPairingPin ? 'Hide PIN' : 'Reveal PIN'}
						>
							<HugeiconsIcon icon={showPairingPin ? ViewOffSlashIcon : ViewIcon} class="h-3.5 w-3.5" />
						</button>
					{:else}
						<span class="font-sans text-muted-foreground">Not configured yet</span>
					{/if}
					<button
						type="button"
						class="cursor-pointer text-muted-foreground hover:text-foreground transition-colors"
						onclick={regenerateRemoteSyncPin}
						title="Generate a new PIN"
					>
						<HugeiconsIcon icon={Refresh01Icon} class="h-3.5 w-3.5" />
					</button>
				</div>
			</div>

			<div class="flex items-center justify-between">
				<span class="text-muted-foreground">WebSocket Port:</span>
				<span class="font-mono text-foreground">{syncInfo?.port || 8080}</span>
			</div>

			<div class="flex items-center justify-between">
				<span class="text-muted-foreground">UDP Discovery Port:</span>
				<span class="font-mono text-foreground">8081</span>
			</div>
		</div>

		{#if syncInfo && syncInfo.connected_clients.length > 0}
			<div class="space-y-1.5 pt-1">
				<span class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Connected Devices</span>
				{#each syncInfo.connected_clients as client (client.id)}
					<div class="flex items-center justify-between rounded-lg border border-border/60 bg-muted/40 dark:bg-muted/20 px-3 py-2 text-xs">
						<div class="flex items-center gap-2">
							<HugeiconsIcon icon={SmartPhone01Icon} class="h-3.5 w-3.5 text-primary" />
							<span class="font-medium text-foreground">{client.name}</span>
							<span class="text-muted-foreground">({client.ip})</span>
						</div>
						<span class="text-[10px] font-semibold text-emerald-500">Connected</span>
					</div>
				{/each}
			</div>
		{/if}

		{#if syncInfo && syncInfo.paired_devices && syncInfo.paired_devices.length > 0}
			<div class="space-y-1.5 pt-2">
				<div class="flex items-center justify-between">
					<span class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Paired & Trusted Devices</span>
					<span class="text-[10px] text-muted-foreground">Favorites bypass PIN auto-trust</span>
				</div>
				{#each syncInfo.paired_devices as device (device.id)}
					<div class="flex items-center justify-between rounded-lg border border-border/60 bg-muted/30 dark:bg-muted/15 px-3 py-2 text-xs transition-colors hover:border-border">
						<div class="flex items-center gap-2 min-w-0 flex-1 mr-2">
							<button
								type="button"
								class="cursor-pointer transition-transform hover:scale-110 shrink-0"
								onclick={() => toggleFavorite(device)}
								title={device.is_favorite ? "Trusted Favorite (Click to unstar)" : "Star as Trusted Favorite (Bypasses PIN)"}
							>
								<HugeiconsIcon
									icon={FavouriteIcon}
									class="h-3.5 w-3.5 {device.is_favorite ? 'text-amber-500 fill-amber-500' : 'text-muted-foreground/60 hover:text-foreground'}"
								/>
							</button>

							{#if editingDeviceId === device.id}
								<form
									class="flex items-center gap-1.5 flex-1 min-w-0"
									onsubmit={(e) => { e.preventDefault(); saveRename(device.id); }}
								>
									<input
										type="text"
										bind:value={editingDeviceName}
										class="h-6 w-full rounded border border-input bg-background px-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
										placeholder="Device name"
										autofocus
									/>
									<button
										type="submit"
										class="cursor-pointer text-emerald-500 hover:text-emerald-400 p-1"
										title="Save"
									>
										<HugeiconsIcon icon={Tick02Icon} class="h-3.5 w-3.5" />
									</button>
									<button
										type="button"
										class="cursor-pointer text-muted-foreground hover:text-foreground p-1"
										onclick={() => { editingDeviceId = null; }}
										title="Cancel"
									>
										<HugeiconsIcon icon={Cancel01Icon} class="h-3.5 w-3.5" />
									</button>
								</form>
							{:else}
								<div class="flex flex-col min-w-0">
									<div class="flex items-center gap-1.5">
										<span class="font-medium text-foreground truncate">{device.name}</span>
										{#if device.is_favorite}
											<span class="rounded bg-amber-500/15 px-1.5 py-0.2 text-[9px] font-semibold text-amber-500">Favorite</span>
										{/if}
									</div>
									{#if device.name !== device.original_name}
										<span class="text-[10px] text-muted-foreground truncate">{device.original_name}</span>
									{/if}
								</div>
							{/if}
						</div>

						<div class="flex items-center gap-1.5 shrink-0">
							{#if editingDeviceId !== device.id}
								<button
									type="button"
									class="cursor-pointer text-muted-foreground hover:text-foreground p-1 transition-colors"
									onclick={() => startRename(device)}
									title="Rename device"
								>
									<HugeiconsIcon icon={Edit02Icon} class="h-3.5 w-3.5" />
								</button>
								<button
									type="button"
									class="cursor-pointer text-muted-foreground hover:text-destructive p-1 transition-colors"
									onclick={() => removeDevice(device.id)}
									title="Unpair device"
								>
									<HugeiconsIcon icon={Delete02Icon} class="h-3.5 w-3.5" />
								</button>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		{/if}
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
{#snippet aggressiveTrimSwitch()}<Switch
		checked={settings.aggressive_memory_trimming === 'true'}
		onCheckedChange={async (on) => {
			settings.aggressive_memory_trimming = on ? 'true' : 'false';
			await api.setSetting('aggressive_memory_trimming', settings.aggressive_memory_trimming);
			if (on) {
				toast('Aggressive memory trimming enabled. Note: May cause random crashes on some Linux distributions while idle.');
			} else {
				toast.info('Aggressive memory trimming disabled.');
			}
		}}
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
		<Button variant="outline" size="sm" onclick={applyBalanced} class="text-xs cursor-pointer">
			<HugeiconsIcon icon={ComputerIcon} class="mr-1.5 h-3.5 w-3.5 text-sky-500" />
			Balanced (Default)
		</Button>
		<Button variant="outline" size="sm" onclick={applyHighQuality} class="text-xs cursor-pointer">
			<HugeiconsIcon icon={PaintBoardIcon} class="mr-1.5 h-3.5 w-3.5 text-primary" />
			High Quality
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

{#snippet glassySpeedSlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{((appearance.glassySpeed - 0.1) / 2.9) * 100}%"
			min="0.1"
			max="3.0"
			step="0.1"
			value={appearance.glassySpeed}
			oninput={(e) => setAppearance({ glassySpeed: parseFloat(e.currentTarget.value) })}
			aria-label="Glassy animation speed"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{appearance.glassySpeed.toFixed(1)}x</span>
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

{#snippet fullscreenSpeedSlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{((appearance.fullscreenSpeed - 0.1) / 2.9) * 100}%"
			min="0.1"
			max="3.0"
			step="0.1"
			value={appearance.fullscreenSpeed}
			oninput={(e) => setAppearance({ fullscreenSpeed: parseFloat(e.currentTarget.value) })}
			aria-label="Fullscreen animation speed"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{appearance.fullscreenSpeed.toFixed(1)}x</span>
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

{#snippet dialogOpacitySlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{((appearance.dialogOpacity - 0.3) / 0.7) * 100}%"
			min="0.30"
			max="1.0"
			step="0.05"
			value={appearance.dialogOpacity}
			oninput={(e) => setAppearance({ dialogOpacity: parseFloat(e.currentTarget.value) })}
			aria-label="Dialog content opacity"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{Math.round(appearance.dialogOpacity * 100)}%</span>
	</div>
{/snippet}

{#snippet sidebarOpacitySlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{((appearance.sidebarOpacity - 0.1) / 0.9) * 100}%"
			min="0.10"
			max="1.0"
			step="0.05"
			value={appearance.sidebarOpacity}
			oninput={(e) => setAppearance({ sidebarOpacity: parseFloat(e.currentTarget.value) })}
			aria-label="Sidebar rail opacity"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{Math.round(appearance.sidebarOpacity * 100)}%</span>
	</div>
{/snippet}

{#snippet cardOpacitySlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{((appearance.cardOpacity - 0.2) / 0.8) * 100}%"
			min="0.20"
			max="1.0"
			step="0.05"
			value={appearance.cardOpacity}
			oninput={(e) => setAppearance({ cardOpacity: parseFloat(e.currentTarget.value) })}
			aria-label="Card surface opacity"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{Math.round(appearance.cardOpacity * 100)}%</span>
	</div>
{/snippet}

{#snippet overlayDimmingSlider()}
	<div class="flex items-center gap-3 w-48">
		<input
			type="range"
			class="range flex-1"
			style="--pct:{(appearance.overlayDimming / 0.8) * 100}%"
			min="0.0"
			max="0.80"
			step="0.05"
			value={appearance.overlayDimming}
			oninput={(e) => setAppearance({ overlayDimming: parseFloat(e.currentTarget.value) })}
			aria-label="Dialog backdrop dimming"
		/>
		<span class="w-10 text-right text-xs font-mono text-muted-foreground">{Math.round(appearance.overlayDimming * 100)}%</span>
	</div>
{/snippet}

{#snippet resetTranslucencyButton()}
	<Button variant="outline" size="sm" onclick={resetTranslucencyVisuals}>Reset translucency</Button>
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
		Clients provide playback streams. You can benchmark connection latencies or specify custom fallback priority.
	</p>
	<div class="mb-3 flex flex-wrap items-center justify-between gap-2 rounded-lg border border-border/60 bg-muted/40 dark:bg-muted/20 backdrop-blur-sm p-2.5">
		<div class="flex items-center gap-2">
			<Button
				variant="outline"
				size="sm"
				class="h-8 gap-1.5 text-xs font-medium cursor-pointer"
				onclick={runClientBenchmark}
				disabled={benchmarkingClients}
			>
				<HugeiconsIcon icon={FlashIcon} class="h-3.5 w-3.5 text-amber-500 {benchmarkingClients ? 'animate-spin' : ''}" />
				{benchmarkingClients ? 'Testing latencies…' : 'Test Latencies'}
			</Button>
		</div>
		<div class="flex items-center gap-2">
			<span class="text-xs text-muted-foreground">Auto-rank by latency</span>
			<Switch checked={autoRankClients} onCheckedChange={toggleAutoRank} />
		</div>
	</div>
	<div class="flex flex-col gap-2">
		{#each clients as name, i (name)}
			{@const stat = clientStats[name]}
			<div class="flex items-center justify-between rounded-lg bg-muted/50 dark:bg-muted/30 py-1.5 pr-2 pl-3">
				<div class="flex items-center gap-2">
					{#if !autoRankClients}
						<div class="flex flex-col gap-0.5">
							<button
								type="button"
								class="flex h-3.5 w-3.5 cursor-pointer items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-30"
								disabled={i === 0}
								onclick={() => moveClientUp(i)}
								title="Move up priority"
							>
								<HugeiconsIcon icon={ArrowUp01Icon} class="h-3 w-3" />
							</button>
							<button
								type="button"
								class="flex h-3.5 w-3.5 cursor-pointer items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-30"
								disabled={i === clients.length - 1}
								onclick={() => moveClientDown(i)}
								title="Move down priority"
							>
								<HugeiconsIcon icon={ArrowDown01Icon} class="h-3 w-3" />
							</button>
						</div>
						<span class="w-4 text-center font-mono text-[11px] text-muted-foreground">#{i + 1}</span>
					{/if}
					<span class="font-mono text-xs">{name}</span>
					{#if stat}
						<span
							class="rounded px-1.5 py-0.5 text-[10px] font-medium {stat.penalty > 0
								? 'bg-amber-500/15 text-amber-400'
								: stat.latency_ms < 250
									? 'bg-emerald-500/15 text-emerald-400'
									: 'bg-muted text-muted-foreground'}"
						>
							{stat.penalty > 0 ? '⚠️ Demoted' : `⚡ ${Math.round(stat.latency_ms)}ms`}
						</span>
					{/if}
				</div>
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

{#snippet cacheStatsBreakdown()}
	{#if cacheStats}
		<div class="grid grid-cols-3 gap-2 pt-1 pb-1">
			<div class="rounded-lg border border-border/60 bg-muted/30 p-2.5 text-center">
				<span class="text-[10px] uppercase font-semibold text-muted-foreground block">Audio Buffers</span>
				<span class="text-xs font-mono font-medium text-foreground mt-0.5 block">{formatBytes(cacheStats.audio_cache_bytes)}</span>
				<button
					type="button"
					onclick={() => clearCacheKind('audio')}
					disabled={clearing || cacheStats.audio_cache_bytes === 0}
					class="mt-1.5 text-[10px] text-destructive hover:underline cursor-pointer disabled:opacity-40"
				>
					Clear audio
				</button>
			</div>
			<div class="rounded-lg border border-border/60 bg-muted/30 p-2.5 text-center">
				<span class="text-[10px] uppercase font-semibold text-muted-foreground block">Cipher Cache</span>
				<span class="text-xs font-mono font-medium text-foreground mt-0.5 block">{formatBytes(cacheStats.cipher_cache_bytes)}</span>
				<button
					type="button"
					onclick={() => clearCacheKind('cipher')}
					disabled={clearing || cacheStats.cipher_cache_bytes === 0}
					class="mt-1.5 text-[10px] text-destructive hover:underline cursor-pointer disabled:opacity-40"
				>
					Clear cipher
				</button>
			</div>
			<div class="rounded-lg border border-border/60 bg-muted/30 p-2.5 text-center">
				<span class="text-[10px] uppercase font-semibold text-muted-foreground block">Local Covers</span>
				<span class="text-xs font-mono font-medium text-foreground mt-0.5 block">{formatBytes(cacheStats.covers_cache_bytes)}</span>
				<button
					type="button"
					onclick={() => clearCacheKind('covers')}
					disabled={clearing || cacheStats.covers_cache_bytes === 0}
					class="mt-1.5 text-[10px] text-destructive hover:underline cursor-pointer disabled:opacity-40"
				>
					Clear covers
				</button>
			</div>
		</div>
	{/if}
{/snippet}

{#snippet cacheLimitSelector()}
	<select
		value={cacheLimitMb ?? 0}
		onchange={(e) => {
			const val = parseInt(e.currentTarget.value, 10);
			updateCacheLimit(val === 0 ? null : val);
		}}
		class="h-8 rounded-md border border-border bg-background px-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
	>
		<option value={0}>Unlimited</option>
		<option value={512}>512 MB</option>
		<option value={1024}>1 GB</option>
		<option value={2048}>2 GB</option>
		<option value={4096}>4 GB</option>
		<option value={8192}>8 GB</option>
	</select>
{/snippet}

{#snippet clearButton()}
	<Button variant="destructive" size="sm" onclick={() => clearCacheKind('all')} disabled={clearing}>
		{clearing ? 'Clearing…' : 'Clear all caches'}
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
