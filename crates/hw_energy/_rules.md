# hw_energy — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- Soul Energy システムの **型・定数・Relationship** を定義するドメインクレート
- `PowerGrid`, `PowerGenerator`, `PowerConsumer`, `Unpowered` 等のコンポーネント定義
- `GeneratesFor` / `ConsumesFrom` Relationship（Bevy 0.18 ECS Relationships）
- `SoulSpaSite` / `SoulSpaTile` / `SoulSpaPhase` の構造定義
- 発電・消費・ランプバフ等の全定数

## 禁止事項（AI がやってはいけないこと）

- **`bevy_app` への逆依存禁止**（Cargo 循環依存制約）
- **他の hw_* クレートへの依存禁止**（このクレートは `bevy` のみに依存する最軽量 leaf crate）
- **Grid 再計算やバフ等のシステムロジックをこのクレートに書かない**（システムは `bevy_app/src/systems/energy/` が担当）
- **`#[allow(dead_code)]` を使用しない**（使われないコードは削除する）
- **Bevy 0.14 以前の API を推測で使わない**（0.18 の変更点が多い。既存コードまたは docs.rs/bevy/0.18.0 で確認する）

## crate 境界ルール（docs/crate-boundaries.md に基づく）

- このクレートは **最軽量 leaf crate**：依存は `bevy` のみ
- `bevy_app` への逆依存は **完全禁止**
- 他の hw_* クレートへの依存も **禁止**（hw_core にすら依存しない）
- 詳細: [docs/crate-boundaries.md](../../docs/crate-boundaries.md)

## 依存制約（Cargo.toml 実体）

```
# 許可
bevy  ✓

# 禁止（全 hw_* クレート）
hw_core        ✗
hw_world       ✗
hw_jobs        ✗
hw_logistics   ✗
hw_soul_ai     ✗
hw_familiar_ai ✗
hw_spatial     ✗
hw_ui          ✗
hw_visual      ✗
bevy_app       ✗
```

## plugin / system 登録責務

- このクレートは **Plugin を持たない**（型・定数・Relationship の定義のみ）
- システム登録は `bevy_app/src/plugins/logic.rs` が担う
- Relationship 型の Reflect 登録は `bevy_app/src/plugins/logic.rs` で行う

## 主要な不変条件

- **PowerGrid は Yard と 1:1**: `on_yard_added` Observer が自動スポーン、`on_yard_removed` が自動 despawn
- **PowerConsumer は初期 Unpowered**: `#[require(Unpowered)]` によりグリッド接続前はデフォルト停電。`grid_recalc_system` が通電時に除去する
- **空グリッドは powered**: `consumption == 0` のとき `powered = true`（消費者なし = 停電ではない）

## 既知のサイレント失敗トラップ

- Yard 外に配置された PowerConsumer は `on_power_consumer_added` Observer で ConsumesFrom が付与されず、`grid_recalc_system` の対象外 → 常時 Unpowered（ログなし）
- `SoulSpaSite` が `GeneratesFor` なしだと `GridGenerators` に含まれず発電が集計されない（`soul_spa_place/input.rs` の `power_grid_entity` が None のとき発生）

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/soul_energy.md](../../docs/soul_energy.md)
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md)（依存変更時）
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md)（境界ルール変更時）
- `crates/hw_energy/_rules.md`（このファイル）

## 検証方法

```bash
# コンパイル確認（必須）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/soul_energy.md](../../docs/soul_energy.md): Soul Energy システム仕様
- [docs/building.md](../../docs/building.md): OutdoorLamp / SoulSpa 建物仕様
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
