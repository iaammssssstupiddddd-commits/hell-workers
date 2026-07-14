# 地獄のインフラ（換気・導水・部屋認可）実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `hvac-plumbing-plan-2026-07-13` |
| ステータス | `Draft` |
| 作成日 | `2026-07-13` |
| 最終更新日 | `2026-07-13` |
| 作成者 | `Codex` |
| 関連提案 | [hvac-plumbing-proposal.md](../proposals/hvac-plumbing-proposal.md)（採用済み。世界観・採否理由の正本） |
| 関連Issue/PR | `N/A` |
| 前提ドキュメント | [room_detection.md](../room_detection.md) / [soul_energy.md](../soul_energy.md) / [building.md](../building.md) / [save_load.md](../save_load.md) / [crate-boundaries.md](../crate-boundaries.md) |

> 本計画を実装契約の正本とする。関連提案と矛盾する場合は、2026-07-13 時点のコードを再確認して作成した本計画を優先する。

## 1. 目的

### 解決したい課題

- 検出済み `Room` が現在は境界線表示にしか使われず、部屋を成立させるゲーム上の目的が弱い。
- Soul Energy の恒常的な消費先が少なく、中盤以降の電力配分判断が限定的である。
- 水は Tank / MudMixer 間の手動搬送に閉じており、本設区画へ恒久インフラを伸ばす遊びがない。
- 採用済み提案には、現行の Room 占有判定、壁タイル逆引き、単一 `WorldMap` 建物層、Room の再スポーン契約と両立しない実装案が残っている。

### 到達したい状態

- Room ごとに換気量を集計し、`Ventilated / Stagnant / MiasmaInfested` を決定できる。
- 非電化換気から電動換気へ段階的に発展し、`ScourgeFan` と `LetheIntakeGate` が既存 Yard 単位の電力網を消費する。
- 建物占有とは別のインフラ層へ導水管を敷設し、取水門から浄化盤までの 4 近傍接続を管理できる。
- 換気と排泥を満たした Room だけが `Sanctioned` となり、本設設備の稼働条件へ反映される。
- MudMixer / Tank / SoulSpa とバケツ搬送は変更せず、停止時にも復旧経路を残す。

### 成功指標

- 壁の建て直しで Room entity が全再生成されても、次の再計算で同じ換気・認可結果へ収束する。
- 3×3 の Room は `ScreamingVent` 1 基で `Ventilated` になる。
- 電動換気だけの Room は停電で `MiasmaInfested` に戻り、通電復旧で自動回復する。
- Intake → Conduit → Purifier の途中を 1 タイル撤去すると対象 Room が `Unwatered` になり、再接続で `Drained` に戻る。
- Room 内の Tank / MudMixer / SoulSpa はインフラ状態にかかわらず従来どおり稼働する。
- 換気・流体トポロジーを毎フレーム全再構築しない。

## 2. スコープ

### 対象（In Scope）

- `crates/hw_infra` の新設と、換気・流体・認可状態のデータモデルおよび純粋計算。
- Room 検出入力の「床を無効化する上物」分類と、壁境界から Room を逆引きする lookup。
- `ScreamingVent`、`SighChimney`、`ScourgeFan`、`LetheIntakeGate`、`OssuaryConduit`、`SludgePurifier`。
- `BuildingCategory::Permanent` と本設設備のインフラ停止契約。
- 壁設備の置換、電力接続、導水管のドラッグ配置・erase 撤去、インフラオーバーレイ。
- 物理設備の Save/Load、派生 lookup / grid / Room 状態のロード後再構築。
- Room / 設備 info panel、配置失敗理由、認可失敗理由。
- 実装に追従する恒久ドキュメントと索引の更新。

### 非対象（Out of Scope）

- Soul の快適度・健康・ストレス・疲労への影響。
- 温度、気圧、流量、圧力、汚泥量の連続シミュレーション。
- 新規 ResourceType。既存の Wood / Rock / Bone を使う。
- Tank / MudMixer / SoulSpa への配管強制と既存バケツ搬送の置換。
- `ConfessionValve`、`SludgeBasin`、`MiasmaGauge`。M4 の評価後に別計画へ切り出す。
- 侵食猶予タイマー、配管容量、複数系統の優先度制御。
- Soul Energy の将来構想である Room 単位 PowerGrid への変更。
- 完成設備の一般 demolition タスク。導水管はインフラ編集モードの erase 操作で撤去する。

## 3. 現状とギャップ

