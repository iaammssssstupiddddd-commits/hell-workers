# hw_ui — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- ゲーム UI（パネル・リスト・ダイアログ・ツールチップ・サブメニュー）のセットアップと入力処理
- `UiAssets` trait の定義（`GameAssets: UiAssets` の実装は bevy_app 側）
- `HwUiPlugin` によるシステム登録（ゲーム固有の ECS クエリを持たない UI システムのみ）
- `UiIntent` メッセージ型の定義（ユーザー操作意図の型安全な表現）

## 禁止事項（AI がやってはいけないこと）

- **`bevy_app` への逆依存禁止**（Cargo 循環依存制約）
- **`DamnedSoul` / `Familiar` / `AssignedTask` 等ゲームエンティティを直接クエリするシステムをここに書かない**（ゲーム固有 ViewModel 構築は bevy_app 側）
- **`Res<GameAssets>` を引数に取るシステムをここに書かない**（Bevy は `Res<dyn Trait>` 不可。GameAssets への依存は bevy_app 側で解決）
- **ゲーム状態遷移 (`PlayMode`) の変更をここに書かない**（ルートクレートの責務）
- **`#[allow(dead_code)]` を使用しない**
- **Bevy 0.14 以前の API を推測で使わない**

## crate 境界ルール（docs/crate-boundaries.md に基づく）

- leaf crate：`bevy`, `hw_core`, `hw_jobs`, `hw_logistics` に依存
- `bevy_app` への逆依存は **完全禁止**
- UI コンポーネントを定義するが、ゲームロジックを持たない
- 詳細: [docs/crate-boundaries.md](../../docs/crate-boundaries.md)

## 依存制約（Cargo.toml 実体）

```
# 許可
bevy         ✓
hw_core      ✓
hw_jobs      ✓
hw_logistics ✓

# 禁止
bevy_app       ✗
hw_soul_ai     ✗
hw_familiar_ai ✗
hw_spatial     ✗
hw_visual      ✗
hw_world       ✗
hw_energy      ✗
```

## plugin / system 登録責務

- **`HwUiPlugin`** がゲーム固有クエリを持たない UI システムのみを登録する
- ゲーム固有クエリを必要とする UI システム（entity_list 更新等）は `bevy_app/plugins/input.rs` 等が登録する

## UiAssets 抽象化パターン

新しいアセット（フォント・アイコン等）が必要になった場合:
1. `setup/mod.rs` の `UiAssets` trait にメソッドを追加
2. `bevy_app/src/entities/game_assets.rs` の `impl UiAssets for GameAssets` に実装を追加
3. このクレートに `GameAssets` への直接依存は追加しない

## BuildingType メニュー追加時のルール

新 `BuildingType` を建設メニューに追加する場合:
1. `setup/submenus.rs` の該当カテゴリの `architect_building_specs()` に `MenuEntrySpec` を追加
2. `panels/task_list/presenter.rs` に表示ラベルを追加

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/building.md](../../docs/building.md)（BuildingType メニュー追加時）
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md)（Cargo.toml 変更時）
- `crates/hw_ui/_rules.md`（このファイル）

## 検証方法

```bash
# コンパイル確認（必須）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/building.md](../../docs/building.md): BuildingType 一覧・カテゴリ
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
