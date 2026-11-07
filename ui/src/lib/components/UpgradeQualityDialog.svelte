<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		CheckmarkCircle02Icon,
		SparklesIcon,
		HeadphonesIcon,
		Loading03Icon
	} from '@hugeicons/core-free-icons';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { thumb } from '$lib/thumb';
	import * as api from '$lib/api';
	import { ui, playback, closeUpgradeQuality, toast } from '$lib/player.svelte';

	interface QualityTierOption {
		id: string;
		label: string;
		description: string;
		badge: string;
		badgeColor: string;
	}

	const TIERS: QualityTierOption[] = [
		{
			id: 'DOLBY_ATMOS',
			label: 'Dolby Atmos',
			description: 'Immersive multi-channel spatial audio. Falls back to Hi-Res or Lossless FLAC if unavailable.',
			badge: 'Dolby Atmos',
			badgeColor: 'bg-sky-500/15 text-sky-400 border-sky-500/30'
		},
		{
			id: 'HI_RES_LOSSLESS',
			label: 'Hi-Res Lossless',
			description: 'Master quality up to 24-bit / 192 kHz FLAC. Falls back to 16-bit Lossless FLAC if unavailable.',
			badge: '24-bit Hi-Res',
			badgeColor: 'bg-amber-500/15 text-amber-400 border-amber-500/30'
		},
		{
			id: 'LOSSLESS',
			label: 'Lossless FLAC',
			description: 'Bit-perfect CD quality 16-bit / 44.1 kHz FLAC. Full uncompressed acoustic fidelity.',
			badge: '16-bit Lossless',
			badgeColor: 'bg-emerald-500/15 text-emerald-400 border-emerald-500/30'
		}
	];

	let selectedTier = $state('DOLBY_ATMOS');
	let upgrading = $state(false);

	const isOpen = $derived(ui.upgradeQualityOpen);
	const currentSong = $derived(playback.now);

	async function confirmUpgrade() {
		if (upgrading || !currentSong) return;
		upgrading = true;
		try {
			const res = await api.upgradePlaybackQuality(selectedTier);
			toast.success(`Upgraded playback to ${res}`);
			closeUpgradeQuality();
		} catch (err: any) {
			const msg = typeof err === 'string' ? err : err?.message || 'Failed to fetch HQ track, it is unavailable on the selected addons';
			toast.error(msg);
			closeUpgradeQuality();
		} finally {
			upgrading = false;
		}
	}
</script>

<Dialog.Root
	open={isOpen}
	onOpenChange={(open) => {
		if (!open && !upgrading) closeUpgradeQuality();
	}}
>
	<Dialog.Content class="max-w-md gap-0 overflow-hidden p-0 rounded-2xl border border-border/80 bg-card/95 shadow-2xl backdrop-blur-2xl">
		<Dialog.Description class="sr-only">Choose high quality playback tier</Dialog.Description>

		<!-- Header -->
		<div class="border-b border-border/60 bg-gradient-to-b from-primary/10 via-primary/5 to-transparent p-5">
			<div class="flex items-center gap-3">
				{#if currentSong?.thumbnail}
					<img
						src={thumb(currentSong.thumbnail, 96)}
						alt=""
						class="h-12 w-12 rounded-lg object-cover shadow-md shrink-0 border border-border/40"
					/>
				{:else}
					<div class="flex h-12 w-12 items-center justify-center rounded-lg bg-primary/15 text-primary border border-primary/25 shrink-0">
						<HugeiconsIcon icon={HeadphonesIcon} class="h-6 w-6" />
					</div>
				{/if}
				<div class="min-w-0 flex-1">
					<div class="flex items-center gap-1.5 text-primary font-semibold text-xs tracking-wide uppercase">
						<HugeiconsIcon icon={SparklesIcon} class="h-3.5 w-3.5" />
						<span>Upgrade Quality</span>
					</div>
					<Dialog.Title class="mt-0.5 truncate text-sm font-bold text-foreground">
						{currentSong?.title ?? 'Current Track'}
					</Dialog.Title>
					<p class="truncate text-xs text-muted-foreground mt-0.5">
						{currentSong?.artists ?? 'Unknown Artist'}
					</p>
				</div>
			</div>
		</div>

		<!-- Body: Quality Options -->
		<div class="p-4 space-y-2.5">
			<p class="text-[11px] font-medium text-muted-foreground px-1 uppercase tracking-wider">
				Select target audiophile tier:
			</p>

			<div class="space-y-2">
				{#each TIERS as tier (tier.id)}
					{@const isSelected = selectedTier === tier.id}
					<button
						type="button"
						class="group flex w-full items-start gap-3 rounded-xl border p-3 text-left transition-all cursor-pointer {isSelected
							? 'border-primary bg-primary/10 shadow-sm ring-1 ring-primary/40'
							: 'border-border/60 bg-muted/20 hover:border-border hover:bg-muted/40'}"
						onclick={() => (selectedTier = tier.id)}
					>
						<div class="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border transition-colors {isSelected ? 'border-primary bg-primary text-primary-foreground' : 'border-muted-foreground/40 group-hover:border-foreground'}">
							{#if isSelected}
								<HugeiconsIcon icon={CheckmarkCircle02Icon} class="size-3.5" />
							{/if}
						</div>
						<div class="min-w-0 flex-1">
							<div class="flex items-center justify-between gap-2">
								<span class="text-xs font-bold text-foreground">{tier.label}</span>
								<span class="rounded px-1.5 py-0.5 text-[9px] font-mono font-semibold border {tier.badgeColor}">
									{tier.badge}
								</span>
							</div>
							<p class="mt-1 text-[11px] leading-relaxed text-muted-foreground">
								{tier.description}
							</p>
						</div>
					</button>
				{/each}
			</div>

			<p class="text-[10px] text-muted-foreground/80 pt-1 px-1 leading-normal">
				Waterfall fallback automatically finds the highest matching bitrate available across your active streaming addons without interruption.
			</p>
		</div>

		<!-- Footer -->
		<div class="flex items-center justify-end gap-2 border-t border-border/60 bg-muted/20 p-4">
			<Button
				variant="ghost"
				size="sm"
				class="text-xs cursor-pointer"
				disabled={upgrading}
				onclick={closeUpgradeQuality}
			>
				Cancel
			</Button>
			<Button
				size="sm"
				class="gap-1.5 text-xs font-semibold cursor-pointer min-w-[7rem]"
				disabled={upgrading}
				onclick={confirmUpgrade}
			>
				{#if upgrading}
					<HugeiconsIcon icon={Loading03Icon} class="h-3.5 w-3.5 animate-spin" />
					<span>Upgrading...</span>
				{:else}
					<HugeiconsIcon icon={SparklesIcon} class="h-3.5 w-3.5" />
					<span>Upgrade Stream</span>
				{/if}
			</Button>
		</div>
	</Dialog.Content>
</Dialog.Root>