| 現行契約 | 実装上のギャップ | 本計画の対応 |
| --- | --- | --- |
| `detect_rooms_system` は Room を全 despawn して再生成する | Room を target にした永続 Relationship と Room 上の猶予 Timer は消える | Room 状態は派生 Component とし、永続 Relationship / Timer を持たせず即時再計算する |
| `RoomTileLookup` は室内床だけを引ける | 壁埋め込み Vent / Chimney の所属 Room を引けない | `RoomBoundaryLookup: GridPos -> Vec<Entity>` を `hw_world` に追加する |
| Floor と同じタイルに `WorldMap` 上の建物があると Floor が検出入力から外れる | Fan / Purifier 等を室内へ置くと Room が消える | `has_building_on_top` を room-detection role 判定へ置換し、室内設備は Floor を維持する |
| Room 境界として Wall / Door だけを扱う | 壁を Vent / Chimney に置換すると密閉が壊れる | Vent / Chimney を Wall と同じ境界 role にする |
| `WorldMap.buildings` は 1 grid 1 entity | Conduit を床・壁・設備と同じタイルへ置けない | Conduit を通常 `Building` 占有から外し、専用 lookup と visual layer を持つ |
| 電力型は `hw_energy`、集計・lifecycle adapter は root `bevy_app` にある | `hw_energy::grid_recalc_system` を直接模倣する前提が古い | 型と純粋ロジックは leaf、cross-domain ordering は root facade に置く |
| Room overlay は `Added/Changed<Room>` で境界線を再生成する | 換気状態だけが変化しても色が変わらない | geometry sync 後に別の状態色 sync を実行する |
| `BuildingType` / `BuildingCategory` は exhaustive match が多い | variant 追加の追従漏れが compile error または placeholder 表示になる | M1〜M3 各回で全 match、visual mirror、placement、task list、rehydrate を棚卸しする |

## 4. 実装方針

### 4.1 採用するゲームルール

#### 換気

```text
required = Room.tile_count * 0.05
provided = Room に属する稼働中 VentilationSource の capacity 合計
```

| `VentilationState` | 条件 | 効果 |
| --- | --- | --- |
| `Ventilated` | `provided >= required` | 換気条件を満たす |
| `Stagnant` | `0 < provided < required` | 警告のみ。本設設備は停止させない |
| `MiasmaInfested` | `provided == 0` | 本設設備の換気条件を満たさない |

- 初期実装は即時判定とする。猶予 Timer は持たせない。
- `ScreamingVent` は接する全 Room へ能力を配る。共有壁で 2 Room に接する場合、能力を Room 数で均等分割する。
- `SighChimney` は「隣接 Room が 1 つだけ」の外周壁にのみ置け、全能力をその Room へ与える。
- `ScourgeFan` は設置床を `RoomTileLookup` で逆引きし、`Unpowered` がない場合だけ能力を与える。
- PowerGrid entity は従来どおり Yard が所有するが、Consumer のサービス範囲をその Yard と paired Site の union へ広げる。Room 単位 grid は作らない。
- 川岸の Intake は通常の位置包含では grid を引けないため、配置時に「取水門 footprint と水平に重なり、規定距離内にある Site」を 1 件に解決し、その `PairedYard` の PowerGrid へ `ConsumesFrom` を明示接続する。0件または複数候補は配置拒否する。

#### 排泥と認可

- `LetheIntakeGate` と `SludgePurifier` が同じ `FluidGrid` へ接続し、取水門が通電中なら Purifier は `Drained` とする。
- Purifier は設置床を `RoomTileLookup` で逆引きし、1 基でその Room 全体の排泥要件を満たす。
- `RoomSanctionState::Sanctioned` は `Ventilated && Drained` のときだけ成立する。それ以外は `Enclosed` とし、理由を換気・排泥の別軸で保持する。
- `Stagnant + Drained` は `Enclosed` のままだが警告のみとし、本設設備は停止させない。認可状態と停止条件を同一 bool にしない。
- `BuildingCategory::Permanent` の完成設備だけをインフラ要件の対象とする。既存 4 category と既存設備には適用しない。
- `SludgePurifier` を最初の `Permanent` 設備とする。`MiasmaInfested` または `Unwatered` の場合だけ派生 `InfrastructureDisabled` を付け、復旧時に除去する。単なる `Enclosed` / `Stagnant` は停止理由にしない。

#### 初期バランス

| 設備 | category | 初期能力 / 需要 | 初期資材 |
| --- | --- | --- | --- |
| `ScreamingVent` | `Architecture` | 換気 1.0 / 非電化 | Rock 2 + Bone 1 |
| `SighChimney` | `Architecture` | 換気 2.0 / 非電化 | Wood 2 + Rock 2 |
| `ScourgeFan` | `Architecture` | 換気 5.0 / 0.5W | Wood 2 + Bone 6 |
| `LetheIntakeGate` | `Plant` | 供給 source / 1.0W | Wood 6 + Rock 6 |
| `OssuaryConduit` | `Permanent`（配置メニュー分類のみ） | 接続 / Bone 1 per tile | Bone 1 |
| `SludgePurifier` | `Permanent` | Room 1 室を排泥 | Rock 2 + Bone 2 |

