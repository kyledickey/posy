#!/bin/sh
# Installs posy: a little 3D bouquet for your terminal.
#
#   curl -fsSL https://raw.githubusercontent.com/kyledickey/posy/main/install.sh | sh
#
# It downloads the ready-made program for your computer from GitHub, checks
# it arrived intact, and puts it in ~/.local/bin. Things you can set first:
#
#   POSY_VERSION=0.2.0      install that version instead of the newest
#   POSY_INSTALL_DIR=/path  put it somewhere else
#   POSY_PLAIN=1            plain output (no gum)
#   POSY_DOWNLOAD_URL=url   fetch the files from here instead of GitHub

set -eu

REPO="kyledickey/posy"
GUM_VERSION="2.0.2"
INSTALL_DIR="${POSY_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${POSY_VERSION:-}"
PINK=212
GREY=245

WORK="$(mktemp -d "${TMPDIR:-/tmp}/posy-install.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT INT TERM

# ----- what kind of computer is this -----

case "$(uname -s)" in
Linux) os=linux gum_os=Linux ;;
Darwin) os=macos gum_os=Darwin ;;
*) os=other gum_os= ;;
esac
case "$(uname -m)" in
x86_64 | amd64) arch=x86_64 gum_arch=x86_64 ;;
arm64 | aarch64) arch=aarch64 gum_arch=arm64 ;;
*) arch=other gum_arch= ;;
esac
case "$os-$arch" in
linux-x86_64) TARGET=x86_64-unknown-linux-musl ;;
linux-aarch64) TARGET=aarch64-unknown-linux-musl ;;
macos-x86_64) TARGET=x86_64-apple-darwin ;;
macos-aarch64) TARGET=aarch64-apple-darwin ;;
*) TARGET= ;;
esac

# ----- pretty output, with gum when we can get it -----

GUM=
find_gum() {
	[ -z "${POSY_PLAIN:-}" ] || return 0
	[ -t 1 ] || return 0
	if command -v gum >/dev/null 2>&1; then
		GUM="$(command -v gum)"
		return 0
	fi
	# Borrow a copy just for this install; it is deleted afterwards.
	[ -n "$gum_os" ] && [ -n "$gum_arch" ] || return 0
	name="gum_${GUM_VERSION}_${gum_os}_${gum_arch}"
	url="https://github.com/charmbracelet/gum/releases/download/v${GUM_VERSION}/${name}.tar.gz"
	if curl -fsSL --max-time 20 -o "$WORK/gum.tar.gz" "$url" 2>/dev/null &&
		tar -xzf "$WORK/gum.tar.gz" -C "$WORK" 2>/dev/null &&
		[ -x "$WORK/$name/gum" ]; then
		GUM="$WORK/$name/gum"
	fi
}

banner() {
	if [ -n "$GUM" ]; then
		"$GUM" style --border rounded --border-foreground "$PINK" --padding "1 4" --margin "1 0" \
			--align center \
			"$("$GUM" style --foreground "$PINK" --bold '✿  posy  ✿')" \
			"$("$GUM" style --foreground "$GREY" 'a little bouquet for your terminal')"
	else
		printf '\n  posy: a little bouquet for your terminal\n\n'
	fi
}

# step "label" "detail": one finished line.
step() {
	if [ -n "$GUM" ]; then
		printf '  %s %s %s\n' \
			"$("$GUM" style --foreground "$PINK" '♥')" \
			"$1" \
			"$("$GUM" style --foreground "$GREY" "${2:-}")"
	else
		printf '  * %s %s\n' "$1" "${2:-}"
	fi
}

# busy "label" command...: run something slow behind a spinner. The work
# happens in the background and the spinner just waits for it, because gum
# can only run programs, not this script's own functions.
busy() {
	title="$1"
	shift
	if [ -n "$GUM" ]; then
		"$@" &
		job=$!
		"$GUM" spin --spinner dot --spinner.foreground "$PINK" --title "  $title" -- \
			sh -c "while kill -0 $job 2>/dev/null; do sleep 0.1; done" || true
		wait "$job"
	else
		printf '  ... %s\n' "$title"
		"$@"
	fi
}

fail() {
	if [ -n "$GUM" ]; then
		printf '\n  %s %s\n\n' "$("$GUM" style --foreground 203 --bold '✕')" "$1" >&2
	else
		printf '\n  error: %s\n\n' "$1" >&2
	fi
	exit 1
}

# ----- getting files from a release -----

