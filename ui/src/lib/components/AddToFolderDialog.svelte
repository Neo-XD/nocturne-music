<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Folder01Icon,
		FolderAddIcon,
		CheckmarkCircle01Icon,
		Cancel01Icon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Dialog from '$lib/components/ui/dialog';
	import type { BrowseItem } from '$lib/api';
	import {
		personal,
		addPlaylistToFolder,
		removePlaylistFromFolder,
		createPlaylistFolder,
		findPlaylistFolder,
		toast
	} from '$lib/player.svelte';

	let {
		open = $bindable(false),
		playlist = null
	}: {
		open?: boolean;
		playlist?: BrowseItem | null;
	} = $props();

	let newFolderName = $state('');
	let showNewFolderInput = $state(false);

	const currentFolder = $derived(playlist ? findPlaylistFolder(playlist.id) : undefined);
	const folders = $derived(personal.folders ?? []);

	function selectFolder(folderId: string) {
		if (!playlist) return;
		addPlaylistToFolder(folderId, playlist.id);
		const folder = folders.find((f) => f.id === folderId);
		toast.success(`Moved "${playlist.title}" to ${folder?.name ?? 'folder'}`);
		open = false;
	}

	function removeFromFolder() {
		if (!playlist || !currentFolder) return;
		removePlaylistFromFolder(currentFolder.id, playlist.id);
		toast.success(`Removed "${playlist.title}" from folder`);
		open = false;
	}

	function handleCreateFolder() {
		const name = newFolderName.trim();
		if (!name || !playlist) return;
		const folder = createPlaylistFolder(name);
		addPlaylistToFolder(folder.id, playlist.id);
		toast.success(`Created "${name}" and moved "${playlist.title}"`);
		newFolderName = '';
		showNewFolderInput = false;
		open = false;
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Organize in Folder</Dialog.Title>
			<Dialog.Description>
				{#if playlist}
					Choose a folder for <span class="font-medium text-foreground">"{playlist.title}"</span>.
				{:else}
					Choose a folder for this playlist.
				{/if}
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex flex-col gap-2 py-2 max-h-72 overflow-y-auto">
			{#if currentFolder}
				<button
					class="flex items-center justify-between rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2.5 text-left text-sm text-destructive transition hover:bg-destructive/10 cursor-pointer"
					onclick={removeFromFolder}
				>
					<span class="flex items-center gap-2.5">
						<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4 shrink-0" />
						<span>Remove from "{currentFolder.name}"</span>
					</span>
				</button>
			{/if}

			{#if folders.length === 0 && !showNewFolderInput}
				<div class="py-4 text-center text-sm text-muted-foreground">
					No folders created yet. Create your first folder to organize your playlists!
				</div>
			{/if}

			{#each folders as f (f.id)}
				{@const isSelected = currentFolder?.id === f.id}
				<button
					class="flex items-center justify-between rounded-lg border px-3 py-2.5 text-left text-sm transition cursor-pointer {isSelected
						? 'border-primary bg-primary/10 text-primary font-medium'
						: 'border-border/60 hover:bg-accent/40 text-foreground'}"
					onclick={() => selectFolder(f.id)}
				>
					<span class="flex items-center gap-2.5 min-w-0">
						<HugeiconsIcon icon={Folder01Icon} class="h-4 w-4 shrink-0 text-muted-foreground" />
						<span class="truncate">{f.name}</span>
						<span class="text-xs text-muted-foreground shrink-0">({f.playlistIds.length})</span>
					</span>
					{#if isSelected}
						<HugeiconsIcon icon={CheckmarkCircle01Icon} class="h-4 w-4 text-primary shrink-0" />
					{/if}
				</button>
			{/each}

			{#if showNewFolderInput}
				<form
					class="mt-2 flex gap-2"
					onsubmit={(e) => {
						e.preventDefault();
						handleCreateFolder();
					}}
				>
					<Input
						bind:value={newFolderName}
						placeholder="Folder name"
						autofocus
						class="h-9 flex-1 text-sm"
					/>
					<Button type="submit" size="sm" disabled={!newFolderName.trim()}>Create</Button>
					<Button
						type="button"
						variant="ghost"
						size="sm"
						onclick={() => {
							showNewFolderInput = false;
							newFolderName = '';
						}}
					>
						Cancel
					</Button>
				</form>
			{/if}
		</div>

		<Dialog.Footer class="flex sm:justify-between items-center gap-2 pt-2">
			{#if !showNewFolderInput}
				<Button
					type="button"
					variant="outline"
					size="sm"
					class="gap-1.5"
					onclick={() => (showNewFolderInput = true)}
				>
					<HugeiconsIcon icon={FolderAddIcon} class="h-4 w-4" /> New folder
				</Button>
			{:else}
				<div></div>
			{/if}
			<Button type="button" variant="ghost" size="sm" onclick={() => (open = false)}>Close</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
