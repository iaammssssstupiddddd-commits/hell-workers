# セーブ/ロード機能 — bevy_world_serialization 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `save-load-world-serialization-plan-2026-07-05` |
| ステータス | `Draft` |
| 作成日 | `2026-07-05` |
| 最終更新日 | `2026-07-05` |
| 作成者 | Claude (調査ベース) / Codex (具体化) |
| 関連提案 | N/A |
| 関連Issue/PR | 前提: `bevy-0-19-migration-plan-2026-07-05.md`（移行完了済み）。`serialize` feature は `settings-screen-plan-2026-07-05.md` と共用 |

## 1. 目的

- 解決したい課題: コロニーシムなのにセーブ/ロードが存在しない。ゲームを閉じると Soul・建物・資材・時間・Dream 等の全進捗が失われる
- 到達したい状態: シミュレーション状態を RON ファイルに保存し、再起動後に復元できる
- 成功指標（M4 完了時）:
  - Soul の位置・バイタル、Stockpile 内容、建物/Blueprint 進捗、`GameTime` / `DreamPool` / `PopulationManager` が復元される
  - ロード後 60 秒で panic / B0001 / タスク finder のサイレント失敗なし
  - `cargo check` + `cargo clippy --workspace`（警告 0）成功

## 2. スコープ

### 対象（In Scope）

- **M1**: 永続化対象の棚卸し（§5.1〜5.4）+ Reflect / `MapEntities` 対応 + Relationship 復元 PoC
- **M2**: セーブ経路（`DynamicWorldBuilder` + allow-list → RON → `saves/world.scn.ron`）
- **M3**: ロード経路（シミュレーション despawn → `write_to_world_with` → reconcile → キャッシュ再構築）
- **M4**: セーブ/ロード UI（BSN + `UiIntent`）+ 単一スロット

### 非対象（Out of Scope）

- 複数スロット・オートセーブ・セーブデータのバージョンマイグレーション
- WASM（`std::fs` 前提）
- ビジュアル/UI 状態（`hw_visual` / `hw_ui` マーカー、RtT テクスチャ、スピーチバブル等）— ロード後に visual_mirror / observer で再生成
- 設定（キーバインド等）— `settings-screen-plan-2026-07-05.md` の担当
- **実行中タスクの完全復元（Phase B）** — M1〜M3 では §4.3 の正規化方針。完全復元は MapEntities 全対応後の follow-up

## 3. 現状とギャップ

### 3.1 コードベース調査結果（2026-07-05）

| 項目 | 現状 |
| --- | --- |
| `bevy_world_serialization` | `2d`/`3d`/`ui` feature 経由で依存済み。GLB ロードで `Handle<WorldAsset>` 使用中 |
| `serialize` feature | **workspace `Cargo.toml` 未追加** → `bevy::world_serialization::serde` が使えない |
| `Reflect` derive | **約 40 ファイル**に存在（Relationship 22 型、Soul/Familiar、TransportRequest 等）。ただし **`register_type` は一部のみ**（`DamnedSoulPlugin` / `LogicPlugin` の energy 系等） |
| `MapEntities` / `#[entities]` | **0 件** — Entity 参照の再マップ未対応 |
| セーブ/ロードコード | **なし**（`crates/bevy_app/src/systems/save/` 未作成） |

### 3.2 最大リスク

1. **Entity 参照の再マップ漏れ** — `AssignedTask` 配下 15+ バリアント、`TransportRequest`、`WorldMap` の HashMap、 `FamiliarAiState` 等
2. **Relationship Target 非復元** — `DynamicWorld::write_to_world_with` は `RelationshipHookMode::Skip`（`dynamic_world.rs:154`）。Source 保存だけでは `TaskWorkers` / `IncomingDeliveries` 等が空のまま
3. **`WorldMap` Resource** — パス探索・容量判定の根。`Reflect` 未導入。Entity 参照フィールド多数

## 4. 実装方針（高レベル）

### 4.1 基本アーキテクチャ

```
[セーブ] exclusive system (&mut World)
  → DynamicWorldBuilder::from_world + allow_component/resource (§5.1)
  → DynamicWorld::serialize
  → IoTaskPool: saves/world.scn.ron.tmp 書込 → rename

[ロード] exclusive system (&mut World)
  → std::fs::read + WorldDeserializer
  → シミュレーション Entity despawn（§5.4 逆集合）
  → 派生キャッシュ Resource リセット（§5.3）
  → DynamicWorld::write_to_world_with（※ DynamicWorldRoot は使わない）
  → relationship reconcile pass（§4.7）
  → 空間グリッド / SharedResourceCache 再構築
  → visual 再生成（既存 observer 経由）
```

