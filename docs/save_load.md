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
F9 は save file が存在する場合だけ確認ダイアログを開き、存在しない場合は warning を記録して何もしない。
AreaEdit の active drag 中は未確定の `TaskArea` を保存しないため、keyboard F5 actionを生成しない。

## アーキテクチャ

```text
[PreUpdate::InputPreUpdateSet::Resolve]
  F5/F9 exact chord → frame-local ResolvedInputFrame

[Update::InputResolutionSet::Consume]
  SaveGame / RequestLoadGame → UiIntent

[Update::Interface]
  SaveGame → SaveLoadState::SaveRequested
  RequestLoadGame → save path確認 → load confirmを開く（stateはIdleのまま）
  ConfirmLoadGame → SaveLoadState::LoadRequested

[Last::SaveLoadApplySet] exclusive dispatcher
  [セーブ]
    → DynamicWorldBuilder (deny-all + allow-list)
    → extract_entities(collect_persisted_entities)
    → DynamicWorld RON body を serialize → v1 external header → atomic rename

  [ロード]
    → SavePath から read
    → external header を decode（v1 の version / worldgen seed を body deserialize 前に照合）
    → RON body deserialize (WorldDeserializer) → legacy v0 だけ body 内 seed を照合
    → PreparedLoad schema検証 → staging World preflight → rehydrate prerequisite検証
    → rollback snapshot を取得
    → LoadResetRegistry（message / selection / UI / visual / cache）を実行
    → stale presentation と persisted entityをdespawn → flush → WorldEpochを進める
    → old RemovedComponents bufferを2回clear_trackersして破棄
    → DynamicWorld::write_to_world_with (Entity remap)
    → finalize: runtime cache reset → AssignedTask::None → rehydrate
    → live apply失敗時: partial entityを掃除 → resetを再実行 → snapshot復元 → 同じfinalize
```

実装: `crates/bevy_app/src/systems/save/`（`SavePlugin`）。

`ResolvedInputFrame` は入力resolverの当該frame snapshotであり、save schema、Reflect、永続queueへ入れない。
UI buttonもkeyboardも同じ `UiIntent` handlerを通るため、F9から直接`LoadRequested`へ遷移する経路はない。

## ファイル形式と互換性

v1 は DynamicWorld RON body の外側に registry 非依存の header を持つ。

```text
HELL_WORKERS_SAVE
(format_version: 1, worldgen_seed: 12345)
---
<DynamicWorld RON body>
```

- `SavePath` Resource の既定値は `saves/world.scn.ron`。UI のロード確認、save、load は同じ Resource を参照するため、テストまたは将来の slot 選択でパスを差し替えても判定経路が分岐しない。
- magic を持つファイルは current format version と完全一致しなければ、DynamicWorld body を deserialize せず reject する。future version と旧 version の migration を header 形式で推測しない。
- v1 の `worldgen_seed` は header が正本であり、body に `SavedWorldgenSeed` を含めない。seed mismatch は DynamicWorld の型 registry や entity を触る前に中止する。
- magic 無しの既存ファイルだけを legacy v0 として読む。v0 は body の `SavedWorldgenSeed` を後方互換の seed guard として使用し、存在しない場合は警告して継続する。
- v0 の `SavedWorldgenSeed` は seed 照合だけに使い、照合後にDynamicWorldから除去する。live worldへは適用しない。
- `ReservedForTask` は header 無し v0 body を読む間だけ registry に登録する legacy shim である。v0 deserialize 後、schema 検証より前に全 entity から除去する。v1 の allow-list には含めず、v1 body に混入した同型は schema reject とする。
- 書き込みは同一ディレクトリの `create_new` で確保した一意 temp file を `sync_all` した後に rename する。固定 `.tmp` 名を共有しないため、並列 test や別プロセスと temp file 名が衝突しない。保存先そのものの複数プロセス排他はこの機構の対象外である。

## 保存対象

### Resources

- `GameTime`, `DreamPool`, `PopulationManager`, `WorldMap`

`SavedWorldgenSeed` は header 無し legacy v0 body を読むためだけに Reflect 登録を維持する。v1 の保存 allow-list には含めない。