数値は着手ブロッカーにせず初期定数として採用する。M2 / M3 の手動プレイで変更した場合は、理由と結果を計画の更新履歴へ残す。

### 4.2 データ所有と依存方向

`hw_infra` の依存方向は次に固定する。

```text
bevy_app (scheduling / assets / game-aware UI adapter)
  -> hw_infra
       -> hw_world  (Room / lookup / WorldMap座標)
       -> hw_jobs   (Building / BuildingType / category)
       -> hw_energy (PowerConsumer / Unpowered)
       -> hw_core   (GridPos / shared contracts)
```

- `hw_world`、`hw_jobs`、`hw_energy` から `hw_infra` へ逆依存させない。
- `hw_infra` は換気・流体・認可の Component / Resource / Relationship / pure function / ECS system 実装を所有する。
- Room 検出 pipeline と電力集計との厳密な順序だけは、`bevy_app::plugins::logic` の ordering facade から leaf system を一度だけ登録する。
- Room overlay geometry は `hw_world` のまま保ち、インフラ状態による色反映は `hw_infra` 実装を root `VisualPlugin` が geometry sync 後に登録する。これにより crate cycle を作らない。
- game entity を読む info-panel ViewModel は root に置き、`hw_ui` は表示 widget と intent 発行だけを担当する。

### 4.3 主な型

| 型 | 所有 | 保存 | 用途 |
| --- | --- | --- | --- |
| `VentilationSource { capacity }` | `hw_infra` | 完成設備側の再構成で付与 | 換気供給能力 |
| `RoomVentilationState { required, provided, state }` | `hw_infra` | しない | 揮発 Room 上の派生状態 |
| `RoomDrainageState { state, active_purifiers }` | `hw_infra` | しない | Purifier 不在を含む Room 単位の `Unwatered / Drained` |
| `RoomSanctionState` | `hw_infra` | しない | `Enclosed / Sanctioned` |
| `RoomFootprintKey` | `hw_infra` | しない | sorted Room tiles から作る再検出をまたぐ安定キー |
| `RoomSanctionHistory` | `hw_infra` | しない | footprint ごとの前回認可状態。stamp の重複抑止 |
| `RoomBoundaryLookup` | `hw_world` | しない | 壁 grid から隣接 Room 群を逆引き |
| `OssuaryConduit` | `hw_infra` | する | 配管 1 tile の物理 entity |
| `ConduitTileLookup` | `hw_infra` | しない | duplicate 防止・近傍探索 |
| `ConduitBlueprintLookup` | `hw_infra` | しない | 未完成 Conduit の duplicate 防止。WorldMap 予約の代替 |
| `FluidGrid` | `hw_infra` | しない | 4 近傍連結成分の派生 grid entity |
| `SuppliesTo / FluidSuppliers` | `hw_infra` | しない | Intake と FluidGrid の Relationship |
| `DrainsFrom / FluidConsumers` | `hw_infra` | しない | Purifier と FluidGrid の Relationship |
| `Unwatered` | `hw_infra` | しない | 給水不能な Purifier の派生マーカー |
| `InfrastructureDisabled` | `hw_infra` | しない | 本設設備の停止理由を保持する派生状態 |
| `InfrastructureTopologyDirty` | `hw_infra` | しない | 配管追加・削除時だけ topology rebuild |
| `InfrastructureSupplyDirty` | `hw_infra` | しない | Intake の停電等で到達性だけ再評価 |

- tuple key を持つ lookup Resource は save 対象にしない。
- Room を target にする換気 Relationship は作らない。
- `FluidGrid` entity はロード後に Conduit から再生成し、save entity root に含めない。
- `Unwatered` は FluidGrid と通電中 Intake の到達性だけから決める。`InfrastructureDisabled` を FluidGrid / drainage の入力に戻して循環依存を作らない。
- `RoomDrainageState::Unwatered` は「Room 内に Purifier がない」「Purifier はあるが active Intake へ到達できない」の両方を表す。info panel は内訳 reason を別に表示する。
- `InfrastructureDisabled` は `MiasmaInfested` と断水理由を独立に合成する。Purifier が同じ Room の排泥要件を提供しても、network reachability → Room drainage → sanction / disabled の一方向を維持する。`Stagnant` と単なる非認可は停止理由にしない。
- 認可 stamp は `Added<RoomSanctionState>` ではなく、`RoomSanctionHistory` 上の `Enclosed -> Sanctioned` 遷移だけで発火する。Room 再検出では同じ footprint の履歴を再利用し、load rehydrate の初回再構築は履歴を seed して演出を抑止する。

### 4.4 配置・建設レイヤー

