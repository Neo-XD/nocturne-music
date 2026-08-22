<script lang="ts">
	// Ctrl+H: what the keyboard can do. Nothing in the chrome points at the shortcuts, so this is
	// where they are discoverable. It documents the zoom keys too (zoom.ts owns those) — from the
	// outside they are the same feature, and a list that only covers half of them is worse than none.
	import * as Dialog from '$lib/components/ui/dialog';
	import { MOD } from '$lib/shortcuts';
	import { ui } from '$lib/player.svelte';

	const GROUPS: { title: string; rows: [string, string][] }[] = [
		{ title: 'Search', rows: [[`${MOD}K`, 'Search from anywhere']] },
		{
			title: 'Playback',
			rows: [
				[`${MOD}E`, 'Show or hide the now-playing view'],
				[`${MOD}>`, 'Volume up'],
				[`${MOD}<`, 'Volume down']
			]
		},
		{
			title: 'Window',
			rows: [
				[`${MOD}+`, 'Zoom in'],
				[`${MOD}-`, 'Zoom out'],
				[`${MOD}0`, 'Reset zoom'],
				[`${MOD}H`, 'Show this list']
			]
		}
	];
</script>

<Dialog.Root bind:open={ui.shortcutsOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Keyboard shortcuts</Dialog.Title>
			<Dialog.Description>{MOD}H brings this back at any time.</Dialog.Description>
		</Dialog.Header>
		<div class="flex flex-col gap-5">
			{#each GROUPS as group (group.title)}
				<section>
					<h3 class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
						{group.title}
					</h3>
					<dl class="flex flex-col gap-1.5">
						{#each group.rows as [keys, what] (keys)}
							<div class="flex items-center justify-between gap-4">
								<dt class="min-w-0 truncate text-sm">{what}</dt>
								<dd>
									<kbd
										class="rounded border bg-muted px-1.5 py-0.5 font-mono text-[0.6875rem] font-medium tracking-wide text-muted-foreground"
									>
										{keys}
									</kbd>
								</dd>
							</div>
						{/each}
					</dl>
				</section>
			{/each}
		</div>
	</Dialog.Content>
</Dialog.Root>
