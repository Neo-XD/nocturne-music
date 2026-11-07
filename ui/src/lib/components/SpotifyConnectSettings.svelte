<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { Switch } from '$lib/components/ui/switch';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		CheckmarkCircle02Icon,
		ComputerIcon,
		HeadphonesIcon,
		MusicNote01Icon,
		PencilEdit02Icon,
		RefreshIcon
	} from '@hugeicons/core-free-icons';
	import SpotifyIcon from '$lib/components/icons/SpotifyIcon.svelte';
	import * as api from '$lib/api';
	import type { SpotifyConnectStatus, SpotifyScrobbleRecord } from '$lib/api';
	import { toast } from '$lib/player.svelte';

	let status = $state<SpotifyConnectStatus>({
		enabled: true,
		device_name: 'Nocturne',
		device_id: '',
		active: false,
		scrobble_enabled: true,
		total_scrobbles: 0
	});

	let deviceNameInput = $state('Nocturne');
	let isEditingName = $state(false);
	let savingName = $state(false);
	let scrobbles = $state<SpotifyScrobbleRecord[]>([]);
	let loadingScrobbles = $state(false);

	let unlistenState: (() => void) | null = null;
	let unlistenScrobble: (() => void) | null = null;

	async function loadStatus() {
		try {
			status = await api.spotifyConnectStatus();
			deviceNameInput = status.device_name;
		} catch (e) {
			console.error('Failed to load Spotify Connect status', e);
		}
	}

	async function loadScrobbles() {
		loadingScrobbles = true;
		try {
			scrobbles = await api.spotifyConnectGetScrobbles(15);
		} catch (e) {
			console.error('Failed to load Spotify scrobbles', e);
		} finally {
			loadingScrobbles = false;
		}
	}

	async function toggleConnect(on: boolean) {
		status.enabled = on;
		try {
			await api.spotifyConnectSetEnabled(on);
			toast.success(on ? 'Spotify Connect device enabled' : 'Spotify Connect device disabled');
		} catch (e) {
			toast.error(String(e));
			await loadStatus();
		}
	}

	async function toggleScrobbler(on: boolean) {
		status.scrobble_enabled = on;
		try {
			await api.spotifyConnectSetScrobbleEnabled(on);
			toast.success(on ? 'Spotify scrobbling enabled' : 'Spotify scrobbling disabled');
		} catch (e) {
			toast.error(String(e));
			await loadStatus();
		}
	}

	async function saveDeviceName() {
		const clean = deviceNameInput.trim();
		if (!clean) return;
		savingName = true;
		try {
			await api.spotifyConnectSetDeviceName(clean);
			status.device_name = clean;
			isEditingName = false;
			toast.success(`Spotify Connect device renamed to "${clean}"`);
		} catch (e) {
			toast.error(String(e));
		} finally {
			savingName = false;
		}
	}

	function timeAgo(secs: number): string {
		const now = Math.floor(Date.now() / 1000);
		const diff = Math.max(0, now - secs);
		if (diff < 60) return 'Just now';
		if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
		if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
		return `${Math.floor(diff / 86400)}d ago`;
	}

	onMount(() => {
		void loadStatus();
		void loadScrobbles();

		void api.onSpotifyConnectState((event) => {
			status.active = event.active;
		}).then((un) => {
			unlistenState = un;
		});

		void api.onSpotifyScrobbled((record) => {
			status.total_scrobbles += 1;
			scrobbles = [record, ...scrobbles.slice(0, 14)];
		}).then((un) => {
			unlistenScrobble = un;
		});
	});

	onDestroy(() => {
		unlistenState?.();
		unlistenScrobble?.();
	});
</script>

