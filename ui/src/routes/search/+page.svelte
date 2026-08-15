<script module lang="ts">
	// Survives remounts (module scope), so coming back to /search — from a result you clicked, or
	// from the sidebar — shows the last search instead of a blank page. The results themselves come
	// back from the page cache, so the rerun paints instantly and just revalidates.
	let lastQuery = '';
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Search01Icon, MusicNote01Icon, UserIcon } from '@hugeicons/core-free-icons';
	import { Input } from '$lib/components/ui/input';
	import { Button } from '$lib/components/ui/button';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import MediaCardSkeleton from '$lib/components/MediaCardSkeleton.svelte';
	import TrackRow from '$lib/components/TrackRow.svelte';
	import TrackRowSkeleton from '$lib/components/TrackRowSkeleton.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import Shelf from '$lib/components/Shelf.svelte';
	import ExplicitIcon from '$lib/components/ExplicitIcon.svelte';
	import * as api from '$lib/api';
	import type { BrowseItem, SearchResults } from '$lib/api';
	import { getCached, putCached } from '$lib/pagecache';
	import { openAddToPlaylist, playSong } from '$lib/player.svelte';
	import { asSong, openItem } from '$lib/browse';
	import { thumb } from '$lib/thumb';

	let query = $state(lastQuery);
	let res = $state<SearchResults | null>(null);
	let searched = $state('');
	let searching = $state(false);
	let error = $state<string | null>(null);

	// The query of the most recent runSearch call, so an older in-flight one can't clobber it.
	let latest = '';

	async function runSearch() {
		if (!query.trim()) return;
		const q = query;
		latest = q;
		lastQuery = q;
		const key = `search:${q}`;
		const hit = getCached<SearchResults>(key);
		if (hit) {
			res = hit;
			searched = q;
			searching = false;
		} else {
			searching = true;
		}
		error = null;
		try {
			const fresh = await api.searchAll(q);
			if (latest !== q) return; // a newer search superseded this one
			res = fresh;
			searched = q;
			putCached(key, fresh);
		} catch (e) {
			if (latest !== q) return;
			if (!hit) error = String(e);
		} finally {
			if (latest === q) searching = false;
		}
	}

	// --- typeahead preview -----------------------------------------------------------------------
	//
	// Same `search_all` the page runs, debounced and cut down to a handful of rows. No separate
	// suggestions endpoint: this fills the very cache `runSearch` reads, so hitting Enter on a
	// previewed query paints instantly instead of searching a second time.

	let suggestOpen = $state(false);
	let suggestions = $state<BrowseItem[]>([]);
	let suggesting = $state(false);
	let active = $state(-1); // keyboard-highlighted row, -1 = none (Enter runs the full search)
	let suggestFor = ''; // query `suggestions` belongs to, so a stale response can't land
	let debounce: ReturnType<typeof setTimeout> | undefined;

	const KIND = { song: 'Song', album: 'Album', artist: 'Artist', playlist: 'Playlist' };

	/** A few rows across the categories rather than six songs: one top hit, then the mix. */
	function preview(r: SearchResults): BrowseItem[] {
		const out: BrowseItem[] = [];
		const seen = new Set<string>();
		const take = (items: BrowseItem[], n: number) => {
			for (const i of items) {
				if (n <= 0) break;
				if (seen.has(i.id)) continue;
				seen.add(i.id);
				out.push(i);
				n--;
			}
		};
		take(r.top, 1);
		take(r.songs, 3);
		take(r.artists, 1);
		take(r.albums, 1);
		take(r.playlists, 1);
		return out;
	}

	async function loadSuggest(q: string) {
		suggestFor = q;
		active = -1;
		const key = `search:${q}`;
		const hit = getCached<SearchResults>(key);
		if (hit) {
			suggestions = preview(hit);
			suggesting = false;
			return;
		}
		suggesting = true;
		try {
			const fresh = await api.searchAll(q);
			if (suggestFor !== q) return;
			putCached(key, fresh);
			suggestions = preview(fresh);
		} catch {
			if (suggestFor === q) suggestions = [];
		} finally {
			if (suggestFor === q) suggesting = false;
		}
	}

	// Reads the element, not `query`: the bound value lands on the same event and the order of the
	// two listeners is not ours to assume.
	function onType(e: Event & { currentTarget: HTMLInputElement }) {
		clearTimeout(debounce);
		const q = e.currentTarget.value.trim();
		if (q.length < 2) {
			closeSuggest();
			return;
		}
		suggestOpen = true;
		if (q !== suggestFor) suggestions = [];
		debounce = setTimeout(() => loadSuggest(q), 300);
	}

	function closeSuggest() {
		clearTimeout(debounce);
		suggestOpen = false;
		suggesting = false;
		active = -1;
	}

	function chooseSuggestion(item: BrowseItem) {
		closeSuggest();
		lastQuery = query;
		openItem(item); // a song plays, everything else opens its page
	}

	function submit() {
		if (active >= 0 && suggestions[active]) {
			chooseSuggestion(suggestions[active]);
			return;
		}
		closeSuggest();
		runSearch();
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape' && suggestOpen) {
			e.preventDefault();
			closeSuggest();
		} else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && suggestions.length) {
			e.preventDefault();
			suggestOpen = true;
			const n = suggestions.length;
			active = e.key === 'ArrowDown' ? (active + 1) % n : (active <= 0 ? n : active) - 1;
		}
	}

	function showMore(cat: 'songs' | 'albums' | 'artists' | 'playlists') {
		goto(`/search-more?${new URLSearchParams({ q: searched, cat }).toString()}`);
	}

	// Run the search when arriving with a ?q= (e.g. from the Home search box). Keyed on the URL
	// alone: typing a new query in the field must not look like a URL change and bounce us back.
	const urlQuery = $derived(page.url.searchParams.get('q') ?? '');
	let lastUrlQuery = '';
	$effect(() => {
		if (urlQuery && urlQuery !== lastUrlQuery) {
			lastUrlQuery = urlQuery;
			query = urlQuery;
			runSearch();
		}
	});

	// Arriving without a ?q= (back from a result, or the sidebar link): rerun whatever was last
	// searched. onMount, not the effect above, so a ?q= arrival still wins.
	onMount(() => {
		if (!urlQuery && query) runSearch();
	});

	// Sections are horizontal card rows, except Songs which is a vertical list. `top` has no "show more".
	const sections = $derived(
		res
			? [
					{ key: 'top', label: 'Top results', items: res.top, max: 4, more: false, list: false },
					{ key: 'songs', label: 'Songs', items: res.songs, max: 6, more: true, list: true },
					{ key: 'albums', label: 'Albums', items: res.albums, max: 5, more: true, list: false },
					{ key: 'artists', label: 'Artists', items: res.artists, max: 3, more: true, list: false },
					{ key: 'playlists', label: 'Playlists', items: res.playlists, max: 5, more: true, list: false }
				].filter((s) => s.items.length)
			: []
	);

