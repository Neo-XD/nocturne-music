<script lang="ts">
	// What the keyboard can do. Displays dynamic user keybindings and zoom controls.
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { formatKey, keybindings, MOD } from '$lib/shortcuts.svelte';
	import { ui } from '$lib/player.svelte';

	const groups = $derived([
		{
			title: 'Playback',
			rows: [
				['Play or pause', `${formatKey(keybindings.playPause)} or ;`],
				['Next song', formatKey(keybindings.nextTrack)],
				['Previous song', formatKey(keybindings.prevTrack)],
				['Shuffle queue', formatKey(keybindings.shuffle)],
				['Toggle repeat', formatKey(keybindings.repeat)],
				['Mute or unmute', formatKey(keybindings.mute)],
				['Volume up', formatKey(keybindings.volumeUp)],
				['Volume down', formatKey(keybindings.volumeDown)],
				['Toggle fullscreen player', formatKey(keybindings.fullscreen)]
			]
		},
		{
			title: 'General & Zoom',
			rows: [
				['Search & command palette', formatKey(keybindings.search)],
				['Toggle now playing panel', formatKey(keybindings.nowPlaying)],
				['Show this list', formatKey(keybindings.shortcutsList)],
				['Zoom in', `${MOD}+`],
				['Zoom out', `${MOD}-`],
				['Reset zoom', `${MOD}0`]
			]
		}
	]);

	function openSettingsKeybinds() {
		ui.shortcutsOpen = false;
		ui.settingsOpen = true;
	}
</script>

<Dialog.Root bind:open={ui.shortcutsOpen}>
	<Dialog.Content class="sm:max-w-2xl">
		<Dialog.Header>
			<div class="flex items-center justify-between pr-6">
				<div>
					<Dialog.Title>Keyboard shortcuts</Dialog.Title>
					<Dialog.Description>{formatKey(keybindings.shortcutsList)} brings this back at any time.</Dialog.Description>
				</div>
				<Button
					variant="outline"
					size="sm"
					class="text-xs font-medium cursor-pointer"
					onclick={openSettingsKeybinds}
				>
					Edit keybinds
				</Button>
			</div>
		</Dialog.Header>
		<!-- Two columns that flow, so adding a row never means rebalancing the layout by hand. -->
		<div class="gap-x-10 sm:columns-2">
			{#each groups as group (group.title)}
				<section class="mb-6 break-inside-avoid">
					<h3 class="mb-2 text-base font-semibold">{group.title}</h3>
					<dl>
						{#each group.rows as [what, keys] (what)}
							<div class="grid grid-cols-2 items-center gap-4 border-b py-2 last:border-0">
								<dt class="text-sm text-muted-foreground">{what}</dt>
								<dd class="font-mono text-xs font-medium">{keys}</dd>
							</div>
						{/each}
					</dl>
				</section>
			{/each}
		</div>
	</Dialog.Content>
</Dialog.Root>