<div class="space-y-4">
	<!-- Spotify Connect Device Card -->
	<div class="rounded-xl border border-white/10 bg-white/[0.03] p-4 backdrop-blur-md">
		<div class="flex items-start justify-between gap-3">
			<div class="flex items-center gap-3">
				<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-[#1ed760]/10 text-[#1ed760] border border-[#1ed760]/20">
					<SpotifyIcon class="h-5 w-5" />
				</div>
				<div>
					<div class="flex items-center gap-2">
						<h4 class="text-sm font-semibold text-foreground">Spotify Connect Receiver</h4>
						{#if status.active}
							<span class="inline-flex items-center gap-1 rounded-full bg-[#1ed760]/15 px-2 py-0.5 text-[10px] font-medium text-[#1ed760] border border-[#1ed760]/30 animate-pulse">
								<span class="h-1.5 w-1.5 rounded-full bg-[#1ed760]"></span>
								Streaming
							</span>
						{:else if status.enabled}
							<span class="inline-flex items-center gap-1 rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-medium text-emerald-400 border border-emerald-500/20">
								<span class="h-1.5 w-1.5 rounded-full bg-emerald-400"></span>
								Ready on LAN
							</span>
						{:else}
							<span class="rounded-full bg-white/5 px-2 py-0.5 text-[10px] font-medium text-muted-foreground border border-white/5">
								Disabled
							</span>
						{/if}
					</div>
					<p class="text-xs text-muted-foreground">
						Broadcasts Nocturne as a Spotify Connect speaker on your local network.
					</p>
				</div>
			</div>
			<Switch checked={status.enabled} onCheckedChange={(val) => toggleConnect(val)} />
		</div>

		{#if status.enabled}
			<div class="mt-4 pt-3 border-t border-white/5 flex flex-wrap items-center justify-between gap-3 text-xs">
				<div class="flex items-center gap-2 text-muted-foreground">
					<HugeiconsIcon icon={ComputerIcon} size={15} />
					<span>Device name:</span>
					{#if isEditingName}
						<div class="flex items-center gap-1.5">
							<Input
								class="h-7 w-40 text-xs bg-white/5 border-white/10"
								bind:value={deviceNameInput}
								placeholder="Device name"
								disabled={savingName}
							/>
							<Button
								variant="secondary"
								size="sm"
								class="h-7 px-2 text-xs"
								disabled={savingName || !deviceNameInput.trim()}
								onclick={() => saveDeviceName()}
							>
								Save
							</Button>
							<Button
								variant="ghost"
								size="sm"
								class="h-7 px-2 text-xs"
								onclick={() => {
									deviceNameInput = status.device_name;
									isEditingName = false;
								}}
							>
								Cancel
							</Button>
						</div>
					{:else}
						<span class="font-medium text-foreground">{status.device_name}</span>
						<button
							type="button"
							class="text-muted-foreground hover:text-foreground transition-colors p-1"
							onclick={() => (isEditingName = true)}
							title="Rename device"
						>
							<HugeiconsIcon icon={PencilEdit02Icon} size={13} />
						</button>
					{/if}
				</div>

				<div class="text-[11px] text-muted-foreground font-mono">
					ID: {status.device_id.slice(0, 10)}...
				</div>
			</div>
		{/if}
	</div>

	<!-- Spotify Scrobbler Card -->
	<div class="rounded-xl border border-white/10 bg-white/[0.03] p-4 backdrop-blur-md">
		<div class="flex items-start justify-between gap-3">
			<div class="flex items-center gap-3">
				<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
					<HugeiconsIcon icon={HeadphonesIcon} size={20} />
				</div>
				<div>
					<div class="flex items-center gap-2">
						<h4 class="text-sm font-semibold text-foreground">Scrobble to Spotify</h4>
						{#if status.scrobble_enabled}
							<span class="rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-medium text-emerald-400 border border-emerald-500/20">
								Active
							</span>
						{/if}
					</div>
					<p class="text-xs text-muted-foreground">
						Automatically counts songs played in Nocturne toward your Spotify listening history and stats.
					</p>
				</div>
			</div>
			<Switch checked={status.scrobble_enabled} onCheckedChange={(val) => toggleScrobbler(val)} />
		</div>

		{#if status.scrobble_enabled}
			<div class="mt-4 pt-3 border-t border-white/5 space-y-3">
				<div class="flex items-center justify-between text-xs">
					<span class="text-muted-foreground">Total tracks scrobbled to Spotify</span>
					<span class="font-mono font-medium text-emerald-400 bg-emerald-500/10 px-2 py-0.5 rounded border border-emerald-500/20">
						{status.total_scrobbles}
					</span>
				</div>

				{#if scrobbles.length > 0}
					<div class="space-y-1.5 pt-1">
						<div class="flex items-center justify-between text-[11px] font-medium text-muted-foreground px-1 uppercase tracking-wider">
							<span>Recent Spotify Scrobbles</span>
							<button
								type="button"
								class="hover:text-foreground transition-colors flex items-center gap-1"
								onclick={() => loadScrobbles()}
								title="Refresh list"
							>
								<HugeiconsIcon icon={RefreshIcon} size={12} class={loadingScrobbles ? 'animate-spin' : ''} />
								Refresh
							</button>
						</div>

						<div class="divide-y divide-white/5 rounded-lg border border-white/5 bg-black/20 overflow-hidden max-h-48 overflow-y-auto">
							{#each scrobbles as s (s.id)}
								<div class="flex items-center justify-between gap-3 px-3 py-2 text-xs">
									<div class="min-w-0 flex-1 flex items-center gap-2">
										<HugeiconsIcon icon={MusicNote01Icon} size={13} class="text-muted-foreground shrink-0" />
										<div class="min-w-0 truncate">
											<span class="font-medium text-foreground truncate">{s.title}</span>
											<span class="text-muted-foreground truncate"> · {s.artist}</span>
										</div>
									</div>
									<div class="flex items-center gap-2 shrink-0">
										{#if s.spotify_id}
											<span class="text-[10px] text-[#1ed760] font-mono bg-[#1ed760]/10 px-1.5 py-0.5 rounded">
												Spotify Matched
											</span>
										{/if}
										<span class="text-[11px] text-muted-foreground font-mono">
											{timeAgo(s.scrobbled_at)}
										</span>
									</div>
								</div>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>
