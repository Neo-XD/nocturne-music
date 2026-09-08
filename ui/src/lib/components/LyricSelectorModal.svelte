<script lang="ts">
	import * as api from '$lib/api';
	import { toast } from '$lib/player.svelte';

	let {
		open = $bindable(false),
		videoId,
		initialTitle = '',
		initialArtist = '',
		duration,
		onApplied
	}: {
		open: boolean;
		videoId: string;
		initialTitle?: string;
		initialArtist?: string;
		duration?: number;
		onApplied?: (lyrics: api.Lyrics) => void;
	} = $props();

	let titleQuery = $state('');
	let artistQuery = $state('');
	let searching = $state(false);
	let candidates = $state<api.LyricCandidate[]>([]);
	let selectedCandidate = $state<api.LyricCandidate | null>(null);
	let applying = $state(false);

	$effect(() => {
		if (open) {
			titleQuery = initialTitle;
			artistQuery = initialArtist;
			selectedCandidate = null;
			if (initialTitle || initialArtist) {
				handleSearch();
			}
		}
	});

	async function handleSearch() {
		if (!titleQuery.trim() && !artistQuery.trim()) return;
		searching = true;
		try {
			const res = await api.searchLyricsCandidates({
				title: titleQuery.trim(),
				artist: artistQuery.trim(),
				duration,
				videoId
			});
			candidates = res;
			if (res.length > 0) {
				selectedCandidate = res[0];
			} else {
				selectedCandidate = null;
			}
		} catch (e) {
			toast.error(`Search error: ${e}`);
		} finally {
			searching = false;
		}
	}

	async function applyCandidate(candidate: api.LyricCandidate) {
		applying = true;
		try {
			await api.applySelectedLyric({
				videoId,
				lyrics: candidate.lyrics
			});
			toast.success(`Lyrics applied from ${candidate.source}`);
			onApplied?.(candidate.lyrics);
			open = false;
		} catch (e) {
			toast.error(`Failed to apply lyrics: ${e}`);
		} finally {
			applying = false;
		}
	}

	function handleKeyDown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			open = false;
		}
	}
</script>

<svelte:window onkeydown={handleKeyDown} />

