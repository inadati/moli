# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

moli は、YAML仕様（`moli.yml`）からコードを生成する宣言的開発フレームワーク。Rust製CLIツール。
対応言語: Rust, Go, Python, TypeScript, JavaScript

## 開発コマンド

```bash
# ビルド
cargo build

# リリースビルド（_test/にコピー）
bash build.sh

# テスト実行
cargo test

# 単一テスト実行
cargo test <テスト名>

# 開発時の実行
cargo run -- new          # プロジェクト初期化（インタラクティブ）
cargo run -- new --lang rust  # プロジェクト初期化（言語指定）
cargo run -- up           # コード生成
```

## アーキテクチャ

4層構造:

- **`src/cli/`** - CLIコマンド層（clap）。`new`, `up`, `scan`, `rm`, `completion`サブコマンド
- **`src/project_management/config/`** - YAML設定の解析（`parser.rs`）、バリデーション（`validator.rs`）、データモデル（`models.rs`）
- **`src/code_generation/core/`** - コード生成エンジン。`generator.rs`がエントリーポイントで、`directory_builder.rs`と`file_builder.rs`に委譲
- **`src/code_generation/language/`** - 言語別の生成処理（`rust/`, `go/`, `python/`, `typescript/`, `javascript/`, `any/`）

共有ユーティリティは `src/shared/utils/` に配置。`content_updater.rs`がマーカーベースのファイル部分更新を担当。

## ファイル保護システム（3層）

moliの生成エンジンは3層のファイル保護を実装しており、コード変更時に理解が必要:

1. **コードファイル（完全保護）**: `.rs`, `.go`, `.py`等 → 既存ファイルは上書きしない
2. **管理ファイル（部分更新）**: `mod.rs`, `__init__.py`, `index.ts`等 → マーカー（`// start auto exported by moli.` / `// end auto exported by moli.`）間のみ更新
3. **設定ファイル（初回のみ）**: `Cargo.toml`, `package.json`等 → 存在しない場合のみ作成

## moli.yml設定構造

```yaml
- name: プロジェクト名
  root: true          # ルートプロジェクト（カレントディレクトリに生成）
  lang: rust           # 対象言語
  tree:                # ディレクトリ構造（再帰的）
    - name: src
      file:            # 生成するファイル（拡張子なしなら言語に応じて自動付与）
        - name: main
      tree:
        - name: domain
```

複数プロジェクトを1つのmoli.ymlで管理可能。Rustの場合はワークスペース構成を自動生成。

### git cloneサポート（2026-02-05追加）

`lang: any` プロジェクトで `from` フィールドを使用することで、外部リポジトリをgit cloneできる：

```yaml
- name: docs
  root: false
  lang: any
  tree:
    - from: git@github.com:user/repo.git
      name: external-repo  # 省略時はリポジトリ名（.git除く）を使用
```

**制約:**
- `from` フィールドは `lang: any` でのみ使用可能
- `from` を指定した場合、`tree` と `file` は禁止（cloneしたリポジトリは変更しない）
- `name` または `from` のいずれかは必須
- SSH/HTTPS両対応
- 既存ディレクトリはスキップ、clone失敗時は警告表示して処理継続

実装: `src/code_generation/language/any/file_handler.rs`