- **ロードは `DynamicWorldRoot` を使わない**: upstream example は AssetServer 経由の child spawn 用。本ゲームは **`write_to_world_with` でルート直下に Entity を復元** する（既存 Query が `Without<ChildOf>` 等を使わない前提）
- **保存先**: 実行ディレクトリ直下 `saves/`（assets 外 → ホットリロード対象外）
- **配置**: `crates/bevy_app/src/systems/save/`（root crate のみ全型に届く）

### 4.2 検証済み 0.19 API

| API | 場所 | 要点 |
| --- | --- | --- |
| `DynamicWorld::from_world_with` / `.serialize` | `bevy_world_serialization` | RON 文字列生成 |
| `DynamicWorldBuilder` + `allow_component` / `deny_component` / `allow_resource` | `dynamic_world_builder.rs` | デフォルトは **全 Reflect 登録型** を抽出。allow-list 必須 |
| `DynamicWorld::write_to_world_with` | `dynamic_world.rs:87-204` | Entity を `spawn_empty` でルート復元。Resource は既存を上書き |
| `WorldDeserializer` / `DynamicWorldSerializer` | `serde.rs` | `std::fs` 読み書き用 |
| `RelationshipHookMode::Skip` | `dynamic_world.rs:154,198` | reconcile pass **必須** |
| `#[entities]` / `#[component(map_entities)]` / `MapEntities` | `bevy_ecs` | Reflect だけでは Entity remap されない |
| `#[reflect(skip_serializing)]` / `FromWorld` | `bevy_reflect` | ランタイム専用フィールド除外 |
| `bsn!` / `spawn_scene` | `bevy_scene` | M4 UI |
| `serialize` feature | `bevy/Cargo.toml` | workspace features に `"serialize"` 追加 |

### 4.3 AssignedTask 保存方針（最初の設計判断）

**M1〜M3 推奨: Phase A（正規化）**

| タイミング | 処理 |
| --- | --- |
| セーブ直前 | 全 Soul で `AssignedTask != None` → `unassign_task(emit=false)` 相当の正規化。`WorkingOn` / `DeliveringTo` / `LoadedIn` / `PushedBy` 等も解除 |
| ロード後 | `Designation` + `TransportRequest` が残っていれば、0.5s 以内に Familiar AI が再割当（§tasks.md §4.2） |

**理由**: `AssignedTask` 全バリアント + `WheelbarrowLease` + `Inventory` + Relationship の MapEntities 整合を一度に通すと M1 が膨らむ。Phase A なら PoC は `BelongsTo` / `StoredIn` / `Stockpile` 中心で成立する。

**Phase B（follow-up）**: 実行中タスク完全復元。§5.2 の MapEntities 表を全 ✅ にしてから切替。

### 4.4 WorldMap 保存方針

`WorldMap`（`hw_world/src/map/mod.rs`）は **必ず保存** する。

| フィールド | 内容 | MapEntities |
| --- | --- | --- |
| `tiles` | 地形（player による変更なしだが、橋/床で walkability 変化） | 不要 |
| `tile_entities` | `Vec<Option<Entity>>` | **必要** |
| `buildings` / `doors` / `stockpiles` | `HashMap<(i32,i32), Entity>` | **必要** |
| `door_states` | `HashMap<(i32,i32), DoorState>` | 不要 |
| `obstacles` / `bridged_tiles` | bool / HashSet | 不要 |

実装: `hw_world` に `#[derive(Reflect)]` + MapEntities 対応を追加（HashMap 内 Entity を walk）。

**⚠️ Resource の remap 配線に注意**: ロード時の remap は `ReflectComponent::apply_or_insert_mapped` → **`Component::map_entities`**（Component derive が `#[entities]` / `#[component(map_entities)]` から生成）経由で行われる（`bevy_ecs/src/reflect/component.rs:331-353` で確認済み）。手書きの `impl MapEntities for WorldMap` だけでは **この経路に接続されない可能性がある**。`bevy_world_serialization` 自身のテスト（`dynamic_world.rs:262-263`）が Resource remap の正解パターンで、`#[derive(Resource, Reflect, MapEntities)]` + `#[reflect(Resource, MapEntities)]` を使っている。**このパターンをそのまま踏襲し、PoC A に「Entity 参照を含む Resource の remap 検証」を含めること**（HashMap キー `(i32,i32)` は remap 不要、値の Entity のみ対象なので `#[entities]` フィールド属性で足りるかは derive の HashMap 対応次第 — ダメなら MapEntities derive ではなく手動 impl + `#[reflect(MapEntities)]` 登録を試す）。

