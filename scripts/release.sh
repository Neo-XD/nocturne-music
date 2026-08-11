#!/usr/bin/env bash
# Cut a signed release: build the .rpm, publish it to GitHub Releases with an empty updater
# manifest, and let CI fill in the rest.
#
# The AppImage is NOT built here. An AppImage inherits its build host's glibc floor, and this
# machine is Fedora (newest glibc in existence) — one built here starts nowhere else. It is built by
# .github/workflows/linux-release.yml on a pinned older runner, which also attaches it, adds the
# linux-x86_64 entry to latest.json and marks the release "Latest". Windows does the same for its
# own entry. So this script publishes the release NOT-latest on purpose: until CI has attached the
# binaries, the updater endpoint (.../releases/latest/download/latest.json) keeps resolving to the
# previous, complete manifest instead of one with no platforms in it.
#
# Usage:  scripts/release.sh ["release notes"]
# Bump "version" in src-tauri/tauri.conf.json BEFORE running (that's the app version the updater
# compares against; it overrides the Cargo version when present).
#
# Requires: the private signing key at ~/.tauri/limusic.key, `gh` authed, jq.
set -euo pipefail
cd "$(dirname "$0")/.."

REPO="SimoHypers/limusic"
KEY="${TAURI_SIGNING_PRIVATE_KEY_FILE:-$HOME/.tauri/limusic.key}"
NOTES="${1:-See the commit history for changes.}"

VERSION="$(jq -r .version src-tauri/tauri.conf.json)"
[ "$VERSION" != "null" ] && [ -n "$VERSION" ] || { echo "no version in tauri.conf.json"; exit 1; }
TAG="v$VERSION"
echo "==> Releasing $TAG"

[ -f "$KEY" ] || { echo "signing key not found at $KEY"; exit 1; }
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

# Bake a snapshot of the community player-cipher registry into the binary, same as both CI
# workflows do. src-tauri/cipher_configs.json is `include_str!`d and tracked-empty; the app
# refreshes it from these registries at runtime, so the snapshot only matters on a first run that
# can't reach raw.githubusercontent.com — without it that user has no cipher and every restricted
# track is skipped. Never fatal: a third-party registry being down must not block a release.
#
# This machine is not a throwaway CI checkout, so the file is put back on exit however the script
# ends — a 34 KB snapshot left staged in the working tree would get committed by accident.
CIPHER_CONFIGS=src-tauri/cipher_configs.json
restore_cipher_configs() { git checkout -- "$CIPHER_CONFIGS" 2>/dev/null || true; }
trap restore_cipher_configs EXIT

echo "==> Baking in the player-cipher registry…"
snap="$(mktemp)"
for url in \
  https://raw.githubusercontent.com/MetrolistGroup/faraday/master/registry/player_configs.json \
  https://raw.githubusercontent.com/ZemerTeam/zemer-cipher/master/library/src/main/assets/player_configs.json
do
  # Validate before overwriting: a 404 body must not replace the table with something the app
  # silently parses as empty.
  if curl -fsSL --max-time 30 "$url" -o "$snap" \
     && jq -e '(.players | objects | length) > 0' "$snap" >/dev/null 2>&1; then
    cp "$snap" "$CIPHER_CONFIGS"
    echo "    bundled $(jq '.players | length' "$snap") player configs"
    break
  fi
  echo "    WARNING: player-cipher registry unusable: $url"
done

echo "==> Building the rpm…"
cargo tauri build --bundles rpm

# Pin to $VERSION — a stale bundle from a previous build otherwise sorts first and gets shipped
# (e.g. an old 0.1.1 rpm uploaded to the 0.1.2 release).
RPM="$(ls target/release/bundle/rpm/limusic-${VERSION}-*.rpm 2>/dev/null | head -1)"
[ -n "$RPM" ] || { echo "no rpm for $VERSION in target/release/bundle/rpm"; exit 1; }

# latest.json: the manifest the updater reads. Published with no platforms — each CI workflow
# merges its own entry in (jq, so they don't clobber each other) once its binary is attached.
mkdir -p target/release/bundle
cat > target/release/bundle/latest.json <<EOF
{
  "version": "$VERSION",
  "notes": $(jq -Rs . <<<"$NOTES"),
  "pub_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "platforms": {}
}
EOF

echo "==> Publishing GitHub release $TAG…"
# --latest=false: the Linux workflow flips it once the AppImage is on the release. See the header.
gh release create "$TAG" \
  --repo "$REPO" \
  --title "$TAG" \
  --notes "$NOTES" \
  --latest=false \
  "$RPM" target/release/bundle/latest.json

# Dispatched from master rather than fired by the `release: published` event, and that ref matters:
# Actions caches are readable only from the ref that wrote them plus the default branch. A
# release-triggered run executes on the tag ref, so it could never reuse the previous release's
# cache — Windows recompiled all of Rust from scratch every time, 11m25s of a 15m run. Dispatching
# from master shares one cache across releases. Both workflows check out $TAG regardless, so the
# binaries still come from the tagged source. They also run in parallel now.
echo "==> Dispatching the build workflows…"
gh workflow run "Linux release binaries"   --repo "$REPO" --ref master -f tag="$TAG"
gh workflow run "Windows release binaries" --repo "$REPO" --ref master -f tag="$TAG"

echo "==> Published $TAG (not yet marked Latest). Both builds are running in parallel; when Linux"
echo "    finishes it marks the release Latest and testers get prompted."
echo "    Watch: gh run watch   |   Actions ▸ Linux / Windows release binaries"
