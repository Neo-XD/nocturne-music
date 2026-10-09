<script lang="ts">
	// Edit home: a live side panel rather than a modal. Every change lands on home as it's made,
	// so the page behind is the preview and Done only closes. Reset puts home back the way it ships.
	import { tick, untrack } from 'svelte';
	import { Dialog as DialogPrimitive, RadioGroup } from 'bits-ui';
	import { HugeiconsIcon, type IconSvgElement } from '@hugeicons/svelte';
	import {
		ArrowTurnBackwardIcon,
		Cancel01Icon,
		Clock01Icon,
		DashboardSquare02Icon,
		DashboardSquareEditIcon,
		DragDropVerticalIcon,
		RefreshIcon,
		Tick02Icon,
		UserStar01Icon,
		YoutubeIcon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Switch } from '$lib/components/ui/switch';
	import * as Dialog from '$lib/components/ui/dialog';
	import { dragScroll, SECTION_ROW_MIME } from '$lib/dnd';
	import { personal, resetHome, saveHome } from '$lib/player.svelte';
	import { CARD_SIZES, hiddenSections, homeIsDefault, type CardSize } from '$lib/personal';

	let {
		open = $bindable(false),
		sections
	}: { open: boolean; sections: { key: string; title: string }[] } = $props();

	type Row = { key: string; title: string; shown: boolean; fresh: boolean };
	let rows = $state<Row[]>([]);
	let dragging = $state<number | null>(null);
	let list = $state<HTMLElement | null>(null);
	let announce = $state('');

	const shownCount = $derived(rows.filter((r) => r.shown).length);
	const customized = $derived(!homeIsDefault(personal));

	const OURS: Record<string, IconSvgElement> = {
		'@shortcuts': DashboardSquare02Icon,
		'@recent': ArrowTurnBackwardIcon,
		'@familiar': UserStar01Icon,
		'@forgotten': Clock01Icon
	};

	const SIZE_LABEL: Record<CardSize, string> = {
		small: 'Small',
		medium: 'Medium',
		large: 'Large'
	};
	const PREVIEW: Record<CardSize, { n: number; px: number }> = {
		small: { n: 4, px: 11 },
		medium: { n: 3, px: 15 },
		large: { n: 2, px: 24 }
	};

	function build() {
		const hidden = hiddenSections(personal);
		const ranked = new Set(personal.home.order);
		rows = sections.map((s) => ({
			key: s.key,
			title: s.title,
			shown: !hidden.has(s.key),
			fresh: ranked.size > 0 && !ranked.has(s.key)
		}));
		dragging = null;
	}

	$effect(() => {
		if (open) untrack(build);
	});

	const arrangement = () => ({
		order: rows.map((r) => r.key),
		hidden: rows.filter((r) => !r.shown).map((r) => r.key)
	});
	const commit = () => saveHome(arrangement());

	function moveTo(to: number) {
		if (dragging === null || dragging === to) return;
		const next = rows.slice();
		next.splice(to, 0, ...next.splice(dragging, 1));
		rows = next;
		dragging = to;
	}

	async function nudge(i: number, by: number) {
		const to = i + by;
		if (to < 0 || to >= rows.length) return;
		const next = rows.slice();
		next.splice(to, 0, ...next.splice(i, 1));
		rows = next;
		commit();
		const row = rows[to];
		announce = `${row.title} moved to position ${to + 1} of ${rows.length}`;
		await tick();
		list?.querySelector<HTMLElement>(`[data-grip="${CSS.escape(row.key)}"]`)?.focus();
	}

	function show(row: Row, on: boolean) {
		row.shown = on;
		commit();
	}

	function showAll(on: boolean) {
		for (const r of rows) r.shown = on;
		commit();
	}

	function setShowNew(on: boolean) {
		saveHome(on ? { hideNew: false } : { hideNew: true, ...arrangement() });
		if (on) build();
	}

	function reset() {
		resetHome();
		build();
	}
</script>