`GeneratedWorldLayoutResource`（`master_seed`）は **M1 では保存しない**（`WorldMap.tiles` で地形は復元可能）。将来 diff 検証用に `saves/meta.ron` へ seed を書くのは optional。

### 4.5 SavePlugin 配線

```rust
// crates/bevy_app/src/systems/save/mod.rs
pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveLoadState>()
            .add_message::<SaveLoadRequest>()  // Save / Load / 完了通知
            .add_systems(
                Update, // ※ add_systems の第1引数は Schedule。SystemSet は .in_set() で指定する
                (
                    save_world_system.run_if(on_save_requested),
                    load_world_system.run_if(on_load_requested),
                    reconcile_relationships_system.after(load_world_system),
                    rebuild_caches_after_load.after(reconcile_relationships_system),
                )
                    .chain()
                    .in_set(GameSystemSet::Logic), // 実際の schedule は logic.rs の既存登録パターンに合わせる
            );
    }
}
```

- 登録: `crates/bevy_app/src/plugins/logic.rs` の `add_plugins(SavePlugin)`
- ロード中は `SaveLoadState::Loading` で `Time<Virtual>` pause + AI system `run_if(not_loading)`（新 State は追加せず Resource 1 つ）

### 4.6 セーブ allow-list 構築（M2 実装スケッチ）

```rust
fn build_save_scene(world: &World, registry: &TypeRegistry) -> DynamicWorld {
    DynamicWorldBuilder::from_world(world, registry)
        .deny_all_components()
        .deny_all_resources()
        // §5.1 (a) の Component を allow_component::<T>() で列挙
        .allow_component::<Transform>()
        .allow_component::<DamnedSoul>()
        // ...（完全リストは §5.1）
        .allow_resource::<GameTime>()
        .allow_resource::<DreamPool>()
        .allow_resource::<PopulationManager>()
        .allow_resource::<WorldMap>()
        // ※ extract_entities は引数に Iterator<Item = Entity> を取る（dynamic_world_builder.rs:293）。
        //    引数なしの「allow 済み Component を持つ Entity を自動抽出」という API は存在しない。
        //    対象 Entity は Query で収集して渡す（builder の doc example は query.iter(&world)）
        .extract_entities(save_target_entities)
        .extract_resources()
        .build()
}
```

**注意**:
- `deny_all` → `allow_*` の順。逆にすると漏れる
- 対象 Entity の収集は §5.1 (a) のマーカー的 Component（`DamnedSoul` / `Building` / `ResourceItem` / `Designation` 等）の Or クエリで行う。allow-list（何を保存するか）と extract 対象（どの Entity か）は**別々に管理される**点に注意

### 4.7 Relationship reconcile pass（M3 必須）

`RelationshipHookMode::Skip` により Target 側が空になるため、ロード直後に **Source を通常 insert 経路で再挿入** する。

```rust
// 疑似コード — crates/bevy_app/src/systems/save/reconcile.rs
fn reconcile_relationships_system(world: &mut World) {
    // 1. WorkingOn: Soul → task
    for (soul, working_on) in query::<(&WorkingOn,)>() {
        world.entity_mut(soul).remove::<WorkingOn>();
        world.entity_mut(soul).insert(WorkingOn(working_on.0));
    }
    // 2. CommandedBy, ManagedBy, StoredIn, DeliveringTo, LoadedIn,
    //    ParkedAt, PushedBy, ParticipatingIn, RestingIn, RestAreaReservedFor
    // 3. hw_energy: GeneratesFor, ConsumesFrom（Source のみ）
    // 各 Source で remove → insert を同一フレーム内で実行
}
```

完了条件: `TaskWorkers.len()` が `WorkingOn` 数と一致、`IncomingDeliveries` が `DeliveringTo` 数と一致（整合性プローブ §7）。

### 4.8 BSN 制約（M4）

- `bsn!` 内コンポーネントは `Default + Clone` 必須
- enum は variant ごと `default_{variant}` が必要な場合あり
- `.bsn` アセットファイルは未リリース — **`bsn!` マクロのみ**
- `UiIntent` に `SaveGame` / `LoadGame` を追加（`crates/hw_ui/src/intents.rs`）

---

## 5. 永続化対象分類表

