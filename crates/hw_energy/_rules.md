# hw_energy — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- Soul Energy システムの **型・定数・Relationship** を定義するドメインクレート
- ECS worldへ触れない決定的なpure allocator（priority prefix / legacy all-or-none）
- `PowerGrid`, `PowerGenerator`, `PowerConsumer`, `Unpowered` 等のコンポーネント定義
- `GeneratesFor` / `ConsumesFrom` Relationship（Bevy 0.19 ECS Relationships）
- `SoulSpaSite` / `SoulSpaTile` / `SoulSpaPhase` の構造定義
- 発電・消費・ランプバフ等の全定数

## 禁止事項（AI がやってはいけないこと）

- **`bevy_app` への逆依存禁止**（Cargo 循環依存制約）
- **他の hw_* クレートへの依存禁止**（このクレートは `bevy` のみに依存する最軽量 leaf crate）
- **Bevy System / Plugin、Grid topology再構築、runtime state同期、バフ処理を書かない**
  （worldを読む/書く処理は `bevy_app/src/systems/energy/` が担当）
- pure allocatorはEntity・座標・需要・policy・直前stateを明示入力にし、Query / Resource / Commandsへ依存させない
- **`#[allow(dead_code)]` を使用しない**（使われないコードは削除する）
- **Bevy 0.14 以前の API を推測で使わない**（0.19 の変更点が多い。既存コードまたは docs.rs/bevy/0.19.0 で確認する）

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

- このクレートは **Plugin / System を持たない**（domain型とECS非依存のpure policyのみ）
- システム登録は `bevy_app/src/plugins/logic.rs` が担う
- Relationship 型の Reflect 登録は `bevy_app/src/plugins/logic.rs` で行う

## 主要な不変条件

- **PowerGrid は Yard と 1:1**: Observerはdirty通知だけを行い、rootのordered reconcilerが欠落作成・重複/orphan除去・connection修復を一括で行う
- **PowerConsumer は初期 fail-closed**: `#[require(Unpowered, PowerConsumerPolicy)]`。rootのallocation transactionだけが`PowerSupplyState`と`Unpowered` mirrorを同期する
- **空グリッドは powered**: `consumption == 0` のとき `powered = true`（消費者なし = 停電ではない）
- **runtime stateは非保存**: `PowerSupplyState`、`PowerGridAllocationSummary`、`Unpowered`はload後に再構築する
- **Soul Spa集計の正本は `SoulSpaTile.parent_site`**: 表示用`ChildOf`を発電量・占有数の成立条件にしない

## 既知のサイレント失敗トラップ

- Yard外または有効gridを失ったPowerConsumerは`Disconnected + Unpowered`へ正規化される。未接続を`Shed`として扱わない
- `GeneratesFor` / `ConsumesFrom`を直接補修せず、TransformとYardからordered reconcilerに再判定させる
- `LegacyAllOrNone`由来のshedはPriority modeへ戻す際のhysteresis履歴ではない。mode復帰はcold startとしてraw prefixを再構築する

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/soul_energy.md](../../docs/soul_energy.md)
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md)（依存変更時）
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md)（境界ルール変更時）
- `crates/hw_energy/_rules.md`（このファイル）

## 検証方法

```bash
# コンパイル確認（必須）
python3 scripts/dev.py check

# pure allocation契約
cargo test -p hw_energy allocation
```

## 参照ドキュメント

- [docs/soul_energy.md](../../docs/soul_energy.md): Soul Energy システム仕様
- [docs/building.md](../../docs/building.md): OutdoorLamp / SoulSpa 建物仕様
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