### シミュレーション Entity

マーカーコンポーネントで選別（`collect_persisted_entities`）。例:

- Soul / Familiar（`DamnedSoul`, `SoulIdentity`, `Familiar`）
- タスク・建築（`Designation`, `Blueprint`, `Building`, construction site 等）
- 物流（`ResourceItem`, `Stockpile`, `TransportRequest`, `Wheelbarrow` 等）
- エネルギー（`PowerGrid`, `SoulSpaSite` 等）
- ワールド採取対象・ゾーン（`Tree`, `Rock`, `Tile`, `Site`, `Yard`, `PairedSite`/`PairedYard`）

各 Entity に付く **永続 simulation state の Relationship Source / Target**（runtime-derived obstacle marker / mirror と transient gathering relationship を除く）、および `Transform` 等の allow-list コンポーネントも保存する。

`WorldMap` は Resource として保存し、内部の Entity 参照（`buildings`, `doors`, `stockpiles`, `tile_entities`）は `map_world_map_entities` で remap する。

### 保存しないもの

| カテゴリ | 例 | ロード後 |
| --- | --- | --- |
| 実行中タスク状態 | `AssignedTask`, `Path`, `Destination`, `FamiliarAiState` | Soul へ `AssignedTask::None` を付与、shell 側で `FamiliarAiState` 等をデフォルト再挿入。Familiar AI が Designation から再割当 |
| 派生キャッシュ | 空間グリッド、`SharedResourceCache`、`ReservationSignatureCache`、transport producer cache、`CachedStockpileGroups`、`ObstaclePositionIndex` | root reset hookでdefault化する。予約同期 timerもresetし、次のPerceiveが初回同期として完全snapshotを再構築 |
| runtime obstacle provenance / navigation cache | `ObstacleSourceKind`、`BuildingFootprint`、`ObstaclePositionIndex`、raw `WorldMap.obstacles` / `doors` / `bridged_tiles` | `rehydrate_obstacle_runtime` が durable semantic source から marker / cache を再構築。保存済み Door state は最終 override として使う |
| transient gathering | `GatheringSpot`、`GatheringVisuals`、`ParticipatingIn`、`GatheringParticipants` | v1 saveから除外。旧bodyのrelationship componentはschema検証前に破棄し、replace hookはspotとlinked aura/objectをdespawn。Soulは非参加状態から通常AIへ戻る |
| legacy task marker | `ReservedForTask` | header 無し v0 body の deserialize だけで受け付け、schema 検証前に除去する。v1 save / load には含めず、v1 body の混入は reject |
| ビジュアル / UI | `hw_visual/*`, `hw_ui/*`, `SoulUiLinks`, Sprite / 3D プロキシ | **rehydrate**（下記）と observer / startup で再生成 |
| 地形描画 | `TerrainChunk` | 起動時 seed から生成（v1 header の seed 照合で整合を保証） |
| セッション入力 | `BuildContext`, `SelectedEntity` 等 | root reset hookでdefault化し、`NextState<PlayMode>`は`Normal`を予約する |

## Save schema

`crates/bevy_app/src/systems/save/schema.rs` が保存型の唯一の正本である。X-macro の分類から
Reflect 登録、`DynamicWorldBuilder` の allow-list、root entity の収集を生成するため、
`saving.rs` や別の型一覧へ同じ型を追加してはならない。

| 分類 | schema の出力 | 用途 |
| --- | --- | --- |
| `persisted_resource` | `register_type` + `allow_resource` | `GameTime`、`WorldMap` などの durable Resource |
| `persisted_component` | `register_type` + `allow_component` | entity に保存する simulation component と Relationship Source / Target |
| `reflect_dependency` | `register_type` のみ | component/resource の reflect field で使う enum・値型、および v0 の `SavedWorldgenSeed` |
| `root_marker` | `collect_persisted_entities` | 保存対象 entity を選ぶ marker。component の保存可否とは別の分類 |

`Transform` は persisted component と同様に allow するが、Bevy 0.19 の production registry が
`reflect_auto_register` で登録する `external_registered_component` として別記する。schema はこれを
重複登録せず、通常の `App` の registry に `ReflectComponent` data があることをテストで固定する。

