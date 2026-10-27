<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Analytics01Icon,
		PlayIcon,
		MusicNote01Icon
	} from '@hugeicons/core-free-icons';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import * as api from '$lib/api';
	import type { SongItem, LastfmProfile } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import { playback } from '$lib/player.svelte';

	let {
		open = $bindable(false)
	}: {
		open: boolean;
	} = $props();

	let loading = $state(true);
	let songs = $state<SongItem[]>([]);
	let playCounts = $state<Record<string, number>>({});
	let lastfm = $state<LastfmProfile | null>(null);

	const totalPlays = $derived(
		Object.values(playCounts).reduce((acc, count) => acc + (typeof count === 'number' ? count : 0), 0)
	);

	const uniqueTracks = $derived(Object.keys(playCounts).length);

	const topSong = $derived(songs.length > 0 ? songs[0] : null);

	$effect(() => {
		if (!open) return;
		loading = true;
		Promise.allSettled([
			api.getPlaylist(api.ON_REPEAT_ID),
			api.getPlayCounts(),
			api.lastfmProfile()
		]).then(([resOnRepeat, resCounts, resLastfm]) => {
			if (resOnRepeat.status === 'fulfilled' && resOnRepeat.value?.items) {
				songs = resOnRepeat.value.items;
			} else {
				songs = [];
			}

			if (resCounts.status === 'fulfilled' && resCounts.value) {
				playCounts = resCounts.value;
			} else {
				playCounts = {};
			}

			if (resLastfm.status === 'fulfilled' && resLastfm.value) {
				lastfm = resLastfm.value;
			} else {
				lastfm = null;
			}

			loading = false;
		});
	});

	function playSong(item: SongItem) {
		api.play(item);
	}

	function playAll() {
		if (songs.length === 0) return;
		api.playPlaylist(songs, 0, api.ON_REPEAT_ID, 'On Repeat', false);
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="max-w-2xl max-h-[85vh] flex flex-col p-0 gap-0 overflow-hidden sm:rounded-2xl border-border/60 bg-card/95 backdrop-blur-2xl">
		<!-- Header -->
		<div class="flex items-center justify-between border-b border-border/50 px-6 py-4.5">
			<div class="flex items-center gap-3">
				<div class="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10 text-primary">
					<HugeiconsIcon icon={Analytics01Icon} class="h-5 w-5" />
				</div>
				<div>
					<Dialog.Title class="text-lg font-semibold tracking-tight">Listening Stats</Dialog.Title>
					<Dialog.Description class="text-xs text-muted-foreground">
						Your listening trends and top played tracks over the past 30 days
					</Dialog.Description>
				</div>
			</div>
			{#if songs.length > 0}
				<Button
					size="sm"
					variant="secondary"
					class="gap-1.5 text-xs rounded-lg cursor-pointer"
					onclick={playAll}
				>
					<HugeiconsIcon icon={PlayIcon} class="h-3.5 w-3.5 fill-current" />
					Play On Repeat
				</Button>
			{/if}
		</div>

		<!-- Body Scroll Container -->
		<div class="flex-1 overflow-y-auto px-6 py-5 space-y-6">
			{#if loading}
				<div class="flex flex-col items-center justify-center py-16 gap-3 text-muted-foreground">
					<div class="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent"></div>
					<p class="text-xs">Gathering your listening insights…</p>
				</div>
			{:else}
				<!-- Stats Overview Cards -->
				<div class="grid grid-cols-2 sm:grid-cols-3 gap-3">
					<div class="flex flex-col rounded-xl border border-border/40 bg-muted/20 p-3.5">
						<span class="text-xs text-muted-foreground font-medium">Total Plays</span>
						<span class="mt-1 text-2xl font-bold tracking-tight text-foreground">{totalPlays.toLocaleString()}</span>
						<span class="text-[11px] text-muted-foreground/80 mt-0.5">past 30 days</span>
					</div>
					<div class="flex flex-col rounded-xl border border-border/40 bg-muted/20 p-3.5">
						<span class="text-xs text-muted-foreground font-medium">Unique Tracks</span>
						<span class="mt-1 text-2xl font-bold tracking-tight text-foreground">{uniqueTracks.toLocaleString()}</span>
						<span class="text-[11px] text-muted-foreground/80 mt-0.5">in your history</span>
					</div>
					<div class="col-span-2 sm:col-span-1 flex flex-col rounded-xl border border-border/40 bg-muted/20 p-3.5">
						<span class="text-xs text-muted-foreground font-medium">Most Played</span>
						<span class="mt-1 text-2xl font-bold tracking-tight text-primary">
							{topSong && playCounts[topSong.video_id] ? playCounts[topSong.video_id] : 0}
						</span>
						<span class="text-[11px] text-muted-foreground/80 mt-0.5 truncate">
							{topSong ? topSong.title : 'No plays yet'}
						</span>
					</div>
				</div>

				<!-- Top Song Highlight -->
				{#if topSong}
					<div class="relative overflow-hidden rounded-2xl border border-primary/20 bg-gradient-to-r from-primary/10 via-accent/5 to-transparent p-4 flex items-center justify-between gap-4">
						<div class="flex items-center gap-3.5 min-w-0">
							{#if topSong.thumbnail}
								<img
									src={thumb(topSong.thumbnail, 120)}
									alt={topSong.title}
									class="h-14 w-14 shrink-0 rounded-xl object-cover shadow-sm ring-1 ring-border/30"
								/>
							{:else}
								<div class="flex h-14 w-14 shrink-0 items-center justify-center rounded-xl bg-muted text-muted-foreground">
									<HugeiconsIcon icon={MusicNote01Icon} class="h-6 w-6" />
								</div>
							{/if}
							<div class="min-w-0">
								<div class="flex items-center gap-2">
									<span class="text-[10px] font-semibold tracking-wider uppercase text-primary bg-primary/15 px-2 py-0.5 rounded-full">
										#1 Top Track
									</span>
									{#if playCounts[topSong.video_id]}
										<span class="text-xs font-medium text-muted-foreground">
											{playCounts[topSong.video_id]} plays
										</span>
									{/if}
								</div>
								<h4 class="mt-1 font-semibold text-sm truncate text-foreground">{topSong.title}</h4>
								<p class="text-xs text-muted-foreground truncate">{topSong.artists}</p>
							</div>
						</div>
						<button
							type="button"
							onclick={() => {
								if (topSong) playSong(topSong);
							}}
							class="flex h-10 w-10 shrink-0 cursor-pointer items-center justify-center rounded-full bg-primary text-primary-foreground shadow-md transition-transform hover:scale-105 active:scale-95"
							title="Play {topSong.title}"
							aria-label="Play {topSong.title}"
						>
							<HugeiconsIcon icon={PlayIcon} class="h-5 w-5 fill-current ml-0.5" />
						</button>
					</div>
				{/if}

				<!-- Top Tracks List -->
				<div>
					<div class="flex items-center justify-between mb-2.5">
						<h3 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
							Top Played Songs ({songs.length})
						</h3>
					</div>

					{#if songs.length === 0}
						<div class="flex flex-col items-center justify-center rounded-xl border border-dashed border-border/60 py-10 text-center text-muted-foreground">
							<HugeiconsIcon icon={MusicNote01Icon} class="h-8 w-8 mb-2 opacity-50" />
							<p class="text-xs font-medium">No listening history recorded yet.</p>
							<p class="text-[11px] text-muted-foreground/70 mt-1">Play your favorite songs to see them appear here!</p>
						</div>
					{:else}
						<div class="divide-y divide-border/30 rounded-xl border border-border/40 bg-muted/10 overflow-hidden">
							{#each songs as song, index}
								{@const count = playCounts[song.video_id] ?? 0}
								{@const isCurrent = playback.now?.videoId === song.video_id}
								<div
									role="button"
									tabindex="0"
									onclick={() => playSong(song)}
									onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && playSong(song)}
									class="group flex items-center gap-3.5 px-3 py-2.5 transition-colors hover:bg-muted/40 cursor-pointer {isCurrent ? 'bg-primary/10' : ''}"
								>
									<!-- Rank -->
									<span class="w-5 text-center text-xs font-bold {index < 3 ? 'text-primary' : 'text-muted-foreground/70'}">
										{index + 1}
									</span>

									<!-- Thumbnail -->
									{#if song.thumbnail}
										<img
											src={thumb(song.thumbnail, 80)}
											alt={song.title}
											class="h-10 w-10 shrink-0 rounded-lg object-cover ring-1 ring-border/20 group-hover:scale-105 transition-transform"
										/>
									{:else}
										<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
											<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4" />
										</div>
									{/if}

									<!-- Title & Artist -->
									<div class="flex-1 min-w-0">
										<p class="text-xs font-medium truncate text-foreground {isCurrent ? 'text-primary' : ''}">
											{song.title}
										</p>
										<p class="text-[11px] text-muted-foreground truncate">{song.artists}</p>
									</div>

									<!-- Play Count Badge -->
									{#if count > 0}
										<span class="shrink-0 rounded-full bg-secondary/80 px-2 py-0.5 text-[11px] font-semibold text-secondary-foreground tabular-nums">
											{count} {count === 1 ? 'play' : 'plays'}
										</span>
									{/if}

									<!-- Duration -->
									{#if song.duration}
										<span class="shrink-0 text-[11px] text-muted-foreground/70 tabular-nums">
											{song.duration}
										</span>
									{/if}
								</div>
							{/each}
						</div>
					{/if}
				</div>

				<!-- Last.fm Integration Section -->
				{#if lastfm}
					<div class="rounded-xl border border-border/40 bg-muted/15 p-4 space-y-3">
						<div class="flex items-center justify-between">
							<span class="text-xs font-semibold text-foreground flex items-center gap-1.5">
								Last.fm Scrobbles
							</span>
							{#if lastfm.url}
								<a
									href={lastfm.url}
									target="_blank"
									rel="noreferrer"
									class="text-[11px] text-primary hover:underline"
								>
									View Last.fm profile
								</a>
							{/if}
						</div>
						<div class="grid grid-cols-3 gap-2">
							<div class="rounded-lg bg-background/50 p-2.5 text-center">
								<span class="block text-sm font-bold text-foreground">{lastfm.scrobbles.toLocaleString()}</span>
								<span class="text-[10px] text-muted-foreground">Scrobbles</span>
							</div>
							<div class="rounded-lg bg-background/50 p-2.5 text-center">
								<span class="block text-sm font-bold text-foreground">{lastfm.artists.toLocaleString()}</span>
								<span class="text-[10px] text-muted-foreground">Artists</span>
							</div>
							<div class="rounded-lg bg-background/50 p-2.5 text-center">
								<span class="block text-sm font-bold text-foreground">{lastfm.tracks.toLocaleString()}</span>
								<span class="text-[10px] text-muted-foreground">Tracks</span>
							</div>
						</div>
					</div>
				{/if}
			{/if}
		</div>

		<!-- Footer -->
		<Dialog.Footer class="border-t border-border/50 px-6 py-3.5 bg-muted/10">
			<Button variant="outline" size="sm" class="cursor-pointer" onclick={() => (open = false)}>
				Close
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
