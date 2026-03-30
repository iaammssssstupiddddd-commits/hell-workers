# hw_energy — Soul Energy 型・定数・Relationship

## 役割

Soul Energy システムの**型定義・定数・ECS Relationship** を提供するドメインクレート。
`bevy` のみに依存する最軽量 leaf crate。システムロジックは含まない。

## 主要モジュール

| ファイル | 内容 |
|---|---|
| `components.rs` | `PowerGrid`, `PowerGenerator`, `PowerConsumer`, `Unpowered`, `YardPowerGrid` |
| `relationships.rs` | `GeneratesFor`/`GridGenerators`, `ConsumesFrom`/`GridConsumers` (Bevy ECS Relationships) |
| `soul_spa.rs` | `SoulSpaSite`, `SoulSpaTile`, `SoulSpaPhase` |
| `constants.rs` | 発電・消費・ランプバフ等の全定数 |

## システム配置（このクレート外）

| システム | 配置先 | 役割 |
|---|---|---|
| `grid_lifecycle` | `bevy_app/src/systems/energy/` | Yard Observer → PowerGrid 生成/削除、ConsumesFrom 付与 |
| `power_output` | 同上 | SoulSpaSite の稼働タイル数 → current_output 更新 |
| `grid_recalc` | 同上 | Grid 再計算 → Unpowered 付与/除去 |
| `lamp_buff` | 同上 | 通電ランプ半径内 Soul の stress/fatigue 軽減 |

## 仕様ドキュメント

- [docs/soul_energy.md](../../docs/soul_energy.md)
