/**
 * Reparent a `fixed` popup to <body>.
 *
 * `position: fixed` is only viewport-relative while no ancestor is a containing block for it, and
 * `contain` / `content-visibility` / `transform` / `filter` on any ancestor all make one. Shelf uses
 * content-visibility, so an in-place popup anchored by `anchorMenu` lands at the shelf's offset
 * instead of the viewport's and is clipped away by paint containment: the further down the feed, the
 * further off it goes. At <body> there is nothing above it to contain it.
 */
export function toBody(el: HTMLElement) {
	document.body.appendChild(el);
	return () => el.remove();
}

/** Inline style + transform origin for a `fixed` popup, from `anchorMenu`. */
export type Anchor = { style: string; origin: string };

// Written out rather than built from the two flags: Tailwind only emits a class it can see as a
// literal string somewhere in the source.
const ORIGIN = {
	'top-left': 'origin-top-left',
	'top-right': 'origin-top-right',
	'bottom-left': 'origin-bottom-left',
	'bottom-right': 'origin-bottom-right'
};

/**
 * Place a `fixed` menu for whatever opened it: a click on a ⋯ trigger anchors to the trigger's box
 * (`e.currentTarget`), a right-click anchors to the pointer. Flips up when the viewport bottom is
 * too close for the menu to fit, and hangs off the right edge when the left one would overflow.
 *
 * ponytail: `height`/`width` are estimates rather than a measure-then-place pass — these menus are
 * small and a few px of early flip is fine.
 */
export function anchorMenu(
	e: Event,
	{
		height = 280,
		width = 224,
		align = 'left'
	}: { height?: number; width?: number; align?: 'left' | 'right' } = {}
): Anchor {
	const atPointer = e.type === 'contextmenu';
	// A point is just a zero-size box, so both cases share the arithmetic below.
	const { clientX: px, clientY: py } = e as MouseEvent;
	const r = atPointer
		? { left: px, right: px, top: py, bottom: py }
		: (e.currentTarget as HTMLElement).getBoundingClientRect();
	const gap = atPointer ? 0 : 4;

	const up = r.bottom + gap + height > window.innerHeight && r.top > height;
	const right = align === 'right' || r.left + width > window.innerWidth;

	const x = right ? `right:${window.innerWidth - r.right}px` : `left:${r.left}px`;
	const y = up ? `bottom:${window.innerHeight - r.top + gap}px` : `top:${r.bottom + gap}px`;
	return { style: `${x};${y}`, origin: ORIGIN[`${up ? 'bottom' : 'top'}-${right ? 'right' : 'left'}`] };
}

/**
 * Where WebKit's own menu still earns its place: a text field, a selection sitting under the
 * pointer, and a held Shift. The first two carry cut/copy/paste; everywhere else the menu is
 * back / reload / inspect, which is nothing you can do to a song.
 */
function wantsNative(e: MouseEvent) {
	// Shift+right-click is the browser convention for "give me the real menu anyway", and it is how
	// you still reach Inspect Element in a dev build.
	if (e.shiftKey) return true;
	const t = e.target;
	if (!(t instanceof Element)) return false;
	if (t.closest('input, textarea, [contenteditable]')) return true;
	const sel = window.getSelection();
	return !!sel && !sel.isCollapsed && sel.containsNode(t, true);
}

/**
 * Window-level handler that takes the native menu away everywhere it is useless. Our own menus
 * (`ctxHost` below) stop the event before it reaches here, so this only ever sees the leftovers.
 */
export function suppressNative(e: MouseEvent) {
	if (!wantsNative(e)) e.preventDefault();
}

/**
 * Attachment for a ⋯ trigger: right-clicking anywhere inside its nearest `[data-ctx]` ancestor
 * opens the same menu at the pointer. The host marks the region (a row, a card, the now-playing
 * block); the menu that lives inside it does the wiring, so a host only ever grows one attribute.
 *
 * Nested hosts are fine: the innermost one wins, because `open` stops the event.
 */
export function ctxHost(open: (e: MouseEvent) => void) {
	return (trigger: HTMLElement) => {
		const host = trigger.closest('[data-ctx]');
		if (!host) return;
		const onContextMenu = (e: Event) => {
			if (wantsNative(e as MouseEvent)) return; // let the field keep its paste menu
			open(e as MouseEvent);
		};
		host.addEventListener('contextmenu', onContextMenu);
		return () => host.removeEventListener('contextmenu', onContextMenu);
	};
}
