# セーブ / ロード

Hell Workers のシミュレーション状態を RON ファイルへ保存し、再起動後に復元する機能の仕様。

## 操作

| 入力 | 動作 |
| --- | --- |
| **F5** / Pause メニュー「Save Game」 | 現在のワールドを `saves/world.scn.ron` へ保存 |
| **F9** / Pause メニュー「Load Game」 | 確認ダイアログ後、`saves/world.scn.ron` からロード |
| **Space** / 時間パネル `||` | 一時停止 → Pause メニュー表示 |
| **Esc**（ロード確認中） | ロード確認ダイアログを閉じる |

保存先は実行ディレクトリ直下の `saves/`（`assets/` 外。AssetServer 非経由）。

## アーキテクチャ

```
[セーブ] exclusive system
  → DynamicWorldBuilder (deny-all + allow-list)
  → extract_entities(collect_persisted_entities)
  → RON serialize
  → saves/world.scn.ron.tmp → rename

[ロード] exclusive system
  → RON deserialize (WorldDeserializer)
  → worldgen seed 照合（不一致なら中止 — 後述）
  → despawn persisted entities
  → DynamicWorld::write_to_world_with (Entity remap)
  → rebuild transient caches
  → AssignedTask::None を Soul へ再付与
  → rehydrate（shell 再付与 — 後述）
```

実装: `crates/bevy_app/src/systems/save/`（`SavePlugin`）。

## 保存対象

### Resources

- `GameTime`, `DreamPool`, `PopulationManager`, `WorldMap`, `SavedWorldgenSeed`

### シミュレーション Entity

マーカーコンポーネントで選別（`collect_persisted_entities`）。例:

- Soul / Familiar（`DamnedSoul`, `SoulIdentity`, `Familiar`）
- タスク・建築（`Designation`, `Blueprint`, `Building`, construction site 等）
- 物流（`ResourceItem`, `Stockpile`, `TransportRequest`, `Wheelbarrow` 等）
- エネルギー（`PowerGrid`, `SoulSpaSite` 等）
- ワールド採取対象・ゾーン（`Tree`, `Rock`, `Tile`, `Site`, `Yard`, `PairedSite`/`PairedYard`）

各 Entity に付く **Relationship Source / Target 両方**、および `Transform` 等の allow-list コンポーネントも保存する。

`WorldMap` は Resource として保存し、内部の Entity 参照（`buildings`, `doors`, `stockpiles`, `tile_entities`）は `map_world_map_entities` で remap する。

### 保存しないもの

| カテゴリ | 例 | ロード後 |
| --- | --- | --- |
| 実行中タスク状態 | `AssignedTask`, `Path`, `Destination`, `FamiliarAiState` | Soul へ `AssignedTask::None` を付与、shell 側で `FamiliarAiState` 等をデフォルト再挿入。Familiar AI が Designation から再割当 |
| 派生キャッシュ | 空間グリッド、`SharedResourceCache`、`ReservationSignatureCache`、transport producer cache | `rebuild_transient_caches` で default 化する。予約同期 timer も reset し、次の Perceive が初回同期として完全 snapshot を再構築 |
| ビジュアル / UI | `hw_visual/*`, `hw_ui/*`, `SoulUiLinks`, Sprite / 3D プロキシ | **rehydrate**（下記）と observer / startup で再生成 |
| 地形描画 | `TerrainChunk` | 起動時 seed から生成（`SavedWorldgenSeed` 照合で整合を保証） |
| セッション入力 | `BuildContext`, `SelectedEntity` 等 | 保持しない |

## Rehydrate（ロード後の shell 再付与）

セーブが復元するのは allow-list の simulation 状態のみで、spawn 関数がその場で挿入する
実行時コンポーネントと随伴エンティティは含まれない。`rehydrate.rs` がロード直後に再付与する。

| カテゴリ | shell の内容 | 実装 |
| --- | --- | --- |
| Soul | `Destination`/`Path`/`AnimationState`/UI リンク/speech 状態 + GLB 3D プロキシ×3 | `attach_soul_shell`（spawn と共用） |
| Familiar | `FamiliarAiState`/`FamiliarOperation`/`ActiveCommand`/Sprite + 3D プロキシ + 指揮範囲インジケーター×3 | `attach_familiar_shell`（同上） |
| Building（SoulSpa 含む） | `Name`/バウンス演出 + VisualLayer 子 Sprite + 独立 3D ビジュアル | `attach_building_shell`（同上） |
| Tree / Rock / ResourceItem / Stockpile | Sprite（spawn 箇所と同じ画像・サイズ） | rehydrate 内で直接挿入 |

shell 欠落の判定は「shell が必ず挿入するコンポーネントの不在」
（Soul/Familiar は `Without<Destination>`、Building は `Without<BuildingBounceEffect>`、他は `Without<Sprite>`）。

付随処理:

- **孤児インベントリのドロップ**: Phase A ではロード後の全 Soul が `AssignedTask::None` になるため、`Inventory(Some)` のアイテムは Soul の足元へドロップして物流ループに戻す
- **猫車積載アイテム**: `LoadedIn` 付きアイテムは `Visibility::Hidden` で復元
- **旧形式セーブ**: `SoulIdentity` が無い場合はランダム生成でフォールバック（名前は失われる）

**新しい spawn 時コンポーネントを追加する時の規約**: 永続化すべき simulation 状態なら
`saving.rs` の allow-list + `register.rs` へ、実行時状態なら該当する `attach_*_shell` へ追加する。
どちらにも入れないと、ロード後にだけ欠落するサイレントバグになる。

**⚠️ タプルキーのマップは reflect デシリアライズ不可（bevy_reflect 0.19 の制約）**:
`HashMap<(i32,i32), _>` / `HashSet<(i32,i32)>` を含む型を保存対象にすると、ロード時に
`DynamicMap::insert_boxed` がタプルキーの `reflect_hash`（未実装）を要求して panic する。
enum キー（`ResourceType` 等）は `enum_hash` があるため問題ない。対処は `WorldMap` と同じく
**serde derive + `#[reflect(Serialize, Deserialize)]`** で型全体を serde 経路にすること
（`crates/hw_world/src/map/mod.rs` の doc コメント参照）。

## Worldgen seed ガード

地形チャンク等のビジュアルは起動時に `GeneratedWorldLayoutResource` の seed から生成され、
セーブには含まれない。セーブ時に `SavedWorldgenSeed` を焼き込み、ロード時に現セッションの
seed と照合する。**不一致ならロードを中止**し、`HELL_WORKERS_WORLDGEN_SEED=<saved>` で
起動し直すようエラーログで案内する（同一セッション内の F5→F9 は常に一致する）。

## Phase A（タスク正規化）

計画書の Phase A は「セーブ前に `unassign_task` で正規化」を想定していたが、本実装では **allow-list から除外** する方式を採用している。

- セーブ中もライブワールドのタスク実行状態は変更しない
- ロード後は `AssignedTask` が無い Soul に `None` を挿入
- `Designation` + `TransportRequest` が残っていれば Familiar AI が再割当する

Phase B（実行中タスクの完全復元）は follow-up。

## Relationship と reconcile

計画書は `RelationshipHookMode::Skip` 前提の reconcile pass を想定していたが、本実装では **Relationship Target 型も allow-list に含めて保存** する。保存時点で Source/Target が整合したスナップショットとして書き出されるため、追加の reconcile pass は不要。

## Reflect 登録

セーブ対象型の `register_type` は `SavePlugin` 内の `register_save_types` に集約（`crates/bevy_app/src/systems/save/register.rs`）。

## 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace 2>&1 | grep "^warning:" | grep -v generated
```

手動: プレイ → F5 → 再起動 → F9。Soul 数・Stockpile 内容・建築進捗・`GameTime` が復元されること。

## 未実装

- 複数スロット・オートセーブ・バージョンマイグレーション
- 設定画面からのセーブ/ロード（settings-screen-plan 側）

## 既知の制限

- **別 seed セッションへのロード不可**: seed ガードで中止される（`HELL_WORKERS_WORLDGEN_SEED` 指定で再起動すれば可）。地形チャンクをロード時に再生成できれば解消するが未対応
- **Building footprint の子エンティティ**（`ObstaclePosition` + `Name` のみの子）は復元しない。歩行可否は保存済みの `WorldMap.obstacles` で保たれる。現状デモリッション経路が存在しないため実害なし — 建物解体を実装する際は footprint 子の復元（または `WorldMap` からの解決）を合わせて対応すること
- **SoulSpaTile の `ChildOf` 階層**は復元されない（`parent_site` フィールドで論理は維持。Transform は絶対値保存のため表示影響なし）
- **旧形式セーブ**（`SoulIdentity` / `SavedWorldgenSeed` を含まない）は Soul 名がランダム再生成され、seed 照合は warn のみ

## UI 構成（M4）

- **Pause メニュー**（`hw_ui/src/setup/pause_menu.rs`）: `Time<Virtual>` 一時停止中に overlay 中央へ表示。`MenuButton` → `UiIntent::SaveGame` / `RequestLoadGame`
- **ロード確認ダイアログ**（`hw_ui/src/setup/dialogs.rs`）: 単一スロットの上書き不可を前提に「現在の進行を破棄」警告。`ConfirmLoadGame` / `CancelLoadConfirm`
- **Intent 処理**（`bevy_app/.../handlers/save_game.rs`）: `SaveLoadState` へ橋渡し

既存 UI と同様 **`MenuButton` + `UiIntent` パターン** を採用（plan の `bsn!` は本リポジトリ未使用のため見送り）。
