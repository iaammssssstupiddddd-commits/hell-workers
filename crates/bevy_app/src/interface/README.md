# interface — プレイヤー入力・UI インタラクション

## 役割

プレイヤーからの入力（マウス・キーボード）処理、建物配置・選択システム、および UI の統合を担うディレクトリ。
`GameSystemSet::Interface` フェーズで実行される。

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
| `building_move/` | 建物移動（`mod.rs` root shell、`system.rs` の `building_move_system`、`preview.rs` の `building_move_preview_system`） |
| `floor_place/` | 床・壁の一括配置（`floor_placement_system`, `wall_apply.rs`, `validation.rs`） |

補足:
`MainCamera` は `hw_core::camera` が所有し、`world_cursor_pos` は `hw_ui::camera` に残る。`bevy_app` 側は `interface::camera` のような再公開層を持たず、selection / command / visual から直接 import する。

## ui/ ディレクトリ

`hw_ui` クレートのシステム群をルートクレートに統合するサブシステム。詳細は `ui/README.md` を参照。
