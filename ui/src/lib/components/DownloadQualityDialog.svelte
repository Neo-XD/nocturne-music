<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Download01Icon,
		CheckmarkCircle02Icon,
		InformationCircleIcon,
		SparklesIcon,
		MusicNote03Icon,
		Playlist01Icon as PlayList01Icon
	} from '@hugeicons/core-free-icons';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { thumb } from '$lib/thumb';
	import {
		ui,
		prefs,
		closeDownloadPicker,
		executeDownloadSong,
		executeDownloadPlaylist,
		setDownloadQuality,
		setDownloadAlwaysAsk
	} from '$lib/player.svelte';

	interface QualityChoice {
		id: string;
		label: string;
		detail: string;
		badge: string;
		badgeColor: string;
		source: 'addon' | 'ytm';
	}

	const CHOICES: QualityChoice[] = [
		{
			id: 'LOSSLESS',
			label: 'Lossless (FLAC)',
			detail: '16-bit / 44.1 kHz bit-perfect FLAC audio via community addons with Lofty metadata tags.',
			badge: 'FLAC 16-bit',
			badgeColor: 'bg-emerald-500/15 text-emerald-500 border-emerald-500/25',
			source: 'addon'
		},
		{
			id: 'HI_RES_LOSSLESS',
			label: 'Hi-Res Lossless',
			detail: '24-bit / up to 192 kHz studio master FLAC audio stream from high-fidelity source addons.',
			badge: '24-bit Studio',
			badgeColor: 'bg-amber-500/15 text-amber-500 border-amber-500/25',
			source: 'addon'
		},
		{
			id: 'DOLBY_ATMOS',
			label: 'Dolby Atmos',
			detail: 'Immersive spatial multichannel audio if supported by your connected streaming addons.',
			badge: 'Spatial Audio',
			badgeColor: 'bg-purple-500/15 text-purple-400 border-purple-500/25',
			source: 'addon'
		},
		{
			id: 'HIGH',
			label: 'High Quality',
			detail: '256 kbps AAC / 160 kbps Opus from YouTube Music. Universal device compatibility.',
			badge: '256 kbps AAC',
			badgeColor: 'bg-primary/15 text-primary border-primary/25',
			source: 'ytm'
		},
		{
			id: 'LOW',
			label: 'Data Saver',
			detail: '64 kbps Opus stream from YouTube Music. Minimal storage space.',
			badge: 'Compact',
			badgeColor: 'bg-muted text-muted-foreground border-border/40',
			source: 'ytm'
		}
	];

	let selectedQuality = $state('LOSSLESS');
	let rememberChoice = $state(false);
	let downloading = $state(false);

	$effect(() => {
		if (ui.downloadPicker) {
			selectedQuality = prefs.downloadQuality || 'LOSSLESS';
			rememberChoice = !prefs.downloadAlwaysAsk;
			downloading = false;
		}
	});

	const target = $derived(ui.downloadPicker);
	const isPlaylist = $derived(target?.itemType === 'playlist');
	const isOpen = $derived(target !== null);

	const isAboveHigh = $derived(
		['LOSSLESS', 'HI_RES_LOSSLESS', 'DOLBY_ATMOS', 'VERY_HIGH'].includes(
			selectedQuality.toUpperCase()
		)
	);

	async function confirmDownload() {
		if (!target || downloading) return;
		downloading = true;
		try {
			if (rememberChoice) {
				setDownloadQuality(selectedQuality);
				setDownloadAlwaysAsk(false);
			}

			const qualityToUse = selectedQuality;
			const currentTarget = target;
			closeDownloadPicker();

			if (currentTarget.itemType === 'song' && currentTarget.song) {
				await executeDownloadSong(currentTarget.song, qualityToUse);
			} else if (currentTarget.itemType === 'playlist' && currentTarget.playlistItems) {
				await executeDownloadPlaylist(
					currentTarget.playlistItems,
					currentTarget.playlistName || 'Playlist',
					qualityToUse
				);
			}
		} finally {
			downloading = false;
		}
	}
</script>

<Dialog.Root
	open={isOpen}
	onOpenChange={(open) => {
		if (!open && !downloading) closeDownloadPicker();
	}}
