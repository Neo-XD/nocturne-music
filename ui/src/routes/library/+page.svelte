<script module lang="ts">
	// Module scope, so returning to the library (back from an album you opened, or via the sidebar)
	// keeps the tab you were on instead of snapping to All.
	let lastTab = 'all';
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Add01Icon,
		CloudSyncIcon,
		CloudUploadIcon,
		DriveIcon,
		MusicNote01Icon,
		MusicNoteSquare02Icon,
		Playlist02Icon,
		SquareStackIcon,
		UserSharingIcon,
		Folder01Icon,
		FolderAddIcon,
		FolderOpenIcon,
		ArrowLeft01Icon,
		ArrowRight01Icon,
		Download01Icon,
		Edit02Icon,
		Delete02Icon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Tabs from '$lib/components/ui/tabs';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import LibrarySongs from '$lib/components/LibrarySongs.svelte';
	import LocalMusic from '$lib/components/LocalMusic.svelte';
	import DownloadedMusic from '$lib/components/DownloadedMusic.svelte';
	import MediaCard from '$lib/components/MediaCard.svelte';
	import MediaCardSkeleton from '$lib/components/MediaCardSkeleton.svelte';
	import FolderCard from '$lib/components/FolderCard.svelte';
	import CreatePlaylistDialog from '$lib/components/CreatePlaylistDialog.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import type { BrowseItem } from '$lib/api';
	import {
		auth,
		personal,
		toast,
		library,
		loadLibrary,
		loadLibraryExtras,
		createLibraryPlaylist,
		syncSavedToYouTube,
		createPlaylistFolder,
		renamePlaylistFolder,
		deletePlaylistFolder,
		movePlaylistToFolder,
		moveFolderToFolder,
		spotify,
		loadSpotifyPlaylists
	} from '$lib/player.svelte';
	import { PLAYLIST_DND_MIME, FOLDER_DND_MIME } from '$lib/dnd';
	import { mergeSaved, unsynced } from '$lib/personal';
	import { reveal } from '$lib/reveal.svelte';

	let dialogOpen = $state(false);
	let dialogInitialPlatform = $state<'ytm' | 'spotify' | undefined>(undefined);
	let songsPlatform = $state<'ytm' | 'spotify'>('ytm');
	$effect(() => {
		if (!auth.account?.signedIn && spotify.status.linked) {
			songsPlatform = 'spotify';
		}
	});
	// `?tab=local` so anything that sends you back here (an album whose files were deleted) lands
	// on the tab you came from instead of a sign-in prompt.
	let tab = $state(page.url.searchParams.get('tab') ?? lastTab);
	$effect(() => {
		lastTab = tab;
	});

	// Everything here lives in the shared `library` store, so a revisit renders the cached grid
	// immediately and the forced refresh below swaps in fresh data behind it. What was saved on this
	// machine merges in per tab (`mergeSaved`), which is the whole library when signed out.
	const playlists = $derived(mergeSaved(personal, library.items, 'playlist'));
	const albums = $derived(mergeSaved(personal, library.albums, 'album'));
	const artists = $derived(mergeSaved(personal, library.artists, 'artist'));
	const all = $derived([...playlists, ...albums, ...artists]);
	// One per tab rather than one shared instance reset on switch: an `$effect` reset lands
	// after the render it is meant to govern, so switching tabs would build the new tab's grid
	// against the old tab's count and immediately tear the excess back down. A tab keeping its
	// own depth also means coming back to one lands where you left it.
	const rvAll = reveal();
	const rvPlaylists = reveal();
	const rvAlbums = reveal();
	const rvArtists = reveal();
	const rvSpotify = reveal();

	const spotifyPlaylistItems = $derived<BrowseItem[]>([
		...(spotify.status.linked
			? [
					{
						kind: 'playlist' as const,
						id: 'sp_liked',
						title: 'Liked Songs',
						subtitle: 'Spotify Liked Songs',
						thumbnail: undefined
					}
			  ]
			: []),
		...spotify.playlists.map((pl) => ({
			kind: 'playlist' as const,
			id: `sp_${pl.id}`,
			title: pl.title,
			subtitle: pl.subtitle || 'Spotify Playlist',
			thumbnail: pl.thumbnail ?? undefined
		}))
	]);
	const loading = $derived((library.loading || library.extrasLoading) && !all.length);
	const error = $derived(library.error ?? library.extrasError);
	// Only the empty states differ: signed out there is no account library to be missing yet.
	const signedOut = $derived(!auth.account?.signedIn);
	// What the sync button has left to push. Synced rows stay in the local library (they are what
	// the user still has after signing out), so counting all of `personal.saved` would nag forever.
	const toSync = $derived(unsynced(personal));

	onMount(load);

	function load() {
		loadLibrary(true);
		loadLibraryExtras(true);
	}

	let syncing = $state(false);
	async function sync() {
		if (syncing) return;
		syncing = true;
		const n = toSync.length;
		try {
			const { synced, failed } = await syncSavedToYouTube();
			if (failed && synced) toast(`Synced ${synced} of ${n}. ${failed} failed, still saved here.`);
			else if (failed) toast.error(`Nothing synced. ${failed} failed, still saved here.`);
			else toast.success(`Synced ${synced} to YouTube Music`);
		} catch (e) {
			toast.error(String(e));
		} finally {
			syncing = false;
		}
	}

	// Playlist folders state & actions
	let activeFolderId = $state<string | null>(null);
	let folderDialogOpen = $state(false);
	let newFolderName = $state('');
	let newFolderParentId = $state<string | null>(null);
	let renameDialogOpen = $state(false);
	let renameFolderId = $state<string | null>(null);
	let renameFolderName = $state('');
	let deleteDialogOpen = $state(false);
	let deleteFolderId = $state<string | null>(null);
	let dragOverBack = $state(false);

	let tabListEl: HTMLElement | null = null;
	let canScrollLeft = $state(false);
	let canScrollRight = $state(false);

	function updateScrollState() {
		if (!tabListEl) return;
		canScrollLeft = tabListEl.scrollLeft > 4;
		canScrollRight = tabListEl.scrollLeft + tabListEl.clientWidth < tabListEl.scrollWidth - 4;
	}

	function scrollTabs(delta: number) {
		if (!tabListEl) return;
		tabListEl.scrollBy({ left: delta, behavior: 'smooth' });
	}

	function trackTabScroll(node: HTMLElement) {
		tabListEl = node;
		updateScrollState();
		const onScroll = () => updateScrollState();
		const onResize = () => updateScrollState();
		node.addEventListener('scroll', onScroll, { passive: true });
		window.addEventListener('resize', onResize);
		const ro = new ResizeObserver(() => updateScrollState());
		ro.observe(node);

		return () => {
			node.removeEventListener('scroll', onScroll);
			window.removeEventListener('resize', onResize);
			ro.disconnect();
			if (tabListEl === node) tabListEl = null;
		};
	}

	const folders = $derived(personal.folders ?? []);
	const rootFolders = $derived(folders.filter((f) => !f.parentId));
	const activeFolder = $derived(folders.find((f) => f.id === activeFolderId));
	const parentFolderOfActive = $derived(
		activeFolder?.parentId ? folders.find((f) => f.id === activeFolder.parentId) : null
	);
	const childFoldersInActive = $derived(
		activeFolder ? folders.filter((f) => f.parentId === activeFolder.id) : []
	);
	const playlistsInActiveFolder = $derived(
		activeFolder ? playlists.filter((p) => activeFolder.playlistIds.includes(p.id)) : []
	);
	const unfiledPlaylists = $derived(
		folders.length > 0
			? playlists.filter((p) => !folders.some((f) => f.playlistIds.includes(p.id)))
			: playlists
	);

	function createFolder() {
		const name = newFolderName.trim();
		if (!name) return;
		const f = createPlaylistFolder(name, newFolderParentId);
		toast.success(`Created folder "${f.name}"`);
		newFolderName = '';
		newFolderParentId = null;
		folderDialogOpen = false;
	}

	function openRename(id: string, currentName: string) {
		renameFolderId = id;
		renameFolderName = currentName;
		renameDialogOpen = true;
	}

	function handleRename() {
		if (!renameFolderId) return;
		const name = renameFolderName.trim();
		if (!name) return;
		renamePlaylistFolder(renameFolderId, name);
		toast.success('Folder renamed');
		renameDialogOpen = false;
	}

	function promptDelete(id: string) {
		deleteFolderId = id;
		deleteDialogOpen = true;
	}

	function confirmDelete() {
		if (!deleteFolderId) return;
		if (activeFolderId === deleteFolderId) {
			activeFolderId = parentFolderOfActive?.id ?? null;
		}
		deletePlaylistFolder(deleteFolderId);
		toast.success('Folder deleted');
		deleteDialogOpen = false;
	}
