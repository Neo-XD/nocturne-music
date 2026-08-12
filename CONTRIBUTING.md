# Contributing

Thanks for wanting to help. This is a small project, so this is short.

## Getting set up

Prerequisites and the build are in the [README](README.md#building-from-source).
Windows and macOS specifics live in [docs/BUILD-PLATFORMS.md](docs/BUILD-PLATFORMS.md).

Run the app with hot reload:

```bash
cargo tauri dev
```

That is `cargo tauri`, not `pnpm tauri`: the CLI is the Rust one
(`cargo install tauri-cli`), and `ui/package.json` has no tauri script.

## Formatting

Run it, don't fight it:

```bash
cargo fmt --all
```

`rustfmt.toml` is committed, so `cargo fmt` matches the existing style and
should produce no changes outside the code you actually touched. If it wants to
reformat files you never opened, something is wrong with the config rather than
with you, so please open an issue instead of committing the churn.

Keep formatting out of feature commits either way. A reformat of unrelated files
buries the real change and makes review much harder.

## Tests

```bash
cargo test --all                                        # everything, no network
cargo test -p limusic-app --lib -- --ignored --nocapture   # hits live lyrics APIs
cd ui && pnpm check                                     # svelte-check + types
```

Tests that talk to the network are `#[ignore]`d so the default run works
offline. If you touch a lyrics provider, run the ignored ones: a provider whose
endpoint has changed returns "no lyrics" rather than an error, so it looks
exactly like a track that simply has none.

## Pull requests

- **Open from a branch, not your fork's `master`.** It keeps your default branch
  clean and makes it much easier to take your changes.
- One concern per PR where you can manage it.
- Say what you tested. "Played five tracks, checked light and dark" is worth
  more than a description of the code.

## House conventions

- **Icons: [HugeIcons](https://hugeicons.com) only** (`@hugeicons/svelte` plus
  `@hugeicons/core-free-icons`). Not Lucide, not a second icon set, not inline
  SVGs.
- **UI primitives: shadcn-svelte** before hand-rolling a component.
- **The frontend never talks to YouTube.** Everything YouTube-shaped stays
  behind the Rust command boundary; the UI goes through Tauri commands and
  events only.
- **Colours come from theme tokens** (`--foreground`, `--muted-foreground`, and
  friends), never hardcoded hex or rgb. There are light and dark themes, and a
  hardcoded white is invisible in half of them.

## A note on scope

This talks to YouTube's private API and breaks whenever YouTube changes
something. Anything extraction-related needs to be updatable: client versions in
config rather than hardcoded, and graceful degradation when a step fails.
