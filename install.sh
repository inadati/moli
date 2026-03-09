#!/bin/sh

set -e

REPO="inadati/moli"

VERSION=$(gh release view --repo $REPO --json tagName -q '.tagName')

OS="$(uname -s)"
ARCH="$(uname -m)"

case $OS in
    "Linux")
        case $ARCH in
            "x86_64")
                TARGET=x86_64-unknown-linux-musl
            ;;
            "aarch64")
                TARGET=aarch64-unknown-linux-gnu
            ;;
        esac
    ;;
    "Darwin")
        case $ARCH in
            "x86_64")
              TARGET=x86_64-apple-darwin
            ;;
            "arm64")
              TARGET=aarch64-apple-darwin
            ;;
        esac
    ;;
esac

INSTALL_TARGET="moli-${VERSION}-${TARGET}.tar.gz"

HOME_BIN="$HOME/.local/bin"
if [ ! -e "$HOME_BIN" ]; then
    mkdir -p $HOME_BIN
    echo "[info] Created directory because $HOME_BIN was not found."
fi

gh release download "$VERSION" --repo $REPO --pattern "$INSTALL_TARGET" --output - | tar -xzf - -C $HOME_BIN

echo "[info] moli ${VERSION} installed to ${HOME_BIN}/moli"

# 現在のシェルを検出して適切な設定ファイルを選択
CURRENT_SHELL=$(basename "$SHELL")
case "$CURRENT_SHELL" in
    "zsh")
        SHELL_RC="$HOME/.zshrc"
    ;;
    "bash")
        SHELL_RC="$HOME/.bashrc"
    ;;
    *)
        SHELL_RC="$HOME/.profile"
    ;;
esac

# 表示用のメッセージを蓄積
MESSAGES=""

# PATH チェック
if [ "${PATH#*$HOME_BIN}" = "$PATH" ]; then
    MESSAGES="${MESSAGES}echo 'export PATH=\"\$PATH:\$HOME/.local/bin\"' >> $SHELL_RC\n"
fi

# エイリアスチェック
alias_name="moli_install"
if ! grep -q "$alias_name" "$SHELL_RC" 2>/dev/null; then
    MESSAGES="${MESSAGES}echo 'alias ${alias_name}=\"gh release download --repo ${REPO} -p 'moli-*-${TARGET}.tar.gz' --output - | tar -xzf - -C \\\$HOME/.local/bin && exec \\\$SHELL -l\"' >> $SHELL_RC\n"
fi

# メッセージがある場合のみ表示
if [ -n "$MESSAGES" ]; then
    echo ""
    echo "[info] Please run the following commands to complete the setup:"
    echo ""
    printf "$MESSAGES"
fi
