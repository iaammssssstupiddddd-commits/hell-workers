# hw_visual — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- レンダリング・ビジュアル演出：スピーチバブル・アニメーション・進捗バー・タスクエリアオーバーレイ
- `SpeechPlugin`：スピーチシステムの登録（会話・Typewriter・定期感情表現）
- Familiar / Soul の視覚フィードバックシステム（`familiar_idle_visual_apply_system`, `max_soul_visual_system`）
- `SpeechHandles`, `TaskAreaMaterial` 等のビジュアル専用 Resource の管理

## 禁止事項（AI がやってはいけないこと）

- **シミュレーション状態を直接変更しない**（`AssignedTask`, `WorkingOn`, `PopulationManager` 等を mutate しない）
- **`GameAssets` を直接参照しない**（専用 handles / `SpeechHandles` 等を通じて注入する）
- **`bevy_app` への逆依存禁止**（Cargo 循環依存制約）
- **`hw_soul_ai` / `hw_familiar_ai` に依存しない**（ビジュアルは hw_core events / hw_core types を通じて接続する）
- **`#[allow(dead_code)]` を使用しない**
- **Bevy 0.14 以前の Window / UI API を推測で使わない**（0.18 は Window / UI API の変更が多い。`docs.rs/bevy/0.18.0` または既存コードを参照）

## crate 境界ルール（docs/crate-boundaries.md に基づく）

- leaf crate：Bevy 型の利用は許可
- `bevy_app` への逆依存は **完全禁止**
- Visual は **読み取り専用**：ECS の読み取りは許可、シミュレーション状態の書き込みは禁止
- 異なるドメインとの連携は `hw_core` に定義されたイベント（Pub/Sub）を通じて行う
- 詳細: [docs/crate-boundaries.md](../../docs/crate-boundaries.md)

## 依存制約（Cargo.toml 実体）

```
# 許可
hw_core      ✓  (visual_mirror::* を通じてドメイン状態を受け取る)
hw_spatial   ✓
hw_world     ✓
hw_ui        ✓  (UI 型の参照)
bevy         ✓
rand         ✓

# 残存依存（Out of Scope ファイルのみ・別提案で解消予定）
hw_jobs      △  (mud_mixer.rs / tank.rs / wall_connection.rs の Building 系のみ)
hw_logistics △  (tank.rs の Stockpile のみ)

# 禁止
bevy_app       ✗
hw_soul_ai     ✗  (AI ロジック型は hw_core events 経由で受け取る)
hw_familiar_ai ✗
```

新規コードで `hw_jobs` / `hw_logistics` を直接インポートしてはならない。
ドメイン状態の参照は必ず `hw_core::visual_mirror::*` を通じて行うこと。

## ⚠️ Bevy 0.18 API 注意事項

- **Window / UI API**：Bevy 0.18 では大きく変更された部分がある。Window 関連のコードを変更する前に必ず `docs.rs/bevy/0.18.0` または既存コードを確認すること
- **シェーダー / Material**：`Material2d` / `AsBindGroup` の API が変更されている可能性がある。既存の `TaskAreaMaterial` 実装を参考にすること
- **UI ノード**：Bevy 0.18 の UI は `Node` / `PickingInteraction` を使う（旧 `Style` 廃止）

## plugin / system 登録責務

- **`SpeechPlugin`** がスピーチ系システムの唯一の登録者
- `familiar_idle_visual_apply_system` / `max_soul_visual_system` は `bevy_app` 側の `FamiliarAiPlugin` が ordering 込みで登録する（`hw_visual` では登録しない）

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/speech_system.md](../../docs/speech_system.md)（スピーチシステム変更時）
- [docs/gather_haul_visual.md](../../docs/gather_haul_visual.md)（視覚フィードバック変更時）
- `crates/hw_visual/_rules.md`（このファイル）

## 検証方法

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/speech_system.md](../../docs/speech_system.md): スピーチバブル仕様
- [docs/gather_haul_visual.md](../../docs/gather_haul_visual.md): 採取・搬送ビジュアル
- [docs/events.md](../../docs/events.md): イベントカタログ（Visual が消費するイベント）
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