- Vent / Chimney は Door の壁置換処理を「壁設備置換」へ一般化して建設する。完成後も Room 境界として扱う。
- Fan / Purifier 等の室内設備は Floor を Room 検出から除外しない。
- Fan / Purifier は pathfinding 上の障害物になっても room-detection role は `InteriorFixture` とし、下の完成 Floor を Room tile として維持する。
- Fan は `Architecture` に分類するが、通常の Architecture 共通制約には頼らず「完成 Floor 上・Room 内・Site 内」を専用 validator で要求する。
- `Permanent` の通常設備は paired Site / Yard の union 内を許可する。Purifier は追加で Room 内、Conduit は専用 line validator、Intake は下記の川岸 validator を使う。
- Intake は 3×2 footprint とし、少なくとも指定辺が River、設備本体は建設可能な岸側であることを pure placement validator で判定する。さらに、対応 Site が一意に解決できる場合だけ、その `PairedYard` の PowerGrid へ接続する。
- Conduit blueprint は既存 `Blueprint` / `DeliverToBlueprint` / `Build` を再利用し、`BuildingType::OssuaryConduit` と専用 marker を持たせる。
- Conduit の配置は通常 building validator / `reserve_building_footprint` を通さず、`ConduitTileLookup` と `ConduitBlueprintLookup` だけで重複を判定・予約する。Blueprint entity 自体は既存 `BlueprintSpatialGrid` へ同期して task discovery を再利用する。
- Conduit 完成は generic Building spawn / WorldMap release より前に専用分岐し、同じ grid に `OssuaryConduit` entity を生成して blueprint lookup を解除する。`WorldMap.buildings`、obstacle map、通常 building footprint の予約・解放は一度も行わない。
- Conduit blueprint / 完成 entity は床・壁・通常建物と同じ grid を許可するが、同じ grid の Conduit / Conduit blueprint 重複は拒否する。
- cancel / erase / load reset は dedicated blueprint lookup を解除・再構築し、generic building reservation cleanup を呼ばない。
- 非 walkable な壁下 Conduit の建設は、対象中心ではなく隣接到達点を使う既存 task execution 契約へ合わせる。
- インフラ編集モードは line drag で blueprint を生成し、erase drag で Conduit または未完成 blueprint を即時撤去する。一般 demolition task は追加しない。
- 通常表示で壁下の配管が隠れても、インフラ overlay では必ず経路を確認できる visual layer を用意する。

### 4.5 システム順序

Logic の必須順序は次とする。

```text
PowerGrid recalc
  -> Room validate / detect
  -> RoomBoundaryLookup rebuild
  -> ventilation recalc
  -> FluidGrid topology rebuild (topology dirty 時のみ)
  -> fluid supply recalc (supply dirty 時のみ)
  -> sanction / InfrastructureDisabled sync
```

Visual は次の順序とする。

```text
Room overlay geometry sync
  -> Room infrastructure color sync
  -> infrastructure overlay sync
```

- `Added/Changed<Room>` だけに依存せず、換気・認可 state の変更でも色を同期する。
- 通常 frame に全 Conduit の連結成分再構築を置かない。
- `Unpowered` の Added/Removed は topology を変えず、supply dirty だけを立てる。

### 4.6 Bevy 0.19 と現行実装上の注意

- Observer は leaf で完結する場合 `hw_infra` plugin に一元登録し、root との二重登録を禁止する。
- `Commands` で生成された Room と同 frame で recalc する必要がある箇所は、Bevy 0.19 の deferred command 適用点を一次ソースまたは回帰 test で確認してから順序を固定する。
- 既存 `PowerConsumer + #[require(Unpowered)] + on_power_consumer_added` を再利用する。M2 で consumer lookup を Yard 内だけから paired Site まで拡張し、M3 の Intake は配置時に対応 Yard grid を明示する。どちらでも grid を解決できない場合は `No Power Grid` を表示する。
- `BuildingType` 追加時は `rg -n "BuildingType::|BuildingCategory::" crates -g '*.rs'` で exhaustive match を全件確認する。
- `crates/bevy_app/src/assets.rs` と `plugins/startup/asset_catalog.rs` は本計画作成時点で別作業の差分がある。実装時は current diff を読み、無関係な変更を上書きしない。

## 5. 期待する性能影響

- 換気再計算は Room / VentilationSource / power state の dirty 時だけ行い、lookup 構築後は Room 数と source 数に比例する処理へ限定する。
- FluidGrid topology rebuild は Conduit の追加・削除時だけ実行し、4 近傍 flood fill の `O(conduit_count)` とする。
- 停電・復電は topology を再構築せず、既存 FluidGrid の supplier / consumer 到達性だけを再評価する。
- Room / FluidGrid / lookup は派生データのため save payload を増やさない。増加する保存量は物理 Conduit と新設備 entity のみ。
- M3 で 1,000 tile の配管を生成・分断・再接続する計測シナリオを実行し、rebuild が毎フレーム発生していないことと処理時間を記録する。絶対 budget は実測 baseline を取得してから恒久 docs に定める。

## 6. マイルストーン

## M0: Room 検出契約の固定

### 変更内容

