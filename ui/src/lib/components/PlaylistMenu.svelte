<script lang="ts">
	// The ⋯ menu on a sidebar library row. Positioned `fixed` and moved to <body> like TrackMenu:
	// the playlist list is a scroll container, so an absolute popup would be clipped by it.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		MoreHorizontalIcon,
		MoreVerticalIcon,
		PinIcon,
		PinOffIcon,
		Radio02Icon,
		ArrowUpNarrowWideIcon,
		ArrowDownWideNarrowIcon,
		BookmarkMinus02Icon,
		DashboardSquare02Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { BrowseItem } from '$lib/api';
	import { enqueueItem } from '$lib/browse';
	import { anchorMenu, toBody } from '$lib/menu';
	import {
		addPick,
		auth,
		isSaved,
		isSynced,
		personal,
		startRadio,
		toast,
		togglePin,
		toggleSaved
	} from '$lib/player.svelte';

	let {
		item,
		showPin = true,
		vertical = false,
		iconClass = 'h-4 w-4',
		// Visibility lives here too: most triggers only appear on hover, but a row that has nothing
		// else to reveal on hover shows its ⋯ all the time.
		triggerClass = 'absolute right-1 top-1/2 flex h-7 w-7 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md text-muted-foreground opacity-0 transition hover:bg-sidebar-accent hover:text-foreground focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring group-hover/row:opacity-100'
	}: {
		item: BrowseItem;
		showPin?: boolean;
		vertical?: boolean;
		iconClass?: string;
		triggerClass?: string;
	} = $props();

	const pinned = $derived(personal.pins.includes(item.id));
	// A synced row is on the account too, and dropping only the local copy would leave the card on
	// screen with a "removed" toast under it. Signed out, the local copy is the whole library again.
	const savedHere = $derived(
		isSaved(item.id) && !(auth.account?.signedIn && isSynced(item.id))
	);
	// Radio needs a YouTube item behind it: local folders and the locally-built On Repeat have none.
	const hasRadio = $derived(!api.isLocalId(item.id) && item.id !== api.ON_REPEAT_ID);
	// An artist isn't a track list — there's nothing unambiguous to queue. Songs, albums and
	// playlists (local ones included) all are.
	const canQueue = $derived(item.kind === 'song' || item.kind === 'album' || item.kind === 'playlist');

	// The tracks have to be fetched before anything can be queued, so the menu stays open and the
	// row shows it's working. Guards a second click from queueing the album twice.
	let queueing = $state(false);
	async function queue(next: boolean) {
		if (queueing) return;
		queueing = true;
		try {
			await enqueueItem(item, next);
			menuOpen = false;
		} finally {
			queueing = false;
		}
	}

	let menuOpen = $state(false);
	let mx = $state(0);
	let my = $state(0);
	let openUp = $state(false);

	function openMenu(e: MouseEvent) {
		e.stopPropagation();
		({ right: mx, y: my, openUp } = anchorMenu(e.currentTarget as HTMLElement, 130));
		menuOpen = true;
	}
	// stopPropagation everywhere: the trigger sits over a clickable host (a card's whole surface is a
	// play/navigate target), so its click must not reach the host's handler. The popup itself now
	// lives at <body> and no longer bubbles into the host, but these stay: the trigger needs them.
	function run(e: MouseEvent, action?: () => void) {
		e.stopPropagation();
		menuOpen = false;
		action?.();
	}
	function close(e: MouseEvent) {
		e.stopPropagation();
		menuOpen = false;
	}
</script>

<button
	class="{triggerClass} {menuOpen ? 'opacity-100' : ''}"
	onclick={openMenu}
	aria-label="Playlist options"
>
	<!-- icon swap via altIcon/showAlt — `icon` is frozen at mount -->
	<HugeiconsIcon
		icon={MoreHorizontalIcon}
		altIcon={MoreVerticalIcon}
		showAlt={vertical}
		class={iconClass}
	/>
</button>

{#if menuOpen}
	<button
		class="fixed inset-0 z-40 cursor-default"
		onclick={close}
		aria-label="Close menu"
		{@attach toBody}
	></button>
	<div
		class="fixed z-50 min-w-48 animate-in rounded-lg border bg-popover p-1 text-popover-foreground shadow-xl duration-150 fade-in-0 zoom-in-95 {openUp
			? 'origin-bottom-right'
			: 'origin-top-right'}"
		style="right:{mx}px; {openUp ? 'bottom' : 'top'}:{my}px;"
		{@attach toBody}
	>
		{#if showPin}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => togglePin(item.id))}
			>
				<HugeiconsIcon icon={pinned ? PinOffIcon : PinIcon} class="h-4 w-4" />
				{pinned ? 'Unpin' : 'Pin to top'}
			</button>
		{/if}
		{#if canQueue}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10 disabled:opacity-50"
				disabled={queueing}
				onclick={(e) => {
					e.stopPropagation();
					queue(true);
				}}
			>
				<HugeiconsIcon icon={ArrowUpNarrowWideIcon} class="h-4 w-4" /> Play next
			</button>
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10 disabled:opacity-50"
				disabled={queueing}
				onclick={(e) => {
					e.stopPropagation();
					queue(false);
				}}
			>
				<HugeiconsIcon icon={ArrowDownWideNarrowIcon} class="h-4 w-4" /> Add to queue
			</button>
		{/if}
		{#if hasRadio}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => startRadio(item.kind as 'artist' | 'album' | 'playlist', item.id, item.title))}
			>
				<HugeiconsIcon icon={Radio02Icon} class="h-4 w-4" /> Start radio
			</button>
		{/if}
		<button
			class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={(e) => run(e, () => addPick(item))}
		>
			<HugeiconsIcon icon={DashboardSquare02Icon} class="h-4 w-4" /> Add to shortcuts
		</button>
		<!-- Only for cards saved on this machine: YouTube's own library rows are unsaved from their
		     page, where the button knows which write action to send. -->
		{#if savedHere}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) =>
					run(e, () => {
						toggleSaved(item);
						toast.success('Removed from library');
					})}
			>
				<HugeiconsIcon icon={BookmarkMinus02Icon} class="h-4 w-4" /> Remove from library
			</button>
		{/if}
	</div>
{/if}
