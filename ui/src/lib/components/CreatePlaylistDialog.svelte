<script lang="ts">
	import { open as pickFile } from '@tauri-apps/plugin-dialog';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ImageAdd02Icon, Delete02Icon } from '@hugeicons/core-free-icons';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Switch } from '$lib/components/ui/switch';
	import { auth, createLibraryPlaylist, spotify, toast } from '$lib/player.svelte';

	let {
		open = $bindable(false),
		initialPlatform,
		onCreated
	}: {
		open: boolean;
		initialPlatform?: 'ytm' | 'spotify';
		onCreated?: (playlistId: string) => void;
	} = $props();

	let defaultPlatform = $derived<'ytm' | 'spotify'>(
		initialPlatform ?? (!auth.account?.signedIn && spotify.status.linked ? 'spotify' : 'ytm')
	);
	let platform = $state<'ytm' | 'spotify'>('ytm');
	let title = $state('');
	let description = $state('');
	let isPublic = $state(false);
	let coverPath = $state<string | null>(null);
	let coverPreview = $state<string | null>(null);
	let creating = $state(false);

	$effect(() => {
		if (open) {
			platform = defaultPlatform;
		} else {
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
				platform === 'ytm' ? coverPath : null,
				platform
			);
			toast.success(`Created "${trimmedTitle}"${platform === 'spotify' ? ' on Spotify' : ''}`);
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
			{#if spotify.status.linked && auth.account?.signedIn}
				<div class="flex items-center gap-2">
					<span class="text-xs font-medium text-muted-foreground">Platform:</span>
					<div class="inline-flex rounded-lg bg-muted/60 p-0.5 text-xs">
						<button
							type="button"
							class="px-2.5 py-1 rounded-md font-medium transition cursor-pointer {platform === 'ytm'
								? 'bg-background shadow-xs text-foreground'
								: 'text-muted-foreground hover:text-foreground'}"
							onclick={() => (platform = 'ytm')}
						>
							YouTube Music
						</button>
						<button
							type="button"
							class="px-2.5 py-1 rounded-md font-medium transition cursor-pointer {platform === 'spotify'
								? 'bg-[#1ed760] text-black font-semibold shadow-xs'
								: 'text-muted-foreground hover:text-foreground'}"
							onclick={() => {
								platform = 'spotify';
								coverPath = null;
								coverPreview = null;
							}}
						>
							Spotify
						</button>
					</div>
				</div>
			{:else if spotify.status.linked && !auth.account?.signedIn}
				<div class="flex items-center gap-2 text-xs text-muted-foreground">
					<span>Platform:</span>
					<span class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-[#1ed760]/15 text-[#1ed760] font-semibold">
						<svg class="h-3 w-3" viewBox="0 0 24 24" fill="currentColor">
							<path d="M12 2C6.477 2 2 6.477 2 12c0 5.524 4.477 10 10 10 5.524 0 10-4.476 10-10 0-5.523-4.476-10-10-10zm4.586 14.424a.627.627 0 0 1-.86.208c-2.355-1.439-5.32-1.765-8.812-.966a.625.625 0 0 1-.277-1.22c3.824-.874 7.099-.508 9.74 1.107.292.179.387.568.209.871zm1.226-2.723a.784.784 0 0 1-1.077.26c-2.695-1.656-6.804-2.136-9.992-1.168a.785.785 0 1 1-.462-1.501c3.642-1.106 8.188-.574 11.27 1.321a.784.784 0 0 1 .261 1.088zm.105-2.833c-3.232-1.919-8.566-2.096-11.657-1.157a.94.94 0 1 1-.552-1.8c3.553-1.078 9.444-.87 13.14 1.323a.94.94 0 0 1-.931 1.634z"/>
						</svg>
						Spotify
					</span>
				</div>
			{/if}

			<div class="flex gap-4">
				{#if platform === 'ytm'}
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
				{/if}

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
							? platform === 'spotify'
								? 'Anyone can find and listen to this playlist on Spotify.'
								: 'Anyone can find and listen to this playlist on YouTube Music.'
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