### 5.1 (a) 保存する simulation 状態

#### Resources

| Resource | 定義場所 | Reflect | register_type | 備考 |
| --- | --- | --- | --- | --- |
| `GameTime` | `hw_core/src/time.rs` | ✅ | ❌ 要追加 | 日時 |
| `DreamPool` | `hw_core/src/soul.rs` | ✅ | ✅ | 通貨 |
| `PopulationManager` | `hw_core/src/population.rs` | ❌ 要追加 | ❌ | `Timer` 含む |
| `WorldMap` | `hw_world/src/map/mod.rs` | ❌ 要追加 | ❌ | **MapEntities 必須** |

#### Soul / Familiar

| Component | 定義場所 | Reflect | MapEntities | 備考 |
| --- | --- | --- | --- | --- |
| `Transform` | bevy | ✅ (built-in) | — | 位置 |
| `DamnedSoul` | `hw_core/src/soul.rs` | ✅ | — | バイタル |
| `IdleState` | 同上 | ✅ | — | |
| `DreamState` | 同上 | ✅ | — | |
| `StressBreakdown` | 同上 | ✅ | — | |
| `RestAreaCooldown` | 同上 | ✅ | — | |
| `DriftingState` | 同上 | ✅ | — | |
| `AssignedTask` | `hw_jobs/src/tasks/mod.rs` | ✅ | **Phase B** | Phase A では `None` 正規化 |
| `Destination` | `hw_core/src/soul.rs` | ❌ 要追加 | — | 移動先 |
| `Path` | 同上 | ❌ 要追加 | — | ロード後再計算でも可 → skip 可 |
| `Inventory` | `hw_logistics/src/types.rs` | ✅ | **要** `Option<Entity>` | |
| `Familiar` | `hw_core/src/familiar.rs` | ✅ | — | |
| `FamiliarOperation` | 同上 | ❌ 要追加 | — | |
| `ActiveCommand` | 同上 | ❌ 要追加 | — | |
| `FamiliarAiState` | 同上 | ✅ | **要** enum 内 Entity | Phase A では Idle 正規化可 |
| `TaskArea` | `hw_core/src/area.rs` | ❌ 要追加 | — | |

#### Relationship Source（保存） / Target（保存しない）

| Source（保存） | Target（derive のみ、保存しない） | 定義 |
| --- | --- | --- |
| `CommandedBy` | `Commanding` | `hw_core/src/relationships.rs` |
| `WorkingOn` | `TaskWorkers` | 同上 |
| `ManagedBy` | `ManagedTasks` | 同上 |
| `StoredIn` | `StoredItems` | 同上 |
| `LoadedIn` | `LoadedItems` | 同上 |
| `ParkedAt` | `ParkedWheelbarrows` | 同上 |
| `PushedBy` | `PushingWheelbarrow` | 同上 |
| `DeliveringTo` | `IncomingDeliveries` | 同上 |
| `ParticipatingIn` | `GatheringParticipants` | 同上 |
| `RestingIn` | `RestAreaOccupants` | 同上 |
| `RestAreaReservedFor` | `RestAreaReservations` | 同上 |
| `GeneratesFor` | `GridGenerators` | `hw_energy/src/relationships.rs` |
| `ConsumesFrom` | `GridConsumers` | 同上 |

Source 型は既に `Reflect` + `#[relationship]` 付き。Entity フィールドの remap は PoC で要確認（自動か `#[component(map_entities)]` 要否）。

#### タスク / 建築 / 物流

| Component | 定義場所 | Reflect | 備考 |
| --- | --- | --- | --- |
| `Designation` | `hw_jobs/src/model.rs` | ❌ 要追加 | タスク発見の前提 |
| `TaskSlots` | 同上 | ✅ | |
| `Priority` | 同上 | ✅ | |
| `Blueprint` | 同上 | ❌ 要追加 | HashMap 含む |
| `Building` | 同上 | ✅ | |
| `Door` | 同上 | ✅ | |
| `RestArea` | 同上 | ✅ | |
| `ProvisionalWall` | 同上 | ❌ 要追加 | |
| `ResourceItem` | `hw_logistics/src/types.rs` | ✅ | |
| `BelongsTo` | 同上 | ✅ | MapEntities 要 |
| `ReservedForTask` | 同上 | ✅ | legacy だが保存してよい |
| `Wheelbarrow` / `WheelbarrowParking` | 同上 | ✅ | |
| `Stockpile` | `hw_logistics/src/zone.rs` | ✅ | |
| `TransportRequest` | `hw_logistics/.../components.rs` | ✅ | MapEntities 要（`anchor`, `issued_by`, `stockpile_group`） |
| `TransportDemand` / `TransportPolicy` | 同上 | ✅ | |
| `WheelbarrowLease` | 同上 | ✅ | Phase A ではセーブ前解除 |
| `FloorTileBlueprint` / `WallTileBlueprint` | `hw_jobs/src/construction.rs` | ❌ 要追加 | `parent_site: Entity` |
| `FloorConstructionSite` / `WallConstructionSite` | 同上 | ❌ 要追加 | |
| Soul Spa / Power 系 | `hw_energy/src/` | ✅ 一部 | `register_type` は logic.rs に一部済 |

