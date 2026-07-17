# interface — プレイヤー入力・UI インタラクション

## 役割

プレイヤーからの resolved keyboard action と mouse/pointer 入力、建物配置・選択システム、および UI の統合を担うディレクトリ。
capture/resolver は `PreUpdate`、pointer ingress は `GameSystemSet::Input`、UI/placement mutation は主に
`GameSystemSet::Interface` で実行される。

project-owned edge keyboard の raw owner は crate root の `input_actions` resolver だけである。このディレクトリの
consumer は `ResolvedInputFrame` を読み、mouse 系 consumer は `UiInputState::world_input_blocked()` と
selection suppression に従う。Modal/Pause capture の pending/visible sync と rollback は
`input_actions/capture.rs`、各 selection/placement の domain mutation はこのディレクトリが所有する。

## ディレクトリ構成

| ディレクトリ | 内容 |
|---|---|
| `selection/` | エンティティ選択・建物配置プレビュー・ヒットテスト |
| `ui/` | UI セットアップ・パネル・リスト・インタラクション |

## selection/ ディレクトリ

プレイヤーがワールドに対して行う操作（クリック選択・建物配置・床配置）を処理する。

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | 公開 API (`SelectedEntity`, `HoveredEntity` 等) |
| `state.rs` | `SelectedEntity`, `HoveredEntity`, `SelectionIndicator` の re-export（実体は `hw_core::selection`） |
| `mode.rs` | 選択モード（`clear_companion_state_outside_build_mode`） |
| `input.rs` | `handle_mouse_input`, `update_hover_entity` |
| `hit_test.rs` | ワールド座標 → エンティティのヒットテスト |
| `building_place/` | 建物ブループリット配置（`blueprint_placement`, `preview.rs`, `companion.rs`） |
| `building_move/` | 建物移動（`mod.rs` root shell、`system.rs` entrypoint、`preview.rs`、`context.rs`、`click_handlers.rs`、`finalization.rs`） |
| `floor_place/` | 床・壁の一括配置（`floor_placement_system`, `wall_apply.rs`, `validation.rs`） |

補足:
`MainCamera` は `hw_core::camera` が所有し、`world_cursor_pos` は `hw_ui::camera` に残る。`bevy_app` 側は `interface::camera` のような再公開層を持たず、selection / command / visual から直接 import する。

## ui/ ディレクトリ

`hw_ui` クレートのシステム群をルートクレートに統合するサブシステム。詳細は `ui/README.md` を参照。