`ParticipatingIn` / `GatheringParticipants` は `runtime_derived_exclusion` inventoryに置く。型登録は
legacy bodyのdeserializeのため維持するが、新規saveのallow-listには含めない。deserialize後かつschema
検証前にこれらを除去するため、旧bodyが持つ消滅済み`GatheringSpot`へのEntity参照をlive worldへ渡さない。

`ReservedForTask` も同じ TypePath を保つ loader 専用の Reflect component として別登録する。ただし
`persisted_component` には入れない。legacy v0 のみ deserialize 後に除去し、v1 body に同型があれば
schema validation で reject する。この区別により、旧ファイルは読み直せる一方で新形式へ marker が
再流入しない。

新しい spawn 時 component を追加する際は次の順序で判断する。

1. durable simulation state なら `schema.rs` の `persisted_component` または `persisted_resource` にだけ追加する。値型だけなら `reflect_dependency`、entity の保存起点なら `root_marker` も追加する。
2. Bevy/外部crateが登録責務を持つ allowed component は `external_registered_component` に明記し、registry data test を追加する。登録を保証できない型は schema-owned type として登録する。
3. runtime shell、cache、visual/UI hierarchy、obstacle provenance は schema に入れず、rehydrate または cache rebuild の契約へ追加する。

schema の回帰テストは空の `AppTypeRegistry` で schema-owned type の `ReflectComponent` /
`ReflectResource` data を検査する。production `App` では external component を検査し、27種類の
root marker matrix は collect、extract、RON serialize/deserialize、Relationship の Entity remap まで確認する。

ロード時も同じschemaを検証する。bodyに含まれるresourceとcomponentはallow-list由来の型だけを
受理し、全`persisted_resource`の存在と、各DynamicWorld entityに少なくとも1つの`root_marker`を
要求する。これは`WorldDeserializer`がregistry登録済みの別型を読めるためで、保存側のallow-list
だけではlive applyを保護できない。root markerなしのallowed componentだけを含むentityも拒否し、
次回loadで収集・despawnできない孤立entityを作らない。

## Load transaction

`PreparedLoad` はheader分類済みの`SaveFormat`とdeserialize済み`DynamicWorld`を保持する。live worldを
変更する前に、次を順に完了しなければならない。

1. header/seed、RON deserialize、legacy v0 の runtime-derived component / shim の除去、schema allow-list、必須Resourceを検証する。
2. 空のstaging `World`へ`write_to_world_with`を実行し、registry、`ReflectComponent`、`ReflectResource`、Entity remapの静的契約を検証する。
3. `AssetServer`、`GeneratedWorldLayoutResource`、`GameAssets`、`Building3dHandles`、`SoulTaskHandles`、`WorldMap`と、Tree再水和に必要な非空の`GameAssets.trees`を検証する。

staging preflightの成功はtransaction成功を保証しない。live applyは別境界であり、write開始後に
`Result`エラーが返った場合は、apply時の`EntityHashMap`に記録された全entityを直接despawnして
partial entityを除去する。finalizeが途中まで生成したrehydrate所有presentation shellもこの時点で
掃除してから、despawn前に取得した同じschemaのrollback snapshotを適用し、success時と同じfinalize
経路を通す。

rollback成功時の保証は「persistent graphを復元し、非保存runtime stateを通常loadと同じ初期化済み
状態へ正規化する」ことである。raw Entity IDやRON byte列の一致は保証しない。`AssignedTask`、
orphan inventoryのdrop、Move designation除去、obstacle cache再構築、presentation shellはfinalizeで
意図的に正規化される。reflect applyのpanicはtransactionの回復対象ではない。

## フレーム境界と reset ownership

`SaveLoadApplySet`はproject-ownedな`Last` phaseの末尾である。`SettingsPersistenceSet`の後に
順序付けられ、Input/Interfaceを含む全`Update` producerが`SaveLoadState`へ書き込んだ後に、
exclusive dispatcherが一度だけ要求を消費する。新しいproject-owned `Last` systemを追加する場合は、
このsetとの前後関係を明示する。

