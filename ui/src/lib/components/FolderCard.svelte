<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Folder01Icon,
		FolderOpenIcon,
		MoreVerticalIcon,
		Edit02Icon,
		Delete02Icon
	} from '@hugeicons/core-free-icons';
	import type { PlaylistFolder } from '$lib/personal';
	import type { BrowseItem } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import { anchorMenu, ctxHost, fitMenu, NO_ANCHOR, toBody } from '$lib/menu';

	let {
		folder,
		playlists = [],
		onclick,
		onrename,
		ondelete
	}: {
		folder: PlaylistFolder;
		playlists?: BrowseItem[];
		onclick?: () => void;
		onrename?: () => void;
		ondelete?: () => void;
	} = $props();

	// Get up to 4 covers for the 2x2 mosaic preview
	const covers = $derived(
		playlists
			.map((p) => p.thumbnail)
			.filter((t): t is string => Boolean(t))
			.slice(0, 4)
	);

	let menuOpen = $state(false);
	let anchor = $state(NO_ANCHOR);

	function openMenu(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		anchor = anchorMenu(e, { align: 'right' });
		menuOpen = true;
	}

	function close(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		menuOpen = false;
	}

	function run(e: MouseEvent, action?: () => void) {
		e.stopPropagation();
		menuOpen = false;
		action?.();
	}
</script>

<div class="group relative flex w-full flex-col gap-2" data-ctx>
	<div
		class="flex flex-col text-left transition-colors hover:bg-accent/10 gap-2 rounded-xl p-2 cursor-pointer"
		role="button"
		tabindex="0"
		{onclick}
		onkeydown={(e) => {
			if (e.target !== e.currentTarget) return;
			if (e.key === 'Enter' || e.key === ' ') {
				e.preventDefault();
				onclick?.();
			}
		}}
		title="{folder.name} — {folder.playlistIds.length} playlists"
	>
		<div class="relative">
			<div
				class="pointer-events-none absolute inset-0 opacity-0 shadow-xl transition-opacity duration-300 group-hover:opacity-100 rounded-lg"
			></div>
			<div
				class="relative aspect-square w-full overflow-hidden rounded-lg bg-card/60 border border-border/60 transition-transform duration-300 ease-out group-hover:-translate-y-1 group-active:translate-y-0"
			>
				{#if covers.length >= 4}
					<div class="grid h-full w-full grid-cols-2 grid-rows-2">
						{#each covers as c, i (i)}
							<img src={thumb(c, 200)} alt="" class="h-full w-full object-cover" />
						{/each}
					</div>
				{:else if covers.length > 0}
					<div class="relative h-full w-full">
						<img src={thumb(covers[0], 400)} alt="" class="h-full w-full object-cover" />
						<div class="absolute inset-0 bg-background/30 backdrop-blur-[2px]"></div>
						<div class="absolute inset-0 flex items-center justify-center text-foreground">
							<HugeiconsIcon icon={Folder01Icon} class="h-10 w-10 drop-shadow-md" />
						</div>
					</div>
				{:else}
					<div class="flex h-full w-full flex-col items-center justify-center gap-2 text-muted-foreground/60 bg-muted/20">
						<HugeiconsIcon icon={Folder01Icon} class="h-12 w-12 stroke-[1.5]" />
					</div>
				{/if}

				<!-- Folder badge overlay -->
				<div
					class="absolute top-2 right-2 flex items-center gap-1 rounded-md bg-background/80 px-1.5 py-0.5 text-[10px] font-medium text-foreground backdrop-blur-md border border-border/40 shadow-sm"
				>
					<HugeiconsIcon icon={FolderOpenIcon} class="h-3 w-3" />
					<span>Folder</span>
				</div>
			</div>
		</div>

		<div class="min-w-0">
			<div class="truncate text-sm font-semibold leading-tight text-foreground group-hover:underline">
				{folder.name}
			</div>
			<div class="mt-0.5 truncate text-xs text-muted-foreground">
				{folder.playlistIds.length === 1 ? '1 playlist' : `${folder.playlistIds.length} playlists`}
			</div>
		</div>
	</div>

	<!-- ⋯ Context button -->
	<button
		class="absolute right-3 top-3 flex h-7 w-7 cursor-pointer items-center justify-center rounded-md bg-background/80 text-muted-foreground backdrop-blur-md opacity-0 transition hover:bg-accent hover:text-foreground focus-visible:opacity-100 group-hover:opacity-100 border border-border/40 shadow-sm"
		onclick={openMenu}
		aria-label="Folder options"
		{@attach ctxHost(openMenu)}
	>
		<HugeiconsIcon icon={MoreVerticalIcon} class="h-4 w-4" />
	</button>
</div>

{#if menuOpen}
	<button
		class="fixed inset-0 z-40 cursor-default"
		onclick={close}
		oncontextmenu={close}
		aria-label="Close menu"
		{@attach toBody}
	></button>
	<div
		class="fixed z-50 min-w-44 animate-in rounded-lg border bg-popover p-1 text-popover-foreground shadow-xl duration-150 fade-in-0 zoom-in-95"
		style={anchor.style}
		{@attach toBody}
		{@attach fitMenu(anchor)}
	>
		<button
			class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={(e) => run(e, onrename)}
		>
			<HugeiconsIcon icon={Edit02Icon} class="h-4 w-4" /> Rename folder
		</button>
		<button
			class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-destructive hover:bg-destructive/10"
			onclick={(e) => run(e, ondelete)}
		>
			<HugeiconsIcon icon={Delete02Icon} class="h-4 w-4" /> Delete folder
		</button>
	</div>
{/if}