1. Room detection 入力を `has_building_on_top: bool` から「境界 / 室内設備 / 床を無効化する上物」の明示 role へ変更する。
2. Floor 上の既存 Plant / Temporary と、synthetic な室内 fixture role が Room を壊さない回帰 test を追加する。
3. Wall / Door と同じ generic boundary fixture role を pure detection test で固定する。Vent / Chimney variant の割り当ては M1 / M2 で行う。
4. `RoomBoundaryLookup` を Room 検出・validation と同時に再構築する。
5. Room detection の現行 ownership と stale path を恒久 docs へ反映する。

### 主な変更ファイル

- `crates/hw_world/src/room_detection/{core.rs,ecs.rs,tests.rs}`
- `crates/hw_world/src/room_systems.rs`
- `crates/hw_jobs/src/model.rs`
- `docs/{room_detection.md,cargo_workspace.md,crate-boundaries.md,architecture.md}`
- `docs/README.md`

### 完了条件

- [ ] Floor 上の室内設備が Room tile を消さない
- [ ] generic boundary fixture role が完成壁と同じ境界になる
- [ ] 共有壁は `RoomBoundaryLookup` から 2 Room を返せる
- [ ] Room 再検出後に lookup が stale entity を保持しない
- [ ] room detection の code / docs path と ownership が一致する

### 検証

- `cargo test -p hw_world room_detection`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## M1: 基本換気 vertical slice

### 変更内容

1. `crates/hw_infra` を新設し、M1 で使用する `RoomVentilationState`、換気 pure calculation、dirty 条件、plugin / system registration 境界を実装する。
2. `BuildingType::ScreamingVent` を追加し、壁置換、資材配送、建設完了、Room 境界維持を接続する。
3. 共有壁では capacity を隣接 Room 数で均等分割する。
4. Room 境界線を換気状態別に色分けし、Room info panel に required / provided / state を表示する。
5. `BuildingType` / visual mirror / menu / geometry / placement ghost / completion shell / task list / hit test / rehydrate の match を同期する。
6. 2D sprite / UI icon を追加し、現行 3D-RtT 表示と section view で破綻しないことを確認する。

### 主な変更ファイル

- `crates/hw_infra/{Cargo.toml,README.md,src/lib.rs,src/components.rs,src/ventilation.rs,src/systems.rs,src/visual.rs}`
- `crates/bevy_app/Cargo.toml`
- `crates/hw_jobs/src/{model.rs,visual_sync/}`
- `crates/hw_core/src/visual_mirror/building.rs`
- `crates/hw_ui/src/{setup/submenus.rs,selection/placement/}`
- `crates/bevy_app/src/plugins/{logic.rs,visual.rs}`
- `crates/bevy_app/src/interface/selection/building_place/`
- `crates/bevy_app/src/interface/ui/presentation/`
- `crates/bevy_app/src/systems/jobs/building_completion/`
- `crates/bevy_app/src/systems/save/rehydrate.rs`
- `crates/bevy_app/src/{assets.rs,plugins/startup/asset_catalog.rs}`
- `assets/textures/buildings/infrastructure/`
- `docs/{building.md,room_detection.md,infrastructure.md}`

### 完了条件

- [ ] 3×3 Room + Vent 1 基が `Ventilated`
- [ ] Vent なし Room が即時 `MiasmaInfested`
- [ ] 共有壁の Vent 能力が二重計上されず均等分割される
- [ ] 壁置換中・完成後も Room が維持される
- [ ] Room 再生成後に同じ required / provided へ戻る
- [ ] state 変更だけで境界線色と info panel が更新される

### 検証

- `cargo test -p hw_infra ventilation`
- `cargo test -p hw_world room_detection`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 手動: 小部屋、共有壁、壁再建、通常表示 / section view

## M2: 高度換気と電力連携

### 変更内容

1. `SighChimney` と `ScourgeFan` を追加する。
2. Chimney は 1 Room のみに接する外周壁だけを許可し、共有壁では明示的に配置拒否する。
3. Fan は `Architecture` とし、Site の Room 内床にのみ置け、完成時に `PowerConsumer { demand: 0.5 }` を付与する。
4. `on_power_consumer_added` の grid 解決を Yard 内だけでなく、その Yard と paired な Site 内まで拡張する。PowerGrid の所有 entity は Yard のまま変えない。
5. `Unpowered` の Added/Removed で ventilation dirty を立て、paired Yard grid の通電状態を次の換気再計算へ反映する。
6. paired Site / Yard を解決できない Fan に placement warning と info-panel reason を表示する。
7. 初期能力・資材を手動プレイで確認し、変更する場合は定数と docs を同時更新する。

### 主な変更ファイル

- M1 の BuildingType / visual / placement / completion / asset 対象
- `crates/hw_infra/src/{components.rs,ventilation.rs,systems.rs}`
- `crates/bevy_app/src/systems/energy/`
- `crates/bevy_app/src/plugins/logic.rs`
- `docs/{building.md,soul_energy.md,infrastructure.md}`