#### ワールド Entity マーカー

| Component | 保存 | 備考 |
| --- | --- | --- |
| `Tree` / `Rock` / `Tile` | ✅ | 採取対象 |
| `ObstaclePosition` | ✅ | WorldMap 同期 |
| `BridgeMarker` / `SandPile` / `BonePile` | ✅ | |
| `TerrainChunk` | ❌ | 描画専用。`WorldMap` から再生成 |

### 5.2 Entity 参照 MapEntities 対応表

| 型 | Entity フィールド | 対応方法 | Phase |
| --- | --- | --- | --- |
| `BelongsTo` | `0: Entity` | `#[component(map_entities)]` | A |
| `Inventory` | `Option<Entity>` | 同上 | A |
| `PendingBelongsToBlueprint` | `Entity` | 同上 | A |
| `TransportRequest` | `anchor`, `issued_by`, `stockpile_group: Vec<Entity>` | `#[entities]` on struct | A |
| `TransportRequestFixedSource` | `Entity` | map_entities | A |
| `WheelbarrowLease` | `wheelbarrow`, `items: Vec<Entity>` | map_entities | B |
| `TargetSoulSpaSite` | `Entity` | map_entities | A |
| `MovePlanned` | `task_entity` | map_entities | B |
| `FamiliarAiState::Scouting/Supervising` | `target_soul`, `target` | enum variant map | B（A では Idle 化） |
| `WorldMap` | 複数 HashMap/Vec | `impl MapEntities` | A |
| `AssignedTask::*Data` | 各 variant の Entity | ネスト全体 | B |
| Relationship Source (`WorkingOn` 等) | tuple `Entity` | PoC で確認 → 不足なら map_entities | A |

**M1 PoC A 最小セット**: `BelongsTo` + `StoredIn` + `Inventory` + `ResourceItem` + `Stockpile` + `Designation(Haul)` + `TransportRequest` + **Entity 参照を含む Resource 1 つ（`WorldMap` の縮小版で可）** — Component と Resource で remap 配線が異なるため両方を PoC で通す（§4.4 の注意参照）

### 5.3 (b) 保存しない派生キャッシュ — ロード後リセット対象

| Resource | init 場所 | 再構築方法 |
| --- | --- | --- |
| `SharedResourceCache` | `familiar_ai/mod.rs` | `SharedResourceCache::default()` → 次 `sync_reservations_system`（0.2s） |
| `SpatialGrid` | `startup/mod.rs` | `clear()` + Change Detection 次フレーム |
| `FamiliarSpatialGrid` | 同上 | 同上 |
| `ResourceSpatialGrid` | 同上 | 同上 |
| `GatheringSpotSpatialGrid` | 同上 | 同上 |
| `BlueprintSpatialGrid` | 同上 | 同上 |
| `FloorConstructionSpatialGrid` | 同上 | 同上 |
| `StockpileSpatialGrid` | 同上 | 同上 |
| `DesignationSpatialGrid` | `spatial.rs` | 同上 |
| `TransportRequestSpatialGrid` | 同上 | 同上 |
| `ReachabilityFrameCache` | `hw_familiar_ai` | `default()` |
| `RoomDetectionState` / `RoomTileLookup` / `RoomValidationState` | `logic.rs` | `default()` → 次 detection 周期 |
| `RegrowthManager` | `logic.rs` | **`configure_regrowth_from_generated_layout` 相当をロード後に再実行** |
| `TileSiteIndex` | `spatial.rs` | 再構築 system |
| Transport producer caches | `hw_logistics` (`ActiveUnitCache` 等) | `default()` |

M3 完了条件: §5.3 **全行** のリセット関数が `rebuild_caches_after_load` から呼ばれること。

### 5.4 (c) 保存しないビジュアル / UI / 入力状態

