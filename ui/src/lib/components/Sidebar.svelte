<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { scale } from 'svelte/transition';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		LibraryIcon,
		Add01Icon,
		PinIcon,
		MusicNote01Icon,
		ListRestartIcon,
		SquareArrowLeft01Icon,
		SquareArrowRight01Icon,
		Folder01Icon,
		FolderOpenIcon,
		FolderAddIcon,
		ArrowDown01Icon,
		ArrowRight01Icon,
		ComputerIcon,
		Home01Icon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as api from '$lib/api';
	import { ON_REPEAT_ID, type BrowseItem } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import PlaylistMenu from './PlaylistMenu.svelte';
	import FolderMenu from './FolderMenu.svelte';
	import CreatePlaylistDialog from './CreatePlaylistDialog.svelte';
	import {
		prefs,
		auth,
		library,
		personal,
		ui,
		np,
		spotify,
		loadSpotifyPlaylists,
		toggleDevicesSidebar,
		createLibraryPlaylist,
		createPlaylistFolder,
		renamePlaylistFolder,
		deletePlaylistFolder,
		toggleFolderCollapsed,
		movePlaylistToFolder,
		toggleSidebar,
		setSidebarWidth,
		toast
	} from '$lib/player.svelte';
	import { mergeSaved, orderLibrary, type PlaylistFolder } from '$lib/personal';
	import { PLAYLIST_DND_MIME, setDragPlaylist, setDragItem } from '$lib/dnd';

	let activePlaylistTab = $state<'ytm' | 'spotify'>('ytm');
	const showDualTabs = $derived(
		auth.account?.signedIn && spotify.status.linked && spotify.syncMode === 'seperate'
	);

	const isActive = (href: string) =>
		href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href);

	// Pinned first (in pin order), then everything else by last played. Derived here rather than in
	// the shared `library` store so the Library page keeps YouTube's own ordering. Playlists saved
	// on this machine sit in the same list: signed out they are the only ones there.
	const playlists = $derived(
		orderLibrary(mergeSaved(personal, library.items, 'playlist'), personal)
	);
	// How many of the leading rows are pinned — a rule under the last one explains the split.
	const pinnedCount = $derived(playlists.filter((p) => personal.pins.includes(p.id)).length);

	const folders = $derived(personal.folders ?? []);
	const rootFolders = $derived(folders.filter((f) => !f.parentId));
	const unfiledPlaylists = $derived(
		playlists.filter((pl) => !folders.some((f) => f.playlistIds.includes(pl.id)))
	);

	// YTM's library subtitle is "Owner • 20 tracks" and the rail is too narrow for both, so keep the
	// count and drop the rest. Subtitles without a number (albums: "Album • Artist") stay whole.
	const rowSubtitle = (s?: string) =>
		s
			?.split('•')
			.map((p) => p.trim())
			.filter((p) => /\d/.test(p))
			.at(-1) ?? s;

	const playlistHref = (item: BrowseItem) =>
		item.kind === 'album'
			? `/album/${encodeURIComponent(item.id)}`
			: item.kind === 'artist'
				? `/artist/${encodeURIComponent(item.id)}`
				: `/playlist/${encodeURIComponent(item.id)}`;

	// New-playlist dialog (with name, description, privacy, and image).
	let playlistDialogOpen = $state(false);

	let folderDialogOpen = $state(false);
	let newFolderName = $state('');
	let newFolderParentId = $state<string | null>(null);

	let renameDialogOpen = $state(false);
	let renameFolderId = $state<string | null>(null);
	let renameFolderName = $state('');

	let dragOverFolderId = $state<string | null>(null);
	let dragOverRoot = $state(false);

	function handleCreateFolder() {
		const name = newFolderName.trim();
		if (!name) return;
		const f = createPlaylistFolder(name, newFolderParentId);
		toast.success(`Created folder "${f.name}"`);
		newFolderName = '';
		newFolderParentId = null;
		folderDialogOpen = false;
	}

	function openRenameFolder(folder: PlaylistFolder) {
		renameFolderId = folder.id;
		renameFolderName = folder.name;
		renameDialogOpen = true;
	}

	function handleRenameFolder() {
		if (!renameFolderId) return;
		const name = renameFolderName.trim();
		if (!name) return;
		renamePlaylistFolder(renameFolderId, name);
		toast.success('Folder renamed');
		renameDialogOpen = false;
	}

	function handleDropOnFolder(e: DragEvent, folderId: string, folderName: string) {
		e.preventDefault();
		e.stopPropagation();
		dragOverFolderId = null;
		const plId = e.dataTransfer?.getData(PLAYLIST_DND_MIME) || e.dataTransfer?.getData('text/plain');
		if (plId) {
			movePlaylistToFolder(plId, folderId);
			toast.success(`Moved to "${folderName}"`);
		}
	}

	function handleDropOnRoot(e: DragEvent) {
		e.preventDefault();
		e.stopPropagation();
		dragOverRoot = false;
		const plId = e.dataTransfer?.getData(PLAYLIST_DND_MIME) || e.dataTransfer?.getData('text/plain');
		if (plId) {
			movePlaylistToFolder(plId, null);
			toast.success('Moved to top level');
		}
	}

	// Account lives in the titlebar now — see AccountMenu.svelte.

	// Manual collapse is a large-screen preference: below lg the rail is already collapsed by the
	// breakpoint, so the button is hidden there and `wide()` has nothing to drop. Every expanded
	// style is an `lg:` class, so collapsing is just not emitting them. The flag lives in `ui`
	// because the overlays that offset by the sidebar's width read it too.
	let innerWidth = $state(typeof window !== 'undefined' ? window.innerWidth : 1200);
	const isSmallScreen = $derived(innerWidth < 1024);
	const isNowPlaying = $derived(np.open && !np.fullscreenOpen);
	const collapsed = $derived(isSmallScreen || ((ui.sidebarCollapsed || isNowPlaying) && !ui.sidebarForceExpanded));
	const wide = (cls: string) => (collapsed ? '' : cls);

	function startResize(e: MouseEvent) {
		e.preventDefault();
		const startX = e.clientX;
		const startWidth = ui.sidebarWidth;

		function onMouseMove(ev: MouseEvent) {
			const delta = ev.clientX - startX;
			const newWidth = Math.max(180, Math.min(420, startWidth + delta));
			setSidebarWidth(newWidth);
		}

		function onMouseUp() {
			window.removeEventListener('mousemove', onMouseMove);
			window.removeEventListener('mouseup', onMouseUp);
		}

		window.addEventListener('mousemove', onMouseMove);
		window.addEventListener('mouseup', onMouseUp);
	}
