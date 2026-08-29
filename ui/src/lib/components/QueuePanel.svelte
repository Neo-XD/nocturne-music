<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Cancel01Icon, PlayListAddIcon } from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { playback, openAddToPlaylist, toast } from '$lib/player.svelte';
	import QueueList from './QueueList.svelte';

	let { onClose }: { onClose: () => void } = $props();

	function saveQueue() {
		const items = playback.queue.items;
		if (!items.length) {
			toast('Queue is empty');
			return;
		}
		openAddToPlaylist(items);
	}
</script>

<!-- Below lg: Backdrop scrim dismisses the panel -->
<button
	class="fixed inset-0 z-30 cursor-default bg-black/20 backdrop-blur-xs lg:hidden"
	onclick={onClose}
	aria-label="Close queue"
	transition:fade={{ duration: 150 }}
></button>

<!-- Docked in-flow sidebar on lg+, overlay on smaller screens -->
<aside
	transition:fly={{ x: 32, duration: 220, easing: cubicOut }}
	class="fixed inset-y-0 right-0 z-40 flex h-full w-80 max-w-[85vw] flex-col border-l border-border/70 bg-card/85 dark:bg-card/80 backdrop-blur-2xl shadow-2xl lg:relative lg:inset-auto lg:z-10 lg:w-80 lg:shrink-0 lg:shadow-none"
>
	<div class="flex items-center justify-between border-b px-4 py-3">
		<div class="flex items-center gap-2">
			<h2 class="font-heading text-sm font-semibold">Queue</h2>
			<span class="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
				{playback.queue.items.length}
			</span>
		</div>
		<div class="flex items-center gap-1">
			<Button
				variant="ghost"
				size="sm"
				class="h-7 gap-1 px-2 text-xs text-muted-foreground hover:text-foreground cursor-pointer"
				onclick={saveQueue}
				title="Save queue to a playlist"
			>
				<HugeiconsIcon icon={PlayListAddIcon} class="h-3.5 w-3.5" />
				<span>Save</span>
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={onClose}
				aria-label="Close queue"
				class="hover:text-foreground cursor-pointer"
			>
				<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4" />
			</Button>
		</div>
	</div>
	<QueueList />
</aside>
