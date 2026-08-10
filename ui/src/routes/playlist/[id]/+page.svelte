<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		PlayIcon,
		ShuffleIcon,
		PencilEdit02Icon,
		Delete02Icon,
		MoreVerticalIcon,
		Tick02Icon,
		Cancel01Icon,
		Radio02Icon,
		ArrowUpNarrowWideIcon,
		ArrowDownWideNarrowIcon,
		DashboardSquare02Icon,
		ListRestartIcon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import TrackRow from '$lib/components/TrackRow.svelte';
	import TrackRowSkeleton from '$lib/components/TrackRowSkeleton.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import * as api from '$lib/api';
	import { ON_REPEAT_ID } from '$lib/api';
	import type { BrowseItem, PlaylistPage, SongItem } from '$lib/api';
	import { getCached, putCached, invalidateCached } from '$lib/pagecache';
	import { rowWindow } from '$lib/rows';
	import {
		addPick,
		enqueue,
		playback,
		openAddToPlaylist,
		playFrom,
		startRadio,
		toast,
		bumpLibraryTrackCount,
		lastPlaylistAdd
	} from '$lib/player.svelte';

	let pl = $state<PlaylistPage | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let loadingMore = $state(false);
	let moreError = $state(false);
	let inflight: Promise<void> | null = null;
	let confirmingDelete = $state(false);
	// A random song's cover, used as a blurred hero backdrop (like the artist/album pages).
	let bgImage = $state<string | null>(null);

	// ⋯ options menu, positioned `fixed` at the button so it isn't clipped (matches TrackRow).
	let menuOpen = $state(false);
	let mx = $state(0);
	let my = $state(0);

	// Inline rename state.
	let editingName = $state(false);
	let nameDraft = $state('');

	const id = $derived(page.params.id ?? '');
	const nowId = $derived(playback.now?.videoId);
	// The liked-music auto-playlist isn't a user playlist — no rename/delete, but shuffle is fine.
	const isLiked = $derived(id === 'VLLM');
	// On Repeat is built locally from play counts: no artwork, and no radio to seed autoplay from.
	const isOnRepeat = $derived(id === ON_REPEAT_ID);
	// Only offer rename/delete on playlists the signed-in user actually owns (backend `owned` flag).
	// Liked Music reports owned but can't be renamed/deleted, so exclude it explicitly.
	const editable = $derived((pl?.owned ?? false) && !isLiked);

	async function load(pid: string) {
		const key = `playlist:${pid}`;
		const hit = getCached<PlaylistPage>(key);
		confirmingDelete = false;
		editingName = false;
		if (hit) {
			pl = hit;
			bgImage = pickCover(hit.items);
			loading = false;
		} else {
			loading = true;
			pl = null;
			bgImage = null;
		}
		error = null;
		try {
			const fresh = await api.getPlaylist(pid);
			if (pid !== id) return; // superseded by navigation — drop the stale response
			pl = fresh;
			bgImage = pickCover(fresh.items);
			putCached(key, fresh);
		} catch (e) {
			if (pid !== id) return;
			if (!hit) error = String(e);
		} finally {
			if (pid === id) loading = false;
		}
	}

	// Reload whenever the route param changes (playlist → playlist navigation).
	$effect(() => {
		if (id) load(id);
	});

	// Songs added to THIS playlist via the picker (e.g. from the queue) appear immediately.
	// Epoch-guarded so an add is applied once; adds to other playlists are just marked seen.
	let seenAddEpoch = lastPlaylistAdd.epoch;
	$effect(() => {
		if (lastPlaylistAdd.epoch === seenAddEpoch) return;
		seenAddEpoch = lastPlaylistAdd.epoch;
		if (!pl || lastPlaylistAdd.playlistId !== id) return;
		pl = { ...pl, items: [...pl.items, ...lastPlaylistAdd.songs] };
		cacheCurrent();
		fillSetVideoIds();
	});

	// Optimistic rows lack set_video_id, so "Remove from playlist" is hidden on them. Refetch and
	// patch the real ids into place (merge, not replace — keeps loadMore pages and any row YouTube
	// hasn't reflected yet). Retries because the add is eventually-consistent on YouTube's side.
	async function fillSetVideoIds() {
		if (isLiked) return;
		const pid = id;
		for (const delay of [0, 2000, 4000]) {
			if (delay) await new Promise((r) => setTimeout(r, delay));
			if (pid !== id || !pl) return;
			try {
				const fresh = await api.getPlaylist(pid);
				if (pid !== id || !pl) return;
				const used = new Set(pl.items.map((t) => t.set_video_id).filter(Boolean));
				pl = {
					...pl,
					subtitle: fresh.subtitle, // header track count catches up too
					items: pl.items.map((t) => {
						if (t.set_video_id) return t;
						const match = fresh.items.find(
							(f) => f.video_id === t.video_id && f.set_video_id && !used.has(f.set_video_id)
						);
						if (!match) return t;
						used.add(match.set_video_id);
						return { ...t, set_video_id: match.set_video_id };
					})
				};
				cacheCurrent();
				if (pl.items.every((t) => t.set_video_id)) return;
			} catch {
				/* retry on the next pass */
			}
		}
	}

	// Keep the page cache in step with optimistic mutations so a revisit within the TTL never
	// resurrects pre-mutation data (the optimistic-UI contract). context: plans/007.
	function cacheCurrent() {
		if (pl) putCached(`playlist:${id}`, pl);
	}

	// One page at a time, shared: the scroll sentinel and the "load the rest before playing" walk
	// both go through here, so they can never fire overlapping requests for the same token.
	function loadMore(): Promise<void> {
		inflight ??= fetchPage().finally(() => (inflight = null));
		return inflight;
	}

	async function fetchPage() {
		const token = pl?.continuation;
		if (!token) return;
		loadingMore = true;
		moreError = false;
		try {
			const more = await api.getPlaylistMore(token);
			if (pl?.continuation !== token) return; // stale (navigated or mutated mid-flight)
			pl = {
				...pl,
				items: [...pl.items, ...more.items],
				// An empty page would leave the sentinel in view with nothing to show — that's the end.
				continuation: more.items.length ? more.continuation : undefined
			};
			cacheCurrent();
		} catch {
			// Stop auto-loading and offer a retry — auto-retrying a visible sentinel would spin.
			moreError = true;
		} finally {
			loadingMore = false;
		}
	}

	// Only the rows around the viewport are rendered; the rest are two padded boxes (`rows.ts`).
	// A Liked Songs list runs to five figures, and `content-visibility` spares the layout and the
	// paint but not the DOM node, the style or the component.
	let scrollTop = $state(0);
	let viewportPx = $state(0);
	const win = $derived(rowWindow(scrollTop, viewportPx, pl?.items.length ?? 0));

	function scroller(node: HTMLElement) {
		const read = () => {
			scrollTop = node.scrollTop;
			viewportPx = node.clientHeight;
		};
		read();
		node.addEventListener('scroll', read, { passive: true });
		// The container's own box changes on a window resize, never on a scroll, so this is cheap.
		const ro = new ResizeObserver(read);
		ro.observe(node);
		return () => {
			node.removeEventListener('scroll', read);
			ro.disconnect();
		};
	}

	// One page per approach to the bottom: the observer only fires when the sentinel *enters* view,
	// so an appended page that pushes it back out is required before the next fetch. rootMargin
	// starts the fetch early enough that the rows are usually there by the time you reach them.
	function sentinel(node: HTMLElement) {
		const io = new IntersectionObserver(([e]) => e.isIntersecting && loadMore(), {
			rootMargin: '600px 0px'
		});
		io.observe(node);
		return () => io.disconnect();
	}

	// This playlist as a card, for the sidebar's last-played sort and the Shortcuts grid.
	const asItem = (): BrowseItem => ({
		kind: 'playlist',
		id,
		title: pl?.title ?? 'Playlist',
		subtitle: pl?.subtitle,
		// On Repeat stays artwork-free wherever it's rendered (shortcuts, recents) so it always
		// draws its icon rather than one of its songs' covers.
		thumbnail: isOnRepeat ? undefined : (pl?.thumbnail ?? bgImage ?? undefined)
	});

	// `sourceId` points autoplay at that playlist's radio. On Repeat has no YouTube id, so pass
	// none and let autoplay seed off the last video instead. The queue is the whole playlist, not
	// the pages scrolled so far, but waiting for it here is what made long playlists take forever
	// to start: YouTube hands out tracks 100 at a time and the tokens are chained, so the backend
	// takes the token and walks the rest into the queue while page 1 is already playing.
	function playAll(start: number | null) {
		if (!pl) return;
		playFrom(asItem(), pl.items, start, isOnRepeat ? undefined : id, undefined, pl.continuation);
	}

	// Random cover from the songs, picked once per load so it stays stable while browsing
	// (loadMore appends tracks without changing it).
	function pickCover(items: SongItem[]): string | null {
		const withThumb = items.filter((t) => t.thumbnail);
		if (!withThumb.length) return null;
		const url = withThumb[Math.floor(Math.random() * withThumb.length)].thumbnail!;
		return hiRes(url);
	}

	// List thumbnails come at a small size; YouTube/Google encode the size in the URL, so bump it
	// for a crisp full-width backdrop.
	function hiRes(url: string): string {
		return url.replace(/=w\d+-h\d+/, '=w1200-h1200').replace(/=s\d+/, '=s1200');
	}

	// Same deal as `playAll` for a long playlist: the loaded pages go in now and the token hands
	// the rest to the backend to walk in behind them.
	function queue(next: boolean) {
		if (!pl?.items.length) return;
		enqueue(pl.items, next, pl.title, pl.continuation);
	}

	function shufflePlay() {
		if (!pl?.items.length) return;
		// Real order + shuffle flag — the backend owns shuffling, so the shuffle toggle can
		// restore the true playlist order and every re-shuffle is fresh. It also mixes each page
		// it walks into the unplayed tail, so this stays a shuffle of the whole playlist rather
		// than of the pages that happen to be loaded.
		playFrom(asItem(), pl.items, null, isOnRepeat ? undefined : id, true, pl.continuation);
	}

	function openMenu(e: MouseEvent) {
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
		mx = r.left;
		my = r.bottom + 4;
		menuOpen = true;
	}
	function run(action: () => void) {
		menuOpen = false;
		action();
	}

	function startRename() {
		nameDraft = pl?.title ?? '';
		editingName = true;
	}

	async function saveRename() {
		const name = nameDraft.trim();
		if (!pl || !name || name === pl.title) {
			editingName = false;
			return;
		}
		const prev = pl.title;
		pl = { ...pl, title: name }; // optimistic
		editingName = false;
		try {
			await api.renamePlaylist(id, name);
			cacheCurrent();
			toast.success('Playlist renamed');
		} catch (e) {
			pl = { ...pl, title: prev }; // revert
			cacheCurrent();
			toast.error(String(e));
		}
	}

	// The liked-music auto-playlist can't be edited like a normal one — removing = un-liking.
	async function removeTrack(track: SongItem) {
		if (!pl) return;
		if (!isLiked && !track.set_video_id) return;
		const prev = pl.items;
		// Reassign `pl` (not mutate `pl.items`) so the list re-renders immediately. Match by the
		// per-instance setVideoId on normal playlists (duplicates), by videoId on liked music.
		const kept = pl.items.filter((t) =>
			isLiked ? t.video_id !== track.video_id : t.set_video_id !== track.set_video_id
		);
		pl = { ...pl, items: kept };
		try {
			if (isLiked) {
				await api.like(track.video_id, false);
				toast.success('Removed from Liked Music');
			} else {
				await api.removeFromPlaylist(id, track.video_id, track.set_video_id!);
				bumpLibraryTrackCount(id, -1);
				toast.success('Removed from playlist');
			}
			cacheCurrent();
		} catch (e) {
			pl = { ...pl, items: prev }; // revert
			cacheCurrent();
			toast.error(String(e));
		}
	}

	async function deleteThisPlaylist() {
		try {
			await api.deletePlaylist(id);
			invalidateCached(`playlist:${id}`);
			toast.success('Playlist deleted');
			goto('/library');
		} catch (e) {
			toast.error(String(e));
			confirmingDelete = false;
		}
	}

	function autofocus(node: HTMLInputElement) {
		node.focus();
		node.select();
	}
