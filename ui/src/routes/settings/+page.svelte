<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { ui } from '$lib/player.svelte';

	onMount(() => {
		ui.settingsOpen = true;
		goto('/', { replaceState: true });
	});
</script>

<div class="flex h-full w-full items-center justify-center p-8">
	<p class="text-sm text-muted-foreground">Opening settings...</p>
</div>
