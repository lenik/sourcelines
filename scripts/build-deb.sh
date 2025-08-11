#!/usr/bin/env bash
set -euo pipefail

# This script:
# 1) Updates release-version.txt
# 2) Adds a debian changelog entry
# 3) Builds the package with debuild
# 4) Installs the .deb with dpkg -i
# 5) Uploads using dput -f bodz

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_DIR"

RELEASE_FILE="release-version.txt"
DEB_DIR="debian"
PKG_NAME="wcc"
DEBUILD_ARGS=("-us" "-uc")
DPUT_TARGET="bodz"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || { echo "Error: required command '$1' not found in PATH" >&2; exit 1; }
}

for cmd in debuild dch dput dpkg cargo meson ninja git; do
  require_cmd "$cmd"
done

# 1) Update release-version.txt
if [[ ! -f "$RELEASE_FILE" ]]; then
  echo "0.0.1" > "$RELEASE_FILE"
  date '+%Y-%m-%d %H:%M:%S' >> "$RELEASE_FILE"
  printf "\n======= ABOUT ========\n\n    this file contains the current version information and release date time.\n    in format of\n        Line 1: major.minor.release\n        Line 2: datetime (YYYY-mm-dd HH:MM:SS)\n    \n    for auto build system:\n        each time to release a package should increase release number or minor number.\n    depends on:\n        - when new source file is added or source file is removed, it should increase the minor number and reset release number.\n        - when only editing on existing files, just increase the release number.\n    \n    when minor number is increased, the release number is reset to 0.\n\n    The ABOUT section is useful for copilot system.\n    Always preserve this ABOUT section as is.\n    Don't remove or do any change to this ABOUT section.\n\n======= END-OF-ABOUT ========\n" >> "$RELEASE_FILE"
fi

current_version=$(sed -n '1p' "$RELEASE_FILE")
current_date=$(date '+%Y-%m-%d %H:%M:%S')

# bump release number (x.y.z -> x.y.(z+1))
IFS='.' read -r major minor patch <<< "$current_version"
if [[ -z "${major:-}" || -z "${minor:-}" || -z "${patch:-}" ]]; then
  echo "Invalid version in $RELEASE_FILE: '$current_version'" >&2
  exit 1
fi
new_patch=$((patch + 1))
new_version="$major.$minor.$new_patch"

# Rewrite first two lines, keep ABOUT section intact
awk -v v="$new_version" -v d="$current_date" 'NR==1{print v; n=1; next} NR==2{print d; n=2; next} {print $0}' "$RELEASE_FILE" > "$RELEASE_FILE.tmp"
mv "$RELEASE_FILE.tmp" "$RELEASE_FILE"

echo "Version bumped: $current_version -> $new_version"

# 2) Add debian changelog entry
if [[ ! -d "$DEB_DIR" ]]; then
  mkdir -p "$DEB_DIR"
fi

require_cmd dch

# Ensure maintainer identity for dch
export DEBFULLNAME=${DEBFULLNAME:-$(git config --get user.name || echo "Packager")}
export DEBEMAIL=${DEBEMAIL:-$(git config --get user.email || echo "packager@example.com")}

dist_codename=$(lsb_release -sc 2>/dev/null || echo "unstable")

# Ensure debian/changelog exists
if [[ ! -f "$DEB_DIR/changelog" ]]; then
  dch --create --package "$PKG_NAME" --newversion "$new_version" --distribution "$dist_codename" --empty "Initial release"
else
  dch --newversion "$new_version" --distribution "$dist_codename" "Release $new_version"
fi

# 3) Build the package
export DEB_BUILD_OPTIONS="parallel=$(nproc)"
# Run debuild (it will invoke meson+ninja via debian/rules)
debuild "${DEBUILD_ARGS[@]}"

# 4) Install the .deb
DEB_FILE=$(ls -1t ../${PKG_NAME}_*.deb 2>/dev/null | head -n1 || true)
if [[ -n "${DEB_FILE}" ]]; then
  sudo dpkg -i "$DEB_FILE" || sudo apt-get -f install -y
else
  echo "Warning: .deb not found to install" >&2
fi

# 5) Upload using dput
CHANGES_FILE=$(ls -1t ../${PKG_NAME}_*.changes 2>/dev/null | head -n1 || true)
if [[ -n "${CHANGES_FILE}" ]]; then
  dput -f "$DPUT_TARGET" "$CHANGES_FILE"
else
  echo "Warning: .changes not found to upload" >&2
fi