`LoadResetRegistry`はrootが所有するcallback一覧である。leaf crateはroot型をimportせず、
自crate状態だけを消去する`reset_for_world_replace(&mut World)`を公開し、root plugin facadeが登録する。
root message型は`MessagesPlugin`の単一typed macroから初期化と`Messages<T>::clear()`の両方を生成する。
`TerrainChangedEvent`のようにroot facadeが登録するleaf messageも、同じreplace phaseでclearする。

| 所有者 | 旧Entityを持つ状態 | replace時の方針 |
| --- | --- | --- |
| root interaction | selection、hover、move placement、build/zone/task/companion context、pending PlayMode | default化し、`PlayMode::Normal`を予約 |
| `hw_ui` | rename、inspection/pin、drag、entity list model/index、area edit history、text pending、`UiIntent` / `TextInputIntent` | hookでclear。static UI node registry、サイズ、theme、searchは保持 |
| root task UI | task list snapshot | default化してdirty化し、次frameで再構築 |
| `hw_visual` | owner cache、3D proxy、speech/dream/haul/task-area等の独立transient entity、`GatheringSpot`とlinked aura/object | hookでdespawn + cache clear。root固有のFamiliar range shellもrehydrate cleanupでdespawn |
| root command visual | designation / task-area indicator、area-edit handle、area / dream preview | root VisualPlugin hookでdespawn。`DesignationIndicator`は通常の`RemovedComponents<Designation>` cleanupを使えないためreplace前に明示破棄 |
| simulation cache | spatial/resource/tile/room/reservation/stockpile group/obstacle index | root cache hookでdefault化し、既存systemまたはrehydrateで再構築 |
| `Local<HashMap<Entity, _>>` | Soul移動のdoor wait、world tooltip runtime | `WorldEpoch`不一致を最初の利用前に検出してclear |
| scratch `Local<Vec<Entity>>` / frame map | nearby検索buffer、idleのpending rest reservation | 使用前またはsystem先頭で必ずclearするためretain |

`WorldEpoch`はold persisted entityをdespawnして`flush()`した後、new `DynamicWorld`を書き込む前に一度だけ進める。
`RemovedComponents`は通常のmessage registryと別の二重bufferであるため、同じ位置で
`World::clear_trackers()`を**2回**呼ぶ。1回目はbufferをswapするだけでold removalが残り得る。
new worldを書いた後には手動でclearしない。これによりloaded componentの`Added`/`Changed`は次frameの
rebuild systemが観測できる。

## Rehydrate（ロード後の shell 再付与）

セーブが復元するのはschema allow-listの simulation 状態のみで、spawn 関数がその場で挿入する
実行時コンポーネントと随伴エンティティは含まれない。`rehydrate.rs` がロード直後に再付与する。

物理構成では `schema.rs` に型inventoryと抽出入口を残し、`schema/validation.rs` がdeserialized worldの検証を担当する。`rehydrate.rs` は復元順とshell再付与の入口を保ち、`rehydrate/prerequisites.rs`、`presentation.rs`、`construction_runtime.rs`、`construction_shells.rs`、`obstacles.rs` が各フェーズを担当する。save schemaと復元順の正本は引き続きfacade側の入口である。

| カテゴリ | shell の内容 | 実装 |
| --- | --- | --- |
| Soul | `Destination`/`Path`/`AnimationState`/UI リンク/speech 状態 + GLB 3D プロキシ×3 | `attach_soul_shell`（spawn と共用） |
| Familiar | `FamiliarAiState`/`FamiliarOperation`/`ActiveCommand`/Sprite + 3D プロキシ + 指揮範囲インジケーター×3 | `attach_familiar_shell`（同上） |
| Building（SoulSpa 含む） | `Name`/バウンス演出 + VisualLayer 子 Sprite + 独立 3D ビジュアル | `attach_building_shell`（同上） |
| Blueprint | `Name`、`Sprite`、`BlueprintVisualState`、`BlueprintVisual` | durable `Blueprint` から mirror と搬入履歴を完成形で生成してから付与。資材アイコン・進捗バーはこの mirror を入力に Visual phase で再生成し、保存済み搬入を新規演出として再生しない |
| Floor / wall construction | site / tile の `Name`、site の visual state、tile の visual mirror と Sprite | durable な site / tile state から直接生成。Logic 停止中でも床・壁タイルと進捗表示を復元 |
| Tree / Rock / ResourceItem / Stockpile | Sprite（spawn 箇所と同じ画像・サイズ） | rehydrate 内で直接挿入 |
| 障害物 provenance / pathfinding cache | source-aware marker、Building footprint mirror、`ObstaclePositionIndex`、raw obstacle / Door / Bridge cache | `rehydrate_obstacle_runtime` が Tree/Rock、construction、Building/Blueprint/site の semantic source matrix から再構築 |

