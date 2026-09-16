<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Alert02Icon, InformationCircleIcon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { refreshSpotify, toast } from '$lib/player.svelte';

	let { open = $bindable(false) }: { open: boolean } = $props();

	let spDc = $state('');
	let connecting = $state(false);

	async function submit(e: Event) {
		e.preventDefault();
		const cookie = spDc.trim();
		if (!cookie) {
			toast.error('Please enter your sp_dc cookie');
			return;
		}
		connecting = true;
		try {
			const res = await api.spotifyLink(cookie);
			await refreshSpotify();
			toast.success(
				res.display_name
					? `Linked Spotify account as ${res.display_name}`
					: 'Spotify account linked successfully!'
			);
			spDc = '';
			open = false;
		} catch (err) {
			toast.error(String(err));
		} finally {
			connecting = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<div class="flex items-center gap-2">
				<div class="flex size-7 items-center justify-center rounded-full bg-emerald-500/20 text-emerald-400">
					<svg class="size-4" viewBox="0 0 24 24" fill="currentColor">
						<path d="M12 2C6.477 2 2 6.477 2 12c0 5.524 4.477 10 10 10 5.524 0 10-4.476 10-10 0-5.523-4.476-10-10-10zm4.586 14.424a.627.627 0 0 1-.86.208c-2.355-1.439-5.32-1.765-8.812-.966a.625.625 0 0 1-.277-1.22c3.824-.874 7.099-.508 9.74 1.107.292.179.387.568.209.871zm1.226-2.723a.784.784 0 0 1-1.077.26c-2.695-1.656-6.804-2.136-9.992-1.168a.785.785 0 1 1-.462-1.501c3.642-1.106 8.188-.574 11.27 1.321a.784.784 0 0 1 .261 1.088zm.105-2.833c-3.232-1.919-8.566-2.096-11.657-1.157a.94.94 0 1 1-.552-1.8c3.553-1.078 9.444-.87 13.14 1.323a.94.94 0 0 1-.931 1.634z"/>
					</svg>
				</div>
				<Dialog.Title>Link Spotify Account</Dialog.Title>
			</div>
			<Dialog.Description>
				Direct authentication via <code>sp_dc</code> cookie without requiring third-party developer apps.
			</Dialog.Description>
		</Dialog.Header>

		<!-- Spotify Premium Warning Banner -->
		<div class="rounded-xl border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-200 flex items-start gap-2.5">
			<HugeiconsIcon icon={Alert02Icon} class="h-4 w-4 shrink-0 text-amber-400 mt-0.5" />
			<div class="space-y-0.5">
				<p class="font-semibold text-amber-300">Spotify Premium Notice</p>
				<p class="text-[11px] text-amber-200/85 leading-relaxed">
					Direct Web Player audio streaming and Spotify Connect device control are exclusive to <strong>Spotify Premium</strong> accounts. Free accounts can still import and sync playlists.
				</p>
			</div>
		</div>

		<form class="space-y-3" onsubmit={submit}>
			<div class="space-y-1.5">
				<label for="sp-dc-input" class="text-xs font-semibold text-foreground">Spotify sp_dc Cookie</label>
				<Input
					id="sp-dc-input"
					type="password"
					bind:value={spDc}
					placeholder="Paste your sp_dc cookie value..."
					class="font-mono text-xs"
				/>
			</div>

			<div class="rounded-lg border border-border/50 bg-muted/40 p-2.5 text-[11px] text-muted-foreground flex items-start gap-2">
				<HugeiconsIcon icon={InformationCircleIcon} class="h-3.5 w-3.5 shrink-0 text-primary mt-0.5" />
				<div>
					<strong>How to obtain:</strong> Open <a href="https://open.spotify.com" target="_blank" rel="noreferrer" class="text-primary underline">open.spotify.com</a> in your browser, press <kbd class="px-1 py-0.2 bg-muted rounded font-mono text-[10px]">F12</kbd> (DevTools), navigate to <strong>Application / Storage → Cookies</strong>, and copy the value of <code>sp_dc</code>.
				</div>
			</div>

			<div class="flex justify-end gap-2 pt-1">
				<Button type="button" variant="outline" onclick={() => (open = false)} disabled={connecting}>
					Cancel
				</Button>
				<Button type="submit" disabled={connecting || !spDc.trim()} class="gap-1.5 bg-emerald-600 hover:bg-emerald-500 text-white">
					{connecting ? 'Connecting…' : 'Link Spotify'}
				</Button>
			</div>
		</form>
	</Dialog.Content>
</Dialog.Root>
