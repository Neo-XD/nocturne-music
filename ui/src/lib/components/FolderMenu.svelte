<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		MoreHorizontalIcon,
		MoreVerticalIcon,
		FolderAddIcon,
		Add01Icon,
		Edit02Icon,
		Delete02Icon,
		ArrowUp01Icon
	} from '@hugeicons/core-free-icons';
	import type { PlaylistFolder } from '$lib/personal';
	import { anchorMenu, ctxHost, fitMenu, NO_ANCHOR, toBody } from '$lib/menu';
	import {
		deletePlaylistFolder,
		moveFolderToFolder,
		toast
	} from '$lib/player.svelte';

	let {
		folder,
		vertical = false,
		iconClass = 'h-4 w-4',
		triggerClass = 'flex h-7 w-7 cursor-pointer items-center justify-center rounded-md text-muted-foreground opacity-0 transition hover:bg-sidebar-accent hover:text-foreground focus-visible:opacity-100 group-hover/folder:opacity-100',
		onNewPlaylist,
		onNewSubfolder,
		onRename
	}: {
		folder: PlaylistFolder;
		vertical?: boolean;
		iconClass?: string;
		triggerClass?: string;
		onNewPlaylist?: (folderId: string) => void;
		onNewSubfolder?: (parentFolderId: string) => void;
		onRename?: (folder: PlaylistFolder) => void;
	} = $props();

	let open = $state(false);
	let anchor = $state(NO_ANCHOR);

	function openMenu(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		anchor = anchorMenu(e, { align: 'right' });
		open = true;
	}

	function close(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		open = false;
	}

	function run(e: MouseEvent, action: () => void) {
		e.stopPropagation();
		open = false;
		action();
	}

	function handleDelete() {
		deletePlaylistFolder(folder.id);
		toast.success(`Deleted folder "${folder.name}"`);
	}

	function handleMoveToRoot() {
		moveFolderToFolder(folder.id, null);
		toast.success(`Moved "${folder.name}" to top level`);
	}
</script>

<button
	class={triggerClass}
	onclick={openMenu}
	aria-label="Folder options for {folder.name}"
	aria-expanded={open}
	{@attach ctxHost(openMenu)}
>
	<HugeiconsIcon icon={vertical ? MoreVerticalIcon : MoreHorizontalIcon} class={iconClass} />
</button>

{#if open}
	<button
		class="fixed inset-0 z-40 cursor-default"
		onclick={close}
		oncontextmenu={close}
		aria-label="Close menu"
		{@attach toBody}
	></button>
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fixed z-50 min-w-48 animate-in rounded-lg border border-border/80 bg-popover/85 p-1 text-popover-foreground shadow-2xl backdrop-blur-2xl duration-150 fade-in-0 zoom-in-95"
		style={anchor.style}
		{@attach toBody}
		{@attach fitMenu(anchor)}
		oncontextmenu={(e) => e.preventDefault()}
		onkeydown={(e) => e.key === 'Escape' && (open = false)}
	>
		{#if onNewPlaylist}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => onNewPlaylist(folder.id))}
			>
				<HugeiconsIcon icon={Add01Icon} class="h-4 w-4" /> New playlist inside
			</button>
		{/if}
		{#if onNewSubfolder}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => onNewSubfolder(folder.id))}
			>
				<HugeiconsIcon icon={FolderAddIcon} class="h-4 w-4" /> New subfolder
			</button>
		{/if}
		{#if onRename}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => onRename(folder))}
			>
				<HugeiconsIcon icon={Edit02Icon} class="h-4 w-4" /> Rename folder
			</button>
		{/if}
		{#if folder.parentId}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, handleMoveToRoot)}
			>
				<HugeiconsIcon icon={ArrowUp01Icon} class="h-4 w-4" /> Move to top level
			</button>
		{/if}
		<button
			class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-destructive hover:bg-destructive/10"
			onclick={(e) => run(e, handleDelete)}
		>
			<HugeiconsIcon icon={Delete02Icon} class="h-4 w-4" /> Delete folder
		</button>
	</div>
{/if}