### 完了条件

- [ ] Chimney が外周壁では稼働し、共有壁では配置拒否される
- [ ] Fan が powered のときだけ 5.0 を供給する
- [ ] blackout / recovery が Room 状態へ自動反映される
- [ ] paired Site 内の Fan が対応 Yard PowerGrid へ接続される
- [ ] pair を解決できない Fan は無言停止せず UI に理由が出る
- [ ] 既存 Lamp / SoulSpa の電力挙動が変わらない

### 検証

- `cargo test -p hw_infra ventilation`
- `cargo test -p bevy_app@0.1.0 energy`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 手動: 非電化復旧、Fan のみの Room、Lamp と Fan の需要競合

## M3: 導水配置層と FluidGrid

### 変更内容

1. `BuildingCategory::Permanent` と Intake / Conduit / Purifier の建設種別を追加し、Architect category、validation、geometry、visual mirror を同期する。
2. `OssuaryConduit`、`ConduitTileLookup`、`ConduitBlueprintLookup`、topology / supply dirty、FluidGrid Relationship を実装する。
3. Conduit line drag / erase drag、独立 overlay、重ね敷設、重複拒否を実装する。Blueprint 段階から WorldMap building footprint を予約しない。
4. Conduit completion を generic Building spawn / footprint release より前に分岐し、WorldMap occupancy / obstacle を変更しない。
5. Intake の川岸 3×2 placement と `PowerConsumer { demand: 1.0 }` を実装し、一意に解決した Site の `PairedYard` PowerGrid へ明示接続する。
6. Purifier の Room 内 placement、同一 FluidGrid 接続、Purifier 不在も含む `RoomDrainageState::Unwatered / Drained` を実装する。
7. topology dirty と supply dirty を分離し、Intake の停電で flood fill を再実行しない。
8. Conduit 物理 entity と新設備を save 対象へ追加し、FluidGrid / lookup / dirty / Room state はロード後に再構築する。

### 主な変更ファイル

- `crates/hw_infra/src/{conduit.rs,fluid.rs,relationships.rs,systems.rs,visual.rs}`
- `crates/hw_jobs/src/model.rs`
- `crates/hw_core/src/{game_state.rs,visual_mirror/}`
- `crates/hw_ui/src/{setup/submenus.rs,selection/placement/}`
- `crates/bevy_app/src/interface/{selection,ui/interaction,ui/presentation}/`
- `crates/bevy_app/src/systems/jobs/building_completion/`
- `crates/bevy_app/src/systems/save/{entities.rs,register.rs,saving.rs,load.rs,rehydrate.rs}`（実装開始時の現行 Save API に追従）
- `crates/bevy_app/src/plugins/{logic.rs,visual.rs}`
- `assets/textures/buildings/infrastructure/`
- `docs/{building.md,infrastructure.md,save_load.md,state.md,invariants.md}`

### 完了条件

- [ ] Conduit が床・壁・通常建物と同じ grid に存在できる
- [ ] Conduit 重複と blueprint 重複は拒否される
- [ ] Conduit blueprint の配置・cancel・完成・load の全経路で WorldMap building footprint を予約・解放しない
- [ ] Intake → Conduit → Purifier が同じ FluidGrid に接続される
- [ ] Intake が対応する Site / paired Yard を一意に解決できない場所では配置拒否される
- [ ] 途中 1 tile の erase で Purifier が `Unwatered` になる
- [ ] Intake blackout / recovery は topology entity 数を変えない
- [ ] load 後に Conduit lookup / FluidGrid が stale entity なしで再構築される
- [ ] 1,000 tile 配管の rebuild が dirty frame にだけ発生する

### 検証

- `cargo test -p hw_infra fluid`
- Save/Load round-trip test
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 手動: line drag、壁下表示、erase 分断、停電、save → load

## M4: Room 認可・本設停止・統合 UI

### 変更内容

1. `Ventilated && RoomDrainageState::Drained` から `RoomSanctionState` を決定する。
2. Room 内 `BuildingCategory::Permanent` へ `InfrastructureDisabled` を同期する。停止理由は `MiasmaInfested` / `Unwatered` だけとし、`Stagnant` / 非認可だけでは停止させない。
3. `SludgePurifier` を最初の対象設備として、miasma / unwatered の停止と復旧を接続する。
4. Room overlay に Enclosed / Sanctioned / Miasma の視覚差を追加し、`RoomFootprintKey` と sanction history による実遷移時だけ認可 stamp 演出を発行する。
5. Room / Purifier / Fan / Intake info panel に換気、排泥、電力、認可失敗理由を表示する。
6. 仮設設備免除、復旧経路、Room 再生成、Save/Load を end-to-end で確認する。
7. 恒久 docs とイベント / 接続マップ / crate 境界を最終同期する。

