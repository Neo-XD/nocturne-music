<script lang="ts">
	// The search page's top result, drawn as the answer rather than as one card among many: the
	// artwork large, the title at display size, a play button that does the right thing for the
	// kind, and the few rows YouTube pairs with it (an artist's hits, a song's other takes).
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { MusicNote01Icon, PlayIcon, UserIcon } from '@hugeicons/core-free-icons';
	import type { BrowseItem } from '$lib/api';
	import { asSong, openItem, playItem } from '$lib/browse';
	import { openAddToPlaylist, playSong, playback, startRadio } from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';
	import ExplicitIcon from './ExplicitIcon.svelte';
	import ItemMenu from './ItemMenu.svelte';
	import TrackRow from './TrackRow.svelte';

	let { item, related = [] }: { item: BrowseItem; related?: BrowseItem[] } = $props();

	const round = $derived(item.kind === 'artist');
	const subtitle = $derived(
		item.kind === 'song'
			? ['Song', item.subtitle].filter(Boolean).join(' • ')
			: item.subtitle
	);
	const rows = $derived(
		related
			.filter((r) => r.kind === 'song')
			.slice(0, 3)
			.map(asSong)
	);

	let attempt = $state(0);
	let washOk = $state(true);
	$effect(() => {
		item.thumbnail;
		attempt = 0;
		washOk = true;
	});
	const sized = $derived(thumb(item.thumbnail, 400));
	const src = $derived(attempt === 0 ? sized : item.thumbnail);

	let busy = $state(false);
	async function play() {
		if (item.kind === 'artist') return startRadio('artist', item.id, item.title);
		if (busy) return;
		busy = true;
		try {
			await playItem(item);
		} finally {
			busy = false;
		}
	}
</script>

<div class="relative isolate overflow-hidden rounded-2xl border border-white/10 bg-card/60 backdrop-blur-md" data-ctx>
	{#if item.thumbnail && washOk}
		<div class="art-wash pointer-events-none absolute inset-0 -z-10 overflow-hidden" aria-hidden="true">
			<img
				src={thumb(item.thumbnail, 96)}
				alt=""
				class="absolute inset-0 h-full w-full scale-125 object-cover opacity-60 blur-2xl"
				onerror={() => (washOk = false)}
			/>
			<div class="absolute inset-0 bg-gradient-to-b from-card/30 via-card/75 to-card"></div>
		</div>
	{/if}
	<div class="flex items-center gap-4 p-5">
		<div
			role="button"
			tabindex="0"
			class="flex min-w-0 flex-1 cursor-pointer items-center gap-5"
			onclick={() => openItem(item)}
			onkeydown={(e) => {
				if (e.target !== e.currentTarget) return;
				if (e.key === 'Enter' || e.key === ' ') {
					e.preventDefault();
					openItem(item);
				}
			}}
			title={item.subtitle ? `${item.title} • ${item.subtitle}` : item.title}
		>
			<div
				class="flex h-28 w-28 shrink-0 items-center justify-center overflow-hidden bg-muted text-muted-foreground/50 shadow-xl {round
					? 'rounded-full'
					: 'rounded-xl'}"
			>
				{#if item.thumbnail && attempt < 2}
					<img
						{src}
						alt=""
						class="h-full w-full object-cover"
						draggable="false"
						onerror={() => (attempt = attempt === 0 && sized !== item.thumbnail ? 1 : 2)}
					/>
				{:else}
					<HugeiconsIcon icon={round ? UserIcon : MusicNote01Icon} class="h-9 w-9" />
				{/if}
			</div>
			<div class="min-w-0">
				<div
					class="line-clamp-2 font-heading font-bold leading-tight tracking-tight {item.title
						.length > 28
						? 'text-2xl'
						: 'text-3xl'}"
				>
					{item.title}
				</div>
				{#if subtitle || item.explicit}
					<div class="mt-2 flex items-center gap-1.5 text-sm text-muted-foreground">
						{#if item.explicit}<ExplicitIcon class="h-3.5 w-3.5 shrink-0" />{/if}
						<span class="truncate">{subtitle}</span>
					</div>
				{/if}
			</div>
		</div>
		<button
			class="flex h-12 w-12 shrink-0 cursor-pointer items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg transition-transform hover:scale-105 active:scale-95"
			class:animate-pulse={busy}
			disabled={busy}
			aria-label={item.kind === 'artist' ? 'Start Radio' : 'Play'}
			title={item.kind === 'artist' ? 'Start Radio' : 'Play'}
			onclick={play}
		>
			<HugeiconsIcon icon={PlayIcon} class="h-5 w-5" />
		</button>
		<ItemMenu
			{item}
			triggerClass="flex h-9 w-9 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground"
		/>
	</div>
	{#if rows.length}
		<div class="border-t border-border/60 px-2 py-1.5">
			{#each rows as song, i (song.video_id + ':' + i)}
				<TrackRow
					{song}
					compact
					active={playback.now?.videoId === song.video_id}
					onplay={() => playSong(song)}
					onAdd={() => openAddToPlaylist(song)}
				/>
			{/each}
		</div>
	{/if}
</div>
