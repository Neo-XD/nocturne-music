<script module lang="ts">
	// Survives remounts (module scope), so coming back to /search — from a result you clicked, or
	// from the sidebar — shows the last search instead of a blank page. The results themselves come
	// back from the page cache, so the rerun paints instantly and just revalidates.
	let lastQuery = $state('');
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Search01Icon,
		RotateLeft01Icon,
		MusicNote01Icon,
		Video01Icon,
		SparklesIcon,
		CdIcon,
		UserIcon,
		Playlist02Icon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import MediaCard from '$lib/components/MediaCard.svelte';
	import MediaCardSkeleton from '$lib/components/MediaCardSkeleton.svelte';
	import SearchSuggest from '$lib/components/SearchSuggest.svelte';
	import TrackRow from '$lib/components/TrackRow.svelte';
	import TrackRowSkeleton from '$lib/components/TrackRowSkeleton.svelte';
	import TopResult from '$lib/components/TopResult.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import Shelf from '$lib/components/Shelf.svelte';
	import * as api from '$lib/api';
	import type { BrowseItem, Mood, MoodSection, SearchResults, SongItem } from '$lib/api';
	import { getCached, putCached } from '$lib/pagecache';
	import { openAddToPlaylist, playSong, playback } from '$lib/player.svelte';
	import { asSong } from '$lib/browse';
	import { thumb } from '$lib/thumb';

	type Cached = { res: SearchResults; songs: SongItem[]; videos?: SongItem[] };
	type Category = 'all' | 'songs' | 'videos' | 'albums' | 'artists' | 'playlists';

	let query = $state(lastQuery);
	let res = $state<SearchResults | null>(null);
	let songs = $state<SongItem[]>([]);
	let videos = $state<SongItem[]>([]);
	let searched = $state('');
	let searching = $state(false);
	let error = $state<string | null>(null);
	let activeCategory = $state<Category>('all');

	// Moods & Genres browse data
	let moods = $state<MoodSection[] | null>(null);
	let moodArt = $state<Record<string, string>>({});
	let moodsLoading = $state(false);

	let latest = '';

	async function loadMoods() {
		if (moodsLoading || moods !== null) return;
		moodsLoading = true;
		try {
			const m = await api.getMoods();
			moods = m;
			const allParams = m.flatMap((s) => s.items.map((i) => i.params));
			if (allParams.length) {
				api.getMoodArt(allParams)
					.then((art) => {
						moodArt = { ...moodArt, ...art };
					})
					.catch(() => {});
			}
		} catch {
			moods = [];
		} finally {
			moodsLoading = false;
		}
	}

	function openMood(m: Mood) {
		goto(`/list?browseId=${encodeURIComponent(api.MOODS_CATEGORY_ID)}&params=${encodeURIComponent(m.params)}&title=${encodeURIComponent(m.title)}`);
	}

	function resetSearch() {
		query = '';
		lastQuery = '';
		res = null;
		songs = [];
		videos = [];
		searched = '';
		latest = '';
		error = null;
		activeCategory = 'all';
		lastUrlQuery = '';
		if (page.url.searchParams.has('q')) {
			goto('/search', { replaceState: true });
		}
		if (moods === null) loadMoods();
	}

	async function runSearch() {
		if (!query.trim()) return;
		const q = query.trim();
		latest = q;
		lastQuery = q;
		const key = `search:${q}`;
		const hit = getCached<Cached>(key);
		if (hit) {
			res = hit.res;
			songs = hit.songs;
			videos = hit.videos ?? [];
			searched = q;
			searching = false;
		} else {
			searching = true;
		}
		error = null;
		try {
			const [fresh, freshSongs, freshVideos] = await Promise.all([
				api.searchAll(q),
				api.search(q).catch(() => [] as SongItem[]),
				api.searchVideos(q).catch(() => [] as SongItem[])
			]);
			if (latest !== q) return;
			res = fresh;
			songs = freshSongs;
			videos = freshVideos;
			searched = q;
			putCached(key, { res: fresh, songs: freshSongs, videos: freshVideos });
		} catch (e) {
			if (latest !== q) return;
			if (!hit) error = String(e);
		} finally {
			if (latest === q) searching = false;
		}
	}

	function showMore(cat: 'songs' | 'videos' | 'albums' | 'artists' | 'playlists') {
		activeCategory = cat;
	}

	const urlQuery = $derived(page.url.searchParams.get('q') ?? '');
	let lastUrlQuery = '';
	$effect(() => {
		if (urlQuery && urlQuery !== lastUrlQuery) {
			lastUrlQuery = urlQuery;
			query = urlQuery;
			runSearch();
		} else if (!urlQuery && lastUrlQuery) {
			resetSearch();
		}
	});

	onMount(() => {
		if (!urlQuery && query) {
			runSearch();
		} else if (!query) {
			loadMoods();
		}
	});

	const songRows = $derived(songs.length ? songs : (res?.songs ?? []).map(asSong));
	const topResult = $derived(res?.top?.[0] ?? null);

	const categories: { id: Category; label: string; icon: any }[] = [
		{ id: 'all', label: 'All', icon: SparklesIcon },
		{ id: 'songs', label: 'Songs', icon: MusicNote01Icon },
		{ id: 'videos', label: 'Videos', icon: Video01Icon },
		{ id: 'albums', label: 'Albums', icon: CdIcon },
		{ id: 'artists', label: 'Artists', icon: UserIcon },
		{ id: 'playlists', label: 'Playlists', icon: Playlist02Icon }
	];