### 主な変更ファイル

- `crates/hw_infra/src/{components.rs,sanction.rs,systems.rs,visual.rs}`
- `crates/bevy_app/src/plugins/{logic.rs,visual.rs}`
- `crates/bevy_app/src/interface/ui/presentation/`
- `crates/bevy_app/src/systems/save/`
- `assets/textures/ui/`、`assets/textures/buildings/infrastructure/`
- `docs/{infrastructure.md,building.md,room_detection.md,soul_energy.md,save_load.md,events.md,architecture.md,cargo_workspace.md,crate-boundaries.md,invariants.md}`
- `docs/README.md`

### 完了条件

- [ ] 換気だけでは `Sanctioned` にならない
- [ ] Purifier 不在 Room が `RoomDrainageState::Unwatered` になり、info panel に `No Purifier` と表示される
- [ ] `Stagnant + Drained` は非認可だが本設設備を停止させない
- [ ] 換気 + 排泥で `Sanctioned` になり、同一 footprint の Room 再生成では stamp が再発火しない
- [ ] load 直後の派生状態再構築では stamp を発火せず、以後の実遷移では発火する
- [ ] Miasma / Unwatered の理由が区別される
- [ ] Purifier は条件復旧後に自動再開する
- [ ] Tank / MudMixer / SoulSpa は停止対象外
- [ ] Room entity 再生成と save → load 後も同じ認可結果へ収束する
- [ ] follow-up 3設備を実装せずとも本計画の DoD を満たせる

### 検証

