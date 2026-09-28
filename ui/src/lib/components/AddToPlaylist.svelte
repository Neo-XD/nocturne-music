<script lang="ts">
	import { fade, scale } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Cancel01Icon, Add01Icon } from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import * as api from '$lib/api';
	import type { BrowseItem } from '$lib/api';
	import {
		ui,
		auth,
		toast,
		createLibraryPlaylist,
		bumpLibraryTrackCount,
		notePlaylistAdd,
		noteSavedIn,
		spotify,
		loadSpotifyPlaylists
	} from '$lib/player.svelte';

	let playlists = $state<BrowseItem[]>([]);
	let loading = $state(false);
	let filter = $state('');
	let box = $state<HTMLInputElement | null>(null);
	let showNew = $state(false);
	let newTitle = $state('');
	let creating = $state(false);

	// `autofocus` is unreliable on an element inserted after load (and mid-transition), so focus it
	// ourselves the frame it exists: the modal opens ready to type.
	$effect(() => {
		box?.focus();
	});
	// ponytail: plain substring, not fuzzy. A library is tens of playlists, and "rap" finding
	// "Rap Caviar" is what issue #100 actually asked for.
	const matches = $derived(
		playlists.filter((p) => p.title.toLowerCase().includes(filter.trim().toLowerCase()))
	);

	// Fetch the library playlists fresh each time the picker opens (cheap; picks up new playlists).
	// On Repeat and Liked Music are dropped: On Repeat is built from local play counts, and Liked
	// Music takes likes rather than playlist edits (YouTube 400s the add). The command boundary
	// refuses both too, but a target you can tap and can't use is the bug.
	$effect(() => {
		if (ui.addSongs) {
			loading = true;
			filter = '';
			showNew = false;
			newTitle = '';
			const promises: Promise<unknown>[] = [];
			if (auth.account?.signedIn) {
				promises.push(api.getLibrary().catch(() => []));
			} else {
				promises.push(Promise.resolve([]));
			}
			if (spotify.status.linked) {
				promises.push(api.spotifyGetPlaylists().catch(() => []));
			}
			Promise.all(promises)
				.then(([p, sp]) => {
					let list = ((p as BrowseItem[]) || []).filter(
						(i) => i.id !== api.ON_REPEAT_ID && i.id !== api.LIKED_MUSIC_ID
					);
					if (spotify.status.linked) {
						list.unshift({
							id: 'sp_liked',
							title: 'Liked Songs',
							kind: 'playlist',
							thumbnail: undefined,
							subtitle: 'Spotify Liked Songs'
						});
					}
					if (Array.isArray(sp)) {
						for (const s of sp as api.SpotifyPlaylistSummary[]) {
							list.push({
								id: `sp_${s.id}`,
								title: s.title,
								kind: 'playlist',
								thumbnail: s.thumbnail,
								subtitle: s.subtitle || 'Spotify Playlist'
							});
						}
					}
					playlists = list;
				})
				.catch((e) => toast.error(String(e)))
				.finally(() => (loading = false));
		}
	});

	function close() {
		ui.addSongs = null;
		showNew = false;
	}

	async function createAndPick() {
		const title = newTitle.trim();
		const songs = ui.addSongs;
		if (!title || !songs?.length || creating) return;
		creating = true;
		try {
			const platform = !auth.account?.signedIn && spotify.status.linked ? 'spotify' : 'ytm';
			const id = await createLibraryPlaylist(title, undefined, false, null, platform);
			newTitle = '';
			showNew = false;
			await pick({ id, title, kind: 'playlist' });
		} catch (e) {
			toast.error(String(e));
		} finally {
			creating = false;
		}
	}

	async function pick(pl: BrowseItem) {
		const songs = ui.addSongs;
		close();
		if (!songs?.length) return;
		try {
			// Sequential — a whole album is a handful of requests; don't hammer the API in parallel.
			// YouTube refuses a track the playlist already holds, so only the ones it accepted get
			// counted and drawn: an optimistic row for a refused add is a row that can never be
			// removed (no setVideoId behind it) until the app restarts.
			const added: typeof songs = [];
			for (const song of songs) {
				if (await api.addToPlaylist(pl.id, song.video_id)) added.push(song);
			}
			const dupes = songs.length - added.length;
			// Every song, not just the accepted ones: a refusal means the playlist already holds it,
			// so its "saved" mark is right either way.
			noteSavedIn(pl.id, songs.map((s) => s.video_id));
			if (added.length) {
				bumpLibraryTrackCount(pl.id, added.length);
				notePlaylistAdd(pl.id, added);
			}
			if (pl.id.startsWith('sp_')) {
				void loadSpotifyPlaylists();
			}
			if (!added.length) {
				toast(dupes > 1 ? `All ${dupes} are already in ${pl.title}` : `Already in ${pl.title}`);
			} else if (dupes) {
				toast.success(`Added ${added.length} to ${pl.title} (${dupes} already there)`);
			} else {
				toast.success(
					added.length > 1 ? `Added ${added.length} songs to ${pl.title}` : `Added to ${pl.title}`
				);
			}
		} catch (e) {
			toast.error(String(e));
		}
	}
