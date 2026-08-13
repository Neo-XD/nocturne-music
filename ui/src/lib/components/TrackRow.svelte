<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		FavouriteIcon,
		MusicNote01Icon,
		PlayIcon,
		PlayListAddIcon
	} from '@hugeicons/core-free-icons';
	import type { SongItem } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import { lt } from '$lib/lt.svelte';
	import { isLiked, toggleLike } from '$lib/player.svelte';
	import TrackMenu from './TrackMenu.svelte';
	import ArtistLine from './ArtistLine.svelte';

	let {
		song,
		index,
		active = false,
		hideThumb = false,
		compact = false,
		onplay,
		onAdd,
		onRemove,
		removeLabel = 'Remove from playlist'
	}: {
		song: SongItem;
		/** Position badge when set (playlist/queue); omitted for flat search results. */
		index?: number;
		active?: boolean;
		/** Hide the leading thumbnail (album track lists show a number, not a cover). */
		hideThumb?: boolean;
		/**
		 * Grid variant (home's Forgotten favourites): the duration joins the artist line instead of
		 * claiming its own column, and a like heart sits next to the ⋯ — narrow columns have no room
		 * for a separate duration column, and hearting is the whole point of that shelf.
		 */
		compact?: boolean;
		onplay: () => void;
		/** Adds an "Add to playlist" menu item. */
		onAdd?: () => void;
		/** Adds a remove menu item (label via `removeLabel`). */
		onRemove?: () => void;
		removeLabel?: string;
	} = $props();

	// In a session as guest, clicking a song adds it to the shared queue instead of playing it —
	// reflect that in the hover icon + label so the row doesn't lie.
	const guestAdd = $derived(lt.role === 'guest');

	// The whole row is a play target (role="button"), so mirror native button keyboard activation.
	// Only when the key lands on the row itself — keydowns bubble up from nested interactive
	// elements (⋯ menu, artist link), and hijacking those would play the row instead.
	function onKey(e: KeyboardEvent) {
		if (e.target !== e.currentTarget) return;
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			onplay();
		}
	}
</script>

<!-- content-visibility: a liked-songs playlist runs to thousands of rows and WebKit keeps every one
     in style, layout and paint. 3.5rem is a row (8px padding, 40px thumbnail, 8px); `auto` swaps in
     the measured size after first paint. Not on the compact variant: that one is laid out in CSS
     columns (ForgottenFavourites), where an unsized fragment would upset column balancing, and it
     never has more than 15 rows to skip. -->
<div
	role="button"
	tabindex="0"
	onclick={onplay}
	onkeydown={onKey}
	aria-label={guestAdd ? `Add ${song.title} to the session queue` : `Play ${song.title}`}
	class="group flex w-full cursor-pointer items-center gap-3 rounded-lg p-2 transition-colors hover:bg-accent/10 {active
		? 'bg-accent/10'
		: ''} {compact ? '' : '[content-visibility:auto] [contain-intrinsic-size:auto_3.5rem]'}"
>
	<div class="flex min-w-0 flex-1 items-center gap-3">
		<div class="flex min-w-0 shrink-0 items-center gap-3">
			{#if index !== undefined}
				<span
					class="relative w-5 shrink-0 text-center text-xs {active
						? 'text-primary'
						: 'text-muted-foreground'}"
				>
					<span class="group-hover:opacity-0">{index + 1}</span>
					<HugeiconsIcon
						icon={guestAdd ? PlayListAddIcon : PlayIcon}
						class="absolute inset-0 m-auto h-3.5 w-3.5 opacity-0 group-hover:opacity-100"
					/>
				</span>
			{/if}
			{#if !hideThumb}
				{#if song.thumbnail}
					<img src={thumb(song.thumbnail, 96)} alt="" class="h-10 w-10 shrink-0 rounded-md object-cover" loading="lazy" />
				{:else}
					<!-- An untagged file has no artwork of its own. A music note keeps the row aligned
					     with its neighbours and says so plainly. -->
					<div
						class="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground/50"
					>
						<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4" />
					</div>
				{/if}
			{/if}
		</div>
		<div class="min-w-0 flex-1">
			<div class="flex min-w-0 items-center gap-2">
				<span class="min-w-0 truncate text-sm font-medium {active ? 'text-primary' : ''}">
					{song.title}
				</span>
				{#if song.queued_by}
					<span
						class="shrink-0 rounded-full bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary"
					>
						{song.queued_by}
					</span>
				{/if}
			</div>
			<div class="flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
				<ArtistLine runs={song.artist_runs} text={song.artists} />
				{#if compact && song.duration}
					<span class="shrink-0">· {song.duration}</span>
				{/if}
			</div>
		</div>
	</div>

	<!-- Album rows only, and only where the row has spare width to give: a wide row is mostly empty
	     between the title and the duration. -->
	{#if song.play_count && !compact}
		<div
			class="hidden min-w-0 flex-1 items-center justify-center text-xs text-muted-foreground lg:flex"
		>
			<span class="truncate">{song.play_count} plays</span>
		</div>
	{/if}

	<div class="flex shrink-0 items-center {compact ? 'gap-0.5' : 'gap-2'}">
		{#if song.duration && !compact}
			<span class="text-xs text-muted-foreground">{song.duration}</span>
		{/if}
		{#if compact}
			<!-- Persistent, not hover-only: a filled heart is state the row has to keep showing. -->
			<button
				class="cursor-pointer rounded-md p-1.5 text-muted-foreground transition hover:bg-accent/20 hover:text-foreground"
				aria-label={isLiked(song) ? 'Remove from liked songs' : 'Save to liked songs'}
				aria-pressed={isLiked(song)}
				onclick={(e) => {
					e.stopPropagation();
					toggleLike(song);
				}}
			>
				<HugeiconsIcon
					icon={FavouriteIcon}
					class="h-4 w-4 {isLiked(song) ? 'fill-current text-primary' : ''}"
				/>
			</button>
		{/if}
		<TrackMenu
			{song}
			{onAdd}
			{onRemove}
			{removeLabel}
			triggerClass="cursor-pointer rounded-md p-1.5 text-muted-foreground transition hover:bg-accent/20 hover:text-foreground focus-visible:opacity-100 {compact
				? ''
				: 'opacity-0 group-hover:opacity-100'}"
		/>
	</div>
</div>