# fetch NAME: download one file of the release into $WORK.
fetch() {
	if [ -n "${POSY_DOWNLOAD_URL:-}" ]; then
		url="$POSY_DOWNLOAD_URL/$1"
	elif [ -n "$VERSION" ]; then
		url="https://github.com/$REPO/releases/download/v$VERSION/$1"
	else
		url="https://github.com/$REPO/releases/latest/download/$1"
	fi
	if curl -fsSL -o "$WORK/$1" "$url" 2>/dev/null; then
		return 0
	fi
	# A private repository: use the GitHub CLI, if it is signed in.
	if command -v gh >/dev/null 2>&1; then
		if [ -n "$VERSION" ]; then
			gh release download "v$VERSION" --repo "$REPO" --pattern "$1" --clobber --dir "$WORK" 2>/dev/null
		else
			gh release download --repo "$REPO" --pattern "$1" --clobber --dir "$WORK" 2>/dev/null
		fi
		return $?
	fi
	return 1
}

checksum() {
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$1" | cut -d ' ' -f 1
	elif command -v shasum >/dev/null 2>&1; then
		shasum -a 256 "$1" | cut -d ' ' -f 1
	else
		return 1
	fi
}

# ----- go -----

for tool in curl tar; do
	command -v "$tool" >/dev/null 2>&1 || {
		printf 'posy needs %s to install; please install it and try again.\n' "$tool" >&2
		exit 1
	}
done

find_gum
banner

[ -n "$TARGET" ] || fail "there is no ready-made posy for $(uname -s) $(uname -m) yet.
    With Rust installed you can build it: cargo install --git https://github.com/$REPO"
step "this computer" "$os $arch"

archive="posy-$TARGET.tar.gz"
fetch_both() {
	fetch "$archive" && fetch checksums.txt
}
busy "downloading posy ${VERSION:-(newest)}" fetch_both ||
	fail "couldn't download posy from github.com/$REPO.
    Check your connection, or that version ${VERSION:-(newest)} exists."
step "downloaded" "$archive"

want="$(grep " \*\{0,1\}$archive\$" "$WORK/checksums.txt" | cut -d ' ' -f 1 || true)"
have="$(checksum "$WORK/$archive" || true)"
if [ -z "$have" ]; then
	step "couldn't check the download" "(no sha256sum or shasum here)"
elif [ -z "$want" ] || [ "$want" != "$have" ]; then
	fail "the download doesn't match its checksum, so nothing was installed."
else
	step "checked" "it arrived intact"
fi

tar -xzf "$WORK/$archive" -C "$WORK" || fail "couldn't unpack the download."
[ -f "$WORK/posy" ] || fail "the download didn't have posy in it."
mkdir -p "$INSTALL_DIR" || fail "couldn't create $INSTALL_DIR."
# Copy next to the old one and rename over it, so an update can never
# leave half a program behind.
cp "$WORK/posy" "$INSTALL_DIR/.posy-new" || fail "couldn't write to $INSTALL_DIR."
chmod 755 "$INSTALL_DIR/.posy-new"
mv -f "$INSTALL_DIR/.posy-new" "$INSTALL_DIR/posy"
installed="$("$INSTALL_DIR/posy" --version 2>/dev/null || echo posy)"
step "installed" "$installed"

# Show the folder the way people write it.
case "$INSTALL_DIR" in
"$HOME"/*) shown="~${INSTALL_DIR#"$HOME"}" ;;
*) shown="$INSTALL_DIR" ;;
esac

case ":$PATH:" in
*":$INSTALL_DIR:"*) on_path=1 ;;
*) on_path= ;;
esac

if [ -n "$GUM" ]; then
	echo
	if [ -n "$on_path" ]; then
		"$GUM" style --border rounded --border-foreground "$PINK" --padding "0 2" \
			"$("$GUM" style --bold 'all done ♥')" \
			"" \
			"$("$GUM" style --foreground "$PINK" 'posy')            see today's bouquet" \
			"$("$GUM" style --foreground "$PINK" 'posy -b roses')   or a dozen roses" \
			"$("$GUM" style --foreground "$PINK" 'posy update')     get the newest version later"
	else
		"$GUM" style --border rounded --border-foreground "$PINK" --padding "0 2" \
			"$("$GUM" style --bold 'almost done ♥')" \
			"" \
			"$shown isn't somewhere your terminal looks for programs." \
			"Add this line to your shell's startup file, then open a new terminal:" \
			"" \
			"$("$GUM" style --foreground "$PINK" "  export PATH=\"$INSTALL_DIR:\$PATH\"")" \
			"" \
			"Or run it right now with: $("$GUM" style --foreground "$PINK" "$shown/posy")"
	fi
	echo
else
	if [ -n "$on_path" ]; then
		printf '\n  all done. Run: posy\n\n'
	else
		printf '\n  almost done: %s is not on your PATH.\n  Add:  export PATH="%s:$PATH"\n  Or run it now with: %s/posy\n\n' \
			"$INSTALL_DIR" "$INSTALL_DIR" "$INSTALL_DIR"
	fi
fi
