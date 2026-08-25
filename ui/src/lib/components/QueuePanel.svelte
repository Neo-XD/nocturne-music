<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Cancel01Icon } from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import QueueList from './QueueList.svelte';

	let { onClose }: { onClose: () => void } = $props();
</script>

<!-- Below lg: Backdrop scrim dismisses the panel -->
<button
	class="fixed inset-0 z-40 cursor-default bg-black/40 lg:hidden"
	onclick={onClose}
	aria-label="Close queue"
	transition:fade={{ duration: 150 }}
></button>

<!-- Docked in-flow sidebar on lg+, overlay on smaller screens -->
<aside
	transition:fly={{ x: 32, duration: 220, easing: cubicOut }}
	class="fixed inset-y-0 right-0 z-40 flex h-full w-80 max-w-[85vw] flex-col border-l bg-card shadow-2xl lg:relative lg:inset-auto lg:z-10 lg:w-80 lg:shrink-0 lg:shadow-none"
>
	<div class="flex items-center justify-between border-b px-4 py-3">
		<h2 class="font-heading text-sm font-semibold">Queue</h2>
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={onClose}
			aria-label="Close queue"
			class="hover:text-foreground"
		>
			<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4" />
		</Button>
	</div>
	<QueueList />
</aside>