{#snippet toggleRow(title: string, desc: string, checked: boolean, set: (on: boolean) => void)}
	<div class="flex items-center justify-between gap-6 px-4 py-3.5">
		<div class="min-w-0">
			<div class="text-sm font-medium">{title}</div>
			<p class="mt-1 text-xs leading-relaxed text-muted-foreground">{desc}</p>
		</div>
		<Switch {checked} onCheckedChange={set} aria-label={title} />
	</div>
{/snippet}

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay
			class="bg-transparent bg-gradient-to-l from-black/50 via-black/15 to-transparent backdrop-blur-[2px]"
		/>
		<DialogPrimitive.Content
			class="fixed bottom-3 right-3 top-12 z-50 flex w-[25rem] max-w-[calc(100%-1.5rem)] flex-col overflow-hidden rounded-2xl border border-border/80 bg-background/95 backdrop-blur-xl text-foreground shadow-2xl outline-none duration-200 data-open:animate-in data-open:fade-in-0 data-open:slide-in-from-right-8 data-closed:animate-out data-closed:fade-out-0 data-closed:slide-out-to-right-8"
		>
			<div class="flex items-start gap-3 border-b border-border/60 px-5 py-4">
				<span
					class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-primary/15 text-primary shadow-xs"
				>
					<HugeiconsIcon icon={DashboardSquareEditIcon} class="h-5 w-5" />
				</span>
				<div class="min-w-0 flex-1">
					<Dialog.Title class="font-heading text-base font-semibold">Customize Home</Dialog.Title>
					<Dialog.Description class="text-xs leading-relaxed text-muted-foreground">
						Changes update live on the page behind as you make them.
					</Dialog.Description>
				</div>
				<Button
					variant="ghost"
					size="icon-sm"
					class="-mr-1.5 shrink-0 cursor-pointer text-muted-foreground hover:text-foreground"
					aria-label="Close"
					onclick={() => (open = false)}
				>
					<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4" />
				</Button>
			</div>

			<div
				class="min-h-0 flex-1 overflow-y-auto px-5 py-5"
				{@attach (node) => dragScroll(node, SECTION_ROW_MIME)}
			>
				<h3 class="mb-2 px-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
					Look & Layout
				</h3>
				<div class="mb-7 divide-y divide-border/60 overflow-hidden rounded-xl border border-border/60 bg-card/60 backdrop-blur-sm">
					<div class="px-4 py-3.5">
						<div class="text-sm font-medium">Card size</div>
						<p class="mt-1 text-xs leading-relaxed text-muted-foreground">How big albums, playlists, and artists appear on shelves.</p>
						<RadioGroup.Root
							value={personal.home.cards}
							onValueChange={(v) => saveHome({ cards: v as CardSize })}
							aria-label="Card size"
							class="mt-3 grid grid-cols-3 gap-2"
						>
							{#each CARD_SIZES as size (size)}
								<RadioGroup.Item
									value={size}
									class="group flex cursor-pointer flex-col items-center gap-2 rounded-xl border border-border/60 px-2 pb-2 pt-3 text-xs font-medium text-muted-foreground outline-none transition-all hover:border-foreground/25 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring data-checked:border-primary data-checked:bg-primary/10 data-checked:text-foreground"
								>
									<span class="flex h-6 items-end gap-[3px]" aria-hidden="true">
										{#each Array(PREVIEW[size].n) as _, i (i)}
											<span
												class="rounded-[3px] bg-muted-foreground/30 transition-colors group-data-checked:bg-primary"
												style="width:{PREVIEW[size].px}px;height:{PREVIEW[size].px}px"
											></span>
										{/each}
									</span>
									{SIZE_LABEL[size]}
								</RadioGroup.Item>
							{/each}
						</RadioGroup.Root>
					</div>
					{@render toggleRow(
						'Mood & vibe chips',
						'Show the quick genre and mood filter bar above home feed.',
						personal.home.chips,
						(on) => saveHome({ chips: on })
					)}
					{@render toggleRow(
						'Artwork backdrop',
						'Subtle album art glow behind the greeting header.',
						personal.home.backdrop,
						(on) => saveHome({ backdrop: on })
					)}
				</div>

				<div class="mb-2 flex items-baseline justify-between gap-3 px-1">
					<h3 class="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
						Sections
					</h3>
					<div class="flex items-baseline gap-3">
						<span class="text-xs tabular-nums text-muted-foreground">
							{shownCount} of {rows.length} shown
						</span>
						<button
							onclick={() => showAll(shownCount === 0)}
							disabled={!rows.length}
							class="cursor-pointer text-xs font-medium text-primary transition-colors hover:text-primary/80 disabled:pointer-events-none disabled:opacity-50"
						>
							{shownCount === 0 ? 'Turn all on' : 'Turn all off'}
						</button>
					</div>
				</div>
				<div
					bind:this={list}
					role="list"
					class="divide-y divide-border/60 overflow-hidden rounded-xl border border-border/60 bg-card/60 backdrop-blur-sm"
				>
					{#each rows as row, i (row.key)}
						<div
							role="listitem"
							draggable="true"
							ondragstart={(e) => {
								dragging = i;
								if (!e.dataTransfer) return;
								e.dataTransfer.setData(SECTION_ROW_MIME, row.key);
								e.dataTransfer.effectAllowed = 'move';
							}}
							ondragover={(e) => {
								if (dragging === null || !e.dataTransfer?.types.includes(SECTION_ROW_MIME)) return;
								e.preventDefault();
								e.dataTransfer.dropEffect = 'move';
								const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
								const past = e.clientY > r.top + r.height / 2;
								if (i > dragging ? past : !past) moveTo(i);
							}}
							ondragend={() => {
								dragging = null;
								commit();
							}}
							class="flex cursor-grab items-center gap-2.5 py-2 pl-1.5 pr-3 transition-colors {dragging ===
							i
								? 'bg-muted/70 opacity-60'
								: 'hover:bg-muted/40'}"
						>
							<button
								data-grip={row.key}
								aria-label={`Move section ${row.title}`}
								onkeydown={(e) => {
									if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
									e.preventDefault();
									nudge(i, e.key === 'ArrowUp' ? -1 : 1);
								}}
								class="flex h-8 w-6 shrink-0 cursor-grab items-center justify-center rounded-md text-muted-foreground/50 outline-none transition-colors hover:text-foreground focus-visible:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
							>
								<HugeiconsIcon icon={DragDropVerticalIcon} class="h-4 w-4" />
							</button>
							<HugeiconsIcon
								icon={OURS[row.key] ?? YoutubeIcon}
								class="h-4 w-4 shrink-0 transition-colors {row.shown
									? 'text-primary'
									: 'text-muted-foreground/40'}"
							/>
							<span
								class="min-w-0 flex-1 truncate text-sm transition-colors {row.shown
									? 'text-foreground'
									: 'text-muted-foreground line-through opacity-70'}"
								title={row.title}
							>
								{row.title}
							</span>
							{#if row.fresh}
								<span
									class="shrink-0 rounded-full bg-primary/15 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-primary"
								>
									New
								</span>
							{/if}
							<Switch
								size="sm"
								checked={row.shown}
								onCheckedChange={(on) => show(row, on)}
								aria-label={`Show section ${row.title}`}
							/>
						</div>
					{/each}
				</div>
				<p class="mt-2 px-1 text-xs leading-relaxed text-muted-foreground">
					Drag a section handle to reorder, or focus it and use arrow keys.
				</p>

				<div class="mt-4 overflow-hidden rounded-xl border border-border/60 bg-card/60 backdrop-blur-sm">
					{@render toggleRow(
						'Show new sections',
						'Automatically show newly added shelves from YouTube Music.',
						!personal.home.hideNew,
						setShowNew
					)}
				</div>
			</div>

			<div class="flex items-center gap-2 border-t border-border/60 px-5 py-3 bg-muted/20">
				<Button
					variant="ghost"
					size="sm"
					class="mr-auto text-muted-foreground hover:text-foreground cursor-pointer"
					disabled={!customized}
					onclick={reset}
				>
					<HugeiconsIcon icon={RefreshIcon} class="h-4 w-4 mr-1" />
					Reset
				</Button>
				<Button size="sm" onclick={() => (open = false)} class="cursor-pointer">
					<HugeiconsIcon icon={Tick02Icon} class="h-4 w-4 mr-1" />
					Done
				</Button>
			</div>

			<div class="sr-only" aria-live="polite">{announce}</div>
		</DialogPrimitive.Content>
	</Dialog.Portal>
</Dialog.Root>
