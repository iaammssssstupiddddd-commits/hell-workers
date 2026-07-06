# テキスト入力 UI — EditableText + clipboard 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `text-input-ui-plan-2026-07-05` |
| ステータス | `Draft` |
| 作成日 | `2026-07-05` |
| 最終更新日 | `2026-07-05` |
| 作成者 | Claude (調査ベース) |
| 関連提案 | N/A |
| 関連Issue/PR | 方針の根拠: `crates/hw_ui/_rules.md`「テキスト入力・スクロール UI の方針」 |

## 1. 目的

- 解決したい課題: テキスト入力を使う機能が一切なく、Soul を個体識別する手段が表示名の初期値のみ。エンティティリストが増えると目的の Soul を探せない
- 到達したい状態: Soul のリネームとエンティティリストのインクリメンタル検索が使える。テキスト入力基盤は 0.19 標準（`EditableText`）で、自前実装ゼロ
- 成功指標: 日本語（IME）でリネーム・検索ができる。`cargo check` / `cargo clippy --workspace`（警告 0）成功

## 2. スコープ

### 対象（In Scope）

- **M1**: `EditableText` 入力フィールドの PoC（**BSN で構築**、フォーカス・IME・確定/キャンセル）
- **M2**: Soul リネーム UI（Info Panel から起動）
- **M3**: エンティティリストの検索/フィルタ
- **M4**: `bevy_clipboard` によるコピー/ペースト対応

### 非対象（Out of Scope）

- タスクリストやチャット等、他画面へのテキスト入力展開（基盤ができれば個別対応で足りる）
- 検索の高度化（あいまい検索・タグ検索）。まずは部分一致のみ
- Familiar のリネーム（Soul で確立したパターンの横展開として、完了後に判断）

## 3. 現状とギャップ

- 現状: `hw_ui/_rules.md` に「自前 text input を作らず `EditableText` を使う」と方針だけ記載済みで、実装ゼロ。`EditableTextInputPlugin`（IME 対応・`ImeSystems` あり）は `ui` feature の `UiWidgetsPlugins` 経由で **DefaultPlugins に登録済み** — プラグイン追加は不要
- 問題: ICU4X 日本語セグメンテーション制約（移行計画の既知の制約、QA 済み・実害なし）とは別に、**IME 入力の実挙動は未検証**
- 本計画で埋めるギャップ: テキスト入力の参照実装を 1 つ確立し、リネームと検索という実用機能に載せる

## 4. 実装方針（高レベル）

- 方針: M1 で入力フィールドの再利用可能な bsn! 断片（`fn text_field(...) -> impl Scene` 形式のヘルパー）を確立し、M2/M3 はそれを配置するだけにする
- 設計上の前提: 表示名は hw_core 側の既存コンポーネント（Soul の名前を保持する型を実装時に特定）を書き換える。`UiIntent` メッセージで hw_ui → bevy_app に確定値を渡す（hw_ui はゲーム状態を直接書かない）
- **BSN 前提**: 入力フィールド・検索バー・リネームダイアログの UI ツリーは `bsn!` + `commands.spawn_scene` で構築（BSN 制約は `save-load-world-serialization-plan-2026-07-05.md` §4.1 参照）

### 4.1 検証済み 0.19 API（registry ソースで確認、2026-07-05）

| API | 場所 | 要点 |
| --- | --- | --- |
| `EditableText` | `bevy_text/src/editing.rs:106` | テキスト編集状態を持つコンポーネント。`bevy::text::EditableText` |
| `EditableTextInputPlugin` / `SelectAllOnFocus` | `bevy_ui_widgets/src/text_input.rs` | 入力処理（IME 対応）。DefaultPlugins に登録済み・個別 add 不要 |
| `ui_widgets::Activate` / `ValueChange<T>` | `bevy_ui_widgets/src/lib.rs:82,90` | EntityEvent。observer で購読 |
| `Clipboard`（Resource）: `fetch_text()` → `ClipboardRead`（`poll_result()`）/ `set_text()` | `bevy_clipboard/src/lib.rs:191,232,299` | 読み取りは**非同期**（poll 式）。`ClipboardPlugin` は `system_clipboard` feature（`bevy/Cargo.toml:2852`）— **未有効、M4 で追加** |
| `bsn!` / `spawn_scene` | `bevy_scene` | UI 宣言構築 |

## 5. マイルストーン

## M1: テキスト入力フィールド PoC（BSN）

- 変更内容:
  1. `crates/hw_ui/src/widgets/text_field.rs`（新規）: `bsn!` で入力フィールド（枠 Node + `EditableText` + カーソル表示）を返すヘルパーを実装
  2. フォーカス管理: `bevy_input_focus`（`ui` feature に内包）でクリックフォーカス・Escape でフォーカス解除。**入力フォーカス中はゲームのキーボードショートカットを無効化する**ガード（既存の入力システムに条件追加）
  3. 確定（Enter）/キャンセル（Escape）で値をイベント化
  4. dev_panel か一時画面に置いて、ASCII + 日本語 IME 入力を確認
- 変更ファイル:
  - `crates/hw_ui/src/widgets/text_field.rs`（新規）
  - `crates/bevy_app/src/interface/`（ショートカット無効化ガード）
- 完了条件:
  - [ ] ASCII / 日本語 IME で入力・編集・確定・キャンセルが動く（ユーザー目視 QA）
  - [ ] 入力中に WASD 等のゲーム操作が発火しない
- 検証: `cargo check` + 目視 QA

## M2: Soul リネーム

