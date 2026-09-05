<script lang="ts">
	import { open as pickFile } from '@tauri-apps/plugin-dialog';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ImageAdd02Icon, Delete02Icon } from '@hugeicons/core-free-icons';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Switch } from '$lib/components/ui/switch';
	import { createLibraryPlaylist, toast } from '$lib/player.svelte';

	let {
		open = $bindable(false),
		onCreated
	}: {
		open: boolean;
		onCreated?: (playlistId: string) => void;
	} = $props();

	let title = $state('');
	let description = $state('');
	let isPublic = $state(false);
	let coverPath = $state<string | null>(null);
	let coverPreview = $state<string | null>(null);
	let creating = $state(false);

	$effect(() => {
		if (!open) {
			title = '';
			description = '';
			isPublic = false;
			coverPath = null;
			coverPreview = null;
			creating = false;
		}
	});

	async function pickCover() {
		try {
			const picked = await pickFile({
				multiple: false,
				title: 'Choose playlist cover image',
				filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png'] }]
			});
			if (typeof picked === 'string') {
				coverPath = picked;
				coverPreview = picked.startsWith('http')
					? picked
					: `https://asset.localhost/${encodeURIComponent(picked).replace(/%2F/g, '/').replace(/%5C/g, '/')}`;
			}
		} catch (e) {
			toast.error(String(e));
		}
	}

	function removeCover() {
		coverPath = null;
		coverPreview = null;
	}

	async function handleCreate() {
		const trimmedTitle = title.trim();
		if (!trimmedTitle || creating) return;
		creating = true;
		try {
			const id = await createLibraryPlaylist(
				trimmedTitle,
				description.trim() || undefined,
				isPublic,
				coverPath
			);
			toast.success(`Created "${trimmedTitle}"`);
			open = false;
			onCreated?.(id);
		} catch (e) {
			toast.error(String(e));
		} finally {
			creating = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-xl">
		<Dialog.Header>
			<Dialog.Title>New playlist</Dialog.Title>
			<Dialog.Description>Create a playlist with custom details, cover, and visibility.</Dialog.Description>
		</Dialog.Header>
		<form
			class="flex flex-col gap-4"
			onsubmit={(e) => {
				e.preventDefault();
				handleCreate();
			}}
		>
			<div class="flex gap-4">
				<div class="flex shrink-0 flex-col items-center gap-1.5">
					<button
						type="button"
						class="group relative h-32 w-32 cursor-pointer overflow-hidden rounded-xl border border-border/80 bg-muted/40 transition hover:border-primary/50"
						onclick={pickCover}
						aria-label="Choose playlist artwork"
					>
						{#if coverPreview}
							<img src={coverPreview} alt="" class="h-full w-full object-cover" />
						{/if}
						<span
							class="absolute inset-0 flex flex-col items-center justify-center gap-1 bg-black/60 text-xs font-medium text-white transition group-hover:opacity-100 group-focus-visible:opacity-100 {coverPreview
								? 'opacity-0'
								: 'opacity-100'}"
						>
							<HugeiconsIcon icon={ImageAdd02Icon} class="h-6 w-6" />
							Choose image
						</span>
					</button>
					{#if coverPath}
						<Button
							type="button"
							variant="ghost"
							size="sm"
							class="gap-1.5 text-xs text-muted-foreground hover:text-destructive cursor-pointer"
							onclick={removeCover}
						>
							<HugeiconsIcon icon={Delete02Icon} class="h-3.5 w-3.5" />
							Remove
						</Button>
					{/if}
				</div>

				<div class="flex min-w-0 flex-1 flex-col gap-3">
					<Input
						bind:value={title}
						placeholder="Playlist name (required)"
						aria-label="Playlist name"
						autofocus
					/>
					<textarea
						bind:value={description}
						placeholder="Description (optional)"
						aria-label="Playlist description"
						rows="4"
						class="w-full flex-1 resize-none rounded-2xl border border-input bg-input/30 px-3 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
					></textarea>
				</div>
			</div>

			<div class="flex items-center justify-between gap-4 rounded-2xl border px-3 py-2.5 bg-muted/20">
				<div class="min-w-0">
					<div class="text-sm font-medium">Public</div>
					<p class="text-xs text-muted-foreground">
						{isPublic
							? 'Anyone can find and listen to this playlist on YouTube Music.'
							: 'Only you can see this playlist.'}
					</p>
				</div>
				<Switch bind:checked={isPublic} aria-label="Public playlist" />
			</div>

			<Dialog.Footer>
				<Button type="button" variant="outline" onclick={() => (open = false)}>Cancel</Button>
				<Button type="submit" disabled={creating || !title.trim()}>
					{creating ? 'Creating…' : 'Create'}
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
