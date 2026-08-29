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
		FolderAddIcon,
		ComputerIcon
	} from '@hugeicons/core-free-icons';
	import { toggleMode } from 'mode-watcher';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Dialog from '$lib/components/ui/dialog';
	import { ON_REPEAT_ID, type BrowseItem } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import PlaylistMenu from './PlaylistMenu.svelte';
	import {
		auth,
		library,
		personal,
		ui,
		np,
		toggleDevicesSidebar,
		createLibraryPlaylist,
		createPlaylistFolder,
		toggleSidebar,
		toast
	} from '$lib/player.svelte';
	import { mergeSaved, orderLibrary } from '$lib/personal';

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

	// New-playlist dialog (mirrors the Library page).
	let dialogOpen = $state(false);
	let newTitle = $state('');
	let creating = $state(false);
	async function createNew() {
		const title = newTitle.trim();
		if (!title || creating) return;
		creating = true;
		try {
			await createLibraryPlaylist(title);
			toast.success(`Created "${title}"`);
			newTitle = '';
			dialogOpen = false;
		} catch (e) {
			toast.error(String(e));
		} finally {
			creating = false;
		}
	}

	let folderDialogOpen = $state(false);
	let newFolderName = $state('');
	function handleCreateFolder() {
		const name = newFolderName.trim();
		if (!name) return;
		const f = createPlaylistFolder(name);
		toast.success(`Created folder "${f.name}"`);
		newFolderName = '';
		folderDialogOpen = false;
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
	class="flex h-full w-16 shrink-0 flex-col border-r bg-sidebar p-3 text-sidebar-foreground {wide(
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
		<button
			onclick={toggleDevicesSidebar}
			title="Connected Devices"
			class="group flex items-center justify-center gap-3 rounded-lg px-3 py-2 text-sm font-medium {np.devicesOpen
				? 'bg-primary/10 text-primary'
				: 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'} transition-colors {wide(
				'lg:justify-start'
			)}"
		>
			<HugeiconsIcon
				icon={ComputerIcon}
				class="h-5 w-5 shrink-0 transition-transform duration-200 group-hover:scale-110"
			/>
			<span class="hidden {wide('lg:inline')}">Devices</span>
		</button>
	</nav>

	<!-- Playlists. Hidden on the icon rail (needs labels; matches YTM's collapsed rail). flex-1 lets
	     the list fill the space and scroll. Signed out the section still appears once there is
	     something in it: On Repeat, or a playlist saved on this machine. -->
	{#if auth.account?.signedIn || playlists.length}
		<div class="mt-3 hidden min-h-0 flex-1 flex-col border-t pt-3 {wide('lg:flex')}">
			<!-- Creating one is a YouTube write action, so it needs an account. -->
			<div class="mb-2 flex items-center gap-1.5">
				{#if auth.account?.signedIn}
					<Button
						variant="outline"
						size="sm"
						class="h-8 flex-1 gap-1.5 text-xs cursor-pointer"
						onclick={() => (dialogOpen = true)}
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
						folderDialogOpen = true;
					}}
					title="New folder"
				>
					<HugeiconsIcon icon={FolderAddIcon} class="h-4 w-4" />
				</Button>
			</div>
			<div class="min-h-0 flex-1 overflow-y-auto">
				{#each playlists as pl, i (pl.id)}
					<!-- The ⋯ is a sibling of the link, not a child: a <button> inside an <a> is invalid
					     HTML. pr-9 keeps the title clear of the button that overlays the row on hover. -->
					<div class="group/row relative" data-ctx>
						<a
							href={playlistHref(pl)}
							title={pl.title}
							class="flex items-center gap-2.5 rounded-lg py-1.5 pl-2 pr-9 transition-colors hover:bg-sidebar-accent/50"
						>
							<div
								class="relative h-10 w-10 shrink-0 overflow-hidden bg-muted {pl.kind === 'artist'
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
										<!-- altIcon/showAlt, not a ternary: `icon` is read once at mount. -->
										<HugeiconsIcon
											icon={MusicNote01Icon}
											altIcon={ListRestartIcon}
											showAlt={pl.id === ON_REPEAT_ID}
											class={pl.id === ON_REPEAT_ID ? 'h-5 w-5' : 'h-4 w-4'}
										/>
									</div>
								{/if}
							</div>
							{#if personal.pins.includes(pl.id)}
								<span
									class="absolute left-9 top-0.5 flex h-4 w-4 items-center justify-center rounded-full bg-primary text-primary-foreground shadow"
								>
									<HugeiconsIcon icon={PinIcon} class="h-2.5 w-2.5" />
								</span>
							{/if}
							<div class="min-w-0 flex-1">
								<div class="truncate text-[13px] font-medium">{pl.title}</div>
								{#if pl.subtitle}
									<div class="truncate text-xs text-muted-foreground">{rowSubtitle(pl.subtitle)}</div>
								{/if}
							</div>
						</a>
						<PlaylistMenu item={pl} />
					</div>
					{#if pinnedCount && i === pinnedCount - 1}
						<div class="mx-3 my-1.5 h-px bg-border"></div>
					{/if}
				{:else}
					{#if library.loading}
						<p class="px-3 py-1.5 text-xs text-muted-foreground">Loading…</p>
					{/if}
				{/each}
			</div>
		</div>

		<Dialog.Root bind:open={dialogOpen}>
			<Dialog.Content class="sm:max-w-md">
				<Dialog.Header>
					<Dialog.Title>New playlist</Dialog.Title>
					<Dialog.Description>Give your playlist a name to get started.</Dialog.Description>
				</Dialog.Header>
				<form
					class="flex flex-col gap-4"
					onsubmit={(e) => {
						e.preventDefault();
						createNew();
					}}
				>
					<Input bind:value={newTitle} placeholder="Playlist name" autofocus />
					<Dialog.Footer>
						<Button type="button" variant="outline" onclick={() => (dialogOpen = false)}>Cancel</Button>
						<Button type="submit" disabled={creating || !newTitle.trim()}>
							{creating ? 'Creating…' : 'Create'}
						</Button>
					</Dialog.Footer>
				</form>
			</Dialog.Content>
		</Dialog.Root>

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
	{/if}

</aside>