shell 欠落の判定は「shell が必ず挿入するコンポーネントの不在」
（Soul/Familiar は `Without<Destination>`、Building は `Without<BuildingBounceEffect>`、Blueprint は mirror / Sprite /
`BlueprintVisual` / `Name`、construction tile は mirror / Sprite / `Name`、construction site は mirror / `Name` を個別に検査する）。
既に存在する shell は再作成しない。

Blueprint と construction の mirror を `default()` で付与して次 frame の Logic 同期へ委ねてはならない。
load は virtual time が pause 中でも成立し、Visual phase は停止しないため、mirror は durable state から完成形として
構築して Visual phase が最初に読む値を正しくする。

Floor / Wall construction は shell の後、Spatial/Logicを再開する前に durable tile から `TileSiteIndex` を同期的に再構築する。同じpassでtile state rankからsite counterを再計算し、Curing siteだけに保存対象外の `CuringFootprint` を作り直す。この処理は保存済み `WorldMap` を正本とし、養生footprintを再reserveしない。

rehydrateは先に前提Resourceを検証して`Result`を返す。前提不備ではinventoryやentityを変更しない。
replace phaseではregistryが全pluginのtransient stateを先にclearし、さらにrehydrate所有の独立
presentation entity（Soul/Familiar proxy、Building 3D visual、Familiar range indicator）と
`SoulProxyOwnerCache`を狭く掃除する。rollback branchでも同じreset phaseを再実行するため、partial
finalizerが残したowner shellはrollback snapshotのrehydrate前に残らない。

付随処理:

- **孤児インベントリのドロップ**: Phase A ではロード後の全 Soul が `AssignedTask::None` になるため、`Inventory(Some)` のアイテムは Soul の足元へドロップして物流ループに戻す
- **猫車積載アイテム**: `LoadedIn` 付きアイテムは `Visibility::Hidden` で復元
- **旧形式セーブ**: `SoulIdentity` が無い場合はランダム生成でフォールバック（名前は失われる）

**新しい spawn 時コンポーネントを追加する時の規約**: 永続化すべき simulation 状態なら
`schema.rs` の該当分類へ、通常の実行時状態なら該当する `attach_*_shell` へ、source-aware
obstacle provenance / navigation cache なら `rehydrate_obstacle_runtime` の durable source matrix へ追加する。
この3経路のどれにも入れないと、ロード後にだけ欠落するサイレントバグになる。
Blueprint / floor / wall construction の visual mirror は durable source から即時に作る rehydrate helper へ追加し、
Logic の変更検知だけに依存しない。

**⚠️ タプルキーのマップは reflect デシリアライズ不可（bevy_reflect 0.19 の制約）**:
`HashMap<(i32,i32), _>` / `HashSet<(i32,i32)>` を含む型を保存対象にすると、ロード時に
`DynamicMap::insert_boxed` がタプルキーの `reflect_hash`（未実装）を要求して panic する。
enum キー（`ResourceType` 等）は `enum_hash` があるため問題ない。対処は `WorldMap` と同じく
**serde derive + `#[reflect(Serialize, Deserialize)]`** で型全体を serde 経路にすること
（`crates/hw_world/src/map/mod.rs` の doc コメント参照）。

## Worldgen seed ガード

地形チャンク等のビジュアルは起動時に `GeneratedWorldLayoutResource` の seed から生成され、
セーブ body には含まれない。v1 は external header の `worldgen_seed` を現在の session と
**body deserialize 前に照合**する。不一致ならロードを中止し、
`HELL_WORKERS_WORLDGEN_SEED=<saved>` で起動し直すようエラーログで案内する。
magic 無し v0 だけは、deserialize 後に body 内 `SavedWorldgenSeed` を読む互換経路を使う。

