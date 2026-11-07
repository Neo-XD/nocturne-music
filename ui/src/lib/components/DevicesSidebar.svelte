<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Cancel01Icon,
		SmartPhone01Icon,
		CheckmarkCircle02Icon,
		RefreshIcon,
		VolumeHighIcon,
		PlayIcon,
		PauseIcon,
		PreviousIcon,
		NextIcon,
		MusicNote01Icon
	} from '@hugeicons/core-free-icons';
	import SpeakerIcon from './SpeakerIcon.svelte';
	import SpotifyIcon from '$lib/components/icons/SpotifyIcon.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as api from '$lib/api';
	import type { RemoteSyncInfo, AudioDeviceInfo, SpotifyDevice, SpotifyPlaybackState, SpotifyConnectStateEvent } from '$lib/api';
	import { prefs, toast, spotify, ui } from '$lib/player.svelte';

	let {
		onClose
	}: {
		onClose: () => void;
	} = $props();

	let syncInfo = $state<RemoteSyncInfo | null>(null);
	let audioInfo = $state<AudioDeviceInfo | null>(null);
	let spotifyDevices = $state<SpotifyDevice[]>([]);
	let spotifyPlayback = $state<SpotifyPlaybackState | null>(null);
	let spotifyConnectActive = $state(false);
	let spotifyConnectEvent = $state<SpotifyConnectStateEvent | null>(null);

	let audioDestination = $state<'pc' | 'phone'>('pc');
	let loadingAudio = $state(true);
	let loadingSpotify = $state(false);
	let pollTimer: any;
	let unlistenConnect: (() => void) | null = null;

	const isSpotifyConnected = $derived(
		spotify.status.linked ||
			spotifyConnectActive ||
			(spotifyPlayback?.is_playing ?? false) ||
			spotifyDevices.some((d) => d.is_active)
	);

	function setAudioDestination(dest: 'pc' | 'phone') {
		audioDestination = dest;
		if (dest === 'phone') {
			toast.info('Audio playback routed to mobile device');
		} else {
			toast.info('Audio playback routed to this PC');
		}
	}

	async function fetchStatus() {
		try {
			syncInfo = await api.getRemoteSyncStatus();
		} catch (e) {
			console.error('Failed to get remote sync status', e);
		}
	}

	async function fetchAudioDevices() {
		try {
			audioInfo = await api.getAudioDevices();
		} catch (e) {
			console.error('Failed to get audio devices', e);
		} finally {
			loadingAudio = false;
		}
	}

	async function fetchSpotify() {
		loadingSpotify = true;
		try {
			const [devs, pb, status] = await Promise.all([
				api.spotifyGetDevices().catch(() => []),
				api.spotifyGetPlayback().catch(() => null),
				api.spotifyConnectStatus().catch(() => null)
			]);
			spotifyDevices = devs;
			spotifyPlayback = pb;
			if (status) {
				spotifyConnectActive = status.active;
			}
		} catch (e) {
			console.error('Failed to fetch Spotify devices/playback', e);
		} finally {
			loadingSpotify = false;
		}
	}

	async function selectDevice(name: string) {
		if (audioInfo) {
			audioInfo = { ...audioInfo, current: name };
		}
		try {
			await api.setAudioDevice(name);
			await fetchAudioDevices();
		} catch (e) {
			console.error('Failed to set audio device', e);
		}
	}

	async function selectSpotifyDevice(device: SpotifyDevice) {
		if (!device.id) {
			toast.info(`Selected ${device.name}`);
			return;
		}
		try {
			toast.info(`Transferring playback to ${device.name}...`);
			await api.spotifyTransferPlayback(device.id, true);
			setTimeout(fetchSpotify, 500);
		} catch (e) {
			toast.error(`Could not transfer playback: ${e}`);
		}
	}

	async function handleSpotifyPlayPause() {
		if (!spotifyPlayback) return;
		try {
			const nextPlaying = !spotifyPlayback.is_playing;
			await api.spotifyPlayPause(nextPlaying);
			spotifyPlayback.is_playing = nextPlaying;
			setTimeout(fetchSpotify, 400);
		} catch (e) {
			toast.error(String(e));
		}
	}

	async function handleSpotifyNext() {
		try {
			await api.spotifyNextTrack();
			setTimeout(fetchSpotify, 600);
		} catch (e) {
			toast.error(String(e));
		}
	}

	async function handleSpotifyPrev() {
		try {
			await api.spotifyPreviousTrack();
			setTimeout(fetchSpotify, 600);
		} catch (e) {
			toast.error(String(e));
		}
	}

	function openSpotifySettings() {
		ui.settingsOpen = true;
		onClose();
	}

	onMount(async () => {
		fetchStatus();
		fetchAudioDevices();
		fetchSpotify();

		try {
			unlistenConnect = await api.onSpotifyConnectState((event) => {
				spotifyConnectEvent = event;
				spotifyConnectActive = event.active;
				if (event.active && event.title) {
					spotifyPlayback = {
						is_playing: event.is_playing,
						progress_ms: event.position_ms,
						device: {
							id: null,
							is_active: true,
							is_private_session: false,
							is_restricted: false,
							name: 'Nocturne (This PC)',
							device_type: 'Computer',
							volume_percent: 100,
							supports_volume: true
						},
						item: {
							id: null,
							name: event.title,
							artists: event.artist || '',
							album_name: event.album,
							album_cover_url: event.cover_url,
							duration_ms: event.duration_ms
						},
						shuffle_state: false,
						repeat_state: 'off'
					};
				}
			});
		} catch (e) {
			console.warn('Failed to listen to spotify-connect-state:', e);
		}

		pollTimer = setInterval(() => {
			fetchStatus();
			fetchSpotify();
		}, 2500);
	});

	onDestroy(() => {
		if (pollTimer) clearInterval(pollTimer);
		if (unlistenConnect) unlistenConnect();
	});