- `cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- §8 の全手動確認シナリオ

## 7. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| Floor 上設備で Room が消える | Fan / Purifier を置くほど機能が壊れる | M0 で room-detection role を先に導入し回帰 test を固定する |
| Room entity の全再生成 | Relationship / Timer / cached Entity が stale になる | Room target Relationship と猶予 Timer を禁止し、lookup と状態を再構築する |
| 共有壁 Vent の能力二重計上 | 1基が複数 Room へ無制限に効く | capacity を隣接 Room 数で均等分割する pure test を置く |
| Conduit を Building として完成させる | WorldMap 占有・walkability・Room 判定を破壊する | completion を専用 entity spawn へ分岐し、WorldMap を変更しない |
| topology と給水状態を同じ dirty にする | blackout ごとに全配管 flood fill | topology dirty / supply dirty を分離する |
| Purifier の停止状態を drainage 判定へ戻す | 自分自身の断水判定が循環して復旧不能 | network reachability を先に独立計算し、disabled は派生結果としてだけ同期する |
| Conduit blueprint が通常 footprint を予約する | 完成前から床・壁と競合し、cleanup で他建物を消す | blueprint 段階から専用 lookup のみを使い、generic reserve / release を通さない |
| Room 再生成ごとに認可 stamp を出す | 壁編集や load で演出が連打される | canonical footprint history で実遷移だけを検出し、load seed 時は抑止する |
| 停電 → 換気停止 → 本設停止 | 復旧不能 | Vent / Chimney と既存仮設設備は対象外。手動水搬送も維持する |
| Site と Yard が別領域のため Fan が常時停電する | 電動換気が成立しない | Yard 所有 grid の consumer lookup を paired Site へ拡張する |
| 川岸 Intake が Yard / Site の外にある | 位置包含では grid へ接続できない | 配置時に対応 Site と `PairedYard` を一意解決し、`ConsumesFrom` を明示する |
| Save/Load hardening 計画との競合 | 古い allow-list 前提で二重修正 | M3 開始時に現行 Save API と active plan を再確認し、物理 / 派生分類だけを契約として維持する |
| `BuildingType` exhaustive match 漏れ | compile error、誤 sprite、選択不能 | variant 追加ごとに全参照を `rg` で棚卸しし、check / all-target clippy を通す |
| asset catalog の別作業差分と衝突 | ユーザー変更を上書きする | 実装開始時に status / diff を再確認し、対象 field だけ patch する |
| workspace-wide rustfmt baseline が既に不一致 | HVAC と無関係な大量 format 差分が混ざる | structural maintenance 計画と調整し、HVAC 実装 commit で全体 format を行わない |

## 8. 検証計画

### 自動検証

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- rust-analyzer diagnostics 0 件

pure test では少なくとも次を固定する。

- required / provided の境界値と 3状態。
- 共有壁 Vent の均等分割、Chimney の外周条件。
- Floor 上 fixture と wall replacement を含む Room 検出。
- Conduit の直線、分岐、loop、分断、再接続の連結成分。
- topology dirty と supply dirty の独立性。
- Room 再生成時に stale Room entity を保持しないこと。
- 同じ footprint の Room 再生成と load seed で sanction stamp を重複発火しないこと。
- save 対象が物理設備だけで、派生 cache を含まないこと。

### 手動確認シナリオ

1. 3×3 Room を作り、Vent 1基の有無で `Ventilated / MiasmaInfested` を切り替える。
2. 共有壁の Vent が両 Room へ 0.5 ずつ供給されることを確認する。
3. Chimney を外周壁と共有壁へ試し、後者だけ配置拒否されることを確認する。
4. Site 内の Fan が paired Yard grid を消費することを確認し、SoulSpa の発電量を下げて blackout / recovery を確認する。
5. Intake → Conduit → Purifier を作り、1 tile erase / 再敷設で給水を切り替える。
6. 壁下 Conduit を通常表示とインフラ overlay の両方で確認する。
7. 壁を建て直して Room を再検出し、換気・排泥・認可が再収束することを確認する。
8. Tank / MudMixer / SoulSpa が Miasma / Unwatered でも従来どおり稼働することを確認する。
9. save → load 後に物理設備、配管接続、認可表示が復元・再計算されることを確認する。
10. 1,000 tile 配管を分断・再接続し、topology rebuild が dirty frame に限定されることを確認する。

## 9. ロールバック方針

- M0〜M4 を独立 commit / PR 単位にし、後続マイルストーンは直前の完了条件を満たしてから開始する。
- M1 / M2 の問題は換気設備・state・visual のマイルストーン単位で戻し、Room detection role の回帰修正だけは test とともに維持できるよう分離する。
- M3 の問題は Conduit 配置層、FluidGrid、Save 対応を同じ単位で戻す。save schema に新型を出荷した後は、単純削除ではなく互換 shim または version 方針を先に決める。
- M4 の停止契約で問題が出た場合は M4 全体を戻し、M3 の換気・給水可視化までは維持できる構造にする。
- 診断用 probe、debug material、一時 feature flag は原因切り分け後に撤去し、恒久経路へ残さない。

## 10. AI引継ぎメモ

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手: M0〜M4
- 採用済み提案を現行コードへ照合し、実装不能だった Room 床占有、壁 lookup、Conduit 重層、状態モデルを本計画で修正済み。

### 次のAIが最初にやること

1. `git status --short` と `git diff` を確認し、並行作業の変更を対象へ混ぜない。
2. M0 だけを対象に `room_detection/core.rs`、`room_systems.rs`、`hw_jobs/model.rs`、現行 Save/Load 計画を再読する。
3. `RoomDetectionBuildingTile` の role 化と pure test から着手し、Floor 上 fixture の回帰を先に固定する。

### ブロッカー/注意点

- Room entity は永続 ID ではない。Room ID を save、Relationship target、長寿命 timer key に使わない。
- `RoomTileLookup` は床専用であり、壁設備には `RoomBoundaryLookup` を使う。
- Conduit 完成時に `Building` を付けたり `WorldMap.add_building` を呼んだりしない。
- PowerConsumer lifecycle は root adapter にあり、`hw_energy` 内だけを見て実装しない。
- M3 時点の Save API は active hardening plan により変わる可能性がある。手動 allow-list の現行形を計画から固定しない。
- 現在の asset catalog 差分は本計画外。ユーザー変更を preserve する。

### 参照必須ファイル

- `docs/proposals/hvac-plumbing-proposal.md`
- `docs/room_detection.md`
- `docs/soul_energy.md`
- `docs/invariants.md`
- `crates/hw_world/src/room_detection/{core.rs,ecs.rs,tests.rs}`
- `crates/hw_world/src/room_systems.rs`
- `crates/hw_jobs/src/model.rs`
- `crates/hw_ui/src/selection/placement/`
- `crates/bevy_app/src/plugins/{logic.rs,visual.rs}`
- `crates/bevy_app/src/systems/energy/`
- `crates/bevy_app/src/systems/save/`

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-07-13 / pass（計画作成時 baseline）`
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: `2026-07-13 / pass（計画作成時 baseline）`
- 最終 `cargo fmt --all --check`: `2026-07-13 / fail（既存 workspace-wide formatting drift。HVAC 文書変更外）`
- 最終 `cargo test --workspace`: `未実施（計画作成のみ）`
- 未解決エラー: `N/A`

### Definition of Done

- [ ] M0〜M4 が完了し、各マイルストーンの完了条件を満たす
- [ ] Room 再生成、停電、配管分断、Save/Load の回帰 test がある
- [ ] 既存仮設設備とバケツ搬送が変化していない
- [ ] `docs/infrastructure.md` と関連恒久 docs が実装に同期している
- [ ] `cargo fmt --all --check` が成功
- [ ] `cargo check --workspace` が成功
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` が成功
- [ ] `cargo test --workspace` が成功
- [ ] `git diff --check` が成功

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-13` | `Codex` | 採用済み HVAC / plumbing 提案を現行コードへ照合し、Room 占有、壁 lookup、Conduit 重層、状態モデル、Save/Load 境界を修正した M0〜M4 計画として昇格 |