</script>

<div class="flex h-full flex-col">
	<div class="border-b p-6">
		<h1 class="mb-4 font-heading text-2xl font-bold">Search</h1>
		<form
			class="flex max-w-xl gap-2"
			onsubmit={(e) => {
				e.preventDefault();
				submit();
			}}
			onfocusout={(e) => {
				// Rows preventDefault on mousedown, so focus never leaves the input while clicking one:
				// anything that gets here is a real move away from the field.
				if (!e.currentTarget.contains(e.relatedTarget as Node | null)) closeSuggest();
			}}
		>
			<div class="relative flex-1">
				<Input
					bind:value={query}
					placeholder="Search songs, albums, artists, playlists…"
					autocomplete="off"
					role="combobox"
					aria-expanded={suggestOpen}
					aria-controls="search-suggest"
					oninput={onType}
					onkeydown={onKeydown}
					onfocus={() => {
						if (suggestions.length && query.trim() === suggestFor) suggestOpen = true;
					}}
				/>
				{#if suggestOpen}
					<div
						id="search-suggest"
						role="listbox"
						aria-label="Search preview"
						class="absolute left-0 right-0 top-full z-50 mt-2 overflow-hidden rounded-xl border bg-popover text-popover-foreground shadow-xl animate-in fade-in-0 zoom-in-95 duration-150"
					>
						{#if suggesting && !suggestions.length}
							{#each Array(4) as _, i (i)}
								<div class="flex items-center gap-3 px-3 py-2">
									<Skeleton class="h-10 w-10 shrink-0 rounded-md" />
									<div class="min-w-0 flex-1">
										<Skeleton class="h-3 w-40 rounded" />
										<Skeleton class="mt-2 h-2.5 w-24 rounded" />
									</div>
								</div>
							{/each}
						{:else if !suggestions.length}
							<div class="px-4 py-3 text-sm text-muted-foreground">Nothing quick for that.</div>
						{:else}
							{#each suggestions as item, i (item.id)}
								{@const hero = i === 0}
								<button
									type="button"
									role="option"
									aria-selected={i === active}
									class="flex w-full cursor-pointer items-center gap-3 px-3 text-left transition-colors {i ===
									active
										? 'bg-accent/60'
										: 'hover:bg-accent/40'} {hero ? 'border-b py-2.5' : 'py-1.5'}"
									onmousedown={(e) => e.preventDefault()}
									onmouseenter={() => (active = i)}
									onclick={() => chooseSuggestion(item)}
								>
									{#if item.thumbnail}
										<!-- 400, the same size the cards below ask for: the CDN doesn't serve every rewritten
										     size, that one is verified, and the row lands on an image the grid already fetched. -->
										<img
											src={thumb(item.thumbnail, 400)}
											alt=""
											class="shrink-0 object-cover {item.kind === 'artist'
												? 'rounded-full'
												: 'rounded-md'} {hero ? 'h-12 w-12' : 'h-10 w-10'}"
										/>
									{:else}
										<div
											class="flex shrink-0 items-center justify-center bg-muted text-muted-foreground/50 {item.kind ===
											'artist'
												? 'rounded-full'
												: 'rounded-md'} {hero ? 'h-12 w-12' : 'h-10 w-10'}"
										>
											<HugeiconsIcon
												icon={item.kind === 'artist' ? UserIcon : MusicNote01Icon}
												class="h-5 w-5"
											/>
										</div>
									{/if}
									<div class="min-w-0 flex-1">
										<div class="truncate {hero ? 'font-semibold' : 'text-sm'}">{item.title}</div>
										<div class="flex items-center gap-1 text-xs text-muted-foreground">
											{#if item.explicit}
												<ExplicitIcon class="h-3 w-3 shrink-0" />
											{/if}
											<span class="truncate">
												{KIND[item.kind]}{item.subtitle ? ` • ${item.subtitle}` : ''}
											</span>
										</div>
									</div>
									{#if hero}
										<span
											class="shrink-0 rounded-full bg-primary/10 px-2 py-0.5 text-[0.625rem] font-semibold uppercase tracking-wide text-primary"
										>
											Top result
										</span>
									{/if}
								</button>
							{/each}
						{/if}
						<button
							type="button"
							class="flex w-full cursor-pointer items-center gap-2 border-t bg-muted/30 px-3 py-2 text-left text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
							onmousedown={(e) => e.preventDefault()}
							onmouseenter={() => (active = -1)}
							onclick={() => {
								closeSuggest();
								runSearch();
							}}
						>
							<HugeiconsIcon icon={Search01Icon} class="h-3.5 w-3.5" />
							All results for “{query.trim()}”
						</button>
					</div>
				{/if}
			</div>
			<Button type="submit" class="gap-2" disabled={searching}>
				<HugeiconsIcon icon={Search01Icon} class="h-4 w-4" />
				{searching ? 'Searching…' : 'Search'}
			</Button>
		</form>
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
			<p class="text-sm text-muted-foreground">Search for a song, album, artist, or playlist.</p>
		{:else if !sections.length}
			<p class="text-sm text-muted-foreground">No results for “{searched}”.</p>
		{:else}
			<div class="content-in flex flex-col gap-10">
				{#each sections as sec (sec.key)}
					<section>
						<div class="mb-3 flex items-center justify-between">
							<h2 class="font-heading text-xl font-bold">{sec.label}</h2>
							{#if sec.more}
								<button
									class="cursor-pointer text-xs font-semibold uppercase text-muted-foreground hover:text-foreground"
									onclick={() => showMore(sec.key as 'songs' | 'albums' | 'artists' | 'playlists')}
								>
									Show more
								</button>
							{/if}
						</div>
						{#if sec.list}
							{#each sec.items.slice(0, sec.max) as item (item.id)}
								{@const song = asSong(item)}
								<TrackRow {song} onplay={() => playSong(song)} onAdd={() => openAddToPlaylist(song)} />
							{/each}
						{:else}
							<Shelf items={sec.items.slice(0, sec.max)} />
						{/if}
					</section>
				{/each}
			</div>
		{/if}
	</div>
</div>
