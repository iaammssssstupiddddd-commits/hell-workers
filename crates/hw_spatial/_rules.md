# hw_spatial — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- ゲームワールドをグリッドに分割し、エンティティ位置の **近傍検索・最近傍検索** を提供する空間インデックスクレート
- 全 `*SpatialGrid` Resource の型定義（`GridData<T>` + `SpatialGridOps` トレイト）
- Change Detection（Added / Changed / RemovedComponents）に基づく差分更新システム

## 禁止事項（AI がやってはいけないこと）

- **`bevy_app` への逆依存禁止**（Cargo 循環依存制約）
- **AI ロジック・ゲームロジックをこのクレートに書かない**（空間クエリの提供のみ）
- **グリッド更新に Change Detection 以外を使わない**（毎フレーム全件スキャンは禁止）
- **`#[allow(dead_code)]` を使用しない**
- **Bevy 0.14 以前の API を推測で使わない**

## crate 境界ルール（docs/crate-boundaries.md に基づく）

- leaf crate：`bevy`, `hw_core`, `hw_jobs`, `hw_world` に依存（`hw_logistics` は依存しない）
- `bevy_app` への逆依存は **完全禁止**
- `hw_logistics` 向け特化システム（`ResourceSpatialGrid` / `StockpileSpatialGrid` の更新）は `hw_logistics` 側に置く
- 詳細: [docs/crate-boundaries.md](../../docs/crate-boundaries.md)

## 依存制約（Cargo.toml 実体）

```
# 許可
bevy     ✓
hw_core  ✓
hw_jobs  ✓
hw_world ✓

# 禁止
bevy_app       ✗
hw_logistics   ✗  (Resource/Stockpile 特化 update は hw_logistics 側)
hw_soul_ai     ✗
hw_familiar_ai ✗
hw_ui          ✗
hw_visual      ✗
hw_energy      ✗
```

## plugin / system 登録責務

- このクレートはシステム登録を **持たない**（Resource 定義と update system の提供のみ）
- システム登録は `bevy_app/src/plugins/spatial.rs` の `SpatialPlugin` が担う
- `crates/bevy_app/src/systems/spatial/` は削除済み — `plugins/spatial.rs` が直接 import する

## 新グリッド追加時のルール

1. `SpatialGridOps` を実装した新 Resource を定義
2. `lib.rs` に `pub use` を追加
3. `bevy_app/src/plugins/spatial.rs` にシステム登録を追加
4. [docs/architecture.md](../../docs/architecture.md) の空間グリッド一覧を更新

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/architecture.md](../../docs/architecture.md)（グリッド一覧の変更時）
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md)（Cargo.toml 変更時）
- `crates/hw_spatial/_rules.md`（このファイル）

## 検証方法

```bash
# コンパイル確認（必須）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/architecture.md](../../docs/architecture.md): 空間グリッド一覧
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