| カテゴリ | 例 | 理由 |
| --- | --- | --- |
| `hw_visual/*` | `SpeechBubble`, `BlueprintVisual`, `SoulProgressBar`, `TerrainChunk` | 派生描画 |
| `hw_core/visual_mirror/*` | `BlueprintVisualState`, `SoulTaskVisualState` | simulation → visual 同期で再生成 |
| `hw_ui/*` Component | `EntityListPanel`, `SoulListItem` | UI ツリーは Startup で再構築 |
| `SoulUiLinks` | bar/icon Entity 参照 | **`#[reflect(skip_serializing)]`** + ロード後再リンク |
| 入力 Resource | `TaskContext`, `BuildContext`, `SelectedEntity`, `MenuState` | セッション状態 |
| Debug | `DebugInstantBuild`, `RenderPerfToggles` | dev のみ |

### 5.5 Reflect 登録チェックリスト（M1）

`register_type` / `register_type_data` は **bevy_app 側**（hw_* クレート規約）。

| クレート | 追加 register 対象（§5.1 で Reflect 新規追加した型） |
| --- | --- |
| `bevy_app/entities/damned_soul/mod.rs` | 既存 + `AssignedTask` 全 sub-types |
| `bevy_app/plugins/logic.rs` | `Designation`, `Blueprint`, `WorldMap`, `PopulationManager`, construction 型 |
| `bevy_app/plugins/startup/mod.rs` | `GameTime` |
| 新規 `bevy_app/plugins/save/mod.rs` | SavePlugin 内で一括登録も可（漏れ防止） |

---

## 6. マイルストーン

### M1: 永続化対象の棚卸しと Relationship 復元 PoC

**目的**: §5.1〜5.5 を確定し、最小ワールドで save → load → Entity remap + Relationship reconcile を通す。

#### 手順

1. **Feature / dependency**
   - `Cargo.toml` workspace bevy features に `"serialize"` 追加
   - `crates/bevy_app/Cargo.toml` に `ron`, `serde` を direct dependency 追加

2. **Reflect + MapEntities（§5.1 / §5.2 の ❌ 行）**
   - 優先: `WorldMap`, `Designation`, `Blueprint`, `PopulationManager`, `BelongsTo` map_entities
   - `SoulUiLinks.bar_entity` / `icon_entity` に `#[reflect(skip_serializing)]`

3. **register_type 一括**（§5.5）

4. **PoC システム** — `crates/bevy_app/src/systems/save/poc.rs`
   - Startup で Soul×2 + Stockpile + ResourceItem + `StoredIn` + `BelongsTo` + `Designation` を spawn
   - F5: save → despawn all → load → assert
   - PoC B: `WorkingOn` + reconcile pass で `TaskWorkers.len() == 1`

5. **Phase A 正規化関数** — `normalize_for_save(world)` スタブ（M2 で本実装）

#### 変更ファイル

| ファイル | 内容 |
| --- | --- |
| `Cargo.toml`, `crates/bevy_app/Cargo.toml` | features / deps |
| `crates/hw_world/src/map/mod.rs` | WorldMap Reflect + MapEntities |
| `crates/hw_core/src/population.rs` | PopulationManager Reflect |
| `crates/hw_jobs/src/model.rs` | Designation, Blueprint Reflect |
| `crates/hw_jobs/src/construction.rs` | construction 型 Reflect |
| `crates/hw_logistics/src/types.rs` | map_entities on Entity 型 |
| `crates/hw_core/src/relationships.rs` | map_entities（PoC 結果次第） |
| `crates/bevy_app/src/systems/save/{mod,poc,reconcile}.rs` | PoC（M2 で poc 削除） |

#### 完了条件

- [ ] §5.1〜5.4 が本計画に記載済み（本更新で ✅）
- [ ] PoC A: `BelongsTo` / `StoredIn` / `Inventory` の Entity 再マップ成功
- [ ] PoC B: reconcile 後 `TaskWorkers` が `WorkingOn` と整合
- [ ] `cargo check` 成功

---

### M2: セーブ経路

#### 手順

1. `serialize.rs`: `normalize_for_save` → `build_save_scene` → `serialize` → IoTaskPool 書込
2. 原子書込: `saves/world.scn.ron.tmp` → `rename("saves/world.scn.ron")`
3. `SaveLoadRequest::Save` メッセージ + 暫定キー `KeyCode::F5`（dev_tools 有効時のみ、`main.rs` パターン踏襲）
4. 計測: `info!("save took {:?}", elapsed)` — 100ms 超なら warn

