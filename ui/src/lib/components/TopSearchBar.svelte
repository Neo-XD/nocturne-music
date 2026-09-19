<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Search01Icon,
		MusicNote01Icon,
		UserIcon,
		Cancel01Icon,
		AudioWave01Icon
	} from '@hugeicons/core-free-icons';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import ExplicitIcon from './ExplicitIcon.svelte';
	import type { BrowseItem } from '$lib/api';
	import { asSong, openItem, searchPreview } from '$lib/browse';
	import { openAddToPlaylist, prefs } from '$lib/player.svelte';
	import TrackMenu from './TrackMenu.svelte';
	import SongRecognitionDialog from './SongRecognitionDialog.svelte';
	import { formatKey, keybindings, registerSearchInput } from '$lib/shortcuts.svelte';
	import { thumb } from '$lib/thumb';
	import { toBody } from '$lib/menu';
	import { onMount } from 'svelte';

	function attachPopup(node: HTMLElement) {
		updatePopupPosition();
		return toBody(node);
	}

	let query = $state('');
	let open = $state(false);
	let items = $state<BrowseItem[]>([]);
	let loading = $state(false);
	let active = $state(-1);
	let loadedFor = '';
	let debounce: ReturnType<typeof setTimeout> | undefined;
	let inputEl: HTMLInputElement | undefined = $state();
	let containerEl: HTMLElement | undefined = $state();
	let popupEl: HTMLElement | undefined = $state();
	let popupStyle = $state('');
	let shazamOpen = $state(false);

	let isTyping = $state(false);
	let typingTimer: ReturnType<typeof setTimeout> | undefined;

	function updatePopupPosition() {
		if (!inputEl) return;
		const rect = inputEl.getBoundingClientRect();
		// Spotlight-style centered geometry: comfortable width for reading results
		const popupWidth = Math.min(Math.max(rect.width, 480), Math.max(300, window.innerWidth - 32));
		const popupLeft = Math.max(16, Math.min(window.innerWidth - popupWidth - 16, rect.left + (rect.width - popupWidth) / 2));
		popupStyle = `top: ${Math.round(rect.bottom + 8)}px; left: ${Math.round(popupLeft)}px; width: ${Math.round(popupWidth)}px;`;
	}

	$effect(() => {
		if (open) {
			updatePopupPosition();
		}
	});

	$effect(() => {
		registerSearchInput(inputEl);
		return () => registerSearchInput(undefined);
	});

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
		clearTimeout(typingTimer);
		isTyping = true;
		typingTimer = setTimeout(() => {
			isTyping = false;
		}, 450);

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
		clearTimeout(typingTimer);
		isTyping = false;
		open = false;
		loading = false;
		active = -1;
	}

	function choose(item: BrowseItem) {
		close();
		openItem(item);
	}

	function submitSearch() {
		const q = query.trim();
		if (!q) return;
		close();
		goto(`/search?q=${encodeURIComponent(q)}`);
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape' && open) {
			e.preventDefault();
			close();
		} else if (e.key === 'Enter') {
			if (active >= 0 && items[active]) {
				e.preventDefault();
				choose(items[active]);
			} else {
				close();
				submitSearch();
			}
		} else if (e.key === 'ArrowDown') {
			e.preventDefault();
			if (!open && items.length) {
				open = true;
				active = 0;
			} else if (items.length) {
				active = (active + 1) % items.length;
			}
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			if (items.length) {
				active = active <= 0 ? items.length - 1 : active - 1;
			}
		}
	}

	function clearQuery() {
		query = '';
		close();
		if (page.url.pathname === '/search') {
			goto('/search', { replaceState: true });
		}
		inputEl?.focus();
	}

	onMount(() => {
		const handleScrollOrResize = () => {
			if (open) updatePopupPosition();
		};
		window.addEventListener('resize', handleScrollOrResize);
		window.addEventListener('scroll', handleScrollOrResize, true);

		const handlePointerDown = (e: PointerEvent) => {
			if (!open) return;
			const target = e.target as Node | null;
			if (containerEl?.contains(target) || popupEl?.contains(target)) return;
			close();
		};
		window.addEventListener('pointerdown', handlePointerDown);

		function onGlobalSlash(e: KeyboardEvent) {
			const target = e.target as HTMLElement | null;
			const isInput =
				target &&
				(target.tagName === 'INPUT' ||
					target.tagName === 'TEXTAREA' ||
					target.isContentEditable);
			if (e.key === '/' && !isInput) {
				e.preventDefault();
				inputEl?.focus();
				inputEl?.select();
			}
		}
		window.addEventListener('keydown', onGlobalSlash);
		return () => {
			window.removeEventListener('resize', handleScrollOrResize);
			window.removeEventListener('scroll', handleScrollOrResize, true);
			window.removeEventListener('pointerdown', handlePointerDown);
			window.removeEventListener('keydown', onGlobalSlash);
		};
	});
</script>

<div
	bind:this={containerEl}
	class="relative w-full max-w-[340px] mx-auto"
