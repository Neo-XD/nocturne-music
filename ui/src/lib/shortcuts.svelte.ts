// App-wide keyboard shortcuts and custom keybindings system.
import { browser } from '$app/environment';
import * as api from './api';
import { cycleRepeat, np, nudgeVolume, playback, toggleMute, ui, toast } from './player.svelte';

export type ShortcutAction =
	| 'search'
	| 'playPause'
	| 'nextTrack'
	| 'prevTrack'
	| 'shuffle'
	| 'repeat'
	| 'mute'
	| 'volumeUp'
	| 'volumeDown'
	| 'fullscreen'
	| 'nowPlaying'
	| 'shortcutsList';

export interface ShortcutDefinition {
	id: ShortcutAction;
	label: string;
	description: string;
	group: 'Playback' | 'Navigation' | 'General';
	defaultKey: string;
}

export const SHORTCUT_DEFINITIONS: ShortcutDefinition[] = [
	{
		id: 'search',
		label: 'Search & Command Palette',
		description: 'Focus top search bar or open command palette',
		group: 'Navigation',
		defaultKey: 'Ctrl+Space'
	},
	{
		id: 'playPause',
		label: 'Play / Pause',
		description: 'Toggle audio playback',
		group: 'Playback',
		defaultKey: 'Space'
	},
	{
		id: 'nextTrack',
		label: 'Next track',
		description: 'Advance to the next track in queue',
		group: 'Playback',
		defaultKey: 'Ctrl+F'
	},
	{
		id: 'prevTrack',
		label: 'Previous track',
		description: 'Go back to previous track or start of current song',
		group: 'Playback',
		defaultKey: 'Ctrl+D'
	},
	{
		id: 'shuffle',
		label: 'Shuffle queue',
		description: 'Toggle queue shuffle order',
		group: 'Playback',
		defaultKey: 'Ctrl+S'
	},
	{
		id: 'repeat',
		label: 'Repeat mode',
		description: 'Cycle repeat (Off, All, One)',
		group: 'Playback',
		defaultKey: 'Ctrl+R'
	},
	{
		id: 'mute',
		label: 'Mute / Unmute',
		description: 'Toggle audio mute',
		group: 'Playback',
		defaultKey: 'Ctrl+M'
	},
	{
		id: 'volumeUp',
		label: 'Volume up',
		description: 'Increase volume by 5%',
		group: 'Playback',
		defaultKey: 'Ctrl+>'
	},
	{
		id: 'volumeDown',
		label: 'Volume down',
		description: 'Decrease volume by 5%',
		group: 'Playback',
		defaultKey: 'Ctrl+<'
	},
	{
		id: 'fullscreen',
		label: 'Toggle fullscreen player',
		description: 'Open or close immersive fullscreen player view',
		group: 'General',
		defaultKey: 'F11'
	},
	{
		id: 'nowPlaying',
		label: 'Toggle now playing view',
		description: 'Open or close now playing panel / lyrics',
		group: 'General',
		defaultKey: 'Ctrl+E'
	},
	{
		id: 'shortcutsList',
		label: 'Keyboard shortcuts list',
		description: 'Show the keyboard shortcuts dialog',
		group: 'General',
		defaultKey: 'Ctrl+H'
	}
];

export const DEFAULT_KEYBINDINGS: Record<ShortcutAction, string> = {
	search: 'Ctrl+Space',
	playPause: 'Space',
	nextTrack: 'Ctrl+F',
	prevTrack: 'Ctrl+D',
	shuffle: 'Ctrl+S',
	repeat: 'Ctrl+R',
	mute: 'Ctrl+M',
	volumeUp: 'Ctrl+>',
	volumeDown: 'Ctrl+<',
	fullscreen: 'F11',
	nowPlaying: 'Ctrl+E',
	shortcutsList: 'Ctrl+H'
};

export const MOD = browser && navigator.platform.startsWith('Mac') ? '⌘' : 'Ctrl+';
const VOLUME_STEP = 5;

// Reactive state holding user-configured keybindings
export const keybindings = $state<Record<ShortcutAction, string>>({ ...DEFAULT_KEYBINDINGS });

let searchInputEl: HTMLInputElement | undefined = undefined;

export function registerSearchInput(el: HTMLInputElement | undefined) {
	searchInputEl = el;
}

export function focusSearchInput() {
	if (searchInputEl && document.contains(searchInputEl)) {
		searchInputEl.focus();
		searchInputEl.select();
	} else {
		ui.paletteOpen = !ui.paletteOpen;
	}
}

/** Check whether user is currently typing in an input element */
export const isTyping = (t: EventTarget | null) =>
	t instanceof HTMLElement &&
	(t.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName));

/** Format a canonical shortcut string for UI display (e.g. replacing Ctrl+ with ⌘ on Mac) */
export function formatKey(combo: string): string {
	if (!combo) return 'None';
	if (browser && navigator.platform.startsWith('Mac')) {
		return combo.replace(/Ctrl\+/g, '⌘').replace(/Alt\+/g, '⌥').replace(/Shift\+/g, '⇧');
	}
	return combo;
}

/** Normalize a KeyboardEvent into a canonical keybinding string */
export function normalizeEvent(e: KeyboardEvent): string | null {
	// Ignore bare modifier keypresses
	if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return null;

	const parts: string[] = [];
	if (e.ctrlKey || e.metaKey) parts.push('Ctrl');
	if (e.altKey) parts.push('Alt');
	if (e.shiftKey && parts.length > 0) parts.push('Shift');

	let keyName = e.key;
	if (e.code === 'Space' || keyName === ' ') {
		keyName = 'Space';
	} else if (keyName.length === 1) {
		keyName = keyName.toUpperCase();
	}

	parts.push(keyName);
	return parts.join('+');
}

