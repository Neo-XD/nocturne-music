<script lang="ts">
	import { page } from '$app/state';
	import { scale } from 'svelte/transition';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Home01Icon,
		Search01Icon,
		LibraryIcon,
		Settings01Icon,
		Sun01Icon,
		Moon02Icon,
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
		ComputerIcon
	} from '@hugeicons/core-free-icons';
	import { toggleMode } from 'mode-watcher';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Dialog from '$lib/components/ui/dialog';
	import { ON_REPEAT_ID, type BrowseItem } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import PlaylistMenu from './PlaylistMenu.svelte';
	import FolderMenu from './FolderMenu.svelte';
	import CreatePlaylistDialog from './CreatePlaylistDialog.svelte';
	import {
		auth,
		library,
		personal,
		ui,
		np,
		toggleDevicesSidebar,
		createLibraryPlaylist,
		createPlaylistFolder,
		renamePlaylistFolder,
		deletePlaylistFolder,
		toggleFolderCollapsed,
		movePlaylistToFolder,
		toggleSidebar,
		toast
	} from '$lib/player.svelte';
	import { mergeSaved, orderLibrary, type PlaylistFolder } from '$lib/personal';
	import { PLAYLIST_DND_MIME, setDragPlaylist, setDragItem } from '$lib/dnd';

	const nav = [
		{ href: '/', label: 'Home', icon: Home01Icon },
		{ href: '/library', label: 'Library', icon: LibraryIcon }
	];
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
	const collapsed = $derived(ui.sidebarCollapsed);
	const wide = (cls: string) => (collapsed ? '' : cls);
</script>

<aside
	class="absolute inset-y-0 left-0 z-20 flex h-full w-16 flex-col border-r bg-sidebar p-3 text-sidebar-foreground {wide(
		'lg:w-60'
	)}"
>
	<div class="flex items-center justify-center px-2 py-2 {wide('lg:justify-between')}">
		<span class="hidden font-heading text-lg font-bold tracking-tight {wide('lg:block')}">Nocturne</span>
		<!-- Column when collapsed: the two buttons don't fit side by side in the 64px rail. -->
		<div class="flex items-center gap-1 {collapsed ? 'flex-col' : ''}">
			<Button
				variant="ghost"
				size="icon-sm"
				class="hidden hover:text-primary lg:inline-flex"
				onclick={toggleSidebar}
				aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
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
			<Button
				variant="ghost"
				size="icon-sm"
				class="hover:text-primary"
				onclick={toggleMode}
				aria-label="Toggle theme"
			>
				<HugeiconsIcon icon={Sun01Icon} strokeWidth={2} class="h-4 w-4 dark:hidden" />
				<HugeiconsIcon icon={Moon02Icon} strokeWidth={2} class="hidden h-4 w-4 dark:block" />
			</Button>
		</div>
	</div>

	<nav class="mt-2 flex flex-col gap-1">
		{#each nav as n (n.href)}
			<a
				href={n.href}
				title={n.label}
				class="group relative flex items-center justify-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors {wide(
					'lg:justify-start'
				)} {isActive(n.href)
					? 'bg-primary/10 text-primary'
					: 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'}"
			>
				{#if isActive(n.href)}
					<span
						transition:scale={{ duration: 200, start: 0.4 }}
						class="absolute left-0 top-1/2 h-5 w-1 -translate-y-1/2 rounded-r-full bg-primary"
					></span>
				{/if}
				<HugeiconsIcon
					icon={n.icon}
					class="h-5 w-5 shrink-0 transition-transform duration-200 group-hover:scale-110"
				/>
				<span class="hidden {wide('lg:inline')}">{n.label}</span>
			</a>
		{/each}
		<button
			onclick={() => (ui.settingsOpen = true)}
			title="Settings"
			class="group flex items-center justify-center gap-3 rounded-lg px-3 py-2 text-sm font-medium text-sidebar-foreground/70 transition-colors hover:bg-sidebar-accent/50 hover:text-sidebar-foreground {wide(
				'lg:justify-start'
			)}"
		>
			<HugeiconsIcon
				icon={Settings01Icon}
				class="h-5 w-5 shrink-0 transition-transform duration-200 group-hover:scale-110"
			/>
			<span class="hidden {wide('lg:inline')}">Settings</span>
		</button>
	</nav>

	<!-- Playlists & Folders. Hidden on the icon rail (needs labels; matches YTM's collapsed rail). flex-1 lets
	     the list fill the space and scroll. Signed out the section still appears once there is
	     something in it: On Repeat, or a playlist saved on this machine. -->
	{#if auth.account?.signedIn || playlists.length || rootFolders.length}
		<div class="mt-3 hidden min-h-0 flex-1 flex-col border-t pt-3 {wide('lg:flex')}">
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