#### 完了条件

- [ ] 実プレイ後 F5 で `saves/world.scn.ron` 生成
- [ ] RON を目視確認: Soul / Building / Stockpile / GameTime が含まれる
- [ ] UI / visual Component が **含まれない**

---

### M3: ロード経路

#### 手順

1. `load.rs`: `SaveLoadState::Loading` → pause
2. `despawn_simulation_entities`: §5.1 (a) の Component のいずれかを持つ Entity を despawn（`WorldMap` tile_entities に登録された Tile anchor は **除外** — `WorldMap` Resource で座標管理）
3. `WorldDeserializer` → `DynamicWorld`
4. `EntityHashMap::default()` + `dynamic_world.write_to_world_with(&mut world, &mut map, &registry)`
5. `reconcile_relationships_system`
6. `rebuild_caches_after_load`（§5.3 全行）
7. `SaveLoadState::Idle` → unpause
8. **`normalize_after_load`（Phase A）**: 念のため `AssignedTask` が残っていたら `None` 化

#### 完了条件

- [ ] セーブ → プロセス再起動 → ロード（UI or F9）で §1 成功指標を満たす
- [ ] ロード直後: Stockpile `StoredItems.len()` + `IncomingDeliveries.len()` で容量判定が動作（I-L3）
- [ ] ロード後 60 秒: panic / B0001 なし
- [ ] `cargo check` + clippy 0 warnings

---

### M4: セーブ/ロード UI（BSN）

#### 手順

1. `crates/hw_ui/src/setup/save_menu.rs`: Pause メニューに Save / Load ボタン（`bsn!`）
2. `Button` + `Activate` observer → `UiIntent::SaveGame` / `LoadGame`
3. `crates/bevy_app/src/interface/ui/interaction/intent_handler.rs` で `SaveLoadRequest` 発行
4. Load 確認ダイアログ（上書き不可 — 単一スロットなので「現在の進行を破棄」警告）
5. `hw_ui/_rules.md` に BSN 知見追記

#### 完了条件

- [ ] UI からセーブ/ロード可能
- [ ] `_rules.md` 更新済み
- [ ] `cargo clippy --workspace` 0 warnings

---

## 7. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `saves/` が AssetServer root 外 | ファイル未検出 | `std::fs` + `WorldDeserializer`。AssetServer 不使用 |
| MapEntities 漏れ | 旧 Entity 参照でタスク/物流破綻 | §5.2 表 + PoC A/B + ロード直後整合性プローブ |
| Relationship Target 空 | 容量判定/finder 破綻 | §4.7 reconcile pass を M3 必須 |
| `WorldMap` 未保存 | パス探索全滅 | §4.4。M1 最優先で Reflect + MapEntities |
| `DynamicWorldRoot` child spawn | Query 漏れ | **`write_to_world_with` を使用**（§4.1） |
| Phase A 正規化でタスク中断 | UX: ロード後に再割当待ち | Designation 残存で 0.5s 以内に再開。Phase B で改善 |
| allow-list 漏れ | サイレント欠落 | M2 で RON 目視 + 必須 Component の assert ログ |
| exclusive system スパイク | セーブヒッチ | IoTaskPool で IO のみ async。serialize 時間を計測 |

## 8. 検証計画

