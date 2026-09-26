<script lang="ts">
	// The Library page's Downloaded tab: tracks downloaded to this machine for 100% offline playback.
	// Completely separate from Local files, with full metadata, offline covers, and offline mode toggle.
	import { onMount } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Download01Icon,
		PlayIcon,
		ShuffleIcon,
		RefreshIcon,
		Delete02Icon,
		FolderOpenIcon,
		HotspotOfflineIcon,
		Search01Icon,
		MusicNote01Icon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import ErrorState from './ErrorState.svelte';
	import TrackRow from './TrackRow.svelte';
	import * as api from '$lib/api';
	import type { DownloadedSong, SongItem } from '$lib/api';
	import {
		downloaded,
		downloadedToSongItem,
		loadDownloadedSongs,
		scanDownloadedSongs,
		deleteDownloadedSong,
		toggleOfflineMode,
		openPlayer,
		playback,
		toast
	} from '$lib/player.svelte';

	onMount(() => {
		loadDownloadedSongs();
	});

	let query = $state('');

	function formatBytes(bytes: number): string {
		if (!bytes || bytes <= 0) return '0 B';
		const k = 1024;
		const sizes = ['B', 'KB', 'MB', 'GB'];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
	}

	const totalBytes = $derived(
		downloaded.songs.reduce((acc, s) => acc + (s.file_size || 0), 0)
	);

	const q = $derived(query.trim().toLowerCase());
	const filteredSongs = $derived(
		q
			? downloaded.songs.filter(
					(s) =>
						s.title.toLowerCase().includes(q) ||
						s.artist.toLowerCase().includes(q) ||
						(s.album && s.album.toLowerCase().includes(q))
				)
			: downloaded.songs
	);

	const songItems = $derived(filteredSongs.map(downloadedToSongItem));

	const SOURCE = 'Downloaded music';

	function playTrack(index: number) {
		if (!songItems.length) return;
		openPlayer();
		api.playPlaylist(songItems, index, undefined, SOURCE, false);
	}

	function playAll(shuffle: boolean) {
		if (!songItems.length) return;
		openPlayer();
		api.playPlaylist(songItems, null, undefined, SOURCE, shuffle);
	}

	function openInFolder(song: DownloadedSong) {
		if (song.file_path) {
			api.showDownloadedFile(song.file_path).catch((err) => {
				toast.error(`Could not open file: ${err}`);
			});
		}
	}
</script>