{#if open}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 animate-in fade-in duration-200"
		onclick={(e) => {
			if (e.target === e.currentTarget) open = false;
		}}
		role="dialog"
		aria-modal="true"
	>
		<div
			class="relative flex flex-col w-full max-w-3xl max-h-[85vh] rounded-2xl bg-card border border-border shadow-2xl overflow-hidden"
			onclick={(e) => e.stopPropagation()}
		>
			<!-- Header -->
			<div class="flex items-center justify-between px-6 py-4 border-b border-border/40">
				<div>
					<h2 class="text-lg font-bold tracking-tight text-foreground flex items-center gap-2">
						<svg class="w-5 h-5 text-primary" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3" />
						</svg>
						Lyrics Selector
					</h2>
					<p class="text-xs text-muted-foreground mt-0.5">
						Search and select synced or unsynced lyrics across providers
					</p>
				</div>
				<button
					onclick={() => (open = false)}
					class="p-1.5 rounded-full hover:bg-foreground/10 text-muted-foreground hover:text-foreground transition-colors"
					aria-label="Close"
				>
					<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
					</svg>
				</button>
			</div>

			<!-- Search Bar -->
			<div class="p-4 border-b border-border/30 bg-muted/20 flex flex-wrap gap-3 items-center">
				<div class="flex-1 min-w-[180px]">
					<input
						type="text"
						placeholder="Track title..."
						bind:value={titleQuery}
						onkeydown={(e) => e.key === 'Enter' && handleSearch()}
						class="w-full px-3 py-2 text-sm rounded-lg bg-background border border-border focus:outline-none focus:ring-2 focus:ring-primary/40 text-foreground"
					/>
				</div>
				<div class="flex-1 min-w-[180px]">
					<input
						type="text"
						placeholder="Artist name..."
						bind:value={artistQuery}
						onkeydown={(e) => e.key === 'Enter' && handleSearch()}
						class="w-full px-3 py-2 text-sm rounded-lg bg-background border border-border focus:outline-none focus:ring-2 focus:ring-primary/40 text-foreground"
					/>
				</div>
				<button
					onclick={handleSearch}
					disabled={searching}
					class="px-4 py-2 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors disabled:opacity-50 flex items-center gap-2"
				>
					{#if searching}
						<svg class="animate-spin w-4 h-4" viewBox="0 0 24 24" fill="none">
							<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
							<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
						</svg>
						Searching...
					{:else}
						Search
					{/if}
				</button>
			</div>

			<!-- Main Split View -->
			<div class="flex-1 flex min-h-[350px] overflow-hidden">
				<!-- Candidate List -->
				<div class="w-1/2 border-r border-border/40 overflow-y-auto p-3 space-y-2">
					{#if searching}
						<div class="p-8 text-center text-sm text-muted-foreground flex flex-col items-center gap-3">
							<svg class="animate-spin w-6 h-6 text-primary" viewBox="0 0 24 24" fill="none">
								<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
								<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
							</svg>
							Searching lyric providers...
						</div>
					{:else if candidates.length === 0}
						<div class="p-8 text-center text-sm text-muted-foreground">
							No lyrics found. Try tweaking the title or artist name above.
						</div>
					{:else}
						{#each candidates as candidate, i (i)}
							{@const isSelected = selectedCandidate === candidate}
							<button
								type="button"
								onclick={() => (selectedCandidate = candidate)}
								class="w-full text-left p-3 rounded-xl border transition-all {isSelected
									? 'bg-primary/10 border-primary/40 shadow-sm'
									: 'bg-background/40 border-border/40 hover:bg-muted/30'}"
							>
								<div class="flex items-center justify-between gap-2">
									<span class="font-medium text-sm text-foreground truncate">
										{candidate.title || titleQuery}
									</span>
									<span class="text-[11px] px-2 py-0.5 rounded-full font-semibold shrink-0 {candidate.source.includes('BetterLyrics') ? 'bg-purple-500/20 text-purple-300 border border-purple-500/30' : 'bg-primary/20 text-primary border border-primary/30'}">
										{candidate.source}
									</span>
								</div>
								<div class="text-xs text-muted-foreground truncate mt-0.5">
									{candidate.artist || artistQuery}
								</div>
								<div class="mt-2 flex items-center gap-1.5 flex-wrap text-[10px]">
									{#if candidate.has_words}
										<span class="px-1.5 py-0.5 rounded bg-amber-500/20 text-amber-300 font-medium">
											Word-by-word
										</span>
									{/if}
									{#if candidate.synced}
										<span class="px-1.5 py-0.5 rounded bg-emerald-500/20 text-emerald-300 font-medium">
											Synced
										</span>
									{:else}
										<span class="px-1.5 py-0.5 rounded bg-muted text-muted-foreground">
											Plain text
										</span>
									{/if}
									<span class="text-muted-foreground ml-auto">
										{candidate.lyrics.lines.length} lines
									</span>
								</div>
							</button>
						{/each}
					{/if}
				</div>

				<!-- Preview Panel -->
				<div class="w-1/2 flex flex-col bg-background/50 overflow-hidden">
					{#if selectedCandidate}
						<div class="p-3 border-b border-border/30 flex items-center justify-between bg-muted/20">
							<div>
								<span class="text-xs font-semibold text-foreground">Preview: {selectedCandidate.source}</span>
								<p class="text-[11px] text-muted-foreground">{selectedCandidate.lyrics.lines.length} lines</p>
							</div>
							<button
								onclick={() => applyCandidate(selectedCandidate!)}
								disabled={applying}
								class="px-3 py-1.5 rounded-lg bg-primary text-primary-foreground text-xs font-medium hover:bg-primary/90 transition-colors disabled:opacity-50 flex items-center gap-1.5"
							>
								{#if applying}
									Applying...
								{:else}
									<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
										<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
									</svg>
									Use these lyrics
								{/if}
							</button>
						</div>
						<div class="flex-1 overflow-y-auto p-4 font-mono text-xs space-y-1.5 text-foreground/80 leading-relaxed">
							{#each selectedCandidate.lyrics.lines as line, idx (idx)}
								<div class="flex gap-2">
									{#if line.time_ms !== undefined}
										<span class="text-muted-foreground/60 select-none text-[10px] w-12 shrink-0">
											{Math.floor(line.time_ms / 60000)}:{String(Math.floor((line.time_ms % 60000) / 1000)).padStart(2, '0')}
										</span>
									{/if}
									<span class={line.words?.length ? 'text-foreground' : ''}>
										{line.text || '♪'}
									</span>
								</div>
							{/each}
						</div>
					{:else}
						<div class="flex-1 flex items-center justify-center text-xs text-muted-foreground p-6 text-center">
							Select a candidate on the left to preview lyrics
						</div>
					{/if}
				</div>
			</div>

			<!-- Footer -->
			<div class="px-6 py-3 border-t border-border/40 flex justify-between items-center text-xs text-muted-foreground bg-muted/10">
				<span>Nocturne Lyric Matcher</span>
				<button
					onclick={() => (open = false)}
					class="px-3 py-1.5 rounded-lg hover:bg-foreground/10 text-foreground transition-colors font-medium"
				>
					Close
				</button>
			</div>
		</div>
	</div>
{/if}
