<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { beforeNavigate } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Maximize01Icon, Minimize01Icon, Cancel01Icon } from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import LyricsView from './LyricsView.svelte';
	import { ui, prefs } from '$lib/player.svelte';

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
	class="fixed inset-0 z-30 cursor-default bg-black/20 backdrop-blur-xs lg:hidden"
	onclick={onClose}
	aria-label="Close lyrics"
	transition:fade={{ duration: 150 }}
></button>

<aside
	style={!expanded ? (prefs.floatingSidebarRight ? 'height: calc(100% - 1rem);' : 'height: 100%;') : ''}
	class={expanded
		? 'fixed inset-0 z-50 flex h-full flex-col border-l border-border/70 bg-card/85 dark:bg-card/80 backdrop-blur-2xl shadow-2xl'
		: `relative flex w-full max-w-[85vw] shrink-0 flex-col transition-[border-radius,margin] duration-200 ${prefs.floatingSidebarRight
			? 'app-floating-panel m-2 rounded-2xl border border-border/70 bg-card/90 shadow-2xl backdrop-blur-xl'
			: 'border-l border-border/70 bg-card/85 dark:bg-card/80 backdrop-blur-2xl shadow-2xl rounded-none m-0'}`}
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
