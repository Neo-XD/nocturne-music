<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Search01Icon,
		MusicNote01Icon,
		UserIcon,
		Cancel01Icon
	} from '@hugeicons/core-free-icons';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import ExplicitIcon from './ExplicitIcon.svelte';
	import type { BrowseItem } from '$lib/api';
	import { openItem, searchPreview } from '$lib/browse';
	import { MOD } from '$lib/shortcuts';
	import { thumb } from '$lib/thumb';
	import { onMount } from 'svelte';

	let query = $state('');
	let open = $state(false);
	let items = $state<BrowseItem[]>([]);
	let loading = $state(false);
	let active = $state(-1);
	let loadedFor = '';
	let debounce: ReturnType<typeof setTimeout> | undefined;
	let inputEl: HTMLInputElement | undefined = $state();

	const KIND = { song: 'Song', album: 'Album', artist: 'Artist', playlist: 'Playlist' };

	// Sync with search page ?q= if on /search
	$effect(() => {
		if (page.url.pathname === '/search') {
			const q = page.url.searchParams.get('q');
			if (q && q !== query && !inputEl?.matches(':focus')) {
				query = q;
			}
		}
	});

	async function load(q: string) {
		loadedFor = q;
		active = -1;
		loading = true;
		try {
			const next = await searchPreview(q);
			if (loadedFor === q) items = next;
		} catch {
			if (loadedFor === q) items = [];
		} finally {
			if (loadedFor === q) loading = false;
		}
	}

	function onType(e: Event & { currentTarget: HTMLInputElement }) {
		clearTimeout(debounce);
		const q = e.currentTarget.value.trim();
		if (q.length < 2) {
			close();
			return;
		}
		open = true;
		if (q !== loadedFor) {
			items = [];
			loading = true;
		}
		debounce = setTimeout(() => load(q), 350);
	}

	function close() {
		clearTimeout(debounce);
		open = false;
		loading = false;
		active = -1;
	}

	function choose(item: BrowseItem) {
		close();
		openItem(item);
		inputEl?.blur();
	}

	function submitSearch() {
		const q = query.trim();
		if (!q) return;
		close();
		inputEl?.blur();
		goto(`/search?q=${encodeURIComponent(q)}`);
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			close();
			inputEl?.blur();
		} else if (e.key === 'Enter') {
			if (active >= 0 && items[active]) {
				e.preventDefault();
				choose(items[active]);
			} else {
				e.preventDefault();
				submitSearch();
			}
		} else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && items.length) {
			e.preventDefault();
			open = true;
			const n = items.length;
			active = e.key === 'ArrowDown' ? (active + 1) % n : (active <= 0 ? n - 1 : active - 1);
		}
	}

	function clearQuery() {
		query = '';
		close();
		inputEl?.focus();
	}

	onMount(() => {
		function onGlobalKey(e: KeyboardEvent) {
			// Ctrl+K / Cmd+K or "/" when not typing in another input
			const target = e.target as HTMLElement | null;
			const isInput = target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable);
			if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
				e.preventDefault();
				inputEl?.focus();
				inputEl?.select();
			} else if (e.key === '/' && !isInput) {
				e.preventDefault();
				inputEl?.focus();
				inputEl?.select();
			}
		}
		window.addEventListener('keydown', onGlobalKey);
		return () => window.removeEventListener('keydown', onGlobalKey);
	});
</script>

<div
	class="relative w-full max-w-sm sm:max-w-md mx-auto"
	onfocusout={(e) => {
		if (!e.currentTarget.contains(e.relatedTarget as Node | null)) close();
	}}
