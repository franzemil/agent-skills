#!/usr/bin/env bash
# agent-skills one-line installer.
#
#   curl -fsSL https://raw.githubusercontent.com/franzemil/agent-skills/main/install.sh | bash
#
# Clones (or updates) the repo to ~/.agent-skills, obtains the installer binary
# (prebuilt release → cargo build → plain-bash fallback), puts `agent-skills`
# on PATH and launches the interactive wizard.
set -euo pipefail

# ------------------------------------------------------------------ config ---
REPO_URL="${AGENT_SKILLS_REPO:-https://github.com/franzemil/agent-skills.git}"
INSTALL_DIR="${AGENT_SKILLS_HOME:-$HOME/.agent-skills}"
BIN_DIR="${AGENT_SKILLS_BIN:-$HOME/.local/bin}"
BIN_NAME="agent-skills"
# -----------------------------------------------------------------------------

say()  { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*"; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

command -v git >/dev/null 2>&1 || die "git is required (https://git-scm.com)"

# ------------------------------------------------------------------- clone ---
if [ -d "$INSTALL_DIR/.git" ]; then
    say "Updating $INSTALL_DIR"
    git -C "$INSTALL_DIR" pull --ff-only || warn "could not update ( continuing with existing clone )"
elif [ -d "$INSTALL_DIR" ]; then
    die "$INSTALL_DIR exists but is not a git clone — remove it or set AGENT_SKILLS_HOME"
else
    say "Cloning $REPO_URL -> $INSTALL_DIR"
    git clone --depth 1 "$REPO_URL" "$INSTALL_DIR"
fi

cd "$INSTALL_DIR"

# --------------------------------------------------------------- platform ---
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
case "$OS:$ARCH" in
    darwin:arm64)  TARGET="aarch64-apple-darwin" ;;
    darwin:x86_64) TARGET="x86_64-apple-darwin" ;;
    linux:arm64|linux:aarch64) TARGET="aarch64-unknown-linux-musl" ;;
    linux:x86_64|linux:amd64)  TARGET="x86_64-unknown-linux-musl" ;;
    *) die "unsupported platform $OS $ARCH" ;;
esac

BIN=""
try_release_binary() {
    # Requires the repo to be on GitHub with tagged releases.
    local remote owner repo tag url tmp
    remote="$(git remote get-url origin 2>/dev/null || true)"
    case "$remote" in
        git@github.com:*) owner="${remote#git@github.com:}"; owner="${owner%%/*}" ;;
        https://github.com/*) owner="${remote#https://github.com/}"; owner="${owner%%/*}" ;;
        *) return 1 ;;
    esac
    repo="${remote##*/}"; repo="${repo%.git}"
    tag="$(git describe --tags --abbrev=0 2>/dev/null || true)"
    [ -n "$tag" ] || return 1
    url="https://github.com/$owner/$repo/releases/download/$tag/$BIN_NAME-$TARGET.tar.gz"
    tmp="$(mktemp -d)"
    if curl -fsSL "$url" -o "$tmp/bin.tar.gz"; then
        tar -xzf "$tmp/bin.tar.gz" -C "$tmp" || return 1
        if [ -x "$tmp/$BIN_NAME" ]; then
            mkdir -p "$INSTALL_DIR/bin"
            mv "$tmp/$BIN_NAME" "$INSTALL_DIR/bin/$BIN_NAME"
            BIN="$INSTALL_DIR/bin/$BIN_NAME"
            return 0
        fi
    fi
    return 1
}

try_cargo_build() {
    # rustup installs to ~/.cargo/bin which non-login shells may miss.
    if [ -f "$HOME/.cargo/env" ]; then
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi
    command -v cargo >/dev/null 2>&1 || return 1
    say "Building with cargo (release)"
    cargo build --release --quiet
    BIN="$INSTALL_DIR/target/release/$BIN_NAME"
}

bash_fallback() {
    warn "no prebuilt binary and no cargo — falling back to plain-bash copy mode"
    bash "$INSTALL_DIR/install-fallback.sh"
    exit $?
}

say "Obtaining the installer binary"
if ! try_release_binary; then
    if ! try_cargo_build; then
        bash_fallback
    fi
fi

# ------------------------------------------------------------------ on PATH ---
say "Linking $BIN_NAME into $BIN_DIR"
mkdir -p "$BIN_DIR"
ln -sf "$BIN" "$BIN_DIR/$BIN_NAME"
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) warn "$BIN_DIR is not on your PATH — add it: export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac

# ------------------------------------------------------------------ wizard ---
say "Launching wizard"
export AGENT_SKILLS_HOME="$INSTALL_DIR"
exec "$BIN" "$@"