</script>

<div class="flex h-full flex-col">
	{#if loading}
		<div class="flex items-end gap-6 border-b p-6">
			<Skeleton class="h-40 w-40 shrink-0 rounded-xl" />
			<div class="flex-1 space-y-3">
				<Skeleton class="h-3 w-16 rounded" />
				<Skeleton class="h-10 w-2/3 rounded-lg" />
				<Skeleton class="h-4 w-40 rounded" />
				<Skeleton class="h-9 w-24 rounded-4xl" />
			</div>
		</div>
		<div class="p-4">
			{#each Array(8) as _, i (i)}
				<TrackRowSkeleton />
			{/each}
		</div>
	{:else if error}
		<div class="p-6"><ErrorState message={error} onRetry={() => load(id)} /></div>
	{:else if pl}
		<div class="content-in relative flex min-h-[38vh] items-end gap-6 overflow-hidden border-b p-6">
			{#if bgImage}
				<img
					src={bgImage}
					alt=""
					class="pointer-events-none absolute inset-0 h-full w-full object-cover object-center"
				/>
			{/if}
			<!-- Fade the cover into the page so the text stays readable: solid at the bottom and on the
			     left (behind the title), the image itself visible toward the top-right. -->
			<div
				class="absolute inset-0 bg-gradient-to-t from-background via-background/60 to-background/20"
			></div>
			<div class="absolute inset-0 bg-gradient-to-r from-background via-background/50 to-transparent"></div>
			{#if isOnRepeat}
				<div
					class="relative flex h-40 w-40 items-center justify-center rounded-xl bg-primary/10 text-primary shadow-lg"
				>
					<HugeiconsIcon icon={ListRestartIcon} class="h-20 w-20" />
				</div>
			{:else if pl.thumbnail}
				<img
					src={pl.thumbnail}
					alt=""
					class="relative h-40 w-40 rounded-xl object-cover shadow-lg"
				/>
			{:else}
				<div class="relative h-40 w-40 rounded-xl bg-muted"></div>
			{/if}
			<div class="relative min-w-0 flex-1">
				<div class="text-xs font-medium uppercase text-muted-foreground">Playlist</div>
				{#if editingName}
					<div class="mt-1 flex items-center gap-2">
						<input
							use:autofocus
							bind:value={nameDraft}
							onkeydown={(e) => {
								if (e.key === 'Enter') saveRename();
								else if (e.key === 'Escape') (editingName = false);
							}}
							class="min-w-0 flex-1 rounded-md border bg-background px-2 py-1 font-heading text-3xl font-bold outline-none focus:border-accent"
							aria-label="Playlist name"
						/>
						<Button size="icon" aria-label="Save name" onclick={saveRename}>
							<HugeiconsIcon icon={Tick02Icon} class="h-5 w-5" />
						</Button>
						<Button
							variant="ghost"
							size="icon"
							aria-label="Cancel rename"
							onclick={() => (editingName = false)}
						>
							<HugeiconsIcon icon={Cancel01Icon} class="h-5 w-5 text-muted-foreground" />
						</Button>
					</div>
				{:else}
					<h1 class="mt-1 font-heading text-4xl font-bold tracking-tight drop-shadow-lg">
					{pl.title ?? 'Playlist'}
				</h1>
				{/if}
				{#if pl.subtitle}<p class="mt-2 text-sm text-muted-foreground">{pl.subtitle}</p>{/if}
				<div class="mt-4 flex items-center gap-2">
					<Button class="gap-2" onclick={() => playAll(null)} disabled={!pl.items.length}>
						<HugeiconsIcon icon={PlayIcon} class="h-4 w-4" />
						Play
					</Button>
					{#if confirmingDelete}
						<div class="flex items-center gap-2 rounded-lg border border-destructive/40 px-2 py-1">
							<span class="text-xs text-muted-foreground">Delete this playlist?</span>
							<Button variant="destructive" size="sm" onclick={deleteThisPlaylist}>Delete</Button>
							<Button variant="ghost" size="sm" onclick={() => (confirmingDelete = false)}>
								Cancel
							</Button>
						</div>
					{:else}
						<Button
							variant="ghost"
							size="icon"
							aria-label="Playlist options"
							onclick={openMenu}
						>
							<HugeiconsIcon icon={MoreVerticalIcon} class="h-5 w-5 text-muted-foreground" />
						</Button>
					{/if}
				</div>
			</div>
		</div>
		<div class="content-in min-h-0 flex-1 overflow-y-auto p-4" {@attach scroller}>
			{#if pl.items.length}
				<!-- The padding stands in for the rows outside the window, so the scrollbar is the
				     length of the whole playlist even though only ~30 rows exist. -->
				<div style="padding-top:{win.padTop}px;padding-bottom:{win.padBottom}px">
					{#each pl.items.slice(win.start, win.end) as item, i (item.video_id + (win.start + i))}
						{@const n = win.start + i}
						<TrackRow
							song={item}
							index={n}
							active={item.video_id === nowId}
							onplay={() => playAll(n)}
							onAdd={() => openAddToPlaylist(item)}
							onRemove={isLiked || (editable && item.set_video_id)
								? () => removeTrack(item)
								: undefined}
						/>
					{/each}
				</div>
			{:else}
				<p class="p-4 text-sm text-muted-foreground">This playlist is empty.</p>
			{/if}
			{#if pl.continuation}
				{#if moreError}
					<div class="p-3 text-center">
						<Button variant="outline" size="sm" onclick={loadMore} disabled={loadingMore}>
							{loadingMore ? 'Loading…' : 'Try again'}
						</Button>
					</div>
				{:else}
					<!-- The sentinel sits above the skeletons: it triggers the next page as it scrolls
					     into range, so the rest of a long playlist arrives without a button. -->
					<div aria-busy={loadingMore}>
						<div {@attach sentinel}></div>
						{#if loadingMore}
							{#each Array(4) as _, i (i)}
								<TrackRowSkeleton />
							{/each}
						{/if}
					</div>
				{/if}
			{/if}
		</div>
	{/if}
</div>

{#if menuOpen}
	<button
		class="fixed inset-0 z-40 cursor-default"
		onclick={() => (menuOpen = false)}
		aria-label="Close menu"
	></button>
	<div
		class="fixed z-50 min-w-52 origin-top-left animate-in rounded-lg border bg-popover p-1 text-popover-foreground shadow-xl duration-150 fade-in-0 zoom-in-95"
		style="left:{mx}px; top:{my}px;"
	>
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={() => run(shufflePlay)}
			disabled={!pl?.items.length}
		>
			<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4" /> Shuffle play
		</button>
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={() => run(() => queue(true))}
			disabled={!pl?.items.length}
		>
			<HugeiconsIcon icon={ArrowUpNarrowWideIcon} class="h-4 w-4" /> Play next
		</button>
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={() => run(() => queue(false))}
			disabled={!pl?.items.length}
		>
			<HugeiconsIcon icon={ArrowDownWideNarrowIcon} class="h-4 w-4" /> Add to queue
		</button>
		<!-- On Repeat is built from local play counts — there is no YouTube playlist to seed a
		     radio from. -->
		{#if !isOnRepeat}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={() => run(() => startRadio('playlist', id, pl?.title))}
			>
				<HugeiconsIcon icon={Radio02Icon} class="h-4 w-4" /> Start radio
			</button>
		{/if}
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={() => run(() => addPick(asItem()))}
		>
			<HugeiconsIcon icon={DashboardSquare02Icon} class="h-4 w-4" /> Add to shortcuts
		</button>
		{#if editable}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={() => run(startRename)}
			>
				<HugeiconsIcon icon={PencilEdit02Icon} class="h-4 w-4" /> Edit name
			</button>
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-destructive hover:bg-destructive/10"
				onclick={() => run(() => (confirmingDelete = true))}
			>
				<HugeiconsIcon icon={Delete02Icon} class="h-4 w-4" /> Delete playlist
			</button>
		{/if}
	</div>
{/if}