<div class="flex flex-col gap-5">
	<!-- Downloaded Music Header Card -->
	<div class="rounded-xl border bg-card/40 p-4">
		<div class="flex flex-wrap items-center justify-between gap-4">
			<div class="min-w-0">
				<div class="flex items-center gap-2 font-medium">
					<HugeiconsIcon icon={Download01Icon} class="h-4 w-4 text-primary" />
					<span>Downloaded Music</span>
					{#if downloaded.songs.length > 0}
						<span class="rounded-full bg-primary/10 px-2 py-0.5 text-xs font-semibold text-primary">
							{downloaded.songs.length} track{downloaded.songs.length === 1 ? '' : 's'}
						</span>
						<span class="text-xs text-muted-foreground">• {formatBytes(totalBytes)}</span>
					{/if}
				</div>
				<p class="mt-0.5 text-xs text-muted-foreground">
					Songs saved directly to your disk with high-fidelity tags and artwork for full offline listening.
				</p>
			</div>

			<div class="flex shrink-0 flex-wrap items-center gap-2">
				<Button
					variant={downloaded.offlineMode ? 'default' : 'outline'}
					size="sm"
					class="gap-1.5 {downloaded.offlineMode ? 'bg-amber-500 hover:bg-amber-600 text-white' : ''}"
					onclick={toggleOfflineMode}
					title="Toggle Offline Mode to block all online streams and only play downloaded songs"
				>
					<HugeiconsIcon icon={HotspotOfflineIcon} class="h-4 w-4" />
					{downloaded.offlineMode ? 'Offline Mode Active' : 'Offline Mode'}
				</Button>

				<Button
					variant="ghost"
					size="sm"
					class="gap-1.5"
					disabled={downloaded.loading}
					onclick={() => scanDownloadedSongs()}
					title="Rescan and verify downloaded files on disk"
				>
					<HugeiconsIcon
						icon={RefreshIcon}
						class="h-4 w-4 {downloaded.loading ? 'animate-spin' : ''}"
					/>
					{downloaded.loading ? 'Verifying…' : 'Verify'}
				</Button>

				{#if downloaded.songs.length > 0}
					<Button
						variant="outline"
						size="sm"
						class="gap-1.5"
						onclick={() => playAll(false)}
					>
						<HugeiconsIcon icon={PlayIcon} class="h-4 w-4" />
						Play all
					</Button>
					<Button
						variant="outline"
						size="sm"
						class="gap-1.5"
						onclick={() => playAll(true)}
					>
						<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4" />
						Shuffle
					</Button>
				{/if}
			</div>
		</div>

		{#if downloaded.offlineMode}
			<div class="mt-3 flex items-center justify-between rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-200">
				<div class="flex items-center gap-2">
					<HugeiconsIcon icon={HotspotOfflineIcon} class="h-4 w-4 text-amber-400 shrink-0" />
					<span>Offline Mode is ON — un-downloaded online tracks are blocked to save bandwidth and prevent hangs.</span>
				</div>
				<button
					type="button"
					onclick={toggleOfflineMode}
					class="underline hover:text-amber-100 font-medium cursor-pointer shrink-0 ml-2"
				>
					Turn Off
				</button>
			</div>
		{/if}
	</div>

	<!-- Search & Track Count -->
	{#if downloaded.songs.length > 0}
		<div class="flex items-center justify-between gap-3">
			<div class="relative flex-1 max-w-sm">
				<HugeiconsIcon
					icon={Search01Icon}
					class="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground"
				/>
				<Input
					bind:value={query}
					placeholder="Search downloaded songs, artists, albums…"
					class="pl-8.5 h-8 text-xs"
				/>
			</div>
			<div class="text-xs text-muted-foreground">
				{#if q}
					Showing {filteredSongs.length} of {downloaded.songs.length}
				{/if}
			</div>
		</div>
	{/if}

	{#if downloaded.error}
		<ErrorState message={downloaded.error} onRetry={() => scanDownloadedSongs()} />
	{:else if downloaded.loading && !downloaded.loaded}
		<div class="flex flex-col gap-2 py-8 text-center text-sm text-muted-foreground">
			<HugeiconsIcon icon={RefreshIcon} class="mx-auto h-6 w-6 animate-spin text-primary" />
			<span>Loading downloaded tracks…</span>
		</div>
	{:else if !downloaded.songs.length}
		<!-- Empty state -->
		<div class="flex flex-col items-center justify-center rounded-2xl border border-dashed border-border/80 p-12 text-center">
			<div class="mb-4 flex h-14 w-14 items-center justify-center rounded-full bg-primary/10 text-primary">
				<HugeiconsIcon icon={Download01Icon} class="h-7 w-7" />
			</div>
			<h3 class="font-heading text-base font-semibold">No downloaded songs yet</h3>
			<p class="mt-1 max-w-md text-xs text-muted-foreground">
				Download any song from search results, albums, or playlists using the "Download song" option in the track's options menu (⋯). Downloaded songs stay completely offline and play instantly with zero network access.
			</p>
		</div>
	{:else if !filteredSongs.length}
		<div class="py-8 text-center text-sm text-muted-foreground">
			No downloaded songs match "{query}"
		</div>
	{:else}
		<!-- Track list -->
		<div class="flex flex-col">
			{#each songItems as song, i (song.video_id)}
				{@const raw = filteredSongs[i]}
				<div class="group/dl flex items-center justify-between gap-2 border-b border-border/30 last:border-0 hover:bg-accent/5 rounded-lg pr-2">
					<div class="min-w-0 flex-1">
						<TrackRow
							{song}
							index={i + 1}
							active={playback.now?.videoId === song.video_id}
							onplay={() => playTrack(i)}
							onRemove={() => deleteDownloadedSong(song.video_id)}
							removeLabel="Delete download"
						/>
					</div>
					<div class="flex shrink-0 items-center gap-2">
						{#if raw?.file_size}
							<span class="rounded bg-muted/70 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
								{formatBytes(raw.file_size)}
							</span>
						{/if}
						<button
							type="button"
							class="flex h-7 w-7 cursor-pointer items-center justify-center rounded-md text-muted-foreground opacity-0 transition group-hover/dl:opacity-100 hover:bg-accent/20 hover:text-foreground"
							title="Show file in folder"
							aria-label="Show file in folder"
							onclick={() => openInFolder(raw)}
						>
							<HugeiconsIcon icon={FolderOpenIcon} class="h-3.5 w-3.5" />
						</button>
						<button
							type="button"
							class="flex h-7 w-7 cursor-pointer items-center justify-center rounded-md text-muted-foreground opacity-0 transition group-hover/dl:opacity-100 hover:bg-destructive/10 hover:text-destructive"
							title="Delete downloaded song"
							aria-label="Delete downloaded song"
							onclick={() => deleteDownloadedSong(song.video_id)}
						>
							<HugeiconsIcon icon={Delete02Icon} class="h-3.5 w-3.5" />
						</button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
