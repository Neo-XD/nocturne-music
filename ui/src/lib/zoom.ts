import { getCurrentWebview } from '@tauri-apps/api/webview';

// Tauri's `zoomHotkeysEnabled` polyfill caps zoom-in at 1000%, which shreds the layout long before
// it gets there (fixed chrome overlaps, the player bar eats the page). Same hotkeys, our own
// ceiling. Not persisted: zoom resets with the window.
const MIN = 0.2;
const MAX = 1.8;
const STEP = 0.2;

let level = 1;

function apply(next: number) {
	next = Math.min(Math.max(next, MIN), MAX);
	if (next === level) return;
	level = next;
	getCurrentWebview().setZoom(level);
}

export function initZoom() {
	const onKey = (e: KeyboardEvent) => {
		if (!e.ctrlKey && !e.metaKey) return;
		if (e.key === '-') apply(level - STEP);
		else if (e.key === '=' || e.key === '+') apply(level + STEP);
		else if (e.key === '0') apply(1);
	};
	const onWheel = (e: WheelEvent) => {
		if (!e.ctrlKey) return;
		e.preventDefault();
		apply(level + (e.deltaY < 0 ? STEP : -STEP));
	};
	window.addEventListener('keydown', onKey);
	window.addEventListener('wheel', onWheel, { passive: false });
	return () => {
		window.removeEventListener('keydown', onKey);
		window.removeEventListener('wheel', onWheel);
	};
}
