# logistics — root shell + app-specific logistics

## 役割

物流ロジック本体は `hw_logistics` クレートが所有する。
このディレクトリは root crate 固有の依存を持つ処理と、既存 import path を維持する thin shell だけを担う。

- `initial_spawn.rs` — `GameAssets` 依存の初期リソーススポーン
- `ui.rs` — ロジスティクス UI ヘルパー
- `transport_request/` — `hw_logistics` 実装への thin shell / re-export
- `mod.rs` — `hw_logistics` 公開 API の再公開と互換パス維持

## 主要ファイル

| ファイル | 内容 |
|---|---|
| `mod.rs` | `hw_logistics` の re-export + `initial_spawn`, `ui` 公開 |
| `initial_spawn.rs` | 初期リソースエンティティのスポーン（GameAssets 依存） |
| `ui.rs` | ロジスティクス UI ヘルパー |

## transport_request/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | `hw_logistics::transport_request` の型を re-export + `plugin`, `producer` を公開 |
| `plugin.rs` | `TransportRequestPlugin` / `TransportRequestSet` の thin shell |

## transport_request/producer/ ディレクトリ

floor / wall construction producer 実装本体は `hw_logistics` に移り、このディレクトリには互換 re-export だけが残る。

| ファイル | 内容 |
|---|---|
| `mod.rs` | thin shell module 宣言 |
| `floor_construction.rs` | `hw_logistics::transport_request::producer::floor_construction` の re-export |
| `wall_construction.rs` | `hw_logistics::transport_request::producer::wall_construction` の re-export |

---

## hw_logistics との境界

このディレクトリが保持するもの:

| ファイル | 残留理由 |
|---|---|
| `initial_spawn.rs` | `GameAssets` リソース（テクスチャ等）に依存 |
| `ui.rs` | UI レンダリングに依存 |
| `transport_request/plugin.rs` | 後方互換の import path を維持する thin shell |
| `transport_request/producer/*.rs` | 後方互換の import path を維持する thin shell |

hw_logistics に移植済み（re-export 経由で公開）:

- 全 transport request producer（`blueprint`, `bucket`, `consolidation`, `mixer`, `provisional_wall`, `stockpile_group`, `tank_water_request`, `task_area`, `upsert`, `wheelbarrow`）
- floor / wall construction producer（`floor_construction`, `wall_construction`）
- 手押し車仲裁システム（`arbitration/`）
- `TransportRequestPlugin`, `TransportRequestSet`
- 建設系需要計算ヘルパー（`floor_construction.rs`, `wall_construction.rs`, `tile_index.rs`）
- アイテムライフサイクル管理（`item_lifetime.rs`）