>
	<form
		class="relative flex items-center"
		onsubmit={(e) => {
			e.preventDefault();
			submitSearch();
		}}
	>
		<!-- Non-drag region on search input with smooth typing feedback on the icon -->
		<div class="pointer-events-none absolute left-2.5 flex items-center justify-center transition-all duration-200 {loading ? 'text-primary animate-pulse scale-110' : isTyping ? 'text-primary scale-105' : 'text-muted-foreground'}">
			<HugeiconsIcon icon={Search01Icon} class="h-3.5 w-3.5 transition-transform duration-200" />
		</div>

		<input
			bind:this={inputEl}
			bind:value={query}
			type="text"
			placeholder="Search songs, artists, albums..."
			class="h-7 w-full rounded-full border border-border/60 bg-muted/40 pl-8 pr-20 text-xs text-foreground placeholder:text-muted-foreground/70 transition-all duration-200 ease-out focus:border-primary/60 focus:bg-background focus:outline-none focus:ring-2 focus:ring-primary/25 focus:shadow-[0_0_12px_rgba(var(--primary-rgb,59,130,246),0.15)] hover:bg-muted/60"
			autocomplete="off"
			spellcheck="false"
			role="combobox"
			aria-expanded={open}
			aria-controls="top-search-suggest"
			oninput={onType}
			onkeydown={onKeydown}
		/>

		<div class="absolute right-2 flex items-center gap-1">
			{#if prefs.pcAudioRecognition}
				<button
					type="button"
					class="flex h-5 w-5 items-center justify-center rounded-full text-muted-foreground transition-all duration-150 hover:bg-primary/20 hover:text-primary cursor-pointer hover:scale-105"
					onclick={() => (shazamOpen = true)}
					title="Recognize music playing on PC (Shazam)"
					aria-label="Recognize music playing on PC"
				>
					<HugeiconsIcon icon={AudioWave01Icon} class="h-3.5 w-3.5" />
				</button>
			{/if}
			{#if query}
				<button
					type="button"
					class="flex h-5 w-5 items-center justify-center rounded-full text-muted-foreground transition-all duration-150 hover:bg-accent/30 hover:text-foreground cursor-pointer hover:scale-105"
					onclick={clearQuery}
					aria-label="Clear search"
				>
					<HugeiconsIcon icon={Cancel01Icon} class="h-3 w-3" />
				</button>
			{:else}
				<kbd
					class="pointer-events-none rounded border border-border/60 bg-muted/60 px-1.5 py-0.5 font-mono text-[9px] font-medium text-muted-foreground select-none transition-opacity duration-200"
				>
					{formatKey(keybindings.search)}
				</kbd>
			{/if}
		</div>
	</form>

	{#if open}
		<div
			{@attach attachPopup}
			bind:this={popupEl}
			id="top-search-suggest"
			role="listbox"
			aria-label="Search preview"
			class="fixed z-[100] max-h-[72vh] overflow-y-auto rounded-2xl border border-border/70 bg-popover/85 dark:bg-card/90 text-popover-foreground shadow-[0_20px_50px_-10px_rgba(0,0,0,0.5)] backdrop-blur-3xl ring-1 ring-white/10 animate-in fade-in-0 zoom-in-[0.98] slide-in-from-top-2 duration-200 ease-out p-1.5"
			style={popupStyle}
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
					<div
						role="option"
						data-ctx
						tabindex="-1"
						aria-selected={i === active}
						class="group relative flex w-full cursor-pointer items-center gap-3 rounded-xl px-2.5 text-left transition-all duration-150 ease-out {i === active
							? 'bg-accent/80 text-accent-foreground shadow-xs translate-x-0.5'
							: 'hover:bg-accent/40 text-muted-foreground hover:text-foreground'} {hero ? 'mb-1 border border-primary/20 bg-primary/5 py-2' : 'py-1.5'}"
						onmouseenter={() => (active = i)}
						onclick={(e) => {
							if (e.target instanceof Element && e.target.closest('button.track-menu-trigger, .track-menu-portal')) return;
							choose(item);
						}}
						onkeydown={(e) => {
							if (e.key === 'Enter' || e.key === ' ') {
								e.preventDefault();
								choose(item);
							}
						}}
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
									{KIND[item.kind as keyof typeof KIND] ?? item.kind}{item.subtitle ? ` • ${item.subtitle}` : ''}
								</span>
							</div>
						</div>
						{#if hero}
							<span
								class="shrink-0 rounded-full bg-primary/15 border border-primary/25 px-2 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-primary shadow-xs"
							>
								Top result
							</span>
						{/if}
						{#if item.kind === 'song'}
							<div
								class="shrink-0 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100 {i === active ? 'opacity-100' : ''}"
							>
								<TrackMenu
									song={asSong(item)}
									triggerClass="track-menu-trigger"
									onAdd={() => {
										close();
										openAddToPlaylist(asSong(item));
									}}
								/>
							</div>
						{/if}
					</div>
				{/each}
			{/if}

			<button
				type="button"
				class="flex w-full cursor-pointer items-center justify-between rounded-xl border border-transparent bg-muted/30 px-3 py-2 text-left text-xs font-medium text-muted-foreground transition-all duration-150 hover:bg-accent/50 hover:text-foreground hover:border-border/40 mt-1"
				onmousedown={(e) => e.preventDefault()}
				onclick={submitSearch}
			>
				<div class="flex items-center gap-2">
					<HugeiconsIcon icon={Search01Icon} class="h-3.5 w-3.5 text-primary" />
					<span class="truncate">All results for “<span class="font-semibold text-foreground">{query.trim()}</span>”</span>
				</div>
				<kbd class="pointer-events-none rounded border border-border/60 bg-muted/60 px-1.5 py-0.5 font-mono text-[9px] font-medium text-muted-foreground select-none">
					↵ Enter
				</kbd>
			</button>
		</div>
	{/if}

	{#if prefs.pcAudioRecognition}
		<SongRecognitionDialog bind:open={shazamOpen} />
	{/if}
</div>
