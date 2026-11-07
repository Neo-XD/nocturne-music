<script lang="ts">
	import { fade } from 'svelte/transition';
	import { prefs } from '$lib/player.svelte';

	let {
		quality = null,
		size = 'sm',
		showDetails = true,
		class: className = ''
	}: {
		quality?: string | null;
		size?: 'xs' | 'sm' | 'md';
		showDetails?: boolean;
		class?: string;
	} = $props();

	const raw = $derived(quality ?? '');
	const rawUpper = $derived(raw.toUpperCase());

	const isAtmos = $derived(
		rawUpper.includes('ATMOS') ||
		rawUpper.includes('DOLBY') ||
		rawUpper.includes('EAC3') ||
		rawUpper.includes('E-AC-3') ||
		rawUpper.includes('EC3') ||
		rawUpper.includes('EC-3') ||
		rawUpper.includes('DD+') ||
		rawUpper.includes('DDP') ||
		rawUpper.includes('JOC') ||
		rawUpper.includes('SPATIAL')
	);
	const isDolbyDigitalPlus = $derived(
		isAtmos &&
		(rawUpper.includes('PLUS') || rawUpper.includes('DD+') || rawUpper.includes('DDP')) &&
		!rawUpper.includes('ATMOS')
	);
	const isHires = $derived(
		!isAtmos && (rawUpper.includes('HI-RES') || rawUpper.includes('HI_RES') || rawUpper.includes('HIRES'))
	);
	const isLossless = $derived(
		!isAtmos && !isHires && (rawUpper.includes('LOSSLESS') || rawUpper.includes('FLAC') || rawUpper.includes('ALAC'))
	);
	const isOpus = $derived(!isAtmos && !isHires && !isLossless && rawUpper.includes('OPUS'));
	const isAac = $derived(!isAtmos && !isHires && !isLossless && (rawUpper.includes('AAC') || rawUpper.includes('M4A')));

	const shortLabel = $derived.by(() => {
		if (isDolbyDigitalPlus) return 'DD+';
		if (isAtmos) return 'ATMOS';
		if (isHires) return 'HIRES';
		if (isLossless) return 'LOSSLESS';
		if (isOpus) return 'OPUS';
		if (isAac) return 'AAC';
		return rawUpper.split(' ')[0] || 'AUDIO';
	});

	const detailText = $derived.by(() => {
		if (!showDetails) return '';
		if (isAtmos) return raw.replace(/DOLBY\s*(ATMOS|DIGITAL\s*PLUS)?/i, '').replace(/·/g, '').trim();
		if (isHires) return raw.replace(/HI-RES\s*LOSSLESS/i, '').replace(/·/g, '').trim();
		if (isLossless) return raw.replace(/LOSSLESS/i, '').replace(/·/g, '').trim();
		return '';
	});
</script>

{#if (prefs.showAudioQuality || isAtmos) && raw}
	{#if size === 'xs'}
		<span
			class="inline-flex items-center px-1 py-0.2 rounded text-[7.5px] font-black tracking-wider uppercase border select-none shrink-0 leading-none {isAtmos
				? 'bg-sky-500/15 text-sky-400 border-sky-500/30'
				: isHires
					? 'bg-amber-500/15 text-amber-400 border-amber-500/30'
					: isLossless
						? 'bg-emerald-500/15 text-emerald-400 border-emerald-500/30'
						: isOpus
							? 'bg-purple-500/15 text-purple-400 border-purple-500/30'
							: isAac
								? 'bg-blue-500/15 text-blue-400 border-blue-500/30'
								: 'bg-muted/60 text-muted-foreground border-border/40'} {className}"
			title="Playback Audio Quality: {raw}"
		>
			{shortLabel}
		</span>
	{:else}
		<div class="inline-flex items-center gap-1 select-none shrink-0 {className}" in:fade={{ duration: 150 }}>
			<span
				class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded border leading-none {size === 'md'
					? 'text-[11px]'
					: 'text-[9.5px]'} font-semibold tracking-wide {isAtmos
					? 'bg-sky-500/15 text-sky-400 border-sky-500/35 shadow-[0_0_10px_rgba(14,165,233,0.15)]'
					: isHires
						? 'bg-amber-500/15 text-amber-400 border-amber-500/30 shadow-[0_0_10px_rgba(245,158,11,0.15)]'
						: isLossless
							? 'bg-emerald-500/15 text-emerald-400 border-emerald-500/30'
							: 'bg-muted/60 text-muted-foreground/90 border-border/40 font-medium'}"
				title="Playback Audio Quality: {raw}"
			>
				{#if isAtmos}
					<span class="flex items-center gap-1 font-extrabold uppercase tracking-wider text-sky-300">
						<!-- Spatial surround sound indicator dots -->
						<span class="inline-flex items-center gap-0.5">
							<span class="size-1 rounded-full bg-sky-400 animate-pulse"></span>
							<span class="size-1 rounded-full bg-sky-300"></span>
						</span>
						{isDolbyDigitalPlus ? 'DOLBY DIGITAL PLUS' : 'DOLBY ATMOS'}
					</span>
					{#if detailText}
						<span class="font-normal opacity-85 text-[9px] text-sky-200/80">· {detailText}</span>
					{/if}
				{:else if isHires}
					<span class="font-extrabold uppercase tracking-wider text-amber-300">HI-RES LOSSLESS</span>
					{#if detailText}
						<span class="font-normal opacity-90 text-[9px] text-amber-200/80">· {detailText}</span>
					{/if}
				{:else if isLossless}
					<span class="font-extrabold uppercase tracking-wider text-emerald-300">LOSSLESS</span>
					{#if detailText}
						<span class="font-normal opacity-90 text-[9px] text-emerald-200/80">· {detailText}</span>
					{/if}
				{:else}
					<span>{raw}</span>
				{/if}
			</span>
		</div>
	{/if}
{/if}
