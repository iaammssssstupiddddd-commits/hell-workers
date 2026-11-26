# 地獄のインフラ（換気・導水・部屋認可）実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `hvac-plumbing-plan-2026-07-13` |
| ステータス | `In Progress` |
| 作成日 | `2026-07-13` |
| 最終更新日 | `2026-11-26` |
| 作成者 | `Codex` |
| 関連提案 | [hvac-plumbing-proposal.md](../proposals/hvac-plumbing-proposal.md)（採用済み。世界観・採否理由の正本） |
| 関連Issue/PR | `N/A` |
| 前提ドキュメント | [room_detection.md](../room_detection.md) / [soul_energy.md](../soul_energy.md) / [building.md](../building.md) / [save_load.md](../save_load.md) / [crate-boundaries.md](../crate-boundaries.md) / [Track C3 ロード後再構築レジストリ計画](archive/save-rehydration-registry-plan-2026-08-03.md)（完了済み） / [P03 Indoor Light Domain Core](3d-rtt/single-scene-light-field/03-indoor-light-domain-core-plan-2026-08-03.md)（`hw_infra` bootstrap owner） |

> 本計画を実装契約の正本とする。関連提案と矛盾する場合は、2026-08-03 時点の現行コード、B3電力契約、
> Track C3再構築契約へ再照合した本計画を優先する。

## 2026-11-25 改訂作業

現行の自然換気を、外部境界・室間開口・部屋ごとの空気収支から再計算する。屋外をマップ端から判定し、完成した風穴だけを流路にする。固定した基準風と小さな双方向交換を使い、各室の圧力、室間移送、外気による希釈能力を分けて求める。外気を持たない閉回路は全室ゼロとする。

1. 圧力と希釈の収支を検証し、密閉室・単一外気口・通り抜け・室間移送・開閉扉をテストする。
2. ゲーム内の表示、Help、セーブ再構築を同じ計算結果へ接続する。旧画像の配置は閉鎖時にゼロ、外部の開口を追加すると回復することを確かめる。
3. 送排風機・煙突・空調機・熱・湿度・汚染物質の実装順と依存条件を定め、旧計画の固定能力・即時汚染判定を更新する。

### 温度・湿度・清浄度の実機調査

通常プレイの自然換気を基準に、計測ビルドの専用シーンで全256室の水蒸気、汚染物質、熱を仮想1秒ごとに輸送する。完成した風穴の実流量を読み、物質は同一の輸送係数、熱は壁面伝熱を加えた係数を使う。構造が固定された区間で行列を分解して使い回す。4倍速の扉開閉中に追加なし、各1項目、湿度＋清浄度、3項目を短い区間で交代させる。各構成の単位更新CPU、初期化、フレーム間隔、物理量の変化と保存則を検証する。64棟／一棟を各3反復し、同じ実行ファイル内に対照を置く。調査結果は室内空気の仕様に記録する。専用出力 `target/perf-runs/air-dimensions-*` はHVAC担当が照合・独立レビューまで使用し、検証台帳確定後に整理する。

