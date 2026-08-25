<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { beforeNavigate } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Maximize01Icon, Minimize01Icon, Cancel01Icon } from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import LyricsView from './LyricsView.svelte';
	import { ui } from '$lib/player.svelte';

	let { onClose, queueOpen = false }: { onClose: () => void; queueOpen?: boolean } = $props();

	let expanded = $state(false);

	// Expanded, the panel covers the page — so navigating anywhere means the user wants to see that
	// page, not the lyrics. The docked panel sits beside the content, so it stays put.
	beforeNavigate(() => {
		if (expanded) onClose();
	});
</script>

<!-- Below lg: Backdrop scrim dismisses the panel -->
<button
	class="fixed inset-0 z-40 cursor-default bg-black/40 lg:hidden"
	onclick={onClose}
	aria-label="Close lyrics"
	transition:fade={{ duration: 150 }}
></button>

<aside
	transition:fly={{ x: 32, duration: 220, easing: cubicOut }}
	class={expanded
		? `fixed inset-y-0 left-16 right-0 z-40 flex h-full flex-col border-l bg-card shadow-2xl ${ui.sidebarCollapsed ? '' : 'lg:left-60'} ${queueOpen ? 'lg:right-80' : ''}`
		: `fixed inset-y-0 right-0 z-40 flex h-full w-80 max-w-[85vw] flex-col border-l bg-card shadow-2xl lg:relative lg:inset-auto lg:z-10 lg:w-80 lg:shrink-0 lg:shadow-none`}
>
	<div class="flex items-center justify-between border-b px-4 py-3">
		<h2 class="font-heading text-sm font-semibold">Lyrics</h2>
		<div class="flex items-center gap-1">
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={() => (expanded = !expanded)}
				aria-label={expanded ? 'Shrink lyrics' : 'Expand lyrics'}
				class="hover:text-foreground"
			>
				<HugeiconsIcon
					icon={Maximize01Icon}
					altIcon={Minimize01Icon}
					showAlt={expanded}
					class="h-4 w-4"
				/>
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={onClose}
				aria-label="Close lyrics"
				class="hover:text-foreground"
			>
				<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4" />
			</Button>
		</div>
	</div>
	<LyricsView {expanded} />
</aside>
