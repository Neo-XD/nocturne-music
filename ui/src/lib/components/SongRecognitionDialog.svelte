<script lang="ts">
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		AudioWave01Icon,
		PlayIcon,
		Add01Icon,
		Search01Icon,
		Refresh01Icon,
		Cancel01Icon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { captureAndRecognizeAudio } from '$lib/shazam';
	import * as api from '$lib/api';
	import type { RecognizedSong } from '$lib/api';
	import { enqueue, toast } from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';

	let { open = $bindable(false) }: { open?: boolean } = $props();

	let listening = $state(false);
	let statusText = $state('Listening to PC audio…');
	let result = $state<RecognizedSong | null>(null);
	let errorMsg = $state<string | null>(null);
	let autoStarted = $state(false);

	function clearRecognition() {
		result = null;
		errorMsg = null;
		listening = false;
	}

	async function startListening() {
		listening = true;
		result = null;
		errorMsg = null;
		statusText = 'Listening to PC audio…';

		try {
			const res = await captureAndRecognizeAudio((msg) => {
				statusText = msg;
			});
			result = res;
			if (!res.found) {
				errorMsg = 'Could not recognize this song. Ensure music is actively playing and try again.';
			}
		} catch (err: unknown) {
			errorMsg = err instanceof Error ? err.message : String(err);
		} finally {
			listening = false;
		}
	}

	$effect(() => {
		if (open) {
			if (!autoStarted && !result && !errorMsg) {
				autoStarted = true;
				startListening();
			}
		} else {
			autoStarted = false;
		}
	});

	async function playRecognized(song: RecognizedSong) {
		const query = song.query || `${song.title} ${song.artist}`;
		toast.info(`Searching "${query}" on YouTube Music…`);
		try {
			const searchRes = await api.getBrowseGrid(query);
			// Find song or search
			open = false;
			goto(`/search?q=${encodeURIComponent(query)}`);
		} catch {
			open = false;
			goto(`/search?q=${encodeURIComponent(query)}`);
		}
	}

	async function addRecognizedToQueue(song: RecognizedSong) {
		const query = song.query || `${song.title} ${song.artist}`;
		toast.info(`Finding "${query}" to add to queue…`);
		try {
			open = false;
			goto(`/search?q=${encodeURIComponent(query)}`);
		} catch (e) {
			toast.error(String(e));
		}
	}

	function viewSearch(song: RecognizedSong) {
		open = false;
		const query = song.query || `${song.title} ${song.artist}`;
		goto(`/search?q=${encodeURIComponent(query)}`);
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md border-border/80 bg-popover/95 p-6 backdrop-blur-2xl shadow-2xl rounded-[calc(var(--radius,0.45rem)+8px)]">
		<div class="flex items-center justify-between pb-3 pr-8 border-b border-border/40">
			<div class="flex items-center gap-2">
				<div class="flex size-8 items-center justify-center rounded-lg bg-primary/15 text-primary">
					<HugeiconsIcon icon={AudioWave01Icon} class="size-4.5" />
				</div>
				<div>
					<h2 class="text-sm font-bold text-foreground">Music Recognition</h2>
					<p class="text-[11px] text-muted-foreground">Identify songs playing on your PC (Shazam)</p>
				</div>
			</div>
			{#if result || errorMsg}
				<button
					type="button"
					class="text-[11px] font-medium text-muted-foreground hover:text-foreground transition-colors cursor-pointer px-2 py-0.5 rounded-md hover:bg-muted/60"
					onclick={clearRecognition}
					title="Clear previous recognition"
				>
					Clear
				</button>
			{/if}
		</div>

		<!-- Listening State -->
		{#if listening}
			<div class="flex flex-col items-center justify-center py-10 text-center">
				<div class="relative flex items-center justify-center size-24 mb-6">
					<div class="absolute inset-0 rounded-full bg-primary/20 animate-ping"></div>
					<div class="absolute inset-2 rounded-full bg-primary/30 animate-pulse"></div>
					<div class="relative flex size-14 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg shadow-primary/30">
						<HugeiconsIcon icon={AudioWave01Icon} class="size-7 animate-bounce" />
					</div>
				</div>
				<p class="text-sm font-semibold text-foreground">{statusText}</p>
				<p class="mt-1 text-xs text-muted-foreground">Keep the music playing through your speakers or mic</p>
			</div>

		<!-- Match Found -->
		{:else if result && result.found}
			<div class="py-4 space-y-4">
				<div class="flex items-center gap-4 rounded-xl border border-border/60 bg-muted/40 p-3.5">
					{#if result.cover}
						<img
							src={thumb(result.cover, 180)}
							alt=""
							class="size-18 rounded-lg object-cover ring-1 ring-border shadow-md shrink-0"
						/>
					{:else}
						<div class="flex size-18 items-center justify-center rounded-lg bg-muted text-muted-foreground shrink-0">
							<HugeiconsIcon icon={AudioWave01Icon} class="size-8" />
						</div>
					{/if}
					<div class="min-w-0 flex-1">
						<span class="rounded bg-primary/15 px-1.5 py-0.5 text-[9px] font-bold text-primary uppercase tracking-wider">
							Recognized
						</span>
						<h3 class="text-base font-bold text-foreground truncate mt-1">{result.title}</h3>
						<p class="text-xs font-medium text-muted-foreground truncate">{result.artist}</p>
						{#if result.album}
							<p class="text-[11px] text-muted-foreground/70 truncate">{result.album}</p>
						{/if}
					</div>
				</div>

				<div class="grid grid-cols-2 gap-2 pt-1">
					<Button
						variant="default"
						class="w-full gap-1.5 cursor-pointer font-semibold rounded-[var(--radius,0.45rem)]"
						onclick={() => playRecognized(result!)}
					>
						<HugeiconsIcon icon={PlayIcon} class="size-4" />
						Play in Nocturne
					</Button>
					<Button
						variant="outline"
						class="w-full gap-1.5 cursor-pointer rounded-[var(--radius,0.45rem)]"
						onclick={() => viewSearch(result!)}
					>
						<HugeiconsIcon icon={Search01Icon} class="size-4" />
						Search Results
					</Button>
				</div>

				<div class="flex items-center justify-between border-t border-border/40 pt-3">
					<Button
						variant="ghost"
						size="sm"
						class="text-xs text-muted-foreground hover:text-foreground gap-1.5 cursor-pointer h-8 px-2.5"
						onclick={clearRecognition}
						title="Clear previous recognition"
					>
						<HugeiconsIcon icon={Cancel01Icon} class="size-3.5" />
						Clear previous
					</Button>
					<Button
						variant="outline"
						size="sm"
						class="text-xs gap-1.5 cursor-pointer rounded-[var(--radius,0.45rem)] h-8 px-3"
						onclick={startListening}
						title="Listen for next song"
					>
						<HugeiconsIcon icon={Refresh01Icon} class="size-3.5" />
						Listen for next song
					</Button>
				</div>
			</div>

		<!-- Error or No Match -->
		{:else if errorMsg}
			<div class="flex flex-col items-center justify-center py-8 text-center space-y-4">
				<div class="flex size-14 items-center justify-center rounded-full bg-destructive/10 text-destructive">
					<HugeiconsIcon icon={AudioWave01Icon} class="size-7" />
				</div>
				<div>
					<h3 class="text-sm font-bold text-foreground">No Match Identified</h3>
					<p class="mt-1 text-xs text-muted-foreground max-w-xs">{errorMsg}</p>
				</div>
				<div class="flex items-center gap-2">
					<Button
						variant="outline"
						size="sm"
						class="gap-1.5 cursor-pointer rounded-[var(--radius,0.45rem)]"
						onclick={startListening}
					>
						<HugeiconsIcon icon={Refresh01Icon} class="size-3.5" />
						Try Listening Again
					</Button>
					<Button
						variant="ghost"
						size="sm"
						class="gap-1.5 cursor-pointer text-muted-foreground hover:text-foreground"
						onclick={clearRecognition}
					>
						<HugeiconsIcon icon={Cancel01Icon} class="size-3.5" />
						Clear
					</Button>
				</div>
			</div>

		<!-- Ready / Idle State -->
		{:else if !listening && !result && !errorMsg}
			<div class="flex flex-col items-center justify-center py-8 text-center space-y-4">
				<div class="flex size-14 items-center justify-center rounded-full bg-primary/10 text-primary">
					<HugeiconsIcon icon={AudioWave01Icon} class="size-7" />
				</div>
				<div>
					<h3 class="text-sm font-bold text-foreground">Ready to Listen</h3>
					<p class="mt-1 text-xs text-muted-foreground max-w-xs">Play music on your PC and click below to identify the song.</p>
				</div>
				<Button
					variant="default"
					size="sm"
					class="gap-1.5 cursor-pointer rounded-[var(--radius,0.45rem)]"
					onclick={startListening}
				>
					<HugeiconsIcon icon={AudioWave01Icon} class="size-3.5" />
					Listen to PC Audio
				</Button>
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