</script>

<div class="flex h-full flex-col">
	<div class="border-b border-border/40 p-6 backdrop-blur-md">
		<h1 class="mb-4 font-heading text-2xl font-bold tracking-tight">Search</h1>
		<form
			class="flex max-w-xl gap-2"
			onsubmit={(e) => {
				e.preventDefault();
				runSearch();
			}}
		>
			<SearchSuggest
				bind:value={query}
				placeholder="Search songs, albums, artists, playlists…"
				onpick={() => (lastQuery = query)}
				onclear={resetSearch}
			/>
			<Button type="submit" class="gap-2 shrink-0 cursor-pointer" disabled={searching}>
				<HugeiconsIcon icon={Search01Icon} class="h-4 w-4" />
				{searching ? 'Searching…' : 'Search'}
			</Button>
			{#if query || res || searched || lastQuery}
				<Button
					type="button"
					variant="outline"
					class="gap-1.5 shrink-0 cursor-pointer text-muted-foreground hover:text-foreground hover:bg-muted/80"
					onclick={resetSearch}
					title="Reset last search"
				>
					<HugeiconsIcon icon={RotateLeft01Icon} class="h-4 w-4" />
					<span>Reset</span>
				</Button>
			{/if}
		</form>

		{#if res}
			<!-- Category filter pills -->
			<div class="mt-4 flex flex-wrap gap-2">
				{#each categories as cat (cat.id)}
					<button
						class="flex items-center gap-1.5 rounded-full px-3.5 py-1 text-xs font-semibold transition-colors cursor-pointer {activeCategory === cat.id
							? 'bg-primary text-primary-foreground shadow-sm'
							: 'bg-muted/70 text-muted-foreground hover:bg-muted hover:text-foreground'}"
						onclick={() => (activeCategory = cat.id)}
					>
						<HugeiconsIcon icon={cat.icon} class="h-3.5 w-3.5" />
						<span>{cat.label}</span>
					</button>
				{/each}
			</div>
		{/if}

		{#if error}<div class="mt-2"><ErrorState message={error} onRetry={runSearch} /></div>{/if}
	</div>

	<div class="min-h-0 flex-1 overflow-y-auto p-6">
		{#if searching}
			<div class="flex flex-col gap-10">
				<section>
					<Skeleton class="mb-3 h-6 w-40 rounded" />
					{#each Array(5) as _, i (i)}
						<TrackRowSkeleton />
					{/each}
				</section>
				<section>
					<Skeleton class="mb-3 h-6 w-32 rounded" />
					<div class="flex gap-2 overflow-hidden pb-2">
						{#each Array(5) as _, i (i)}
							<div class="w-40 shrink-0"><MediaCardSkeleton /></div>
						{/each}
					</div>
				</section>
			</div>
		{:else if !res}
			<!-- Moods & Genres Browse Feed -->
			{#if moodsLoading || moods === null}
				<div class="space-y-8">
					<section>
						<Skeleton class="mb-4 h-6 w-44 rounded" />
						<div class="mood-grid moods">
							{#each Array(8) as _, i (i)}
								<Skeleton class="h-28 rounded-2xl" />
							{/each}
						</div>
					</section>
				</div>
			{:else if moods.length}
				<div class="space-y-10">
					{#each moods as sec, i (sec.title)}
						{@const kind = i === 0 ? 'moods' : 'genres'}
						<section>
							<h2 class="mb-4 font-heading text-xl font-bold tracking-tight">{sec.title}</h2>
							<div class="mood-grid {kind}">
								{#each sec.items as m (m.params)}
									<button class="mood" style="--mood: {m.color}" onclick={() => openMood(m)}>
										<span class="mood-title">
											{#if kind === 'genres'}<span class="mood-dot"></span>{/if}{m.title}
										</span>
										{#if kind === 'moods'}
											<span class="mood-pill"></span>
											<span class="mood-disc">
												<span class="mood-spin">
													{#if moodArt[m.params]}
														<img
															src={thumb(moodArt[m.params], 256)}
															alt=""
															class="mood-art ready"
															loading="lazy"
														/>
													{/if}
												</span>
											</span>
										{:else}
											<span class="mood-sleeve">
												{#if moodArt[m.params]}
													<img
														src={thumb(moodArt[m.params], 256)}
														alt=""
														class="mood-art ready"
														loading="lazy"
													/>
												{/if}
											</span>
										{/if}
									</button>
								{/each}
							</div>
						</section>
					{/each}
				</div>
			{:else}
				<p class="text-sm text-muted-foreground">Search for a song, album, artist, or playlist.</p>
			{/if}
		{:else}
			<!-- Search Results Views -->
			<div class="content-in flex flex-col gap-10">
				{#if activeCategory === 'all'}
					<!-- Top result & Songs side-by-side or stacked -->
					{#if topResult || songRows.length}
						<div class="grid gap-x-8 gap-y-10 {topResult && songRows.length ? 'lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]' : ''}">
							{#if topResult}
								<section class="min-w-0">
									<h2 class="mb-3 font-heading text-xl font-bold flex items-center gap-2">Top Result</h2>
									<TopResult item={topResult} related={res.top.slice(1)} />
								</section>
							{/if}
							{#if songRows.length}
								<section class="min-w-0">
									<div class="mb-3 flex items-center justify-between">
										<h2 class="font-heading text-xl font-bold flex items-center gap-2">Songs</h2>
										<button
											class="cursor-pointer text-xs font-semibold uppercase text-muted-foreground hover:text-foreground"
											onclick={() => showMore('songs')}
										>
											Show more
										</button>
									</div>
									<div class="space-y-1">
										{#each songRows.slice(0, 5) as song (song.video_id)}
											<TrackRow
												{song}
												showPlayCount
												active={playback.now?.videoId === song.video_id}
												onplay={() => playSong(song)}
												onAdd={() => openAddToPlaylist(song)}
											/>
										{/each}
									</div>
								</section>
							{/if}
						</div>
					{/if}

					{#if videos.length}
						<section>
							<div class="mb-3 flex items-center justify-between">
								<h2 class="font-heading text-xl font-bold">Videos</h2>
								<button
									class="cursor-pointer text-xs font-semibold uppercase text-muted-foreground hover:text-foreground"
									onclick={() => showMore('videos')}
								>
									Show more
								</button>
							</div>
							<div class="space-y-1">
								{#each videos.slice(0, 4) as video (video.video_id)}
									<TrackRow
										song={video}
										showPlayCount
										active={playback.now?.videoId === video.video_id}
										onplay={() => playSong(video)}
										onAdd={() => openAddToPlaylist(video)}
									/>
								{/each}
							</div>
						</section>
					{/if}

					{#if res.albums.length}
						<Shelf title="Albums" items={res.albums} onMore={() => showMore('albums')} />
					{/if}

					{#if res.artists.length}
						<Shelf title="Artists" items={res.artists} onMore={() => showMore('artists')} />
					{/if}

					{#if res.playlists.length}
						<Shelf title="Playlists" items={res.playlists} onMore={() => showMore('playlists')} />
					{/if}

				{:else if activeCategory === 'songs'}
					<section>
						<h2 class="mb-4 font-heading text-xl font-bold">Songs</h2>
						<div class="space-y-1">
							{#each songRows as song (song.video_id)}
								<TrackRow
									{song}
									showPlayCount
									active={playback.now?.videoId === song.video_id}
									onplay={() => playSong(song)}
									onAdd={() => openAddToPlaylist(song)}
								/>
							{/each}
						</div>
					</section>

				{:else if activeCategory === 'videos'}
					<section>
						<h2 class="mb-4 font-heading text-xl font-bold">Videos</h2>
						<div class="space-y-1">
							{#each videos as video (video.video_id)}
								<TrackRow
									song={video}
									showPlayCount
									active={playback.now?.videoId === video.video_id}
									onplay={() => playSong(video)}
									onAdd={() => openAddToPlaylist(video)}
								/>
							{/each}
						</div>
					</section>

				{:else if activeCategory === 'albums'}
					<section>
						<h2 class="mb-4 font-heading text-xl font-bold">Albums</h2>
						<div class="card-grid">
							{#each res.albums as item (item.id)}
								<MediaCard {item} />
							{/each}
						</div>
					</section>

				{:else if activeCategory === 'artists'}
					<section>
						<h2 class="mb-4 font-heading text-xl font-bold">Artists</h2>
						<div class="card-grid">
							{#each res.artists as item (item.id)}
								<MediaCard {item} />
							{/each}
						</div>
					</section>

				{:else if activeCategory === 'playlists'}
					<section>
						<h2 class="mb-4 font-heading text-xl font-bold">Playlists</h2>
						<div class="card-grid">
							{#each res.playlists as item (item.id)}
								<MediaCard {item} />
							{/each}
						</div>
					</section>
				{/if}
			</div>
		{/if}
	</div>
</div>
