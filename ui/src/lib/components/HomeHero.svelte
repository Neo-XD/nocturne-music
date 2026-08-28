<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { UserGroup02Icon } from '@hugeicons/core-free-icons';
	import { auth, playback, ui } from '$lib/player.svelte';
	import { lt } from '$lib/lt.svelte';
	import { thumb } from '$lib/thumb';

	// Fixed at mount — a greeting that flips mid-session is uncanny.
	const hour = new Date().getHours();
	const daypart =
		hour < 5 ? 'Good night' : hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening';

	// Google's CDN doesn't serve every rewritten size, so a 404'd backdrop must degrade to nothing
	// rendered, never a broken-image glyph. Re-arm whenever the track changes, mirroring MediaCard.
	let artFailed = $state(false);
	$effect(() => {
		playback.now?.thumbnail; // re-arm when the track changes
		artFailed = false;
	});
</script>

<!-- overflow-hidden lives on the backdrop wrapper, not the hero: the scaled blur has to be clipped,
     but the search preview below has to hang out past the bottom edge. -->
<div class="relative bg-sidebar text-sidebar-foreground">
	<div class="relative p-6 pt-8">
		<div class="flex items-center justify-between gap-4">
			<div class="flex min-w-0 items-center gap-3">
				{#if auth.account?.signedIn && auth.account.thumbnail}
					<!-- max-width:none defeats Tailwind Preflight's `img{max-width:100%}`, which in a tight box
					     clamps width to the content-box while height stays fixed → a vertical oval. Inline so
					     it's immune to Preflight and to stale dev CSS. -->
					<img
						src={thumb(auth.account.thumbnail, 128)}
						alt=""
						style="width:2.75rem;height:2.75rem;max-width:none"
						class="shrink-0 rounded-full object-cover ring-2 ring-border"
					/>
				{/if}
				<h1 class="truncate font-heading text-4xl font-bold tracking-tight drop-shadow">
					{daypart}{auth.account?.name ? `, ${auth.account.name.split(' ')[0]}` : ''}
				</h1>
			</div>
			<div class="flex shrink-0 items-center gap-2">
				<button
					onclick={() => (ui.ltOpen = true)}
					title="Listen Together"
					aria-label="Listen Together"
					class="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-full border transition-colors {lt.role !==
					'none'
						? 'border-primary text-primary hover:bg-primary/10'
						: 'border-border text-muted-foreground hover:bg-muted hover:text-foreground'}"
				>
					<HugeiconsIcon icon={UserGroup02Icon} class="h-5 w-5" />
					{#if lt.role !== 'none'}
						<span
							class="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full bg-primary ring-2 ring-background"
						></span>
					{/if}
				</button>
			</div>
		</div>
	</div>
</div>
