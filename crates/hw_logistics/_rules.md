# hw_logistics — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- リソース予約・在庫管理（`SharedResourceCache`, `Inventory`, `ResourceItem`）
- Auto-Haul 要求（`TransportRequest`）の生成と管理
- `ResourceReservationRequest` メッセージの処理（予約追加・解放）
- 輸送系タスクの Arbitration（輸送割当調停）システム

## 禁止事項（AI がやってはいけないこと）

- **`bevy_app` への逆依存禁止**（Cargo 循環依存制約）
- **root 固有の初期スポーン処理や `GameAssets` 依存をこのクレートに書かない**
- **`unassign_task` を迂回して `Inventory` / `SharedResourceCache` を直接操作しない**（予約リークが発生する）
- **`#[allow(dead_code)]` を使用しない**
- **Bevy 0.14 以前の API を推測で使わない**

## crate 境界ルール（docs/crate-boundaries.md に基づく）

- leaf crate：Bevy 型の利用は許可
- `bevy_app` への逆依存は **完全禁止**
- 詳細: [docs/crate-boundaries.md](../../docs/crate-boundaries.md)

## 依存制約（Cargo.toml 実体）

```
# 許可
hw_core    ✓
hw_world   ✓
hw_jobs    ✓
hw_spatial ✓
bevy       ✓
rand       ✓

# 禁止
bevy_app     ✗
hw_soul_ai   ✗
hw_familiar_ai ✗
hw_ui        ✗
hw_visual    ✗
```

## plugin / system 登録責務

- **`LogisticsPlugin`**（`crates/hw_logistics/src/plugin.rs`）：`apply_reservation_requests_system` を `SoulAiSystemSet::Execute` に登録する唯一の登録元。`bevy_app/plugins/logic.rs` から `add_plugins(hw_logistics::LogisticsPlugin)` で組み込まれる。
- **`TransportRequestPlugin`**：transport request / arbitration 系システムの登録を担う（`transport_request/plugin.rs`）。`bevy_app` から `add_plugins(hw_logistics::transport_request::TransportRequestPlugin)` で組み込まれる。
- `bevy_app` はこれらのシステムを直接 `add_systems` しない。

## ⚠️ 既知のサイレント失敗トラップ（最重要）

### トラップ 1: TransportRequest なしの Haul 系 WorkType
`Haul` / `HaulToMixer` / `GatherWater` / `HaulWaterToMixer` / `WheelbarrowHaul` の WorkType を持つ Designation は、**`TransportRequest` コンポーネントがないと `task_finder` のフィルタで無音スキップされる**（エラーもログも出ない）。

➜ Haul 系タスクを新設する際は必ず `TransportRequest` を同時に添付すること。
詳細: [docs/invariants.md §I-T1](../../docs/invariants.md)

### トラップ 2: 予約解放の漏れ
タスクを中断・放棄する経路で `unassign_task` を呼ばないと `SharedResourceCache` の予約が永続的にリークする。

➜ タスク中断の**全経路**で `hw_soul_ai::soul_ai::helpers::work::unassign_task` を呼ぶこと。
詳細: [docs/invariants.md §I-L1](../../docs/invariants.md)

### トラップ 3: フレーム遅延
`TransportRequestSpatialGrid` は Change Detection で動作し、スポーン後の **次フレーム** で反映される。スポーン直後のフレームでは輸送要求が発見されない可能性がある（仕様）。

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/logistics.md](../../docs/logistics.md)
- [docs/invariants.md](../../docs/invariants.md)（不変条件に変化があった場合）
- [docs/events.md](../../docs/events.md)（イベント変更時）
- `crates/hw_logistics/_rules.md`（このファイル）

## 検証方法

```bash
# コンパイル確認（必須）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/logistics.md](../../docs/logistics.md): Logistics サイレント失敗トラップ詳細
- [docs/building.md](../../docs/building.md): 建築と輸送の接続
- [docs/invariants.md](../../docs/invariants.md): ゲーム不変条件（I-L1, I-T1）
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