## Phase A（タスク正規化）

計画書の Phase A は「セーブ前に `unassign_task` で正規化」を想定していたが、本実装では **allow-list から除外** する方式を採用している。

- セーブ中もライブワールドのタスク実行状態は変更しない
- ロード後は `AssignedTask` が無い Soul に `None` を挿入
- `Designation` + `TransportRequest` が残っていれば Familiar AI が再割当する

Phase B（実行中タスクの完全復元）は follow-up。

## Relationship と reconcile

計画書は `RelationshipHookMode::Skip` 前提の reconcile pass を想定していたが、本実装では **Relationship Target 型も allow-list に含めて保存** する。保存時点で Source/Target が整合したスナップショットとして書き出されるため、追加の reconcile pass は不要。

## Reflect 登録

`SavePlugin` は `schema::register_save_types` を呼ぶ。schema-owned type の `register_type` と allow-list は
同じ X-macro から出力され、`Transform` のような external registration は production registry contract test
で検査する。

## 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace 2>&1 | grep "^warning:" | grep -v generated
cargo test -p bevy_app@0.1.0 --lib systems::save::schema
cargo test -p bevy_app@0.1.0 --lib systems::save
cargo test -p hw_core --lib world_epoch
cargo test -p hw_ui --lib world_replace_reset
```

手動: プレイ → F5 → 再起動 → F9 → 確認ダイアログで Confirm。確認前にはロードされず、Confirm後にSoul 数・Stockpile 内容・建築進捗・`GameTime` が復元されること。save fileを退避した状態のF9がno-opであることも確認する。

## 未実装

- 複数スロット・オートセーブ・バージョンマイグレーション
- 設定画面からのセーブ/ロード（settings-screen-plan 側）

## 既知の制限

- **別 seed セッションへのロード不可**: seed ガードで中止される（`HELL_WORKERS_WORLDGEN_SEED` 指定で再起動すれば可）。地形チャンクをロード時に再生成できれば解消するが未対応
- **runtime obstacle source / footprint mirror**（`ObstacleSourceKind` と `BuildingFootprint` / `PlacementReservation` / `ConstructionProtection` の非保存 marker）は保存しない。load 時は raw `WorldMap.obstacles` を正本にせず、Tree/Rock、movement-blocking な完成 Building、non-Bridge Blueprint、`WallConstructionSite`、Curing 中 FloorTile の durable semantic source から bitmap を再構築する。Door cache は保存済み state を最終 override とし、`bridged_tiles` は完成 `BuildingType::Bridge` から再構築する。raw blocker / Door / Bridge cache は一括更新して、最終 walkability が変わる場合だけ `obstacle_version` を1回進める。Building mirror と `ObstaclePositionIndex` を再生成し、未完了 Move designation とその予約 bit は復元しない
- **SoulSpaTile の `ChildOf` 階層**は復元されない（`parent_site` フィールドで論理は維持。Transform は絶対値保存のため表示影響なし）
- **header 無し v0 セーブ**: `SoulIdentity` を含まなければ Soul 名はランダム再生成される。`SavedWorldgenSeed` も無い場合は seed 照合を warn のみにする

## UI 構成（M4）

- **Pause メニュー**（`hw_ui/src/setup/pause_menu.rs`）: `Time<Virtual>` 一時停止中に overlay 中央へ表示。`MenuButton` → `UiIntent::SaveGame` / `RequestLoadGame`
- **ロード確認ダイアログ**（`hw_ui/src/setup/dialogs.rs`）: 単一スロットの上書き不可を前提に「現在の進行を破棄」警告。`ConfirmLoadGame` / `CancelLoadConfirm`
- **Intent 処理**（`bevy_app/.../handlers/save_game.rs`）: `SaveLoadState` へ橋渡し

既存 UI と同様 **`MenuButton` + `UiIntent` パターン** を採用（plan の `bsn!` は本リポジトリ未使用のため見送り）。