初回6試行は数値・実機検証に合格。3項目追加のCPU時間は全256室・1更新当たり64棟+23.27 µs／一棟+102.64 µs。2回目の計測は1区間が規定の実時間を超えたため比較から除外した。台帳確定後に `target/perf-runs/air-dimensions-01`（1,328,661バイト）と `target/perf-runs/air-dimensions-02`（1,319,487バイト）を整理した。Helpへの追加影響はない。新規の輸送計算は計測専用起動時だけ有効で、通常ゲームの床・風穴の表示や換気判定は従来の経路を使う。[結果と適用範囲](../indoor_air.md#温度湿度清浄度の追加負荷)

### 今回の実装・実機比較

扉は全状態で部屋の境界とし、通気の対象は完成した換気設備に限定する。構造世代だけでM1の更新を判定し、独立成分別に密閉・単室・多室の式を適用する。参照解との照合、部屋成立・保存再開・Helpの同期に続き、実ゲームで同数の部屋を独立棟／一棟に建てる。4倍速、同じ部屋・扉数・開閉周期・画角・人口を使い、初回構築、計測中の再構築回数、換気処理時間、フレーム時間、遅延フレームを分けて評価する。一時出力はHVAC担当が検証まで利用し、結論をこの計画と仕様に残して整理する。

2026-11-25に6試行を完了。扉開閉による換気再計算は0回で、換気CPUのp99は64棟1.26 µs、一棟1.37 µs。ゲーム全体のp99はそれぞれ34.74 ms、36.28 ms。60 FPSを安定維持する条件は満たさなかった。GPUログ照合に失敗した初回の専用出力855,005バイトと、照合・独立レビュー済みの再計測出力3,523,067バイトを、それぞれ台帳記録後に整理した（対象は `target/perf-runs/ventilation-grates-01`、`ventilation-grates-02`）。[条件と結果](../indoor_air.md#2026-11-25-実機計測)

## 1. 目的

### 解決したい課題

- 現在は床を選んで部屋の成立条件を確認できる。次は、所長が壁に設備を施工して室内環境を改善する目標を加える。
- Soul Energy の恒常的な消費先が少なく、中盤以降の電力配分判断が限定的である。
- 水は Tank / MudMixer 間の手動搬送に閉じており、本設区画へ恒久インフラを伸ばす遊びがない。
- 採用済み提案には、現行の Room 占有判定、壁タイル逆引き、単一 `WorldMap` 建物層、Room の再スポーン契約と両立しない実装案が残っている。

### 到達したい状態

- 外気とつながる経路から各室の有効外気を計算し、良好・換気不足・外気交換なしを判定する。
- 非電化換気から電動換気へ段階的に発展し、`ScourgeFan` と `LetheIntakeGate` が既存 Yard 単位の電力網を消費する。
- 建物占有とは別のインフラ層へ導水管を敷設し、取水門から浄化盤までの 4 近傍接続を管理できる。
- 換気と排泥を満たした Room だけが `Sanctioned` となり、本設設備の稼働条件へ反映される。
- MudMixer / Tank / SoulSpa とバケツ搬送は変更せず、停止時にも復旧経路を残す。

### 成功指標

- 壁の建て直しで Room entity が全再生成されても、次の再計算で同じ換気・認可結果へ収束する。
- 3×3室は西壁の風穴一つで有効外気0.20、西・東に一つずつで1.40になる（必要0.45）。
- M2では停電でファンの圧力差が消え、残る自然換気と補給経路から通風量が決まる。
- Intake → Conduit → Purifier の途中を 1 タイル撤去すると対象 Room が `Unwatered` になり、再接続で `Drained` に戻る。
- Room 内の Tank / MudMixer / SoulSpa はインフラ状態にかかわらず従来どおり稼働する。
- 換気・流体トポロジーを毎フレーム全再構築しない。

## 2. スコープ

### 対象（In Scope）

- P03で導入済みの`crates/hw_infra`を拡張し、換気・流体・認可状態のデータモデルおよび純粋計算を追加する。
- Room 検出入力の「床を無効化する上物」分類と、壁境界から Room を逆引きする lookup。
- `ScreamingVent`、`SighChimney`、`ScourgeFan`、`LetheIntakeGate`、`OssuaryConduit`、`SludgePurifier`。
- `BuildingCategory::Permanent` と本設設備のインフラ停止契約。
- 壁設備の置換、電力接続、導水管のドラッグ配置・erase 撤去、インフラオーバーレイ。
- 物理設備の Save/Load、派生 lookup / grid / Room 状態のロード後再構築。
- Room / 設備 info panel、配置失敗理由、認可失敗理由。
- 実装に追従する恒久ドキュメントと索引の更新。

### 非対象（Out of Scope）

- Soul の快適度・健康・ストレス・疲労への影響。
- M1は固定境界・部屋単位の定常収支。温湿度の時間変化と個別の瘴気濃度はM5以降で扱う。
- 新規 ResourceType。既存の Wood / Rock / Bone を使う。
- Tank / MudMixer / SoulSpa への配管強制と既存バケツ搬送の置換。
- `ConfessionValve`、`SludgeBasin`、`MiasmaGauge`。M4 の評価後に別計画へ切り出す。
- 侵食猶予タイマー、配管容量、複数系統の優先度制御。
- Soul Energy の将来構想である Room 単位 PowerGrid への変更。
- 完成設備の一般 demolition タスク。導水管はインフラ編集モードの erase 操作で撤去する。

> `hw_infra` はP03で導入済み。HVAC M1では既存crateを拡張し、P03の `lighting` pure coreの依存境界を維持する。

## 3. 計画策定時のギャップ

2026-11-25時点では、下表のうち床・室内設備の役割分類と共有境界の逆引きは導入済み。M1で完成壁を改修する建築物・叫びの風穴と部屋単位の自然換気を追加した。[部屋検出の現行仕様](../room_detection.md)・[境界の逆引き](../../crates/hw_world/src/room_systems/lookup.rs)

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

完成した開口の両端と実際の屋外を特定し、室圧・流量と室間の負荷移送から各室の有効外気を求める。必要量は床面積×0.05、基準風は西→東。判定は `Ventilated / Insufficient / NoOutdoorExchange`。M2の送風機は通電時に経路へ圧力差を与える。煙突は屋外への出口と補給気経路を必要とする。[現行の計算と将来の設備仕様](../indoor_air.md)

- PowerGrid entity は従来どおり Yard が所有するが、Consumer のサービス範囲をその Yard と paired Site の union へ広げる。Room 単位 grid は作らない。
- 川岸の Intake は通常の位置包含では grid を引けないため、「取水門 footprint と水平に重なり、規定距離内にある Site」を
  1件に解決し、その`PairedYard`を共通energy owner resolverへ渡す。`ConsumesFrom`はtopology reconcilerだけが確定し、
  placement/loadが直接挿入しない。0件または複数候補は配置拒否する。

#### 排泥と認可

- `LetheIntakeGate` と `SludgePurifier` が同じ `FluidGrid` へ接続し、取水門が通電中なら Purifier は `Drained` とする。
- Purifier は設置床を `RoomTileLookup` で逆引きし、1 基でその Room 全体の排泥要件を満たす。
- `RoomSanctionState::Sanctioned` は `Ventilated && Drained` のときだけ成立する。それ以外は `Enclosed` とし、理由を換気・排泥の別軸で保持する。
- 換気不足は認可の未達理由として表示する。
- `BuildingCategory::Permanent` の完成設備だけをインフラ要件の対象とする。既存 4 category と既存設備には適用しない。
- M4の浄化盤は断水を運転停止の理由とし、給水復旧で再開する。瘴気による停止を導入する場合はM5の濃度履歴を使う。

#### 初期バランス

| 設備 | category | 初期能力 / 需要 | 初期資材 |
| --- | --- | --- | --- |
| `ScreamingVent` | `Architecture` | 開口の通りやすさ1 / 非電化 | Rock 2 + Bone 1 |
| `SighChimney` | `Architecture` | 高さと内外温度差による駆動 / 非電化 | Wood 2 + Rock 2 |
| `ScourgeFan` | `Architecture` | 給排気端間に圧力差 / 0.5W案 | Wood 2 + Bone 6 |
| `LetheIntakeGate` | `Plant` | 供給 source / 1.0W | Wood 6 + Rock 6 |
| `OssuaryConduit` | `Permanent`（配置メニュー分類のみ） | 接続 / Bone 1 per tile | Bone 1 |
| `SludgePurifier` | `Permanent` | Room 1 室を排泥 | Rock 2 + Bone 2 |

数値は着手ブロッカーにせず初期定数として採用する。M2 / M3 の手動プレイで変更した場合は、理由と結果を計画の更新履歴へ残す。

### 4.2 データ所有と依存方向

P03 bootstrap時の`lighting` pure coreは`hw_core` / `serde` / `sha2`だけに直接依存する。HVAC M1以降が換気・流体・認可のためにcrateへ追加する依存方向は次に固定する。これらの拡張はP03 coreのpure境界を変えない。

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
| `AirBoundary / Opening / OpeningFlow` | `hw_infra` | しない | 経路・流量・双方向交換 |
| `Ventilation` | `hw_infra` | しない | 各室の有効外気、必要量、流入 |
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
- M4の停止は断水から一方向に導出する。M5で設備別の瘴気保護を追加する場合は別理由を保持する。
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

M1ではPreActorで部屋を検出し、PostActorの換気計算が確定した部屋・外壁・風穴を読む。一時停止中も建築状態から表示を更新する。ロード時は再構築の中で部屋検出後に換気を解く。下記はM2〜M4に追加する領域を含めたデータの依存順を示す。

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
- 既存 `PowerConsumer + #[require(Unpowered, PowerConsumerPolicy)]` とroot energy topology reconcilerを再利用する。
  M2でreconcilerのconsumer owner解決をYard内だけからpaired Siteまで拡張し、M3のIntakeは一意なSite/
  `PairedYard`を同じresolverへ入力する。reconcilerと競合する`ConsumesFrom`の直接挿入だけで済ませない。
  初期priorityは設備ごとに明示し、解決不能なら`Disconnected / No Power Grid`を表示する。
- `BuildingType` 追加時は `rg -n "BuildingType::|BuildingCategory::" crates -g '*.rs'` で exhaustive match を全件確認する。
- `crates/bevy_app/src/assets.rs` と `plugins/startup/asset_catalog.rs` は本計画作成時点で別作業の差分がある。実装時は current diff を読み、無関係な変更を上書きしない。

## 5. 期待する性能影響

- M1の低負荷化では構造確定時に屋外を境界とする区画群を作る。扉の開閉は換気計算を発生させない。構造の変更時に密閉・単室の簡略式、多室の疎な収支計算の順に適用する。M5以降の熱・水蒸気・瘴気は仮想時間で活動中の区画を更新する。回帰テストで全体解との一致を検証し、実機で再構築回数とCPU時間を測定した。[計算モデルと受入条件](../indoor_air.md#cpu負荷を抑える計算モデル)
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

完成壁を岩2・骨1で「叫びの風穴」へ改修する。完成、取消、解体、セーブ再開では建築物のライフサイクルに従う。ゲームの建物と区画から経路を作る処理は `bevy_app::systems::ventilation`、圧力と希釈の収支は `hw_infra::ventilation` が担当する。床詳細・風穴詳細・部屋境界色は同じ結果を使う。

| 試す配置 | 期待する結果 |
| --- | --- |
| 外壁を閉じ、大小二室の共有壁だけに風穴 | 両室の有効外気0。共有風穴に「外気経路なし」 |
| 3×3室の一つの外壁に風穴 | 一面の双方向交換で0.20、必要0.45 |
| 大小二室の両端にも西・東の風穴 | 正味0.67が二室を通り、西は0.72/0.45、東は0.56/0.60 |
| 風穴から囲い庭を経由して屋外につなぐ | 庭の屋外側にも風穴を設置すると給気が届く。扉を開けても外気は増えない |

[計算、表示、数値の意味](../indoor_air.md)

### 表示と戸口の追補

- 壁に沿う格子を一枚だけ描き、集会の道具が建物に重なる配置を防ぐ。
- 戸口を通過する経路を維持し、戸口で待機中の魂は空いた隣接マスへ移す。
- 風穴から接続先ごとの換気状態と不足量を確認できるようにし、大小二室で表示を検証する。

旧モデルの実機確認では、格子一枚と大小二室の詳細が収まることを確認した。その時点の密閉室への配分値は誤りで、現在の受入は上記の外気経路に従う。戸口での退避と無人集会の解散は自動テストの対象。

この表示修正の実機ジョブは `target/native-acceptance/` 下の `ui-usability-feedback-20261124T194613Z-f48cd196`、`ui-usability-feedback-20261124T195139Z-b24014a5`、`ui-usability-feedback-20261124T195727Z-1f225280` を整理した。削除前の合計は6,928,804 bytes。最後のジョブで修正版の画面を確認し、通常のビルドキャッシュは継続利用する。

### 完了条件と検証

圧力収支と室間移送、外気のないループ、同一面・対向面の開口、扉の開閉・施錠、部屋の再生成と保存再開を確認する。実機の `ventilation` と `ventilation-open` で同じ二室について、格子、両室の境界色、風向、不足量の表示を照合する。

2026-11-25、Intel Arc / Vulkanで密閉・通風の同一二室を比較した。密閉時は両室0、通風時は西0.72333・東0.56181。必要量0.45・0.60、境界線10本・12本、選択した格子1枚、画角と詳細枠の位置が両画面で一致した。画像を保存し、検証済みの一時出力 `target/native-acceptance/ui-usability-feedback-20261124T203741Z-60045515`（3,353,359 bytes）と同ディレクトリの `ui-usability-feedback-20261124T204103Z-d7c9bd0b`（3,360,730 bytes）を整理した。共用のCargoキャッシュは引き続き利用する。

## 2026-11-25　M2〜M7 ドラフト実装

施工・給電・選択詳細・保存を通る設備種別を建築カタログに追加する。給排気方向は給気機と排気機を建て分け、ダクトを介して壁の風穴へ接続する。配管は地上建築物と独立して予約する。物理時刻は仮想秒、清浄度は瘴気濃度、水分は水蒸気濃度で保持し、温湿度は室内外の状態から表示する。電力優先度は既存パネルを使う。ドラフトの設定値、代用表現および検証済み範囲を室内空気仕様に記録する。

2026-11-25、担当 `hvac-m7` が主作業ツリーで実機検証を完了した。二室で通常給電・停電・強運転での復電を確認し、手動設定と標準運転の増風要求を表示上で区別した。256室・4倍速の6試行では気候計算の平均CPUは0.14〜0.48 ms／仮想秒。ゲーム全体には両配置で目立つフレーム遅延があり、滑らかさの受入は未達。[実測条件と結果](../indoor_air.md#m7ドラフトの実機検証2026-11-25)

検証台帳で各結果を確定後、`target/native-acceptance/ui-usability-feedback-20261125T072313Z-80af8ac1`（3,308,217 bytes）、`target/native-acceptance/ui-usability-feedback-20261125T085734Z-c4635a3a`（3,309,970 bytes）、`target/perf-runs/hvac-m7-rooms-01`（1,356,093 bytes）を整理した。最終画面はユーザー確認用に保存した。

### M7実測後の計算方式の再検討（提案）

256室一棟の空調CPUは仮想1秒当たり平均0.480 ms、天候が切り替わる更新では中央値7.52 ms、最大15.24 msだった。4倍速では7.5実秒ごとに切替が来る。改善の第一対象をこのピークに置き、仮想1秒ごとの部屋別収支を維持する。ゲーム全体のフレーム時間p99は、扉静止時217.1 ms、開閉時218.0 ms。両区間で空調は動作しており、画面遅延に占める空調の割合は未確定。[実機条件と測定値](../indoor_air.md#m7ドラフトの実機検証2026-11-25)

**接続関係は建築時に確定し、運転時は変わった係数を更新する。** 現行の `AirRuntime::prepare` は一つの外壁端末の駆動圧が変わると全区画を渡して圧力・希釈を解き、熱・物質の行列も作り直す。各行列について、区画の番号・消去順・消去中に必要になる係数の格納位置をあらかじめ決める。運転時は数値を連続配列に入れる。空気と物質は完成した通気設備でつながる区画ごと、熱はそれに壁の伝熱も加えた区画ごとに解く。屋外は境界条件なので、同じ外気に面しただけの棟どうしは連結しない。[現行の更新処理](../../crates/bevy_app/src/systems/ventilation/climate.rs)・[疎行列の構造解析と数値分解](https://libeigen.gitlab.io/eigen/docs-3.4/group__TopicSparseSystems.html)

| 変わったもの | 次の物理計算で行うこと |
| --- | --- |
| 床・壁・風穴・風道の接続、区画の分割・統合 | 空気経路、壁伝熱、設備接続を更新し、影響する成分の計算順を再構築する。床の重なりで熱・水分・瘴気を引き継ぐ。ロード時は現在の形状から再生成する |
| 風の強さ・向き | 自然換気だけの系は、構造確定時の単位風で求めた各開口の正味流量に、符号付き風係数を掛ける。現行の線形抵抗と一つの基準風に対して成立する。設備の駆動圧が加わる系は、同じ圧力行列の分解を使い、新しい端末圧で解く |
| 送風設定・停電・煙突の温度差 | 実際の端末圧が変わった系で正味流量を更新する。外気と室間の流入・流出、希釈による有効外気、局所排気の捕集、熱回収と認可を同じ時刻の流量から判定する |
| ろ過量だけ | 該当する成分の瘴気の除去係数を更新する。ろ過がない成分では水蒸気と瘴気で同じ分解を使う |
| 発生熱・水蒸気・瘴気、屋外温湿度だけ | 流量・除去係数が同じなら、新しい発生量と外気条件を右辺に与えて解く。温湿度に反応して煙突や自動制御の運転段階が変われば、上の設備更新も適用する |
| 扉の開閉・施錠 | 換気の再計算を起こさない。壁・扉の固定の熱伝導は熱の収支に含める |

**正味流量と有効外気の区別。** 無風でも開口の双方向交換は各方向に `0.20 × 通りやすさ` 残る。たとえば同じ室の西と東に風穴を一つずつ置くと、有効外気は基準風で1.40、無風で0.40になる。流向が変われば上流室も変わる。新しい流量から各辺の両方向の係数を一度組み、希釈・水蒸気・瘴気・熱で使う。流量変更時には必要な行列の数値分解を更新する。熱には壁伝熱と熱容量、ろ過中の瘴気には除去項がある。閉鎖系は有効外気ゼロを保ちながら内部の熱・物質を運ぶ。壁内の容積ゼロの接点では、その時刻の流入と流出を釣り合わせる。

**計算順の効果。** 実機試験と同じ16×16室の四近傍グラフで、未消去の隣室が少ない順に消去した場合を静的に数えた。同数のときは元の座標順とした。数字は物質輸送行列1個の構造上の件数で、CPU時間への換算には実測が必要。[現行の輸送行列](../../crates/hw_infra/src/air_transport.rs)・[試験の部屋配置](../../crates/bevy_app/src/plugins/startup/perf_scenario/ventilation_fixture.rs)

| 配置 | 分解後に保持する接続数（現行 → 提案） | 分解時の係数更新数（現行 → 提案） |
| --- | ---: | ---: |
| 4室×64棟 | 320 → 320 | 576 → 576 |
| 256室×一棟 | 3,855 → 1,923 | 60,335 → 21,193 |

係数更新数は、各消去段階で残る隣室数を `d` としたときの `Σ d²`。現行の圧力・希釈計算は既に最小次数順を毎回選んでいるので、そちらの改善点は順序の再利用にある。天候16相の結果を全て保存する方式は、初めて遭遇する相や停電・送風設定の組合せに別途計算が要る。棟全体を一室にまとめる方式は、共有壁しか通じていない下流の部屋で起きる換気不足を表現できない。どちらも初回実装には採用しない。

**実装順と判定方法**

1. 現行版で圧力、希釈、熱・物質の組立／数値分解／代入、結果の公開を個別に計時する。現在の空調CPUには遅延したECS反映や描画待ちは含まれない。時間が集中している工程から置き換える。
2. 旧解法を参照に、全16相の正風・逆風・ほぼ無風、無風の単独入力、途中の停復電、ろ過、密閉室、囲い庭、壁だけで熱が伝わる二室、部屋の再編・保存再開を同じ入力列で照合する。開口の正味流量・有効外気・各室の3量と収支に、絶対＋相対許容差を設ける。換気不足・認可・制御切替は各ステップで判定の一致を確認する。
3. 同じ実機・同じ描画条件で64棟／一棟の旧版と候補を交互に測る。4倍速・各3試行・扉480回、仮想240秒の既存条件を再現し、別枠で天候が一周する仮想480秒と、多数の機器の一斉停復電も測る。初期構築時間、常駐・ピークメモリ、定常更新と天候切替のCPUを記録する。一棟の天候切替は**最大4 ms以内**を最初の目標案とする（60 FPSの予算16.67 msの約4分の1）。64棟の定常負荷も旧版と比較する。
4. 画面のカクつきは、同じ気候結果・設備状態を同じ仮想時刻に再生する対照を設け、UI・描画・他のシミュレーションを動かしたまま比較する。ゲーム全体のフレームp95／p99・50 ms超過率とCPU・GPUの待ちを記録する。空調のCPU目標と、ゲーム全体の滑らかさをそれぞれ判定する。

2026-11-26、担当 `hvac-m7` が主作業ツリーで改訂解法を実装。実機比較は `hvac-solver-compare` 系の登録ジョブを使う。同一Captureバイナリに旧解法・改訂解法・受動設備の結果再生を収録し、各配置で順序を交替する。低負荷時の数値一致、全気象相、32台停復電、時間・プロセスメモリの結果は室内空気仕様に集約する。原本照合を終えたジョブは整理する。

実機Captureは `hvac-solver-compare-02` で24プロセスを検証した。4室×64棟と256室×一棟の天候切替最大は改訂式0.471 ms・1.631 ms。一棟での給排気32台同時停復電の最大は1.745 ms。開閉中のゲーム全体p99は一棟で38.425 ms。再生対照を含む詳しい結果と範囲は[実機比較](../indoor_air.md#改訂解法の実機比較2026-11-26)にまとめた。制御閾値を修正する前に停止したビルド `hvac-solver-compare-01` の出力3,752 bytesと、検証済みCaptureの専用出力43,033,133 bytesを整理した。Memoryでも旧式と改訂式の両配置・計4プロセスを検証した。登録ジョブ `hvac-solver-compare-mem-reference`（3,185,008 bytes）と `hvac-solver-compare-mem-cached`（3,162,802 bytes）の専用出力も、結果を正本へ転記して整理した。

### 改訂解法の次に優先する効率化

256室・4倍速の改訂式3試行を再集計した。各試行240仮想秒のうち天候切替は8回で、その8フレームが使った空調CPU時間の合計は、全区間の空調CPU時間の約13.7〜14.0%（64棟）、16.2〜17.0%（一棟）だった。分母は各試行の公表精度の平均時間×更新数。天候切替時の係数準備・3量の代入と照合・換気結果の公開用コマンド作成は、64棟で順に0.088・0.017・0.208 ms、一棟で0.247・0.037・0.214 ms。通常更新の工程別内訳、公開後の遅延コマンド適用やUIのCPUは別途計測する。[既存の実機比較](../indoor_air.md#改訂解法の実機比較2026-11-26)・[集計方法](../../scripts/ventilation_comparison.py)

| 優先 | 候補 | 対象と成立条件 |
| --- | --- | --- |
| 1 | 風量が変化した系統の部屋・風穴だけに換気結果を書き戻す | 現行は変更した部屋を内部で記録した後、全体で一つの真偽値を返す。該当する部屋・開口の一覧を返し、数値と向きを保持して、詳細文を選択・ピン表示時に生成する。輪郭色は換気判定の段階が変わった時と初回生成時に反映する。切断・撤去・ロード時は旧表示を確実に置換する。 |
| 2 | 毎秒作る配列を再利用し、係数の変更確認を絞る | 室別の発生量、除去量、設備の駆動圧などの作業領域を形状確定時に確保する。室・開口ごとに合算した駆動圧・ろ過量と風の変化から、再検査する空気系と、その流量が影響する壁伝熱を含む熱系を求める。煙突は内外温度差、送風制御は直前の室内状態にも反応する。魂の移動や設備の発熱は各仮想秒に集計し、全室の温度・水蒸気・瘴気、認可・制御の評価を継続する。 |
| 3 | ろ過条件に応じて係数のコピーと走査を減らす | 現行の未ろ過系は水蒸気・瘴気の係数を共有する。流量変更時にこの係数を作る行列の余分な複製を省く。将来、同じ係数で運ぶ物質が増えたら、各物質の発生量を個別に持ち、前進・後退代入で同じ係数列を一度に読み出す。熱およびろ過中の瘴気には各々の係数を使う。 |

対象数は配置から求められる。4室×64棟では各棟に風穴8個がある。風と他設備の運転を固定し、1棟の送風機1台で風量が変わる場合、換気結果の書き戻し候補は現行の256室・512風穴から最大4室・8風穴になる。256室が一つにつながる配置や全棟の風量が変わる天候切替では、全域が更新対象になり得る。この件数からCPU短縮率は推定しない。[試験配置](../../crates/bevy_app/src/plugins/startup/perf_scenario/ventilation_fixture.rs)・[数値計算](../../crates/hw_infra/src/ventilation/network.rs)・[結果の書き戻し](../../crates/bevy_app/src/systems/ventilation/climate.rs)・[同じ係数で複数の右辺を解く例](https://github.com/Reference-LAPACK/lapack/blob/master/SRC/dgetrs.f)

**採否の測り方**　まず優先1を単独で比較する。同じ入力列・4倍速・両配置で、通常更新、天候16相、天候一定での1台切替、32台一斉停復電を分ける。更新した系統・室・開口数、割当回数・バイト、入力集計／係数準備／代入／書き戻し／反映後のUIのCPU時間を取る。床・風穴の選択とピン留め、ロード直後、一時停止中の設定変更も照合する。参照解との各秒の許容差は既存の `1e-9 × (1 + |参照値|)`、換気区分・認可・制御切替は一致、一棟の天候切替は最大4 msを維持する。CPUで再現する改善とゲーム全体のフレームp99は個別に判定する。扉の開閉はゲーム負荷としてのみ試験に含める。

2026-11-26、優先1と2を実装した。優先2を両方式で共通に使い、優先1を単独比較した。4倍速・扉連続開閉で、送風機1台変更時の空調CPU中央値は64棟で0.506→0.054 ms、一棟で0.892→0.635 ms。書き戻し処理の割当は両配置で1,536回→0回。一棟の天候切替最大は1.442 ms。全256室と512風穴の数値・制御を毎仮想秒照合した。ゲーム全体では両配置とも50 msを超えるフレームが残った。優先3の行列コピー削減と複数物質の同時代入は今後の候補。[条件と実測値](../indoor_air.md#系統別書き戻しの実機比較2026-11-26)

専用出力 `target/perf-runs/hvac-publication-capture-01`（64,417,792 bytes）と `target/perf-runs/hvac-publication-memory-01`（24,858,624 bytes）は独立検証を終え、結果を転記して削除した。両ディレクトリの使用量は合計89,276,416 bytesから0になった。主作業ツリーの通常Cargoキャッシュは継続開発で使用する。

## M2: 高度換気と電力連携

### 変更内容

1. `SighChimney` と `ScourgeFan` を追加する。
2. 煙突の屋外端と給気側を接続し、高さと内外温度差から得る圧力差で流量を決める。
3. Fan は `Architecture` とし、Site の Room 内床にのみ置け、完成時に
   `PowerConsumer { demand: 0.5 } + PowerConsumerPolicy`を付与する。共通priority allocatorとinfo panelを使う。
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

- [ ] 煙突は屋外に排出でき、補給が到達する経路で通風する
- [ ] 給排気端と補給経路を伴う給電中のFanが流量を変え、密閉した室内だけの循環で外気は増えない
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

> M0〜M2 は Track C3 と独立して進めてよい。Track C3は完了済みであり、M3では
> [Track C3 ロード後再構築レジストリ計画](archive/save-rehydration-registry-plan-2026-08-03.md) の完成registryを使って、
> 物理設備の保存と派生状態の再構築を production registry へ登録する。

### 変更内容

1. `BuildingCategory::Permanent` と Intake / Conduit / Purifier の建設種別を追加し、Architect category、validation、geometry、visual mirror を同期する。
2. `OssuaryConduit`、`ConduitTileLookup`、`ConduitBlueprintLookup`、topology / supply dirty、FluidGrid Relationship を実装する。
3. Conduit line drag / erase drag、独立 overlay、重ね敷設、重複拒否を実装する。Blueprint 段階から WorldMap building footprint を予約しない。
4. Conduit completion を generic Building spawn / footprint release より前に分岐し、WorldMap occupancy / obstacle を変更しない。
5. Intake の川岸 3×2 placement と`PowerConsumer { demand: 1.0 } + PowerConsumerPolicy`を実装し、
   一意に解決したSiteの`PairedYard`を共通energy owner resolverへ渡す。接続はtopology reconcilerが確定する。
6. Purifier の Room 内 placement、同一 FluidGrid 接続、Purifier 不在も含む `RoomDrainageState::Unwatered / Drained` を実装する。
7. topology dirty と supply dirty を分離し、Intake の停電で flood fill を再実行しない。
8. Conduit物理entityと新設備だけをsave対象へ追加する。`OssuaryConduit`は`Building`を持たないため、
   persisted component登録だけで済ませずschemaのroot marker/collection対象へ明示追加する。
9. durable/derived境界を次に固定する。
   - durable: Conduit/Intake/Purifier物理entity、Transform/footprint、`PowerConsumerPolicy`、既存Site/`PairedYard`。
   - runtime-derived: `ConsumesFrom`、Fluid supplier/drain relationship、FluidGrid entity、lookup、topology/supply dirty、
     Room drainage/sanction/overlay state。Intake ownerはload/rollback後もdurable Transform/footprint + Site/`PairedYard`から再解決する。
10. `hw_infra` ownerの`LoadResetRegistry` hookを追加し、normal/rollback各replace前に旧FluidGrid entity、
    fluid/energy runtime Relationship、ConduitTile/Blueprint lookup、topology/supply cache、infra overlayを破棄する。
    M4導入後はRoomSanctionHistory/派生overlayも同hookのcoverageへ追加する。
11. C3 `RebuildDerived`では`ConduitTileLookup -> ConduitBlueprintLookup -> FluidGrid/Relationship topology`を
    明示依存で1回だけ構築する。`WakeDomains`はenergy full rebuild、room full detection、infra supply/ventilation dirtyだけを立て、
    infra topology dirtyを再度立てない。root adapterはplugin `build`中にstepを登録し、全plugin build後の
    `SavePlugin::finish` coverage/freezeへ必ず含める。通常ロード、rollback、RecoveryFailed専用replaceで同じimmutable planを通す。
12. 最初のunpaused Logic順を
    `energy topology/allocation -> flush -> room detection/boundary -> flush -> ventilation -> fluid supply -> drainage/sanction`
    に固定する。paused load中はdirtyを保持し、runtime表示は`Rebuilding / Unpowered / Unwatered`へfail-closedにする。

### 主な変更ファイル

- `crates/hw_infra/src/{conduit.rs,fluid.rs,relationships.rs,systems.rs,visual.rs}`
- `crates/hw_jobs/src/model.rs`
- `crates/hw_core/src/{game_state.rs,visual_mirror/}`
- `crates/hw_ui/src/{setup/submenus.rs,selection/placement/}`
- `crates/bevy_app/src/interface/{selection,ui/interaction,ui/presentation}/`
- `crates/bevy_app/src/systems/jobs/building_completion/`
- `crates/bevy_app/src/systems/save/{schema.rs,saving.rs,load.rs,transaction.rs,rehydrate.rs}`
- `crates/bevy_app/src/systems/save/rehydrate/registry.rs`（Track C3 で導入）
- `crates/bevy_app/src/plugins/{logic.rs,visual.rs}`
- `assets/textures/buildings/infrastructure/`
- `docs/{building.md,infrastructure.md,save_load.md,state.md,invariants.md,soul_energy.md,room_detection.md,architecture.md,cargo_workspace.md,crate-boundaries.md}`

### 完了条件

- [ ] Conduit が床・壁・通常建物と同じ grid に存在できる
- [ ] Conduit 重複と blueprint 重複は拒否される
- [ ] Conduit blueprint の配置・cancel・完成・load の全経路で WorldMap building footprint を予約・解放しない
- [ ] Intake → Conduit → Purifier が同じ FluidGrid に接続される
- [ ] Intake が対応する Site / paired Yard を一意に解決できない場所では配置拒否される
- [ ] load/rollback後もdurable footprint + Site/PairedYardから同じIntake energy ownerへ再解決される
- [ ] 途中 1 tile の erase で Purifier が `Unwatered` になる
- [ ] Intake blackout / recovery は topology entity 数を変えない
- [ ] load 後に Conduit lookup / FluidGrid が stale entity なしで再構築される
- [ ] duplicate Conduitを含むcandidateはlive entity/WorldEpoch/UI不変でpreflight rejectされる
- [ ] normal/rollback/paused loadの各replaceで旧FluidGrid/relationship/lookup/overlay entityが0件になる
- [ ] load 1回につきFluid topology rebuildは1回だけで、WakeDomainsはtopologyを再dirty化しない
- [ ] paused中はfail-closed表示とdirtyを保持し、unpause最初のLogicでenergy確定後にsupply/drainageが収束する
- [ ] `hw_infra` の durable / derived / wake 契約が C3 の domain ledger と production registry coverage に含まれる
- [ ] `SavePlugin::finish`のproduction snapshotにinfra stepが含まれ、freeze後のlate registrationはrejectされる
- [ ] 通常ロード、rollback、RecoveryFailed専用replaceでinfrastructure stepの順序と実行回数が一致する
- [ ] 1,000 tile 配管の rebuild が dirty frame にだけ発生する

### 検証

- `cargo test -p hw_infra fluid`
- Save/Load: duplicate candidate preflight、normal/rollback/recovery-only同一step、paused stale runtime 0、first Logic収束、topology exactly once
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 手動: line drag、壁下表示、erase 分断、停電、save → load

## M4: Room 認可・本設停止・統合 UI

### 変更内容

1. `Ventilated && RoomDrainageState::Drained` から `RoomSanctionState` を決定する。
2. 本設設備の要件から断水による停止と復旧を同期する。換気判定は認可の理由として記録する。
3. 浄化盤を最初の断水保護対象とする。
4. Room overlay に認可・換気状態を表示し、footprintの認可が変わったときに印章を表示する。
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
- [ ] 換気不足かつ排泥済みの部屋で、認可未達と稼働中の設備を区別する
- [ ] 換気 + 排泥で `Sanctioned` になり、同一 footprint の Room 再生成では stamp が再発火しない
- [ ] load 直後の派生状態再構築では stamp を発火せず、以後の実遷移では発火する
- [ ] 換気不足と断水の理由が区別される
- [ ] Purifier は条件復旧後に自動再開する
- [ ] Tank / MudMixer / SoulSpa は停止対象外
- [ ] Room entity 再生成と save → load 後も同じ認可結果へ収束する
- [ ] follow-up 3設備を実装せずとも本計画の DoD を満たせる

### 検証

- `cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- §8 の全手動確認シナリオ

## M5〜M7: 空気質・暖冷房・運転制御（ドラフト）

M5で汚染物質量、M6で熱・水蒸気の蓄積、M7で外気条件と自動制御を導入する。各段階の設備・保存契約・受入条件は[室内空気と空調](../indoor_air.md#計画時の実装順と受入条件)に従う。

## 7. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| Floor 上設備で Room が消える | Fan / Purifier を置くほど機能が壊れる | M0 で room-detection role を先に導入し回帰 test を固定する |
| Room entity の全再生成 | Relationship / Timer / cached Entity が stale になる | Room target Relationship と猶予 Timer を禁止し、lookup と状態を再構築する |
| 室間空気の二重計上 | 閉鎖された系に外気が湧く | 密閉ループの外気ゼロと、下流室の希釈の数値を検証する |
| Conduit を Building として完成させる | WorldMap 占有・walkability・Room 判定を破壊する | completion を専用 entity spawn へ分岐し、WorldMap を変更しない |
| topology と給水状態を同じ dirty にする | blackout ごとに全配管 flood fill | topology dirty / supply dirty を分離する |
| Purifier の停止状態を drainage 判定へ戻す | 自分自身の断水判定が循環して復旧不能 | network reachability を先に独立計算し、disabled は派生結果としてだけ同期する |
| Conduit blueprint が通常 footprint を予約する | 完成前から床・壁と競合し、cleanup で他建物を消す | blueprint 段階から専用 lookup のみを使い、generic reserve / release を通さない |
| Room 再生成ごとに認可 stamp を出す | 壁編集や load で演出が連打される | canonical footprint history で実遷移だけを検出し、load seed 時は抑止する |
| 停電で電動換気停止 | 給排気不足 | 自然換気・仮設設備・手動水搬送で復旧できる |
| Site と Yard が別領域のため Fan が常時停電する | 電動換気が成立しない | Yard 所有 grid の consumer lookup を paired Site へ拡張する |
| 川岸 Intake が Yard / Site の外にある | 位置包含では grid へ接続できない | footprintからSite/`PairedYard`を一意解決し、共通reconcilerへ渡す。`ConsumesFrom`を直接挿入しない |
| 完成済みC3 registryを迂回してM3の手書き再構築を追加する | root のrehydrateとregistry登録が二重化する | infra stepをproduction coverageとnormal / rollback / recovery-only traceへ同時追加し、別runnerを作らない |
| runtime Gridをpersisted root削除へ任せる | paused load/rollback後に旧FluidGridが残る | hw_infra LoadReset hookでentity/relationship/lookup/cache/overlayを各replace前に破棄 |
| RebuildDerived後にtopology dirtyを再設定 | load 1回でflood fillが二重実行 | RebuildDerivedはtopologyを1回構築、WakeDomainsはsupply/energy/room dirtyだけに限定 |
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
- 共有風穴の閉鎖系・屋外接続系での移送、煙突の出口と補給気。
- Floor 上 fixture と wall replacement を含む Room 検出。
- Conduit の直線、分岐、loop、分断、再接続の連結成分。
- topology dirty と supply dirty の独立性。
- Room 再生成時に stale Room entity を保持しないこと。
- 同じ footprint の Room 再生成と load seed で sanction stamp を重複発火しないこと。
- save 対象が物理設備だけで、派生 cache を含まないこと。

### 手動確認シナリオ

1. 3×3室に外壁風穴を一つ付け、0.20/0.45の不足を確認。反対の外壁を開けて改善する。
2. 共有風穴だけの二室で有効外気がゼロ、東西の外壁を開くと両室を通風することを確認する。
3. Chimney を外周壁と共有壁へ試し、後者だけ配置拒否されることを確認する。
4. Site 内の Fan が paired Yard grid を消費することを確認し、SoulSpa の発電量を下げて blackout / recovery を確認する。
5. Intake → Conduit → Purifier を作り、1 tile erase / 再敷設で給水を切り替える。
6. 壁下 Conduit を通常表示とインフラ overlay の両方で確認する。
7. 壁を建て直して Room を再検出し、換気・排泥・認可が再収束することを確認する。
8. Tank / MudMixer / SoulSpa が換気不足・断水時も従来どおり稼働することを確認する。
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

- M1の自然換気、M2〜M7のドラフトと改訂解法に加え、系統別の書き戻し、作業配列の再利用と係数の変更確認を実装・実機検証した。32台の送風機を備えた一棟の天候切替最大は1.442 ms。[室内空気仕様と実測値](../indoor_air.md#系統別書き戻しの実機比較2026-11-26)
- Track C3とP03の `hw_infra` 導入は完了。設備計画M0の受入は未確定。既存の部屋検出の役割分類と共有境界lookupを起点に、M0の未確認項目を照合する。
- 採用済み提案を現行コードへ照合し、実装不能だった Room 床占有、壁 lookup、Conduit 重層、状態モデルを本計画で修正済み。

### 次のAIが最初にやること

1. `git status --short` と `git diff` を確認し、並行作業の変更を対象へ混ぜない。
2. M0の各受入条件を既存の部屋検出・境界lookupのテストに対応付け、足りない条件だけを検証する。[部屋検出](../room_detection.md)・[P04が確認した範囲](3d-rtt/single-scene-light-field/04-indoor-light-runtime-integration-plan-2026-08-03.md)
3. 残ったゲーム全体のフレーム遅延を調べるときは、遅延コマンド適用、UIレイアウト、描画・GPU待ちそれぞれの時間を遅いフレームと対応付ける。空調に物質を追加する場合は、優先3の同時代入を現在の3量の実装と比較する。

### ブロッカー/注意点

- Room entity は永続 ID ではない。Room ID を save、Relationship target、長寿命 timer key に使わない。
- HVAC M1で`hw_infra`を新設しない。P03のpure `lighting` coreへHVACのECS / GPU依存を追加しない。
- `RoomTileLookup` は床専用であり、壁設備には `RoomBoundaryLookup` を使う。
- Conduit 完成時に `Building` を付けたり `WorldMap.add_building` を呼んだりしない。
- PowerConsumer lifecycle は root adapter にあり、`hw_energy` 内だけを見て実装しない。
- B3以後は`PowerConsumerPolicy`がdurable、`PowerSupplyState` / `Unpowered` / grid summaryがruntime-derivedである。
  HVACは共通strict-prefix allocator、restore margin、Legacy modeを利用し、独自配電やruntime state保存を追加しない。
- M3 は完成済みTrack C3 registryを使用する。Conduit / Intake / Purifier の物理 entity を durable、
  FluidGrid / lookup / Relationship topology / dirty / Room state を runtime-derived として C3 台帳へ追加し、
  hw_infra reset hookと通常ロード/rollback/recovery-onlyの共通runnerから再構築する。stepはplugin build中に登録して
  `SavePlugin::finish` freezeへ含める。RebuildDerivedでtopologyを1回構築し、
  WakeDomainsで再topology dirtyを立てない。
- 現在の asset catalog 差分は本計画外。ユーザー変更を preserve する。

### 参照必須ファイル

- `docs/proposals/hvac-plumbing-proposal.md`
- `docs/room_detection.md`
- `docs/soul_energy.md`
- `docs/invariants.md`
- `docs/plans/archive/save-rehydration-registry-plan-2026-08-03.md`
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
| `2026-11-25` | `Codex` | 初回候補を二部屋の自然換気に具体化。部屋検出・境界lookup・crateの導入状況を反映。 |
| `2026-08-13` | `Codex` | `hw_infra`の初期bootstrap ownerをP03へ同期。HVAC M0の独立性を維持しつつ、M1以降を既存crate拡張へ変更 |
| `2026-08-04` | `Codex` | Track C3の実装・実機受入・archive完了を反映。M3 prerequisiteを充足済みに更新し、完成registryを迂回する手書き再構築を禁止 |
| `2026-08-03` | `Codex` | Track C3 採用に合わせ、M0〜M2 は独立、M3 は C3 完了後という依存を固定。Conduit root marker、hw_infra reset、phase別exactly-once rebuild、normal/rollback/paused回帰へ更新 |
| `2026-08-03` | `Codex` | C3自己レビューへ同期し、infra stepの`SavePlugin::finish` freeze coverageとRecoveryFailed専用replace回帰をM3へ追加 |
| `2026-07-13` | `Codex` | 採用済み HVAC / plumbing 提案を現行コードへ照合し、Room 占有、壁 lookup、Conduit 重層、状態モデル、Save/Load 境界を修正した M0〜M4 計画として昇格 |