</script>

{#snippet grid(items: BrowseItem[], empty: string, rv: ReturnType<typeof reveal>)}
	{#if items.length}
		<div class="card-grid content-in">
			{#each items.slice(0, rv.count(items.length)) as item (item.kind + item.id)}
				<MediaCard {item} />
			{/each}
		</div>
		<!-- Outside the grid, or it would be laid out as a cell. -->
		{#if rv.more(items.length)}<div {@attach rv.sentinel}></div>{/if}
	{:else}
		<p class="text-sm text-muted-foreground">{empty}</p>
	{/if}
{/snippet}

<div class="p-6">
	<div class="mb-6 flex items-center justify-between">
		<h1 class="font-heading text-2xl font-bold">Library</h1>
		{#if auth.account?.signedIn || spotify.status.linked}
			<div class="flex items-center gap-2">
				{#if auth.account?.signedIn && toSync.length}
					<Tooltip.Provider delayDuration={150}>
						<Tooltip.Root>
							<Tooltip.Trigger>
								{#snippet child({ props })}
									<Button
										{...props}
										variant="outline"
										size="icon-sm"
										onclick={sync}
										disabled={syncing}
										aria-label="Sync {toSync.length} saved items to YouTube Music"
									>
										<span class="relative">
											<HugeiconsIcon
												icon={CloudSyncIcon}
												class="h-4 w-4 {syncing ? 'animate-pulse' : ''}"
											/>
											<!-- ring-background so the count reads over the icon's stroke (as in
											     Titlebar). -->
											<span
												class="absolute -right-2 -top-1.5 min-w-3.5 rounded-full bg-accent px-[3px] text-[9px] font-semibold leading-[0.875rem] text-accent-foreground ring-[1.5px] ring-background"
											>
												{toSync.length}
											</span>
										</span>
									</Button>
								{/snippet}
							</Tooltip.Trigger>
							<Tooltip.Content side="bottom">
								{syncing
									? 'Adding them to YouTube Music…'
									: `Add the ${toSync.length} saved on this device to your YouTube Music library`}
							</Tooltip.Content>
						</Tooltip.Root>
					</Tooltip.Provider>
				{/if}
				<Button variant="outline" size="sm" class="gap-2" onclick={() => (folderDialogOpen = true)}>
					<HugeiconsIcon icon={FolderAddIcon} class="h-4 w-4" /> New folder
				</Button>
				<Button
					variant="outline"
					size="sm"
					class="gap-2 cursor-pointer"
					onclick={() => {
						dialogInitialPlatform = !auth.account?.signedIn && spotify.status.linked ? 'spotify' : 'ytm';
						dialogOpen = true;
					}}
				>
					<HugeiconsIcon icon={Add01Icon} class="h-4 w-4" /> New playlist
				</Button>
			</div>
		{:else}
			<div class="flex items-center gap-2">
				<Button variant="outline" size="sm" class="gap-2" onclick={() => (folderDialogOpen = true)}>
					<HugeiconsIcon icon={FolderAddIcon} class="h-4 w-4" /> New folder
				</Button>
			</div>
		{/if}
	</div>

	<!-- Create Playlist Dialog with Picture, Description & Privacy -->
	<CreatePlaylistDialog bind:open={dialogOpen} initialPlatform={dialogInitialPlatform} />

	<!-- Create Folder Dialog -->
	<Dialog.Root bind:open={folderDialogOpen}>
		<Dialog.Content class="sm:max-w-md">
			<Dialog.Header>
				<Dialog.Title>New folder</Dialog.Title>
				<Dialog.Description>Create a folder to group and organize your playlists.</Dialog.Description>
			</Dialog.Header>
			<form
				class="flex flex-col gap-4"
				onsubmit={(e) => {
					e.preventDefault();
					createFolder();
				}}
			>
				<Input bind:value={newFolderName} placeholder="Folder name (e.g. Chill, Workout, Classics)" autofocus />
				<Dialog.Footer>
					<Button type="button" variant="outline" onclick={() => (folderDialogOpen = false)}>
						Cancel
					</Button>
					<Button type="submit" disabled={!newFolderName.trim()}>
						Create folder
					</Button>
				</Dialog.Footer>
			</form>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Rename Folder Dialog -->
	<Dialog.Root bind:open={renameDialogOpen}>
		<Dialog.Content class="sm:max-w-md">
			<Dialog.Header>
				<Dialog.Title>Rename folder</Dialog.Title>
				<Dialog.Description>Enter a new name for this folder.</Dialog.Description>
			</Dialog.Header>
			<form
				class="flex flex-col gap-4"
				onsubmit={(e) => {
					e.preventDefault();
					handleRename();
				}}
			>
				<Input bind:value={renameFolderName} placeholder="Folder name" autofocus />
				<Dialog.Footer>
					<Button type="button" variant="outline" onclick={() => (renameDialogOpen = false)}>
						Cancel
					</Button>
					<Button type="submit" disabled={!renameFolderName.trim()}>
						Save
					</Button>
				</Dialog.Footer>
			</form>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Delete Folder Confirmation Dialog -->
	<Dialog.Root bind:open={deleteDialogOpen}>
		<Dialog.Content class="sm:max-w-md">
			<Dialog.Header>
				<Dialog.Title>Delete folder?</Dialog.Title>
				<Dialog.Description>
					Deleting this folder will not delete your playlists. They will remain in your library as unfiled playlists.
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button type="button" variant="outline" onclick={() => (deleteDialogOpen = false)}>
					Cancel
				</Button>
				<Button type="button" variant="destructive" onclick={confirmDelete}>
					Delete
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>

	<!-- The tabs always render: Local music needs neither an account nor a connection. -->
	<Tabs.Root bind:value={tab}>
		<div class="sticky top-3.5 z-20 mb-4 flex w-fit max-w-full items-center">
			<div class="flex items-center gap-1 rounded-[calc(var(--radius,0.45rem)+4px)] border border-border/60 bg-muted/80 p-1 shadow-md backdrop-blur-md max-w-full">
				{#if canScrollLeft}
					<button
						type="button"
						onclick={() => scrollTabs(-200)}
						aria-label="Scroll tabs left"
						class="flex h-8 w-7 shrink-0 items-center justify-center rounded-[var(--radius,0.45rem)] text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground cursor-pointer"
					>
						<HugeiconsIcon icon={ArrowLeft01Icon} class="h-3.5 w-3.5" />
					</button>
				{/if}

				<div
					{@attach trackTabScroll}
					class="no-scrollbar flex max-w-full items-center overflow-x-auto overflow-y-hidden select-none"
				>
					<Tabs.List class="flex items-center gap-1 bg-transparent !p-0 !border-0 !shadow-none rounded-none">
						<Tabs.Trigger value="all" class="!flex-none w-auto gap-1.5 rounded-[var(--radius,0.45rem)] px-3.5 py-1.5 text-sm font-medium transition-all">
							<HugeiconsIcon icon={SquareStackIcon} class="h-4 w-4" /> All
						</Tabs.Trigger>
						<Tabs.Trigger value="playlists" class="!flex-none w-auto gap-1.5 rounded-[var(--radius,0.45rem)] px-3.5 py-1.5 text-sm font-medium transition-all">
							<HugeiconsIcon icon={Playlist02Icon} class="h-4 w-4" /> Playlists
						</Tabs.Trigger>
						<Tabs.Trigger value="albums" class="!flex-none w-auto gap-1.5 rounded-[var(--radius,0.45rem)] px-3.5 py-1.5 text-sm font-medium transition-all">
							<HugeiconsIcon icon={MusicNoteSquare02Icon} class="h-4 w-4" /> Albums
						</Tabs.Trigger>
						<Tabs.Trigger value="artists" class="!flex-none w-auto gap-1.5 rounded-[var(--radius,0.45rem)] px-3.5 py-1.5 text-sm font-medium transition-all">
							<HugeiconsIcon icon={UserSharingIcon} class="h-4 w-4" /> Artists
						</Tabs.Trigger>
						<Tabs.Trigger value="songs" class="!flex-none w-auto gap-1.5 rounded-[var(--radius,0.45rem)] px-3.5 py-1.5 text-sm font-medium transition-all">
							<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4" /> Songs
						</Tabs.Trigger>
						<Tabs.Trigger value="uploads" class="!flex-none w-auto gap-1.5 rounded-[var(--radius,0.45rem)] px-3.5 py-1.5 text-sm font-medium transition-all">
							<HugeiconsIcon icon={CloudUploadIcon} class="h-4 w-4" /> Uploads
						</Tabs.Trigger>
						<Tabs.Trigger value="downloaded" class="!flex-none w-auto gap-1.5 rounded-[var(--radius,0.45rem)] px-3.5 py-1.5 text-sm font-medium transition-all">
							<HugeiconsIcon icon={Download01Icon} class="h-4 w-4" /> Downloaded
						</Tabs.Trigger>
						<Tabs.Trigger value="local" class="!flex-none w-auto gap-1.5 rounded-[var(--radius,0.45rem)] px-3.5 py-1.5 text-sm font-medium transition-all">
							<HugeiconsIcon icon={DriveIcon} class="h-4 w-4" /> Local
						</Tabs.Trigger>
					</Tabs.List>
				</div>

				{#if canScrollRight}
					<button
						type="button"
						onclick={() => scrollTabs(200)}
						aria-label="Scroll tabs right"
						class="flex h-8 w-7 shrink-0 items-center justify-center rounded-[var(--radius,0.45rem)] text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground cursor-pointer"
					>
						<HugeiconsIcon icon={ArrowRight01Icon} class="h-3.5 w-3.5" />
					</button>
				{/if}
			</div>
		</div>
		<!-- Every branch below is gated on `tab`, because bits-ui never unmounts an inactive panel: it
		     renders every one and hides the inactive ones. Left alone, opening Library builds each card twice
		     (once for All, once for its own tab) and mounts the whole Local tab, disk scan included,
		     for a panel you cannot see. -->
		<!-- Songs, Uploads and Local stand apart: two are track lists rather than card grids, the
		     third needs neither an account nor a connection, and the states below fit none of them. -->
		<Tabs.Content value="songs">
			{#if tab === 'songs'}
				{#if auth.account?.signedIn && spotify.status.linked}
					<!-- Segmented toggle keeping YouTube Music and Spotify Liked Songs strictly separate -->
					<div class="mb-4 flex items-center gap-2">
						<div class="inline-flex rounded-lg bg-muted/80 p-0.5 border border-border/60 text-xs">
							<button
								type="button"
								class="px-3 py-1.5 rounded-md font-medium transition cursor-pointer {songsPlatform === 'ytm'
									? 'bg-background shadow-xs text-foreground font-semibold'
									: 'text-muted-foreground hover:text-foreground'}"
								onclick={() => (songsPlatform = 'ytm')}
							>
								YouTube Music Liked
							</button>
							<button
								type="button"
								class="flex items-center gap-1.5 px-3 py-1.5 rounded-md font-medium transition cursor-pointer {songsPlatform === 'spotify'
									? 'bg-emerald-500/20 text-emerald-400 font-semibold shadow-xs'
									: 'text-muted-foreground hover:text-foreground'}"
								onclick={() => (songsPlatform = 'spotify')}
							>
								<svg class="h-3 w-3" viewBox="0 0 24 24" fill="currentColor">
									<path d="M12 2C6.477 2 2 6.477 2 12c0 5.524 4.477 10 10 10 5.524 0 10-4.476 10-10 0-5.523-4.476-10-10-10zm4.586 14.424a.627.627 0 0 1-.86.208c-2.355-1.439-5.32-1.765-8.812-.966a.625.625 0 0 1-.277-1.22c3.824-.874 7.099-.508 9.74 1.107.292.179.387.568.209.871zm1.226-2.723a.784.784 0 0 1-1.077.26c-2.695-1.656-6.804-2.136-9.992-1.168a.785.785 0 1 1-.462-1.501c3.642-1.106 8.188-.574 11.27 1.321a.784.784 0 0 1 .261 1.088zm.105-2.833c-3.232-1.919-8.566-2.096-11.657-1.157a.94.94 0 1 1-.552-1.8c3.553-1.078 9.444-.87 13.14 1.323a.94.94 0 0 1-.931 1.634z"/>
								</svg>
								Spotify Liked
							</button>
						</div>
					</div>
					{#if songsPlatform === 'spotify'}
						<LibrarySongs spotifyLiked />
					{:else}
						<LibrarySongs />
					{/if}
				{:else if spotify.status.linked && !auth.account?.signedIn}
					<LibrarySongs spotifyLiked />
				{:else if auth.account?.signedIn}
					<LibrarySongs />
				{:else}
					<p class="text-sm text-muted-foreground">
						Sign in to see songs saved in your YouTube Music library, or link your Spotify account in Settings to browse Spotify Liked Songs. Music on this machine is in the Local tab.
					</p>
				{/if}
			{/if}
		</Tabs.Content>
		<Tabs.Content value="uploads">
			{#if tab === 'uploads'}
				{#if signedOut}
					<p class="text-sm text-muted-foreground">
						Sign in to see the music you uploaded to YouTube Music. Local music on this machine is in the Local tab.
					</p>
				{:else}
					<LibrarySongs uploads />
				{/if}
			{/if}
		</Tabs.Content>
		<Tabs.Content value="downloaded">{#if tab === 'downloaded'}<DownloadedMusic />{/if}</Tabs.Content>
		<Tabs.Content value="local">{#if tab === 'local'}<LocalMusic />{/if}</Tabs.Content>
		{#if tab === 'local' || tab === 'songs' || tab === 'uploads' || tab === 'downloaded'}
			<!-- nothing else: the grid states below have no bearing on these four -->
		{:else if loading}
			<div class="card-grid">
				{#each Array(12) as _, i (i)}
					<MediaCardSkeleton />
				{/each}
			</div>
		{:else if error && !all.length}
			<!-- Only when there is nothing to fall back on. Now that the grid is cached across visits, a
			     refresh that fails should leave the library you were looking at on screen. -->
			<ErrorState message={error} onRetry={load} />
		{:else}
			<Tabs.Content value="all">
				{#if tab === 'all'}
					{#if spotify.status.linked && !auth.account?.signedIn}
						{@render grid(
							spotifyPlaylistItems,
							'No Spotify playlists found.',
							rvSpotify
						)}
					{:else}
						{@render grid(
							all,
							signedOut
								? 'Nothing saved yet. Open a playlist or album and hit Save to library, or sign in for the one on your account.'
								: 'Your library is empty.',
							rvAll
						)}
					{/if}
				{/if}
			</Tabs.Content>
			<Tabs.Content value="playlists">
				{#if tab === 'playlists'}
					{#if activeFolder}
						<div class="mb-4">
							<Button
								variant="ghost"
								size="sm"
								class="gap-1.5 text-muted-foreground hover:text-foreground mb-4 cursor-pointer {dragOverBack
									? 'ring-2 ring-primary bg-primary/10 text-primary'
									: ''}"
								ondragover={(e) => {
									if (
										e.dataTransfer?.types.includes(PLAYLIST_DND_MIME) ||
										e.dataTransfer?.types.includes(FOLDER_DND_MIME)
									) {
										e.preventDefault();
										dragOverBack = true;
										if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
									}
								}}
								ondragleave={() => (dragOverBack = false)}
								ondrop={(e) => {
									e.preventDefault();
									dragOverBack = false;
									const plId = e.dataTransfer?.getData(PLAYLIST_DND_MIME);
									const fId = e.dataTransfer?.getData(FOLDER_DND_MIME);
									const targetParentId = parentFolderOfActive?.id ?? null;
									if (plId) {
										movePlaylistToFolder(plId, targetParentId);
										toast.success(
											parentFolderOfActive
												? `Moved to "${parentFolderOfActive.name}"`
												: 'Moved to top level'
										);
									} else if (fId && fId !== targetParentId) {
										const ok = moveFolderToFolder(fId, targetParentId);
										if (ok) {
											toast.success(
												parentFolderOfActive
													? `Moved to "${parentFolderOfActive.name}"`
													: 'Moved to top level'
											);
										} else {
											toast.error('Cannot move folder inside itself');
										}
									}
								}}
								onclick={() => (activeFolderId = parentFolderOfActive?.id ?? null)}
							>
								<HugeiconsIcon icon={ArrowLeft01Icon} class="h-4 w-4" />
								<span>{parentFolderOfActive ? `Back to ${parentFolderOfActive.name}` : 'Back to Playlists'}</span>
							</Button>
							<div class="flex flex-wrap items-center justify-between gap-4 rounded-xl border border-border/60 bg-card/40 p-4 mb-6">
								<div class="flex items-center gap-3">
									<div class="flex h-12 w-12 items-center justify-center rounded-lg bg-primary/10 text-primary">
										<HugeiconsIcon icon={FolderOpenIcon} class="h-6 w-6" />
									</div>
									<div>
										<h2 class="font-heading text-xl font-bold">{activeFolder.name}</h2>
										<p class="text-xs text-muted-foreground">
											{playlistsInActiveFolder.length} {playlistsInActiveFolder.length === 1 ? 'playlist' : 'playlists'}{childFoldersInActive.length ? ` · ${childFoldersInActive.length} subfolders` : ''}
										</p>
									</div>
								</div>
								<div class="flex items-center gap-2">
									<Button
										variant="outline"
										size="sm"
										class="gap-1.5 cursor-pointer"
										onclick={() => {
											newFolderParentId = activeFolder.id;
											newFolderName = '';
											folderDialogOpen = true;
										}}
									>
										<HugeiconsIcon icon={FolderAddIcon} class="h-3.5 w-3.5" /> Subfolder
									</Button>
									<Button
										variant="outline"
										size="sm"
										class="gap-1.5 cursor-pointer"
										onclick={() => openRename(activeFolder.id, activeFolder.name)}
									>
										<HugeiconsIcon icon={Edit02Icon} class="h-3.5 w-3.5" /> Rename
									</Button>
									<Button
										variant="outline"
										size="sm"
										class="gap-1.5 text-destructive hover:bg-destructive/10 hover:text-destructive cursor-pointer"
										onclick={() => promptDelete(activeFolder.id)}
									>
										<HugeiconsIcon icon={Delete02Icon} class="h-3.5 w-3.5" /> Delete
									</Button>
								</div>
							</div>
						</div>

						{#if childFoldersInActive.length > 0}
							<div class="mb-6">
								<h3 class="mb-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
									Subfolders
								</h3>
								<div class="card-grid content-in mb-6">
									{#each childFoldersInActive as folder (folder.id)}
										<FolderCard
											{folder}
											playlists={playlists.filter((p) => folder.playlistIds.includes(p.id))}
											onclick={() => (activeFolderId = folder.id)}
											onrename={() => openRename(folder.id, folder.name)}
											ondelete={() => promptDelete(folder.id)}
										/>
									{/each}
								</div>
							</div>
						{/if}

						{#if playlistsInActiveFolder.length}
							{#if childFoldersInActive.length > 0}
								<h3 class="mb-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
									Playlists
								</h3>
							{/if}
							<div class="card-grid content-in">
								{#each playlistsInActiveFolder as item (item.kind + item.id)}
									<MediaCard {item} />
								{/each}
							</div>
						{:else if !childFoldersInActive.length}
							<p class="text-sm text-muted-foreground">
								No playlists in this folder yet. Click the ⋯ menu on any playlist and choose "Add to folder" or drag and drop to organize it here.
							</p>
						{/if}
					{:else}
						{#if spotify.status.linked}
							<div class="mb-8">
								<div class="mb-3 flex items-center justify-between">
									<h3 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground flex items-center gap-1.5">
										<svg class="h-3.5 w-3.5 text-emerald-400" viewBox="0 0 24 24" fill="currentColor">
											<path d="M12 2C6.477 2 2 6.477 2 12c0 5.524 4.477 10 10 10 5.524 0 10-4.476 10-10 0-5.523-4.476-10-10-10zm4.586 14.424a.627.627 0 0 1-.86.208c-2.355-1.439-5.32-1.765-8.812-.966a.625.625 0 0 1-.277-1.22c3.824-.874 7.099-.508 9.74 1.107.292.179.387.568.209.871zm1.226-2.723a.784.784 0 0 1-1.077.26c-2.695-1.656-6.804-2.136-9.992-1.168a.785.785 0 1 1-.462-1.501c3.642-1.106 8.188-.574 11.27 1.321a.784.784 0 0 1 .261 1.088zm.105-2.833c-3.232-1.919-8.566-2.096-11.657-1.157a.94.94 0 1 1-.552-1.8c3.553-1.078 9.444-.87 13.14 1.323a.94.94 0 0 1-.931 1.634z"/>
										</svg>
										Spotify Playlists
									</h3>
									<Button
										variant="ghost"
										size="sm"
										class="h-7 gap-1 text-xs text-muted-foreground hover:text-emerald-400 cursor-pointer"
										onclick={() => {
											dialogInitialPlatform = 'spotify';
											dialogOpen = true;
										}}
									>
										<HugeiconsIcon icon={Add01Icon} class="h-3.5 w-3.5" />
										<span>New Spotify Playlist</span>
									</Button>
								</div>
								{@render grid(
									spotifyPlaylistItems,
									'No Spotify playlists found.',
									rvSpotify
								)}
							</div>
						{/if}
						{#if auth.account?.signedIn || playlists.length || rootFolders.length || !spotify.status.linked}
							{#if spotify.status.linked}
								<div class="my-6 h-px w-full bg-border/60"></div>
								<h3 class="mb-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
									YouTube Music Playlists
								</h3>
							{/if}
							{#if rootFolders.length > 0}
								<div class="mb-8">
									<h3 class="mb-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
										Folders
									</h3>
									<div class="card-grid content-in mb-8">
										{#each rootFolders as folder (folder.id)}
											<FolderCard
												{folder}
												playlists={playlists.filter((p) => folder.playlistIds.includes(p.id))}
												onclick={() => (activeFolderId = folder.id)}
												onrename={() => openRename(folder.id, folder.name)}
												ondelete={() => promptDelete(folder.id)}
											/>
										{/each}
									</div>
									<h3 class="mb-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
										All Playlists
									</h3>
								</div>
							{/if}
							{@render grid(
								playlists,
								'No playlists yet. Open one and hit Save to library to keep it here.',
								rvPlaylists
							)}
						{/if}
					{/if}
				{/if}
			</Tabs.Content>
			<Tabs.Content value="albums">
				{#if tab === 'albums'}
					{@render grid(
						albums,
						'No saved albums yet. Open an album and hit Save to library.',
						rvAlbums
					)}
				{/if}
			</Tabs.Content>
			<Tabs.Content value="artists">
				{#if tab === 'artists'}
					{@render grid(
						artists,
						signedOut
							? 'No artists yet. Save one from its page to keep it here.'
							: 'No artists yet. They show up once you save their songs or albums.',
						rvArtists
					)}
				{/if}
			</Tabs.Content>
		{/if}
	</Tabs.Root>
</div>