</script>

<svelte:window
	onkeydown={(e) => {
		if (ui.addSongs && e.key === 'Escape') close();
	}}
/>

{#if ui.addSongs}
	<div
		transition:fade={{ duration: 150 }}
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
	>
		<div
			transition:scale={{ duration: 180, start: 0.96, easing: cubicOut }}
			class="flex max-h-[32rem] w-full max-w-sm flex-col rounded-xl border bg-card p-4 shadow-xl"
		>
			<div class="mb-3 flex items-center justify-between">
				<h2 class="font-heading text-base font-semibold">Add to playlist</h2>
				<button
					class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
					onclick={close}
					aria-label="Close"
				>
					<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4" />
				</button>
			</div>
			<div class="mb-2 flex items-center justify-between gap-2 border-b border-border/40 pb-2">
				{#if showNew}
					<form
						class="flex flex-1 items-center gap-1.5"
						onsubmit={(e) => {
							e.preventDefault();
							createAndPick();
						}}
					>
						<input
							bind:value={newTitle}
							placeholder="New playlist name..."
							class="h-8 flex-1 rounded-md border border-border bg-background px-2.5 text-xs outline-none focus:ring-1 focus:ring-primary"
						/>
						<Button
							size="sm"
							type="submit"
							disabled={creating || !newTitle.trim()}
							class="h-8 px-2.5 text-xs cursor-pointer"
						>
							{creating ? 'Creating...' : 'Create'}
						</Button>
						<Button
							size="sm"
							variant="ghost"
							type="button"
							onclick={() => (showNew = false)}
							class="h-8 px-2 text-xs cursor-pointer"
						>
							Cancel
						</Button>
					</form>
				{:else}
					<Button
						variant="outline"
						size="sm"
						class="w-full justify-start gap-2 text-xs font-medium text-primary hover:text-primary cursor-pointer"
						onclick={() => (showNew = true)}
					>
						<HugeiconsIcon icon={Add01Icon} class="h-3.5 w-3.5" />
						<span>New playlist</span>
					</Button>
				{/if}
			</div>

			{#if playlists.length > 1}
				<input
					bind:this={box}
					bind:value={filter}
					placeholder="Search your playlists…"
					onkeydown={(e) => e.key === 'Enter' && matches[0] && pick(matches[0])}
					class="mb-2 w-full rounded-lg border bg-background px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
				/>
			{/if}
			{#if loading}
				<p class="p-2 text-sm text-muted-foreground">Loading…</p>
			{:else if matches.length}
				<div class="min-h-0 flex-1 overflow-y-auto">
					{#each matches as pl (pl.id)}
						<button
							class="flex w-full items-center gap-3 rounded-lg p-2 text-left hover:bg-accent/10 transition-colors cursor-pointer"
							onclick={() => pick(pl)}
						>
							{#if pl.thumbnail}
								<img src={pl.thumbnail} alt="" class="h-10 w-10 rounded-md object-cover" />
							{:else}
								<div class="flex h-10 w-10 items-center justify-center rounded-md {pl.id.startsWith('sp_') ? 'bg-emerald-500/20 text-emerald-400' : 'bg-muted'}">
									{#if pl.id.startsWith('sp_')}
										<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor">
											<path d="M12 2C6.477 2 2 6.477 2 12c0 5.524 4.477 10 10 10 5.524 0 10-4.476 10-10 0-5.523-4.476-10-10-10zm4.586 14.424a.627.627 0 0 1-.86.208c-2.355-1.439-5.32-1.765-8.812-.966a.625.625 0 0 1-.277-1.22c3.824-.874 7.099-.508 9.74 1.107.292.179.387.568.209.871zm1.226-2.723a.784.784 0 0 1-1.077.26c-2.695-1.656-6.804-2.136-9.992-1.168a.785.785 0 1 1-.462-1.501c3.642-1.106 8.188-.574 11.27 1.321a.784.784 0 0 1 .261 1.088zm.105-2.833c-3.232-1.919-8.566-2.096-11.657-1.157a.94.94 0 1 1-.552-1.8c3.553-1.078 9.444-.87 13.14 1.323a.94.94 0 0 1-.931 1.634z"/>
										</svg>
									{/if}
								</div>
							{/if}
							<div class="min-w-0 flex-1">
								<div class="truncate text-sm font-medium flex items-center gap-1.5">
									<span>{pl.title}</span>
									{#if pl.id.startsWith('sp_')}
										<span class="rounded bg-emerald-500/15 px-1 py-0.5 text-[10px] font-semibold text-emerald-400">Spotify</span>
									{/if}
								</div>
								{#if pl.subtitle}
									<div class="truncate text-xs text-muted-foreground">{pl.subtitle}</div>
								{/if}
							</div>
						</button>
					{/each}
				</div>
			{:else}
				<p class="p-2 text-sm text-muted-foreground">
					{filter.trim() ? 'Nothing matches that.' : 'No playlists yet, create one in your Library.'}
				</p>
			{/if}
		</div>
	</div>
{/if}