### 8.1 自動

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace 2>&1 | grep "^warning:" | grep -v generated
```

### 8.2 手動シナリオ

| # | シナリオ | 確認点 |
| --- | --- | --- |
| S1 | 初期 spawn → 即セーブ → 再起動 → ロード | Soul 数、Familiar、GameTime |
| S2 | Stockpile に Wood 10 → セーブ → ロード | `StoredItems` 復元、容量判定 |
| S3 | Blueprint 建設中（資材半分）→ セーブ → ロード | `delivered_materials` 一致 |
| S4 | Floor construction 進行中 → セーブ → ロード | tile state 復元 |
| S5 | Soul 搬送中（Phase A）→ セーブ → ロード | タスク再割当、Designation 残存 |
| S6 | ロード後 60 秒放置 | panic / 予約リークなし |

### 8.3 整合性プローブ（M3 一時、原因切り分け後撤去）

ロード直後 1 フレームのみ:

- 全 `BelongsTo` / `DeliveringTo` / `Inventory` の参照先 Entity が alive か
- `WorkingOn` ↔ `TaskWorkers` 双方向一致
- `WorldMap.buildings` の Entity が alive か

## 9. ロールバック方針

- マイルストーン単位で revert 可能
- M1 の Reflect 付与は他機能に無害 — 残してよい
- PoC ファイル（`poc.rs`）は M2 完了時に削除

## 10. AI引継ぎメモ（最重要）

### 現在地

- 進捗: **5%**（計画具体化完了、実装未着手）
- 完了済み: なし
- 最優先: M1 — `serialize` feature 追加 → `WorldMap` Reflect → PoC A

### 次の AI が最初にやること（順序固定）

1. `Cargo.toml` に `"serialize"` を追加し `cargo check` で `bevy::world_serialization::serde` が使えることを確認
2. `hw_world/src/map/mod.rs` に `WorldMap` の Reflect + MapEntities（§4.4）
3. `crates/bevy_app/src/systems/save/poc.rs` で PoC A（§5.2 最小セット）
4. PoC B で reconcile pass 原型（§4.7）— **`DynamicWorldRoot` は使わず `write_to_world_with`**
5. Phase A/B 判断: **Phase A で進める**（§4.3）。AssignedTask 完全復元は Phase B

### ブロッカー/注意点

- **`#[entities]` / MapEntities は codebase に 0 件** — 推測で書かず Bevy 0.19 registry / docs.rs を確認
- Relationship Source の Entity remap が `#[relationship]` だけで足りるか **PoC B で実測**（足りなければ `#[component(map_entities)]`）
- `Holding` Relationship は **存在しない**（tasks.md legacy）。インベントリは `Inventory` Component
- `Path` / `Destination` は保存 skip 可（ロード後に再計算）
- `Timer` を含む Resource（`PopulationManager`）は Reflect 可能だが、serde 互換を PoC で確認
- hw_visual / hw_ui Component を allow-list に **絶対入れない**

### 参照必須ファイル

| ファイル | 用途 |
| --- | --- |
| `~/.cargo/registry/.../bevy-0.19.0/examples/scene/world_serialization.rs` | 基本パターン |
| `~/.cargo/registry/.../bevy_world_serialization-0.19.0/src/dynamic_world.rs` | `write_to_world_with`, `RelationshipHookMode::Skip` |
| `~/.cargo/registry/.../bevy_world_serialization-0.19.0/src/dynamic_world_builder.rs` | allow/deny API |
| `~/.cargo/registry/.../bevy_world_serialization-0.19.0/src/serde.rs` | `WorldDeserializer` |
| `docs/tasks.md` §2.1 | Relationship Source/Target |
| `docs/invariants.md` | I-S1, I-T2, I-L3 |
| `docs/crate-boundaries.md` | Reflect 登録は bevy_app |

### Definition of Done

- [ ] M1〜M4 完了
- [ ] §8 シナリオ S1〜S6 通過
- [ ] `docs/save_load.md`（新規）に恒久仕様記載 → 本計画アーカイブ

### 最終確認ログ

- 最終 `cargo check`: 未実施
- 未解決エラー: なし

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-05` | Claude | 初版（0.19 API registry 検証） |
| `2026-07-05` | Codex | レビュー反映（saves/ 経路、MapEntities、Relationship リスク） |
| `2026-07-05` | Codex | **具体化**: §5.1〜5.5 分類表、§4.3 Phase A/B、WorldMap 方針、`write_to_world_with` 採用、SavePlugin 配線、reconcile アルゴリズム、検証シナリオ S1〜S6、Reflect 現状調査 |
| `2026-07-05` | Claude | **レビュー検証**: registry/実コード突き合わせで主要主張の正確性を確認（write_to_world_with:92 / Skip:154,198 / builder API / Relationship 11+2 対 / WorldMap フィールド）。修正 3 件: §4.5 add_systems の Schedule/SystemSet 誤用、§4.6 extract_entities の引数（Iterator 必須）、§4.4 Resource remap 配線の注意 + PoC A に Resource remap 検証を追加 |
| `2026-07-06` | Claude | **実装レビュー後の修正**: (1) ロード後の rehydrate 実装（`rehydrate.rs` — spawn を core/shell 分離し `attach_soul_shell` / `attach_familiar_shell` / `attach_building_shell` を spawn とロードで共用、Tree/Rock/Item/Stockpile は Sprite 直接復元、孤児インベントリのドロップ）、(2) 保存漏れ追加（`SoulIdentity` / `Site` / `Yard` / `PairedSite` / `PairedYard` — Stockpile `BelongsTo(yard)` の宙吊り防止）、(3) `SavedWorldgenSeed` によるクロスセッション seed ガード。仕様は `docs/save_load.md` に恒久化 |
