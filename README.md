# moli

`moli`は、YAML仕様からコードを生成する宣言的開発フレームワークです。複数のプログラミング言語をサポートし、シンプルなYAML設定で効率的なプロジェクト構造を生成します。


## 特徴

- **宣言的開発**: シンプルなYAMLファイルでプロジェクト構造を定義
- **マルチ言語サポート**: Rust、Go、Python、TypeScript、JavaScript
- **デュアルモード**: インタラクティブな手動選択とAI向け自動化の両方に対応
- **マルチプロジェクト**: 単一設定ファイルで複数のプロジェクトを管理
- **ファイル保護**: 既存コードを保護しながら構造管理を実現

## インストール

インストールスクリプトを使用してインストール:

```bash
curl -sSL https://raw.githubusercontent.com/asweed888/moli/main/install.sh | bash && exec $SHELL -l
```

初回インストール後のアップデート:

```bash
moli_install
```

## クイックスタート

1. **新しいプロジェクトの初期化**:
   
   **人間向けモード（インタラクティブ）**:
   ```bash
   moli new
   ```
   対話的な言語選択プロンプトが表示されます
   
   **AI向けモード（自動化）**:
   ```bash
   moli new --lang rust
   moli new --lang typescript
   moli new --lang go
   moli new --lang python
   moli new --lang javascript
   ```

2. **仕様からコードを生成**:
   ```bash
   moli up
   ```

## 設定

`moli.yml`ファイルでプロジェクト構造を定義します:

```yaml
- name: my-app
  root: true
  lang: typescript
  tree:
    - name: src
      tree:
        - name: components
          file:
            - name: Button.tsx
            - name: Modal.vue
            - name: utils
      file:
        - name: index
```

### 設定オプション

- **`name`**: プロジェクト名
- **`root`**: ルートプロジェクトかどうか（`true`の場合、現在のディレクトリに直接生成）
- **`lang`**: 対象プログラミング言語 (`rust`, `go`, `python`, `typescript`, `javascript`)
- **`tree`**: ディレクトリ構造を定義（再帰的に使用可能）
- **`file`**: 生成する個別ファイル（拡張子があれば保持、なければ言語に応じて付与）

### マルチプロジェクト構成

複数のプロジェクトを1つの`moli.yml`で管理できます:

```yaml
- name: frontend
  lang: typescript
  tree:
    - name: src
      tree:
        - name: components
          file:
            - name: App.tsx

- name: backend  
  lang: rust
  tree:
    - name: src
      file:
        - name: main
      tree:
        - name: handlers
          file:
            - name: user
```

- 最初のプロジェクトには`root: true`が自動設定
- 2つ目以降のプロジェクトは個別ディレクトリに生成
- Rustの場合、複数プロジェクトでワークスペース構成を自動生成

## コマンド

- `moli new`: 対話的プロンプトで新しいプロジェクトを初期化
- `moli new --lang <言語>`: 指定言語で新しいプロジェクトを初期化（AI向け）
- `moli up`: 現在の`moli.yml`仕様に基づいてコードを生成
- `moli claude-skill`: Claude Code向けのmoli開発スキルを生成
- `moli --help`: ヘルプ情報を表示
- `moli --version`: バージョン情報を表示

## 例

### Rustプロジェクト
```yaml
- name: my-rust-app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
      tree:
        - name: handlers
          file:
            - name: user
            - name: order
        - name: models
          file:
            - name: user
            - name: order
```

### TypeScriptプロジェクト
```yaml
- name: my-web-app
  root: true
  lang: typescript
  tree:
    - name: src
      tree:
        - name: components
          file:
            - name: Button.tsx
            - name: Modal.vue
            - name: utils.ts
      file:
        - name: index
```

### Goプロジェクト
```yaml
- name: my-go-app
  root: true
  lang: go
  tree:
    - name: pkg
      tree:
        - name: models
          file:
            - name: user
        - name: handlers
          file:
            - name: api
  file:
    - name: main
```

## 開発

```bash
# プロジェクトをビルド
cargo build

# リリースバージョンをビルド
cargo build --release

# テストを実行
cargo test

# 引数付きで実行
cargo run -- new
cargo run -- up
```

## ファイル保護システム

moliは3層のファイル保護システムを実装しています:

1. **コードファイル（完全保護）**: `.rs`, `.go`, `.py`, `.js`, `.ts`, `.tsx`, `.vue`等
   - 一度作成されたら決して上書きされません
   
2. **管理ファイル（部分更新）**: `mod.rs`, `__init__.py`, `index.ts`等
   - moliマーカー間のコンテンツのみ更新、カスタムコードは保護
   
3. **設定ファイル（初回のみ）**: `package.json`, `Cargo.toml`, `go.mod`等
   - 存在しない場合のみ作成

## バージョン

現在のバージョン: **v1.0.0**

v1.0では以下の特徴があります:
- シンプルで直感的なYAML構造（`tree`, `file`）
- マルチプロジェクト対応
- ファイル保護システムの実装
- AI向け自動化機能の追加

## ライセンス

このプロジェクトはv1.0に達し、安定したAPIを提供しています。