>
	<Dialog.Content class="download-quality-dialog max-w-lg p-0 overflow-hidden border border-border/80 shadow-2xl rounded-2xl bg-card/95 backdrop-blur-2xl">
		<Dialog.Description class="sr-only">Choose audio download quality</Dialog.Description>
		<Dialog.Header class="p-5 pb-3">
			<div class="flex items-center gap-2.5">
				<div class="flex h-9 w-9 items-center justify-center rounded-xl bg-primary/15 text-primary">
					<HugeiconsIcon icon={Download01Icon} class="h-5 w-5" />
				</div>
				<div>
					<Dialog.Title class="text-base font-bold text-foreground">
						{isPlaylist ? 'Download Playlist' : 'Download Song'}
					</Dialog.Title>
					<p class="text-xs text-muted-foreground">Select preferred audio quality for offline playback</p>
				</div>
			</div>
		</Dialog.Header>

		{#if target}
			<div class="px-5 space-y-4">
				<!-- Target track or playlist preview card -->
				<div class="flex items-center gap-3 rounded-xl border border-border/60 bg-muted/30 p-2.5">
					{#if target.thumbnail}
						<img
							src={thumb(target.thumbnail, 96)}
							alt=""
							class="h-12 w-12 rounded-lg object-cover shadow-sm shrink-0"
						/>
					{:else}
						<div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
							<HugeiconsIcon icon={isPlaylist ? PlayList01Icon : MusicNote03Icon} class="h-6 w-6" />
						</div>
					{/if}
					<div class="min-w-0 flex-1">
						<div class="flex items-center gap-1.5">
							<span class="text-xs font-semibold text-foreground truncate">{target.title}</span>
						</div>
						{#if target.subtitle}
							<p class="text-[11px] text-muted-foreground truncate">{target.subtitle}</p>
						{/if}
					</div>
				</div>

				<!-- Quality Options Radio List -->
				<div class="space-y-2">
					<span class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Audio Format & Resolution</span>
					<div class="grid grid-cols-1 gap-1.5">
						{#each CHOICES as choice (choice.id)}
							{@const selected = selectedQuality === choice.id}
							<button
								type="button"
								onclick={() => (selectedQuality = choice.id)}
								class="flex items-start justify-between gap-3 rounded-xl border p-3 text-left transition-all cursor-pointer {selected
									? 'border-primary bg-primary/10 shadow-sm ring-1 ring-primary/40'
									: 'border-border/60 bg-muted/20 hover:border-border hover:bg-muted/40'}"
							>
								<div class="flex items-start gap-2.5 min-w-0 flex-1">
									<div class="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border transition-colors {selected ? 'border-primary bg-primary text-primary-foreground' : 'border-border/80'}">
										{#if selected}
											<div class="h-1.5 w-1.5 rounded-full bg-primary-foreground"></div>
										{/if}
									</div>
									<div class="min-w-0 flex-1">
										<div class="flex items-center gap-2">
											<span class="text-xs font-semibold text-foreground">{choice.label}</span>
											<span class="rounded px-1.5 py-0.2 text-[9px] font-semibold border {choice.badgeColor}">
												{choice.badge}
											</span>
										</div>
										<p class="text-[11px] text-muted-foreground leading-snug mt-0.5">{choice.detail}</p>
									</div>
								</div>
								{#if choice.source === 'addon'}
									<span class="rounded bg-primary/10 text-primary px-1.5 py-0.5 text-[9px] font-medium shrink-0">
										Addon Stream
									</span>
								{:else}
									<span class="text-muted-foreground text-[10px] shrink-0">
										Standard YTM
									</span>
								{/if}
							</button>
						{/each}
					</div>
				</div>

				<!-- Fallback Disclaimer Note -->
				{#if isAboveHigh}
					<div class="flex items-start gap-2 rounded-xl border border-primary/20 bg-primary/5 p-2.5 text-[11px] text-muted-foreground">
						<HugeiconsIcon icon={SparklesIcon} class="h-4 w-4 shrink-0 text-primary mt-0.5" />
						<p class="leading-relaxed">
							Lossless FLAC resolution queries your enabled <strong>BitChord Streaming Addons</strong> first. If no addon matches the track, Nocturne automatically falls back to YouTube Music High Quality.
						</p>
					</div>
				{/if}

				<!-- Remember Choice Option -->
				<label class="flex items-center gap-2.5 cursor-pointer pt-1 select-none">
					<input
						type="checkbox"
						bind:checked={rememberChoice}
						class="h-4 w-4 rounded border-border/80 accent-primary cursor-pointer transition-colors"
					/>
					<span class="text-xs text-muted-foreground hover:text-foreground transition-colors">
						Don't ask again (remember this choice for future downloads)
					</span>
				</label>
			</div>

			<!-- Dialog Footer Buttons -->
			<div class="mt-4 flex items-center justify-end gap-2.5 border-t border-border/60 bg-muted/20 px-5 py-3">
				<Button
					variant="ghost"
					size="sm"
					class="text-xs cursor-pointer"
					disabled={downloading}
					onclick={closeDownloadPicker}
				>
					Cancel
				</Button>
				<Button
					size="sm"
					class="text-xs cursor-pointer gap-1.5 font-semibold"
					disabled={downloading}
					onclick={confirmDownload}
				>
					{#if downloading}
						<span class="animate-pulse">Starting download...</span>
					{:else}
						<HugeiconsIcon icon={Download01Icon} class="h-3.5 w-3.5" />
						<span>Download {isAboveHigh ? 'Lossless' : ''}</span>
					{/if}
				</Button>
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
