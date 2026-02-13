#!/bin/sh

set -e

VERSION=$(curl -s https://github.com/inadati/moli/releases.atom | grep -o -E "releases/tag/v[0-9]+\.[0-9]+\.[0-9]+" | sed 's/releases\/tag\///' | head -n 1)

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
INSTALL_TARGET_URL="https://github.com/inadati/moli/releases/download/${VERSION}/${INSTALL_TARGET}"

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

HOME_BIN="$HOME/.bin"
if [ ! -e "$HOME_BIN" ]; then
    mkdir -p $HOME_BIN
    echo "[info] Created directory because $HOME_BIN was not found."
fi

curl -L $INSTALL_TARGET_URL -o - | tar -xzvf - && mv ./moli $HOME_BIN

# 表示用のメッセージを蓄積
MESSAGES=""

# PATH チェック - 含まれていない場合のみコマンド出力
if [ "${PATH#*$HOME_BIN}" = "$PATH" ]; then
    MESSAGES="${MESSAGES}echo 'export PATH=\"\$PATH:\$HOME/.bin\"' >> $SHELL_RC\n"
fi

# エイリアスチェック - 含まれていない場合のみコマンド出力
alias_name="moli_install"
if ! grep -q "$alias_name" "$SHELL_RC" 2>/dev/null; then
    MESSAGES="${MESSAGES}echo 'alias $alias_name=\"curl -sSL https://raw.githubusercontent.com/inadati/moli/main/install.sh | sh && exec \\\$SHELL -l\"' >> $SHELL_RC\n"
fi

# メッセージがある場合のみ表示
if [ -n "$MESSAGES" ]; then
    echo ""
    echo "[info] Please run the following commands to complete the setup:"
    echo ""
    printf "$MESSAGES"
fi
