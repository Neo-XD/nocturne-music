<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ArrowRight01Icon, ArrowLeft01Icon, MusicNote01Icon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { spotify, library, toast } from '$lib/player.svelte';

	let { open = $bindable(false) }: { open: boolean } = $props();

	let direction = $state<'spotify_to_ytm' | 'ytm_to_spotify'>('spotify_to_ytm');
	let selectedId = $state('');
	let transferring = $state(false);

	const spotifyPlaylists = $derived(spotify.playlists);
	const ytmPlaylists = $derived(
		library.items.filter((i) => i.kind === 'playlist')
	);

	async function runTransfer() {
		if (!selectedId) {
			toast.error('Please select a playlist to transfer');
			return;
		}
		transferring = true;
		try {
			if (direction === 'spotify_to_ytm') {
				const res = await api.spotifyTransferToYtm(selectedId);
				toast.success(res);
			} else {
				const res = await api.ytmTransferToSpotify(selectedId);
				toast.success(res);
			}
			open = false;
		} catch (e) {
			toast.error(String(e));
		} finally {
			transferring = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title>Transfer Playlists</Dialog.Title>
			<Dialog.Description>
				Transfer your playlists between Spotify and YouTube Music.
			</Dialog.Description>
		</Dialog.Header>

		<!-- Direction Switcher -->
		<div class="grid grid-cols-2 gap-1 rounded-lg bg-muted/60 p-1">
			<button
				type="button"
				class="flex items-center justify-center gap-1.5 rounded-md py-1.5 text-xs font-semibold transition cursor-pointer {direction === 'spotify_to_ytm' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
				onclick={() => {
					direction = 'spotify_to_ytm';
					selectedId = '';
				}}
			>
				<span class="text-emerald-400 font-bold">Spotify</span>
				<HugeiconsIcon icon={ArrowRight01Icon} class="h-3 w-3" />
				<span class="text-red-400 font-bold">YouTube Music</span>
			</button>
			<button
				type="button"
				class="flex items-center justify-center gap-1.5 rounded-md py-1.5 text-xs font-semibold transition cursor-pointer {direction === 'ytm_to_spotify' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
				onclick={() => {
					direction = 'ytm_to_spotify';
					selectedId = '';
				}}
			>
				<span class="text-red-400 font-bold">YouTube Music</span>
				<HugeiconsIcon icon={ArrowRight01Icon} class="h-3 w-3" />
				<span class="text-emerald-400 font-bold">Spotify</span>
			</button>
		</div>

		<!-- Playlist Selector -->
		<div class="space-y-1.5">
			<label for="playlist-select" class="text-xs font-semibold text-foreground">
				Select {direction === 'spotify_to_ytm' ? 'Spotify' : 'YouTube Music'} Playlist to Transfer:
			</label>

			<div class="max-h-64 overflow-y-auto rounded-lg border border-border/60 bg-muted/30 p-1 space-y-1">
				{#if direction === 'spotify_to_ytm'}
					{#each spotifyPlaylists as pl (pl.id)}
						<button
							type="button"
							class="flex w-full items-center gap-2.5 rounded-md p-2 text-left transition cursor-pointer {selectedId === pl.id ? 'bg-emerald-500/15 border border-emerald-500/40 text-foreground' : 'hover:bg-muted/70 text-muted-foreground'}"
							onclick={() => (selectedId = pl.id)}
						>
							{#if pl.thumbnail}
								<img src={pl.thumbnail} alt="" class="h-8 w-8 rounded object-cover" />
							{:else}
								<div class="flex h-8 w-8 items-center justify-center rounded bg-emerald-500/20 text-emerald-400">
									<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4" />
								</div>
							{/if}
							<div class="min-w-0 flex-1">
								<div class="truncate text-xs font-semibold text-foreground">{pl.title}</div>
								{#if pl.subtitle}
									<div class="truncate text-[10px] text-muted-foreground">{pl.subtitle}</div>
								{/if}
							</div>
						</button>
					{:else}
						<p class="py-4 text-center text-xs text-muted-foreground">No Spotify playlists found.</p>
					{/each}
				{:else}
					{#each ytmPlaylists as pl (pl.id)}
						<button
							type="button"
							class="flex w-full items-center gap-2.5 rounded-md p-2 text-left transition cursor-pointer {selectedId === pl.id ? 'bg-red-500/15 border border-red-500/40 text-foreground' : 'hover:bg-muted/70 text-muted-foreground'}"
							onclick={() => (selectedId = pl.id)}
						>
							{#if pl.thumbnail}
								<img src={pl.thumbnail} alt="" class="h-8 w-8 rounded object-cover" />
							{:else}
								<div class="flex h-8 w-8 items-center justify-center rounded bg-red-500/20 text-red-400">
									<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4" />
								</div>
							{/if}
							<div class="min-w-0 flex-1">
								<div class="truncate text-xs font-semibold text-foreground">{pl.title}</div>
								{#if pl.subtitle}
									<div class="truncate text-[10px] text-muted-foreground">{pl.subtitle}</div>
								{/if}
							</div>
						</button>
					{:else}
						<p class="py-4 text-center text-xs text-muted-foreground">No YouTube Music playlists found.</p>
					{/each}
				{/if}
			</div>
		</div>

		<div class="flex justify-end gap-2 pt-2">
			<Button type="button" variant="outline" onclick={() => (open = false)} disabled={transferring}>
				Cancel
			</Button>
			<Button onclick={runTransfer} disabled={transferring || !selectedId} class="gap-1.5">
				{transferring ? 'Transferring Tracks…' : 'Start Transfer'}
			</Button>
		</div>
	</Dialog.Content>
</Dialog.Root>