/** Set and persist a custom keybinding */
export async function setKeybinding(action: ShortcutAction, combo: string) {
	keybindings[action] = combo;
	try {
		localStorage.setItem('nocturne:keybindings', JSON.stringify(keybindings));
		await api.setSetting('custom_keybindings', JSON.stringify(keybindings));
	} catch {}
}

/** Reset all keybindings to factory defaults */
export async function resetKeybindings() {
	for (const def of SHORTCUT_DEFINITIONS) {
		keybindings[def.id] = def.defaultKey;
	}
	try {
		localStorage.removeItem('nocturne:keybindings');
		await api.setSetting('custom_keybindings', JSON.stringify(keybindings));
		toast.success('Keybindings reset to defaults');
	} catch {}
}

/** Initialize keybindings from settings or localStorage */
export async function initKeybindings() {
	if (!browser) return;
	try {
		const s = await api.getSettings().catch(() => null);
		if (s?.custom_keybindings) {
			const parsed = JSON.parse(s.custom_keybindings);
			Object.assign(keybindings, parsed);
			return;
		}
	} catch {}

	try {
		const ls = localStorage.getItem('nocturne:keybindings');
		if (ls) {
			Object.assign(keybindings, JSON.parse(ls));
		}
	} catch {}
}

function matchesCombo(e: KeyboardEvent, combo: string): boolean {
	if (!combo) return false;
	const parts = combo.split('+');
	const keyPart = parts[parts.length - 1];
	const reqCtrl = parts.includes('Ctrl') || parts.includes('⌘');
	const reqAlt = parts.includes('Alt') || parts.includes('⌥');
	const reqShift = parts.includes('Shift') || parts.includes('⇧');

	const hasCtrl = e.ctrlKey || e.metaKey;
	const hasAlt = e.altKey;
	const hasShift = e.shiftKey;

	if (reqCtrl !== hasCtrl || reqAlt !== hasAlt || (reqShift !== hasShift && parts.length > 1)) {
		return false;
	}

	// Match key
	if (keyPart === 'Space') {
		return e.code === 'Space' || e.key === ' ';
	}
	if (keyPart.startsWith('F') && keyPart.length >= 2) {
		return e.key.toUpperCase() === keyPart.toUpperCase();
	}
	if (keyPart === '>' || keyPart === '.') {
		return e.key === '>' || e.key === '.';
	}
	if (keyPart === '<' || keyPart === ',') {
		return e.key === '<' || e.key === ',';
	}
	return e.key.toLowerCase() === keyPart.toLowerCase();
}

export function initShortcuts() {
	initKeybindings();

	const onKey = (e: KeyboardEvent) => {
		const typingNow = isTyping(e.target);

		// 1. Check search shortcut (default Ctrl+Space)
		if (matchesCombo(e, keybindings.search)) {
			e.preventDefault();
			focusSearchInput();
			return;
		}

		// Also support Ctrl+K as secondary fallback search shortcut if search is custom
		if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k' && keybindings.search !== 'Ctrl+K') {
			e.preventDefault();
			focusSearchInput();
			return;
		}

		// 2. Fullscreen toggle
		if (matchesCombo(e, keybindings.fullscreen)) {
			if (!playback.now) return;
			np.fullscreenOpen = !np.fullscreenOpen;
			e.preventDefault();
			return;
		}

		// 3. Play / Pause (Space or ;)
		if (matchesCombo(e, keybindings.playPause) || (!e.ctrlKey && !e.metaKey && e.key === ';')) {
			if (typingNow || e.altKey || e.shiftKey) return;
			api.togglePause();
			e.preventDefault();
			return;
		}

		// Don't intercept shortcuts when typing in inputs unless Ctrl or Alt is held
		if (typingNow && !e.ctrlKey && !e.metaKey && !e.altKey) {
			return;
		}

		// 4. Playback controls
		if (matchesCombo(e, keybindings.nextTrack)) {
			api.nextTrack();
			e.preventDefault();
			return;
		}
		if (matchesCombo(e, keybindings.prevTrack)) {
			api.prevTrack();
			e.preventDefault();
			return;
		}
		if (matchesCombo(e, keybindings.shuffle)) {
			api.toggleShuffle();
			e.preventDefault();
			return;
		}
		if (matchesCombo(e, keybindings.repeat)) {
			cycleRepeat();
			e.preventDefault();
			return;
		}
		if (matchesCombo(e, keybindings.mute)) {
			toggleMute();
			e.preventDefault();
			return;
		}
		if (matchesCombo(e, keybindings.volumeUp)) {
			nudgeVolume(VOLUME_STEP);
			e.preventDefault();
			return;
		}
		if (matchesCombo(e, keybindings.volumeDown)) {
			nudgeVolume(-VOLUME_STEP);
			e.preventDefault();
			return;
		}

		// 5. General navigation
		if (matchesCombo(e, keybindings.nowPlaying)) {
			if (!playback.now) return;
			np.open = !np.open;
			e.preventDefault();
			return;
		}
		if (matchesCombo(e, keybindings.shortcutsList)) {
			ui.shortcutsOpen = !ui.shortcutsOpen;
			e.preventDefault();
			return;
		}
	};

	window.addEventListener('keydown', onKey);
	return () => window.removeEventListener('keydown', onKey);
}