- 変更内容:
  1. Info Panel（`docs/info_panel_ui.md` 参照）の Soul 名表示に編集ボタンを追加 → M1 の text_field を初期値入りで表示
  2. 確定時に `UiIntent`（新バリアント `RenameSoul { entity, name }` 等）を発行し、bevy_app 側で名前コンポーネントを更新
  3. 空文字・最大長のバリデーション（確定拒否）
- 変更ファイル:
  - `crates/hw_ui/src/panels/`（Info Panel）
  - `crates/hw_ui/src/`（UiIntent 定義）
  - `crates/bevy_app/src/interface/ui/`（intent 処理）
  - `docs/info_panel_ui.md`
- 完了条件:
  - [ ] リネームがエンティティリスト・スピーチ等の表示に反映される
- 検証: `cargo check` + 実プレイ確認

## M3: エンティティリスト検索

- 変更内容:
  1. エンティティリスト上部に検索バー（M1 の text_field、`bsn!` 配置）
  2. 入力の都度、リスト項目を名前部分一致でフィルタ（既存の ViewModel dirty ゲートに検索条件を追加。全再構築は避ける）
  3. `ScrollArea` との共存確認（0.19 標準スクロールは導入済み）
- 変更ファイル:
  - `crates/hw_ui/src/setup/entity_list.rs`
  - `crates/bevy_app/src/interface/ui/plugins/entity_list.rs`（ViewModel フィルタ）
- 完了条件:
  - [ ] 日本語含む部分一致で絞り込みでき、クリアで全件に戻る
  - [ ] Soul 数が多い状態でも入力毎のフィルタが軽い（体感ヒッチなし）
- 検証: `cargo check` + 実プレイ確認

## M4: クリップボード対応

- 変更内容:
  1. `Cargo.toml` の bevy features に `"system_clipboard"` を追加
  2. text_field に Ctrl+C / Ctrl+V を実装。**`fetch_text()` は非同期（`ClipboardRead` を `poll_result()` でポーリング）**なので、ペースト要求 → 後続フレームで反映の構造にする
  3. ※ `EditableTextInputPlugin` が既にコピペを内蔵している可能性がある。実装前に `text_input.rs` を精読し、内蔵済みなら M4 は「動作確認 + feature 追加」だけに縮小
- 変更ファイル:
  - `Cargo.toml`
  - `crates/hw_ui/src/widgets/text_field.rs`
- 完了条件:
  - [ ] 外部アプリとの間でテキストのコピー/ペーストができる
- 検証: `cargo check` + `cargo clippy --workspace`（警告 0）+ 目視 QA

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| IME 入力が Linux（X11/Wayland）環境で不安定 | M1 が進まない | まず ASCII で全機能を成立させ、IME は QA 項目として分離。問題があれば upstream issue を確認し、日本語入力は既知の制約として記録 |
| 入力フォーカスとゲームショートカットの競合 | 入力中にゲームが誤動作 | M1 の完了条件に明記。フォーカス状態を単一の判定関数に集約 |
| 検索フィルタで ViewModel 全再構築が走る | リスト大でヒッチ | 既存 dirty ゲート（perf-phase3 で導入済み）に検索条件を統合し、差分更新を維持 |
| `EditableText` の見た目がテーマと不整合 | UX 劣化 | headless widget なので描画は自前スタイル。UiTheme の色を適用 |

## 7. 検証計画

- 必須: `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` + `cargo clippy --workspace`（警告 0）
- 手動確認シナリオ: Soul を日本語名にリネーム → エンティティリストで日本語検索 → コピペ → ゲーム操作に干渉しないこと
- 目視 QA はユーザーの画面確認が必要（IME はサンドボックスで検証不可）

## 8. ロールバック方針

- どの単位で戻せるか: マイルストーン単位（M2〜M4 は M1 に依存、M2/M3 は相互独立）
- 戻す時の手順: 該当コミット revert

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（計画作成のみ）
- 未着手/進行中: M1〜M4 すべて未着手

### 次のAIが最初にやること

1. `bevy_ui_widgets/src/text_input.rs` と `bevy_text/src/editing.rs` を精読（本計画は型の存在確認まで。使用手順・カーソル描画・コピペ内蔵有無は未精査）
2. bevy 0.19 の公式 example にテキスト入力サンプルがないか `bevy-0.19.0/examples/` を確認（あればそれが一次情報）
3. Soul の表示名を保持する既存コンポーネントを特定（`Name` か独自型か）

### ブロッカー/注意点

- `system_clipboard` feature は M4 まで追加しない（不要な依存を先に増やさない）
- hw_ui にゲームエンティティ（DamnedSoul 等）を直接クエリするシステムを書かない（crate 境界ルール）。リネーム反映は UiIntent → bevy_app
- BSN でのコンポーネント使用には `Default + Clone` が要る
- IME・クリップボードの実挙動確認はユーザー目視 QA 必須

### 参照必須ファイル

- `crates/hw_ui/_rules.md`（テキスト入力方針・BSN 知見の記録先）
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ui_widgets-0.19.0/src/text_input.rs`
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_text-0.19.0/src/editing.rs`
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_clipboard-0.19.0/src/lib.rs`
- `docs/entity_list_ui.md` / `docs/info_panel_ui.md`

### 最終確認ログ

- 最終 `cargo check`: 未実施（実装未着手）
- 未解決エラー: なし

### Definition of Done

- [ ] M1〜M4 完了
- [ ] `cargo check` / `cargo clippy --workspace`（警告 0）成功
- [ ] `docs/entity_list_ui.md` / `docs/info_panel_ui.md` が更新され、本計画をアーカイブ可能

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-05` | Claude | 初版作成（EditableText / bevy_clipboard の registry ソース検証に基づく） |