</script>

<aside
	style={prefs.floatingSidebarRight ? 'width: calc(100% - 1rem); height: calc(100% - 1rem);' : 'width: 100%; height: 100%;'}
	class="info-sidebar relative flex max-w-[90vw] shrink-0 flex-col transition-[border-radius,margin] duration-200 {prefs.floatingSidebarRight
		? 'app-floating-panel m-2 rounded-2xl border border-border/70 bg-card/90 shadow-2xl backdrop-blur-xl'
		: 'w-full border-l border-border/70 bg-background/80 dark:bg-background/75 backdrop-blur-2xl shadow-2xl rounded-none m-0'}"
>
	<!-- Header -->
	<div class="flex items-center justify-between border-b border-border/60 bg-background/35 dark:bg-background/20 backdrop-blur-md px-4 py-3.5">
		<div class="flex items-center gap-2">
			{#if isSpotifyConnected}
				<SpotifyIcon class="h-5 w-5 text-[#1ed760]" />
			{:else}
				<HugeiconsIcon icon={VolumeHighIcon} class="h-5 w-5 text-primary" />
			{/if}
			<h2 class="text-sm font-semibold">Playback & Devices</h2>
		</div>
		<Button variant="ghost" size="icon-sm" onclick={onClose} aria-label="Close" class="cursor-pointer hover:bg-foreground/10">
			<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4" />
		</Button>
	</div>

	<!-- Scrollable content -->
	<div class="flex-1 space-y-6 overflow-y-auto p-4">
		{#if isSpotifyConnected}
			<!-- MAIN SECTION: Spotify Connect Devices -->
			<div class="space-y-3.5">
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-2">
						<SpotifyIcon class="h-4 w-4 text-[#1ed760]" />
						<span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
							Where Audio Plays
						</span>
					</div>
					<div class="flex items-center gap-1.5">
						<span class="inline-flex items-center gap-1 rounded-full bg-[#1ed760]/15 px-2 py-0.5 text-[10px] font-semibold text-[#1ed760]">
							<span class="h-1.5 w-1.5 rounded-full bg-[#1ed760] animate-pulse"></span>
							Spotify Connect
						</span>
						<Button
							variant="ghost"
							size="icon-sm"
							onclick={fetchSpotify}
							aria-label="Refresh Spotify devices"
							class="h-6 w-6 cursor-pointer hover:bg-foreground/10"
						>
							<HugeiconsIcon icon={RefreshIcon} class="h-3.5 w-3.5 text-muted-foreground" />
						</Button>
					</div>
				</div>

				<!-- Live Spotify Playback Card -->
				{#if spotifyPlayback?.item}
					<div class="rounded-xl border border-[#1ed760]/30 bg-[#1ed760]/10 dark:bg-[#1ed760]/5 p-3.5 backdrop-blur-md shadow-xs">
						<div class="flex items-center gap-3">
							{#if spotifyPlayback.item.album_cover_url}
								<img
									src={spotifyPlayback.item.album_cover_url}
									alt=""
									class="h-12 w-12 rounded-lg object-cover shadow-xs shrink-0"
								/>
							{:else}
								<div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-lg bg-[#1ed760]/20 text-[#1ed760]">
									<SpotifyIcon class="h-6 w-6" />
								</div>
							{/if}
							<div class="min-w-0 flex-1">
								<div class="text-xs font-semibold text-foreground truncate">
									{spotifyPlayback.item.name}
								</div>
								<div class="text-[11px] text-muted-foreground truncate">
									{spotifyPlayback.item.artists}
								</div>
								<div class="mt-0.5 text-[10px] text-[#1ed760] font-medium truncate flex items-center gap-1">
									<span class="h-1.5 w-1.5 rounded-full bg-[#1ed760]"></span>
									Playing on {spotifyPlayback.device?.name || 'Spotify Device'}
								</div>
							</div>
							<div class="flex items-center gap-0.5 shrink-0">
								<Button
									variant="ghost"
									size="icon-sm"
									onclick={handleSpotifyPrev}
									aria-label="Previous track"
									class="h-7 w-7 text-foreground/80 hover:text-foreground"
								>
									<HugeiconsIcon icon={PreviousIcon} class="h-3.5 w-3.5" />
								</Button>
								<Button
									variant="ghost"
									size="icon-sm"
									onclick={handleSpotifyPlayPause}
									aria-label={spotifyPlayback.is_playing ? 'Pause' : 'Play'}
									class="h-8 w-8 text-foreground hover:scale-105"
								>
									<HugeiconsIcon icon={spotifyPlayback.is_playing ? PauseIcon : PlayIcon} class="h-4.5 w-4.5 text-[#1ed760]" />
								</Button>
								<Button
									variant="ghost"
									size="icon-sm"
									onclick={handleSpotifyNext}
									aria-label="Next track"
									class="h-7 w-7 text-foreground/80 hover:text-foreground"
								>
									<HugeiconsIcon icon={NextIcon} class="h-3.5 w-3.5" />
								</Button>
							</div>
						</div>
					</div>
				{/if}

				<!-- Output Device Picker List -->
				<div>
					<div class="mb-2 text-[11px] font-medium text-muted-foreground">Select a device to stream</div>
					{#if loadingSpotify && spotifyDevices.length === 0}
						<div class="space-y-2">
							{#each { length: 2 } as _, i (i)}
								<div class="h-14 animate-pulse rounded-xl bg-muted/40"></div>
							{/each}
						</div>
					{:else if spotifyDevices.length > 0}
						<div class="space-y-1.5">
							{#each spotifyDevices as dev (dev.id || dev.name)}
								{@const isDevActive = dev.is_active || (spotifyPlayback?.device?.id === dev.id)}
								<button
									type="button"
									onclick={() => selectSpotifyDevice(dev)}
									class="w-full flex items-center justify-between rounded-xl border p-3 text-left transition-all cursor-pointer {isDevActive
										? 'border-[#1ed760]/60 bg-[#1ed760]/10 dark:bg-[#1ed760]/15 shadow-xs ring-1 ring-[#1ed760]/30'
										: 'border-border/60 bg-card/60 dark:bg-card/40 hover:bg-card/90 hover:border-border'}"
								>
									<div class="flex items-center gap-3 min-w-0 pr-2">
										<div
											class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg {isDevActive
												? 'bg-[#1ed760] text-black font-semibold'
												: 'bg-muted/80 text-foreground'}"
										>
											{#if dev.device_type.toLowerCase().includes('phone')}
												<HugeiconsIcon icon={SmartPhone01Icon} class="h-4.5 w-4.5" />
											{:else}
												<SpeakerIcon class="h-4.5 w-4.5" />
											{/if}
										</div>
										<div class="min-w-0 flex-1">
											<div class="text-xs font-semibold text-foreground truncate">
												{dev.name}
											</div>
											<div class="text-[10px] text-muted-foreground truncate">
												{isDevActive ? 'Currently active Spotify device' : dev.device_type || 'Spotify Connect'}
											</div>
										</div>
									</div>
									{#if isDevActive}
										<span class="inline-flex items-center gap-1 rounded-full bg-[#1ed760]/20 px-2 py-0.5 text-[10px] font-semibold text-[#1ed760] shrink-0">
											<HugeiconsIcon icon={CheckmarkCircle02Icon} class="h-3 w-3" />
											Active
										</span>
									{:else}
										<span class="text-[10px] font-medium text-muted-foreground opacity-60 hover:opacity-100 shrink-0">
											Tap to play
										</span>
									{/if}
								</button>
							{/each}
						</div>
					{:else}
						<div class="rounded-xl border border-dashed border-border/60 bg-muted/20 p-4 text-center">
							<SpotifyIcon class="mx-auto mb-2 h-7 w-7 text-muted-foreground/60" />
							<div class="text-xs font-medium text-foreground">No Spotify Connect devices found</div>
							<div class="mt-0.5 text-[11px] text-muted-foreground">
								Launch Spotify on another phone or speaker on your Wi-Fi
							</div>
						</div>
					{/if}
				</div>
			</div>

			<!-- Local Soundcard Output Section -->
			<div>
				<div class="mb-2.5 flex items-center justify-between">
					<span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
						Local Audio Output
					</span>
					<Button
						variant="ghost"
						size="icon-sm"
						onclick={fetchAudioDevices}
						aria-label="Refresh audio devices"
						class="h-6 w-6 cursor-pointer hover:bg-foreground/10"
					>
						<HugeiconsIcon icon={RefreshIcon} class="h-3.5 w-3.5 text-muted-foreground" />
					</Button>
				</div>

				{#if loadingAudio}
					<div class="space-y-2">
						{#each { length: 2 } as _, i (i)}
							<div class="h-11 animate-pulse rounded-xl bg-muted/40"></div>
						{/each}
					</div>
				{:else if audioInfo && audioInfo.devices.length > 0}
					<div class="space-y-1.5">
						{#each audioInfo.devices as device (device.name)}
							{@const isSelected = audioInfo.current === device.name || (audioInfo.current === '' && device.name === 'auto')}
							<button
								type="button"
								onclick={() => selectDevice(device.name)}
								class="w-full flex items-center justify-between rounded-xl border p-2.5 text-left transition-all cursor-pointer {isSelected
									? 'border-primary/50 bg-primary/10 dark:bg-primary/15 shadow-xs ring-1 ring-primary/20'
									: 'border-border/60 bg-card/60 dark:bg-card/40 hover:bg-card/90 hover:border-border'}"
							>
								<div class="flex items-center gap-2.5 min-w-0 pr-2">
									<div class="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg {isSelected ? 'bg-primary text-primary-foreground' : 'bg-muted/80 text-foreground'}">
										<HugeiconsIcon icon={VolumeHighIcon} class="h-3.5 w-3.5" />
									</div>
									<div class="min-w-0 flex-1">
										<div class="text-xs font-medium text-foreground truncate" title={device.description}>
											{device.description || device.name}
										</div>
									</div>
								</div>
								{#if isSelected}
									<span class="inline-flex items-center gap-1 rounded-full bg-primary/20 px-2 py-0.5 text-[10px] font-semibold text-primary shrink-0">
										<HugeiconsIcon icon={CheckmarkCircle02Icon} class="h-3 w-3" />
										Active
									</span>
								{/if}
							</button>
						{/each}
					</div>
				{/if}
			</div>

			<!-- Secondary Nocturne Remote Sync Section -->
			<div>
				<div class="mb-2 flex items-center justify-between">
					<span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
						Nocturne Remote Sync
					</span>
					<Button variant="ghost" size="icon-sm" onclick={fetchStatus} aria-label="Refresh mobile devices" class="h-6 w-6 cursor-pointer hover:bg-foreground/10">
						<HugeiconsIcon icon={RefreshIcon} class="h-3.5 w-3.5 text-muted-foreground" />
					</Button>
				</div>

				{#if syncInfo && syncInfo.connected_clients.length > 0}
					<div class="space-y-2">
						{#each syncInfo.connected_clients as client (client.id)}
							<div
								class="flex items-center justify-between rounded-xl border border-border/60 bg-card/80 dark:bg-card/60 backdrop-blur-md p-3 transition-colors hover:bg-card/95 shadow-xs"
							>
								<div class="flex items-center gap-3">
									<div class="flex h-8 w-8 items-center justify-center rounded-lg bg-secondary text-foreground shadow-xs">
										<HugeiconsIcon icon={SmartPhone01Icon} class="h-4 w-4" />
									</div>
									<div>
										<div class="text-xs font-semibold text-foreground">{client.name}</div>
										<div class="text-[10px] text-muted-foreground">IP: {client.ip}</div>
									</div>
								</div>
								<span class="inline-flex items-center gap-1 rounded-full bg-primary/20 px-2 py-0.5 text-[10px] font-semibold text-primary">
									<HugeiconsIcon icon={CheckmarkCircle02Icon} class="h-3 w-3" />
									Connected
								</span>
							</div>
						{/each}
					</div>
				{:else}
					<div class="rounded-xl border border-dashed border-border/60 bg-muted/20 p-3 text-center text-xs text-muted-foreground">
						No mobile devices connected via Nocturne Sync
					</div>
				{/if}
			</div>
		{:else}
			<!-- MAIN SECTION: Nocturne Sync (Output Device Picker as a LIST) -->
			<div class="space-y-3.5">
				<div class="flex items-center justify-between">
					<span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
						Where Audio Plays
					</span>
					<Button
						variant="ghost"
						size="icon-sm"
						onclick={fetchStatus}
						aria-label="Refresh status"
						class="h-6 w-6 cursor-pointer hover:bg-foreground/10"
					>
						<HugeiconsIcon icon={RefreshIcon} class="h-3.5 w-3.5 text-muted-foreground" />
					</Button>
				</div>

				<!-- Output Device Picker List -->
				<div class="space-y-1.5">
					<!-- This PC Device Option -->
					<button
						type="button"
						onclick={() => setAudioDestination('pc')}
						class="w-full flex items-center justify-between rounded-xl border p-3 text-left transition-all cursor-pointer {audioDestination === 'pc'
							? 'border-primary/50 bg-primary/10 dark:bg-primary/15 shadow-xs ring-1 ring-primary/20'
							: 'border-border/60 bg-card/60 dark:bg-card/40 hover:bg-card/90 hover:border-border'}"
					>
						<div class="flex items-center gap-3 min-w-0 pr-2">
							<div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg {audioDestination === 'pc' ? 'bg-primary text-primary-foreground' : 'bg-muted/80 text-foreground'}">
								<SpeakerIcon class="h-4.5 w-4.5" />
							</div>
							<div class="min-w-0 flex-1">
								<div class="text-xs font-semibold text-foreground truncate">
									{syncInfo?.device_name || 'This PC'}
								</div>
								<div class="text-[10px] text-muted-foreground truncate">
									Local playback engine & speakers
								</div>
							</div>
						</div>
						{#if audioDestination === 'pc'}
							<span class="inline-flex items-center gap-1 rounded-full bg-primary/20 px-2 py-0.5 text-[10px] font-semibold text-primary shrink-0">
								<HugeiconsIcon icon={CheckmarkCircle02Icon} class="h-3 w-3" />
								Active
							</span>
						{/if}
					</button>

					<!-- Connected Mobile Devices -->
					{#if syncInfo && syncInfo.connected_clients.length > 0}
						{#each syncInfo.connected_clients as client (client.id)}
							<button
								type="button"
								onclick={() => setAudioDestination('phone')}
								class="w-full flex items-center justify-between rounded-xl border p-3 text-left transition-all cursor-pointer {audioDestination === 'phone'
									? 'border-primary/50 bg-primary/10 dark:bg-primary/15 shadow-xs ring-1 ring-primary/20'
									: 'border-border/60 bg-card/60 dark:bg-card/40 hover:bg-card/90 hover:border-border'}"
							>
								<div class="flex items-center gap-3 min-w-0 pr-2">
									<div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg {audioDestination === 'phone' ? 'bg-primary text-primary-foreground' : 'bg-muted/80 text-foreground'}">
										<HugeiconsIcon icon={SmartPhone01Icon} class="h-4.5 w-4.5" />
									</div>
									<div class="min-w-0 flex-1">
										<div class="text-xs font-semibold text-foreground truncate">
											{client.name}
										</div>
										<div class="text-[10px] text-muted-foreground truncate">
											Connected mobile • {client.ip}
										</div>
									</div>
								</div>
								{#if audioDestination === 'phone'}
									<span class="inline-flex items-center gap-1 rounded-full bg-primary/20 px-2 py-0.5 text-[10px] font-semibold text-primary shrink-0">
										<HugeiconsIcon icon={CheckmarkCircle02Icon} class="h-3 w-3" />
										Active
									</span>
								{:else}
									<span class="text-[10px] font-medium text-muted-foreground shrink-0">
										Tap to route
									</span>
								{/if}
							</button>
						{/each}
					{:else}
						<div class="rounded-xl border border-dashed border-border/60 bg-muted/20 p-3.5 text-center">
							<div class="mx-auto mb-1.5 flex h-7 w-7 items-center justify-center rounded-full bg-muted text-muted-foreground">
								<HugeiconsIcon icon={SmartPhone01Icon} class="h-3.5 w-3.5" />
							</div>
							<div class="text-xs font-medium text-foreground">No mobile device connected</div>
							<div class="mt-0.5 text-[10px] text-muted-foreground">
								Open Nocturne on your phone to connect over Wi-Fi
							</div>
						</div>
					{/if}
				</div>

				<!-- Connect Spotify Promo Card -->
				<div class="rounded-xl border border-[#1ed760]/30 bg-[#1ed760]/5 p-3.5 flex items-center justify-between gap-3 shadow-xs">
					<div class="flex items-center gap-2.5 min-w-0">
						<SpotifyIcon class="h-6 w-6 text-[#1ed760] shrink-0" />
						<div class="min-w-0">
							<div class="text-xs font-semibold text-foreground">Spotify Connect</div>
							<div class="text-[10px] text-muted-foreground truncate">
								Link Spotify to stream across all devices
							</div>
						</div>
					</div>
					<button
						type="button"
						onclick={openSpotifySettings}
						class="rounded-lg bg-[#1ed760] px-3 py-1.5 text-[11px] font-semibold text-black hover:opacity-90 transition-opacity cursor-pointer shrink-0"
					>
						Connect
					</button>
				</div>
			</div>

			<!-- Audio Output Devices Section -->
			<div>
				<div class="mb-2.5 flex items-center justify-between">
					<span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
						Audio Output Devices
					</span>
					<Button
						variant="ghost"
						size="icon-sm"
						onclick={fetchAudioDevices}
						aria-label="Refresh audio devices"
						class="h-6 w-6 cursor-pointer hover:bg-foreground/10"
					>
						<HugeiconsIcon icon={RefreshIcon} class="h-3.5 w-3.5 text-muted-foreground" />
					</Button>
				</div>

				{#if loadingAudio}
					<div class="space-y-2">
						{#each { length: 3 } as _, i (i)}
							<div class="h-12 animate-pulse rounded-xl bg-muted/40"></div>
						{/each}
					</div>
				{:else if audioInfo && audioInfo.devices.length > 0}
					<div class="space-y-1.5">
						{#each audioInfo.devices as device (device.name)}
							{@const isSelected = audioInfo.current === device.name || (audioInfo.current === '' && device.name === 'auto')}
							<button
								type="button"
								onclick={() => selectDevice(device.name)}
								class="w-full flex items-center justify-between rounded-xl border p-3 text-left transition-all cursor-pointer {isSelected
									? 'border-primary/50 bg-primary/10 dark:bg-primary/15 shadow-xs ring-1 ring-primary/20'
									: 'border-border/60 bg-card/60 dark:bg-card/40 hover:bg-card/90 hover:border-border'}"
							>
								<div class="flex items-center gap-3 min-w-0 pr-2">
									<div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg {isSelected ? 'bg-primary text-primary-foreground' : 'bg-muted/80 text-foreground'}">
										<HugeiconsIcon icon={VolumeHighIcon} class="h-4 w-4" />
									</div>
									<div class="min-w-0 flex-1">
										<div class="text-xs font-medium text-foreground truncate" title={device.description}>
											{device.description || device.name}
										</div>
										<div class="text-[10px] text-muted-foreground truncate" title={device.name}>
											{device.name === 'auto' ? 'System Default' : device.name}
										</div>
									</div>
								</div>
								{#if isSelected}
									<span class="inline-flex items-center gap-1 rounded-full bg-primary/20 px-2 py-0.5 text-[10px] font-semibold text-primary shrink-0">
										<HugeiconsIcon icon={CheckmarkCircle02Icon} class="h-3 w-3" />
										Active
									</span>
								{/if}
							</button>
						{/each}
					</div>
				{:else}
					<div class="rounded-xl border border-dashed border-border/60 bg-muted/20 p-3 text-center text-xs text-muted-foreground">
						No audio output devices detected
					</div>
				{/if}
			</div>
		{/if}
	</div>
</aside>