>
	<form
		class="relative flex items-center"
		onsubmit={(e) => {
			e.preventDefault();
			submitSearch();
		}}
	>
		<!-- Non-drag region on search input so clicks, typing and focus work smoothly -->
		<div class="pointer-events-none absolute left-3 flex items-center justify-center text-muted-foreground">
			<HugeiconsIcon icon={Search01Icon} class="h-4 w-4" />
		</div>

		<input
			bind:this={inputEl}
			bind:value={query}
			type="text"
			placeholder="Search songs, artists, albums..."
			class="h-7.5 w-full rounded-full border border-border/60 bg-muted/40 pl-9 pr-16 text-xs text-foreground placeholder:text-muted-foreground/70 transition-all focus:border-primary/60 focus:bg-background focus:outline-none focus:ring-2 focus:ring-primary/20 hover:bg-muted/60"
			autocomplete="off"
			spellcheck="false"
			role="combobox"
			aria-expanded={open}
			aria-controls="top-search-suggest"
			oninput={onType}
			onkeydown={onKeydown}
			onfocus={() => {
				if (items.length && query.trim() === loadedFor) open = true;
			}}
		/>

		<div class="absolute right-2 flex items-center gap-1">
			{#if query}
				<button
					type="button"
					class="flex h-5 w-5 items-center justify-center rounded-full text-muted-foreground transition hover:bg-accent/20 hover:text-foreground cursor-pointer"
					onclick={clearQuery}
					aria-label="Clear search"
				>
					<HugeiconsIcon icon={Cancel01Icon} class="h-3 w-3" />
				</button>
			{:else}
				<kbd
					class="pointer-events-none rounded border border-border/60 bg-muted/60 px-1 py-0.5 font-mono text-[9px] font-medium text-muted-foreground select-none"
				>
					{MOD}K
				</kbd>
			{/if}
		</div>
	</form>

	{#if open}
		<div
			id="top-search-suggest"
			role="listbox"
			aria-label="Search preview"
			class="absolute top-full left-0 right-0 z-50 mt-1.5 max-h-[75vh] overflow-y-auto rounded-xl border border-border/80 bg-popover text-popover-foreground shadow-2xl backdrop-blur-xl animate-in fade-in-0 zoom-in-95 duration-150"
		>
			{#if loading && !items.length}
				{#each Array(4) as _, i (i)}
					<div class="flex items-center gap-3 px-3 py-2">
						<Skeleton class="h-9 w-9 shrink-0 rounded-md" />
						<div class="min-w-0 flex-1">
							<Skeleton class="h-3 w-36 rounded" />
							<Skeleton class="mt-1.5 h-2.5 w-20 rounded" />
						</div>
					</div>
				{/each}
			{:else if !items.length}
				<div class="px-4 py-3 text-xs text-muted-foreground">
					No instant preview. Press <kbd class="rounded border px-1 py-0.5 text-[10px]">Enter</kbd> to search YouTube.
				</div>
			{:else}
				{#each items as item, i (item.id)}
					{@const hero = i === 0}
					<button
						type="button"
						role="option"
						aria-selected={i === active}
						class="flex w-full cursor-pointer items-center gap-3 px-3 text-left transition-colors {i === active
							? 'bg-accent/60'
							: 'hover:bg-accent/40'} {hero ? 'border-b border-border/40 py-2.5' : 'py-1.5'}"
						onmousedown={(e) => e.preventDefault()}
						onmouseenter={() => (active = i)}
						onclick={() => choose(item)}
					>
						{#if item.thumbnail}
							<img
								src={thumb(item.thumbnail, 200)}
								alt=""
								class="shrink-0 object-cover {item.kind === 'artist'
									? 'rounded-full'
									: 'rounded-md'} {hero ? 'h-10 w-10' : 'h-8 w-8'}"
							/>
						{:else}
							<div
								class="flex shrink-0 items-center justify-center bg-muted text-muted-foreground/50 {item.kind === 'artist'
									? 'rounded-full'
									: 'rounded-md'} {hero ? 'h-10 w-10' : 'h-8 w-8'}"
							>
								<HugeiconsIcon
									icon={item.kind === 'artist' ? UserIcon : MusicNote01Icon}
									class="h-4 w-4"
								/>
							</div>
						{/if}
						<div class="min-w-0 flex-1">
							<div class="truncate {hero ? 'text-xs font-semibold' : 'text-xs font-medium'} leading-tight">
								{item.title}
							</div>
							<div class="flex items-center gap-1 text-[11px] text-muted-foreground">
								{#if item.explicit}
									<ExplicitIcon class="h-2.5 w-2.5 shrink-0" />
								{/if}
								<span class="truncate">
									{KIND[item.kind]}{item.subtitle ? ` • ${item.subtitle}` : ''}
								</span>
							</div>
						</div>
						{#if hero}
							<span
								class="shrink-0 rounded-full bg-primary/10 px-2 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-primary"
							>
								Top result
							</span>
						{/if}
					</button>
				{/each}
			{/if}

			<button
				type="button"
				class="flex w-full cursor-pointer items-center gap-2 border-t border-border/60 bg-muted/20 px-3 py-2 text-left text-xs font-medium text-muted-foreground transition hover:bg-accent/40 hover:text-foreground"
				onmousedown={(e) => e.preventDefault()}
				onclick={submitSearch}
			>
				<HugeiconsIcon icon={Search01Icon} class="h-3.5 w-3.5" />
				<span>All results for “{query.trim()}”</span>
			</button>
		</div>
	{/if}
</div>