</script>

<svelte:window bind:innerWidth />

<aside
	style="{prefs.floatingSidebarLeft ? 'height: calc(100% - 1rem);' : 'height: 100%;'} width: {collapsed ? '4rem' : `${ui.sidebarWidth}px`};"
	class="relative z-30 flex shrink-0 flex-col bg-sidebar p-3 text-sidebar-foreground transition-[border-radius,margin] duration-200 {prefs.floatingSidebarLeft
		? 'app-floating-panel m-2 rounded-2xl border border-border/70 shadow-xl backdrop-blur-xl bg-sidebar/80'
		: 'border-r border-border/70 rounded-none m-0 shadow-none'}"
>
	{#if !collapsed}
		<!-- Resize handle on the right edge -->
		<div
			role="separator"
			tabindex="0"
			aria-orientation="vertical"
			aria-label="Resize sidebar"
			class="absolute -right-1 top-0 bottom-0 w-2 cursor-col-resize z-40 hover:bg-primary/25 active:bg-primary/40 transition-colors select-none"
			onmousedown={startResize}
		></div>
	{/if}

	<div class="flex items-center justify-between px-1 py-1">
		{#if !collapsed}
			<span class="font-heading text-lg font-bold tracking-tight pl-1">Nocturne</span>
		{/if}
		{#if !isSmallScreen}
			<div class="flex items-center gap-1 {collapsed ? 'w-full justify-center' : ''}">
				<Button
					variant="ghost"
					size="icon-sm"
					class="hover:text-primary cursor-pointer text-muted-foreground"
					onclick={toggleSidebar}
					aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
					title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
				>
					<!-- altIcon/showAlt, not a ternary: `icon` is read once at mount. -->
					<HugeiconsIcon
						icon={SquareArrowLeft01Icon}
						altIcon={SquareArrowRight01Icon}
						showAlt={collapsed}
						strokeWidth={2}
						class="h-4 w-4"
					/>
				</Button>
			</div>
		{/if}
	</div>

	{#if prefs.homeInSidebar}
		<div class="mt-1 mb-1 shrink-0">
			<a
				href="/"
				title="Home"
				class="group relative flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-xs font-medium transition-colors {collapsed
					? 'justify-center'
					: 'justify-start'} {isActive('/')
					? 'bg-primary/10 text-primary'
					: 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'}"
			>
				{#if isActive('/')}
					<span
						transition:scale={{ duration: 200, start: 0.4 }}
						class="absolute left-0 top-1/2 h-5 w-1 -translate-y-1/2 rounded-r-full bg-primary"
					></span>
				{/if}
				<HugeiconsIcon
					icon={Home01Icon}
					class="h-4.5 w-4.5 shrink-0 transition-transform duration-200 group-hover:scale-110"
				/>
				{#if !collapsed}
					<span class="truncate">Home</span>
				{/if}
			</a>
		</div>
	{/if}

	<!-- Playlists & Folders. On the collapsed rail, shows playlist icons. On expanded, shows full names & folders. -->
	{#if auth.account?.signedIn || playlists.length || rootFolders.length || (spotify.status.linked && (spotify.playlists.length || spotify.loadingPlaylists))}
		<div class="mt-2 flex min-h-0 flex-1 flex-col pt-1">
			{#if collapsed}
				<!-- Collapsed Icon Rail View -->
				{#if showDualTabs}
					<div class="mb-2 flex items-center justify-center gap-1">
						<button
							type="button"
							class="h-6 w-6 rounded flex items-center justify-center transition-colors cursor-pointer {activePlaylistTab === 'ytm' ? 'bg-primary/20 text-primary' : 'text-muted-foreground hover:text-foreground'}"
							onclick={() => (activePlaylistTab = 'ytm')}
							title="YouTube Music Playlists"
						>
							<HugeiconsIcon icon={MusicNote01Icon} class="h-3 w-3" />
						</button>
						<button
							type="button"
							class="h-6 w-6 rounded flex items-center justify-center transition-colors cursor-pointer {activePlaylistTab === 'spotify' ? 'bg-emerald-500/20 text-emerald-400' : 'text-muted-foreground hover:text-foreground'}"
							onclick={() => {
								activePlaylistTab = 'spotify';
								if (!spotify.playlists.length) loadSpotifyPlaylists();
							}}
							title="Spotify Playlists"
						>
							<svg class="h-3 w-3" viewBox="0 0 24 24" fill="currentColor">
								<path d="M12 2C6.477 2 2 6.477 2 12c0 5.524 4.477 10 10 10 5.524 0 10-4.476 10-10 0-5.523-4.476-10-10-10zm4.586 14.424a.627.627 0 0 1-.86.208c-2.355-1.439-5.32-1.765-8.812-.966a.625.625 0 0 1-.277-1.22c3.824-.874 7.099-.508 9.74 1.107.292.179.387.568.209.871zm1.226-2.723a.784.784 0 0 1-1.077.26c-2.695-1.656-6.804-2.136-9.992-1.168a.785.785 0 1 1-.462-1.501c3.642-1.106 8.188-.574 11.27 1.321a.784.784 0 0 1 .261 1.088zm.105-2.833c-3.232-1.919-8.566-2.096-11.657-1.157a.94.94 0 1 1-.552-1.8c3.553-1.078 9.444-.87 13.14 1.323a.94.94 0 0 1-.931 1.634z"/>
							</svg>
						</button>
					</div>
				{/if}

				{#if showDualTabs && activePlaylistTab === 'spotify'}
					<div class="min-h-0 flex-1 overflow-y-auto space-y-1.5 py-1 flex flex-col items-center [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
						{#each spotify.playlists as pl (pl.id)}
							{@render collapsedSpotifyPlaylistButton(pl)}
						{:else}
							{#if spotify.loadingPlaylists}
								<div class="h-4 w-4 animate-spin rounded-full border-2 border-emerald-500 border-t-transparent"></div>
							{/if}
						{/each}
					</div>
				{:else}
					{#if auth.account?.signedIn}
						<div class="mb-2 flex justify-center">
							<Button
								variant="outline"
								size="icon-sm"
								class="h-8 w-8 rounded-lg border-dashed border-border/80 text-muted-foreground hover:border-primary hover:text-primary transition-colors cursor-pointer"
								onclick={() => (playlistDialogOpen = true)}
								title="New playlist"
							>
								<HugeiconsIcon icon={Add01Icon} class="h-3.5 w-3.5" />
							</Button>
						</div>
					{/if}

					<div class="min-h-0 flex-1 overflow-y-auto space-y-1.5 py-1 flex flex-col items-center [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
						{#each playlists as pl, i (pl.id)}
							{@render collapsedPlaylistButton(pl)}
							{#if pinnedCount && i === pinnedCount - 1}
								<div class="my-1 h-px w-6 bg-border/60"></div>
							{/if}
						{:else}
							{#if library.loading}
								<div class="h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent"></div>
							{/if}
						{/each}
					</div>
				{/if}
			{:else}
				{#if showDualTabs}
					<!-- Dual tabs switcher -->
					<div class="mb-2 flex items-center p-0.5 rounded-lg bg-muted/60 text-xs">
						<button
							type="button"
							class="flex-1 py-1 rounded-md font-medium text-center transition-all cursor-pointer {activePlaylistTab === 'ytm' ? 'bg-background shadow-xs text-foreground' : 'text-muted-foreground hover:text-foreground'}"
							onclick={() => (activePlaylistTab = 'ytm')}
						>
							YouTube Music
						</button>
						<button
							type="button"
							class="flex-1 py-1 rounded-md font-medium text-center transition-all cursor-pointer {activePlaylistTab === 'spotify' ? 'bg-background shadow-xs text-emerald-400 font-semibold' : 'text-muted-foreground hover:text-foreground'}"
							onclick={() => {
								activePlaylistTab = 'spotify';
								if (!spotify.playlists.length) loadSpotifyPlaylists();
							}}
						>
							Spotify
						</button>
					</div>
				{/if}

				{#if showDualTabs && activePlaylistTab === 'spotify'}
					<div class="mb-2 flex items-center justify-between px-1">
						<span class="text-[11px] font-semibold text-muted-foreground/80 uppercase tracking-wider">Spotify Playlists</span>
						<button
							type="button"
							class="p-1 text-muted-foreground hover:text-emerald-400 transition-colors cursor-pointer rounded hover:bg-emerald-500/10"
							onclick={() => loadSpotifyPlaylists()}
							title="Refresh Spotify playlists"
						>
							<HugeiconsIcon icon={ListRestartIcon} class="h-3.5 w-3.5 {spotify.loadingPlaylists ? 'animate-spin' : ''}" />
						</button>
					</div>
					<div class="min-h-0 flex-1 overflow-y-auto space-y-0.5">
						{#each spotify.playlists as pl (pl.id)}
							{@render spotifyPlaylistRow(pl)}
						{:else}
							{#if spotify.loadingPlaylists}
								<p class="px-3 py-2 text-xs text-muted-foreground">Loading Spotify playlists…</p>
							{:else}
								<p class="px-3 py-2 text-xs text-muted-foreground">No Spotify playlists found.</p>
							{/if}
						{/each}
					</div>
				{:else}
					<!-- Action Header -->
					<div class="mb-2 flex items-center gap-1.5">
						{#if auth.account?.signedIn}
							<Button
								variant="outline"
								size="sm"
								class="h-8 flex-1 gap-1.5 text-xs cursor-pointer"
								onclick={() => (playlistDialogOpen = true)}
							>
								<HugeiconsIcon icon={Add01Icon} class="h-3.5 w-3.5" />
								<span>New playlist</span>
							</Button>
						{/if}
						<Button
							variant="ghost"
							size="icon-sm"
							class="h-8 w-8 text-muted-foreground hover:text-foreground cursor-pointer"
							onclick={() => {
								newFolderName = '';
								newFolderParentId = null;
								folderDialogOpen = true;
							}}
							title="New folder"
						>
							<HugeiconsIcon icon={FolderAddIcon} class="h-4 w-4" />
						</Button>
					</div>

					<div class="min-h-0 flex-1 overflow-y-auto space-y-1">
						<!-- Playlist Folders Section -->
						{#each rootFolders as folder (folder.id)}
							{@const folderPlaylists = playlists.filter((p) => folder.playlistIds.includes(p.id))}
							{@const isDragTarget = dragOverFolderId === folder.id}
							<div
								class="group/folder rounded-lg transition-all {isDragTarget
									? 'bg-primary/15 ring-1 ring-primary/50'
									: ''}"
								ondragover={(e) => {
									if (e.dataTransfer?.types.includes(PLAYLIST_DND_MIME)) {
										e.preventDefault();
										dragOverFolderId = folder.id;
									}
								}}
								ondragleave={() => {
									if (dragOverFolderId === folder.id) dragOverFolderId = null;
								}}
								ondrop={(e) => handleDropOnFolder(e, folder.id, folder.name)}
							>
								<!-- Folder Row -->
								<div class="relative flex items-center justify-between rounded-lg py-1 pl-1.5 pr-8 hover:bg-sidebar-accent/50">
									<button
										type="button"
										class="flex min-w-0 flex-1 items-center gap-2 text-left cursor-pointer"
										onclick={() => toggleFolderCollapsed(folder.id)}
									>
										<HugeiconsIcon
											icon={folder.collapsed ? ArrowRight01Icon : ArrowDown01Icon}
											class="h-3.5 w-3.5 shrink-0 text-muted-foreground transition-transform"
										/>
										<HugeiconsIcon
											icon={folder.collapsed ? Folder01Icon : FolderOpenIcon}
											class="h-4 w-4 shrink-0 text-primary"
										/>
										<span class="truncate text-[13px] font-semibold">{folder.name}</span>
										<span class="rounded-full bg-muted px-1.5 py-0.2 text-[10px] font-medium text-muted-foreground">
											{folderPlaylists.length}
										</span>
									</button>
									<FolderMenu
										{folder}
										onNewPlaylist={() => {
											playlistDialogOpen = true;
										}}
										onNewSubfolder={(parent) => {
											newFolderName = '';
											newFolderParentId = parent;
											folderDialogOpen = true;
										}}
										onRename={openRenameFolder}
									/>
								</div>

								<!-- Folder Contents (Nested Playlists) -->
								{#if !folder.collapsed}
									<div class="ml-2 border-l border-border/50 pl-1 space-y-0.5 py-0.5">
										{#each folderPlaylists as pl (pl.id)}
											{@render playlistRow(pl, true)}
										{:else}
											<p class="py-1 pl-6 text-xs text-muted-foreground/60 italic">
												Drop playlists here
											</p>
										{/each}
									</div>
								{/if}
							</div>
						{/each}

						<!-- Top Level / Unfiled Playlists Drop Zone & List -->
						<div
							class="rounded-lg transition-colors {dragOverRoot ? 'bg-primary/10 ring-1 ring-primary/40' : ''}"
							ondragover={(e) => {
								if (e.dataTransfer?.types.includes(PLAYLIST_DND_MIME)) {
									e.preventDefault();
									dragOverRoot = true;
								}
							}}
							ondragleave={() => (dragOverRoot = false)}
							ondrop={handleDropOnRoot}
						>
							{#if rootFolders.length > 0 && unfiledPlaylists.length > 0}
								<div class="px-2 pt-2 pb-1 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground/70">
									Playlists
								</div>
							{/if}

							{#each unfiledPlaylists as pl, i (pl.id)}
								{@render playlistRow(pl, false)}
								{#if pinnedCount && i === pinnedCount - 1}
									<div class="mx-3 my-1.5 h-px bg-border"></div>
								{/if}
							{:else}
								{#if library.loading && !rootFolders.length}
									<p class="px-3 py-1.5 text-xs text-muted-foreground">Loading…</p>
								{/if}
							{/each}
						</div>
					</div>
				{/if}
			{/if}
		</div>

		<!-- New Playlist Modal with Picture & Description -->
		<CreatePlaylistDialog bind:open={playlistDialogOpen} />

		<!-- New Folder Modal -->
		<Dialog.Root bind:open={folderDialogOpen}>
			<Dialog.Content class="sm:max-w-md">
				<Dialog.Header>
					<Dialog.Title>New Folder</Dialog.Title>
					<Dialog.Description>Organize your playlists in a folder.</Dialog.Description>
				</Dialog.Header>
				<form
					class="flex flex-col gap-4"
					onsubmit={(e) => {
						e.preventDefault();
						handleCreateFolder();
					}}
				>
					<Input bind:value={newFolderName} placeholder="Folder name" autofocus />
					<Dialog.Footer>
						<Button type="button" variant="outline" onclick={() => (folderDialogOpen = false)}>Cancel</Button>
						<Button type="submit" disabled={!newFolderName.trim()}>Create</Button>
					</Dialog.Footer>
				</form>
			</Dialog.Content>
		</Dialog.Root>

		<!-- Rename Folder Modal -->
		<Dialog.Root bind:open={renameDialogOpen}>
			<Dialog.Content class="sm:max-w-md">
				<Dialog.Header>
					<Dialog.Title>Rename Folder</Dialog.Title>
					<Dialog.Description>Enter a new name for this folder.</Dialog.Description>
				</Dialog.Header>
				<form
					class="flex flex-col gap-4"
					onsubmit={(e) => {
						e.preventDefault();
						handleRenameFolder();
					}}
				>
					<Input bind:value={renameFolderName} placeholder="Folder name" autofocus />
					<Dialog.Footer>
						<Button type="button" variant="outline" onclick={() => (renameDialogOpen = false)}>Cancel</Button>
						<Button type="submit" disabled={!renameFolderName.trim()}>Rename</Button>
					</Dialog.Footer>
				</form>
			</Dialog.Content>
		</Dialog.Root>
	{/if}

	<!-- See full library button at bottom of sidebar -->
	<div class="mt-auto pt-2 border-t border-border/60 shrink-0">
		<a
			href="/library"
			title="See full library"
			class="group relative flex items-center justify-center gap-2.5 rounded-lg px-2.5 py-2 text-xs font-medium transition-colors {wide(
				'lg:justify-start'
			)} {isActive('/library')
				? 'bg-primary/10 text-primary'
				: 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'}"
		>
			{#if isActive('/library')}
				<span
					transition:scale={{ duration: 200, start: 0.4 }}
					class="absolute left-0 top-1/2 h-5 w-1 -translate-y-1/2 rounded-r-full bg-primary"
				></span>
			{/if}
			<HugeiconsIcon
				icon={LibraryIcon}
				class="h-4.5 w-4.5 shrink-0 transition-transform duration-200 group-hover:scale-110"
			/>
			<span class="hidden {wide('lg:inline')} truncate">See full library</span>
		</a>
	</div>
</aside>

{#snippet playlistRow(pl: BrowseItem, inFolder: boolean = false)}
	<!-- The ⋯ is a sibling of the link, not a child: a <button> inside an <a> is invalid HTML -->
	<div
		class="group/row relative"
		data-ctx
		draggable="true"
		ondragstart={(e) => {
			setDragPlaylist(e, pl.id);
			setDragItem(e, pl);
		}}
	>
		<a
			href={playlistHref(pl)}
			title={pl.title}
			class="flex items-center gap-2.5 rounded-lg py-1.5 pr-9 transition-colors hover:bg-sidebar-accent/50 {inFolder ? 'pl-2' : 'pl-2'}"
		>
			<div
				class="relative h-9 w-9 shrink-0 overflow-hidden bg-muted {pl.kind === 'artist'
					? 'rounded-full'
					: 'rounded-md'}"
			>
				{#if pl.thumbnail && pl.id !== ON_REPEAT_ID}
					<img
						src={thumb(pl.thumbnail, 96)}
						alt=""
						class="h-full w-full object-cover"
						loading="lazy"
					/>
				{:else}
					<!-- On Repeat has no artwork by nature: icon tile, same as its card. -->
					<div
						class="flex h-full w-full items-center justify-center {pl.id === ON_REPEAT_ID
							? 'bg-primary/10 text-primary'
							: 'text-muted-foreground/50'}"
					>
						<HugeiconsIcon
							icon={MusicNote01Icon}
							altIcon={ListRestartIcon}
							showAlt={pl.id === ON_REPEAT_ID}
							class={pl.id === ON_REPEAT_ID ? 'h-4 w-4' : 'h-3.5 w-3.5'}
						/>
					</div>
				{/if}
			</div>
			{#if personal.pins.includes(pl.id)}
				<span
					class="absolute left-8 top-0.5 flex h-3.5 w-3.5 items-center justify-center rounded-full bg-primary text-primary-foreground shadow"
				>
					<HugeiconsIcon icon={PinIcon} class="h-2 w-2" />
				</span>
			{/if}
			<div class="min-w-0 flex-1">
				<div class="truncate text-[13px] font-medium leading-tight">{pl.title}</div>
				{#if pl.subtitle}
					<div class="truncate text-[11px] text-muted-foreground">{rowSubtitle(pl.subtitle)}</div>
				{/if}
			</div>
		</a>
		<PlaylistMenu item={pl} />
	</div>
{/snippet}

{#snippet collapsedPlaylistButton(pl: BrowseItem)}
	<div
		class="group/cpl relative flex items-center justify-center"
		data-ctx
		draggable="true"
		ondragstart={(e) => {
			setDragPlaylist(e, pl.id);
			setDragItem(e, pl);
		}}
	>
		<a
			href={playlistHref(pl)}
			title={pl.title}
			class="relative flex h-9 w-9 items-center justify-center rounded-lg transition-transform duration-150 hover:scale-105 hover:bg-sidebar-accent/50 focus:outline-none focus-visible:ring-2 focus-visible:ring-ring {isActive(
				playlistHref(pl)
			)
				? 'bg-sidebar-accent/60 ring-1 ring-primary/60'
				: ''}"
		>
			<div
				class="relative h-7 w-7 shrink-0 overflow-hidden bg-muted {pl.kind === 'artist'
					? 'rounded-full'
					: 'rounded-md'} shadow-xs"
			>
				{#if pl.thumbnail && pl.id !== ON_REPEAT_ID}
					<img
						src={thumb(pl.thumbnail, 96)}
						alt=""
						class="h-full w-full object-cover"
						loading="lazy"
					/>
				{:else}
					<div
						class="flex h-full w-full items-center justify-center {pl.id === ON_REPEAT_ID
							? 'bg-primary/10 text-primary'
							: 'text-muted-foreground/50'}"
					>
						<HugeiconsIcon
							icon={MusicNote01Icon}
							altIcon={ListRestartIcon}
							showAlt={pl.id === ON_REPEAT_ID}
							class={pl.id === ON_REPEAT_ID ? 'h-3.5 w-3.5' : 'h-3 w-3'}
						/>
					</div>
				{/if}
			</div>
			{#if personal.pins.includes(pl.id)}
				<span
					class="absolute -top-0.5 -right-0.5 flex h-3.5 w-3.5 items-center justify-center rounded-full bg-primary text-primary-foreground shadow"
					title="Pinned"
				>
					<HugeiconsIcon icon={PinIcon} class="h-2 w-2" />
				</span>
			{/if}
		</a>
		<PlaylistMenu item={pl} triggerClass="hidden" />
	</div>
{/snippet}

{#snippet spotifyPlaylistRow(pl: api.SpotifyPlaylistSummary)}
	<div class="group/row relative flex items-center justify-between rounded-lg py-1 pl-2 pr-2 hover:bg-sidebar-accent/50 transition-colors">
		<a
			href="/playlist/sp_{encodeURIComponent(pl.id)}"
			onclick={(e) => {
				e.preventDefault();
				goto(`/playlist/sp_${encodeURIComponent(pl.id)}`);
			}}
			class="flex min-w-0 flex-1 items-center gap-2.5 text-left cursor-pointer"
			title="{pl.title}"
		>
			<div class="relative h-9 w-9 shrink-0 overflow-hidden rounded-md bg-muted">
				{#if pl.thumbnail}
					<img src={pl.thumbnail} alt="" class="h-full w-full object-cover" loading="lazy" />
				{:else}
					<div class="flex h-full w-full items-center justify-center bg-emerald-500/20 text-emerald-400">
						<svg class="h-4 w-4" viewBox="0 0 24 24" fill="currentColor">
							<path d="M12 2C6.477 2 2 6.477 2 12c0 5.524 4.477 10 10 10 5.524 0 10-4.476 10-10 0-5.523-4.476-10-10-10zm4.586 14.424a.627.627 0 0 1-.86.208c-2.355-1.439-5.32-1.765-8.812-.966a.625.625 0 0 1-.277-1.22c3.824-.874 7.099-.508 9.74 1.107.292.179.387.568.209.871zm1.226-2.723a.784.784 0 0 1-1.077.26c-2.695-1.656-6.804-2.136-9.992-1.168a.785.785 0 1 1-.462-1.501c3.642-1.106 8.188-.574 11.27 1.321a.784.784 0 0 1 .261 1.088zm.105-2.833c-3.232-1.919-8.566-2.096-11.657-1.157a.94.94 0 1 1-.552-1.8c3.553-1.078 9.444-.87 13.14 1.323a.94.94 0 0 1-.931 1.634z"/>
						</svg>
					</div>
				{/if}
			</div>
			<div class="min-w-0 flex-1">
				<div class="truncate text-[13px] font-medium leading-tight text-foreground">{pl.title}</div>
				{#if pl.subtitle}
					<div class="truncate text-[11px] text-muted-foreground">{pl.subtitle}</div>
				{/if}
			</div>
		</a>
		<button
			type="button"
			class="opacity-0 group-hover/row:opacity-100 px-1.5 py-0.5 text-muted-foreground hover:text-emerald-400 hover:bg-emerald-500/10 transition rounded text-[10px] font-semibold cursor-pointer shrink-0"
			onclick={async () => {
				toast.info(`Transferring "${pl.title}" to YouTube Music...`);
				try {
					const res = await api.spotifyTransferToYtm(pl.id);
					toast.success(res);
				} catch (e) {
					toast.error(String(e));
				}
			}}
			title="Transfer this playlist to YouTube Music"
		>
			Transfer
		</button>
	</div>
{/snippet}

{#snippet collapsedSpotifyPlaylistButton(pl: api.SpotifyPlaylistSummary)}
	<a
		href="/playlist/sp_{encodeURIComponent(pl.id)}"
		onclick={(e) => {
			e.preventDefault();
			goto(`/playlist/sp_${encodeURIComponent(pl.id)}`);
		}}
		class="relative flex h-9 w-9 items-center justify-center rounded-lg transition-transform duration-150 hover:scale-105 hover:bg-sidebar-accent/50 cursor-pointer {isActive('/playlist/sp_' + pl.id) ? 'bg-sidebar-accent/60 ring-1 ring-primary/60' : ''}"
		title="{pl.title}"
	>
		<div class="relative h-7 w-7 shrink-0 overflow-hidden rounded-md bg-muted shadow-xs">
			{#if pl.thumbnail}
				<img src={pl.thumbnail} alt="" class="h-full w-full object-cover" loading="lazy" />
			{:else}
				<div class="flex h-full w-full items-center justify-center bg-emerald-500/20 text-emerald-400">
					<svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="currentColor">
						<path d="M12 2C6.477 2 2 6.477 2 12c0 5.524 4.477 10 10 10 5.524 0 10-4.476 10-10 0-5.523-4.476-10-10-10zm4.586 14.424a.627.627 0 0 1-.86.208c-2.355-1.439-5.32-1.765-8.812-.966a.625.625 0 0 1-.277-1.22c3.824-.874 7.099-.508 9.74 1.107.292.179.387.568.209.871zm1.226-2.723a.784.784 0 0 1-1.077.26c-2.695-1.656-6.804-2.136-9.992-1.168a.785.785 0 1 1-.462-1.501c3.642-1.106 8.188-.574 11.27 1.321a.784.784 0 0 1 .261 1.088zm.105-2.833c-3.232-1.919-8.566-2.096-11.657-1.157a.94.94 0 1 1-.552-1.8c3.553-1.078 9.444-.87 13.14 1.323a.94.94 0 0 1-.931 1.634z"/>
					</svg>
				</div>
			{/if}
		</div>
	</a>
{/snippet}


