# Save/Load境界強化・互換性リファクタリング計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `save-load-hardening-plan-2026-07-12` |
| ステータス | `Draft` |
| 作成日 | `2026-07-12` |
| 最終更新日 | `2026-07-12` |
| 作成者 | `Codex` |
| 親ロードマップ | [system-wide-correctness-refactoring-plan-2026-07-12.md](system-wide-correctness-refactoring-plan-2026-07-12.md) |
| 関連済み計画 | `archive/save-load-world-serialization-plan-2026-07-05.md` |
| 前提 | M1〜M3: runtime M0完了。M4: runtime M1/M2/M4完了。M5: runtime M3完了 |
| 関連Issue/PR | `N/A` |

## 1. 目的

### 解決したい課題

- Save format versionをDynamicWorld内部Resourceへ置くと、削除済み型を含む旧saveのversionをdeserialize前に判定できない。
- `register.rs` / `saving.rs` / `entities.rs` の手動一覧が役割未分離で同期漏れを起こし得る。
- 一時World preflightが完全なtransaction保証であるかのように扱われている。
- save/load systemが無順序のUpdateへ登録され、simulation更新途中へ割り込める。
- load後resetがcache中心で、旧Entityを保持するMessage、UI/selection Resource、visual cache、System Local、RemovedComponentsが残る。
- fixed `saves/world.scn.ron`へ直接I/Oするため、自動testが実saveを汚染・競合する。
- `ReservedForTask` runtime削除と旧save互換方針が未定義。

### 到達したい状態

- DynamicWorld deserialize前にmagic/version/seedを読める外部headerがある。
- schemaを`persisted_resource` / `persisted_component` / `reflect_dependency` / `root_marker`の4分類で一元管理する。
- parse、deserialize、static preflight、live apply、rehydrateを別Result境界としてtestできる。
- live persisted worldの置換失敗時はin-memory rollback snapshotからpersistent状態を復元し、通常loadと同じruntime正規化・rehydrateを行う。
- exclusive applyは`Last`内の最終SaveLoad setでのみ実行する。
- load reset対象がplugin単位で登録され、旧simulation Entity参照が次frameへ残らない。
- runtimeから`ReservedForTask`を除去しつつ、header無しv0 saveを1世代だけ読めるlegacy shimを維持する。

### 成功指標

- v0 fixture / v1 fixture / future version / corrupt headerをdeserialize前に分類できる。
- schema登録型、allow-list型、root marker、reflect依存型の整合testがある。
- deserialize失敗、preflight失敗、live apply失敗、rehydrate prerequisite失敗を別testで検証する。
- deserialize/preflight/prerequisite失敗ではlive Worldが不変。live apply開始後の失敗ではpersistent snapshotを同値復旧し、非保存runtime状態は通常loadと同じ初期状態へ正規化される。
- load直後に旧Entity IDを持つ対象Resource/Message/Local/RemovedComponentsが0件。
- round-trip testがfilesystemを使わず並列実行できる。

## 2. スコープ

### 対象（In Scope）

- Save file header/version/seedとlegacy v0判定。
- schemaの宣言的定義とregistry/allow/root marker test。
- serialization/deserializationとfilesystem wrapper分離。
- staging preflight、live apply、rollback snapshot、rehydrate prerequisite。
- Save/Load scheduleとload reset registry。
- entity-bearing Resource/Local/UI/visual stateの棚卸し。
- `ReservedForTask` runtime利用削除とlegacy Reflect shim。
- save/load恒久ドキュメントとfixtures。

### 非対象（Out of Scope）

- 複数save slot、autosave、cloud save。
- 任意version間を変換する汎用migration framework。
- Worldgen seed不一致時のterrain再生成。
- Building footprint child rehydrateのロジック本体（runtime計画 M4で実装）。
- production plugin composition全面整理。

## 3. 設計判断

### 3.1 外部Save header

save fileはDynamicWorld RON bodyの前に、registry非依存で読める固定headerを持つ。

```text
HELL_WORKERS_SAVE
(format_version: 1, worldgen_seed: 12345)
---
<DynamicWorld RON body>
```

- `SaveHeader`は通常のSerde型とし、DynamicWorldのTypeRegistryを使わずparseする。
- magic無しの既存fileは`LegacyV0`として扱う。
- `format_version > CURRENT_SAVE_FORMAT_VERSION`はbodyをdeserializeせず明示rejectする。
- v0は本計画中だけacceptし、legacy Reflect shimを登録したregistryでbodyをdeserializeする。
- seed mismatchもbody適用前にheaderからrejectする。v0だけは旧`SavedWorldgenSeed` Resource抽出へfallbackする。

### 3.2 schemaの4分類

| 分類 | 用途 | 例 |
| --- | --- | --- |
| `persisted_resource` | register + allow_resource | `GameTime`, `WorldMap`, `DreamPool` |
| `persisted_component` | register + allow_component | `DamnedSoul`, `WorkingOn`, `Building` |
| `reflect_dependency` | registerのみ | `IdleBehavior`, `BuildingType`, `ResourceType`, phase enum |
| `root_marker` | collect対象entity選択 | `DamnedSoul`, `Building`, `ResourceItem` |

- typed macroまたは型リストmacroから`register_save_types`とDynamicWorldBuilder設定関数を生成する。
- `Transform`等Bevy側登録に依存するallowed componentは`external_registered_component`としてschemaに明記し、registry testで存在を確認する。
- root markerは意味が異なるため別一覧を維持するが、代表archetype matrixでcollect/extract/round-tripを検証する。
- 非保存の派生型は`runtime_derived_exclusion` inventoryへ明記する。runtime M4導入後の`ObstacleSourceKind`とbuilding footprint mirrorはここへ追加し、Save bodyへ混入しないことをtestする。

### 3.3 load transaction境界

```text
read content
  -> parse external header
  -> deserialize DynamicWorld
  -> validate seed/schema/rehydrate prerequisites
  -> write_to_world_with(staging World)  // static registry/Reflect preflight only
  -> capture current persisted rollback snapshot
  -> pre-replace reset + despawn old persisted/proxy entities + flush
  -> discard old RemovedComponents buffers before inserting new components
  -> write new persistent world
     -> failure: clear partial + flush + discard partial removal buffers
                 + restore rollback snapshot + runtime normalization + rehydrate
                 + return RecoveredLoadError
  -> commit persistent simulation world
  -> runtime normalization
  -> idempotent presentation rehydrate
```

- staging preflightの保証は`ReflectComponent` / `ReflectResource` / registry整合に限定する。
- unregistered typeは通常deserialize段階で失敗するため、preflight testとは分ける。
- rehydrateに必要な`AssetServer`、`GameAssets`、visual handle Resourceはdespawn前にvalidateする。
- presentation rehydrateはpersistent commit後に実行する。事前条件検証後はResultを返すidempotent処理とし、同じworldへ2回実行してもshellを重複させない。
- rollback snapshotは同じpersisted schemaで作るため、`AssignedTask` / `Path` / `Destination` / AI state / presentation shellは復元しない。live writeの予期しないResult error後はpersistent状態だけを同値復旧し、通常loadと同じreset・runtime正規化・rehydrateを必ず通すdegraded recoveryとする。
- rollbackの「persistent同値」はraw Entity IDやRON byte列の一致ではなく、Entity remap後のcomponent/resource値、Relationship graph、WorldMap内Entity参照が構造的に一致することと定義する。
- preflight以前の失敗はlive World不変、live apply開始後の失敗は`RecoveredLoadError`を返すplayableな正規化world、という2段階の保証をAPIとdocsに明記する。

### 3.4 load reset

`LoadResetRegistry`は`bevy_app::systems::save::reset`所有のroot Resourceとする。leaf crateはroot型へ依存せず、自crate状態だけを処理する`pub fn reset_for_world_replace(&mut World)`を公開し、`bevy_app`のplugin facadeがcallbackをregistryへ登録する。entity-bearing Localが参照する`WorldEpoch`だけはneutral contractとして`hw_core`が所有する。

#### 必須分類

| 分類 | 例 | 方針 |
| --- | --- | --- |
| root-owned Messages | assignment/reservation/squad/idle/designation等 | typed macroから`add_message<T>`と`Messages<T>::clear()`を生成 |
| plugin-local Messages | `UiIntent`, visual通知等 | leafが公開するreset関数をroot plugin facadeがtype-erased hookとして登録 |
| selection/context | `SelectedEntity`, `HoveredEntity`, `MoveContext`, placement state | Defaultへreset |
| UI操作状態 | rename、drag、info pin、area edit history/clipboard | simulation Entity参照をclear。UI node registry自体は維持 |
| visual owner cache | Soul proxy owner等 | stale proxy despawn + cache clear |
| simulation cache | spatial/resource/tile/room/reachability | Defaultへresetし既存systemで再build |
| entity-bearing System Local | door waits、pending reservation等 | reset可能Resourceへ移すか`WorldEpoch`不一致時にclear |
| RemovedComponents | old persisted entityのdespawn通知 | Bevy 0.19で検証した`World::clear_trackers()` flush手順で全bufferを破棄 |

- Bevy `MessageRegistry`のprivate entry列挙には依存しない。
- root message型はtyped macro、plugin-local型はroot facade経由のreset hookで明示登録する。
- `World::clear_trackers()`によるRemovedComponents flushは代表型で「load前のremovalを次frame readerが受け取らない」testを追加する。必要なupdate回数はBevy 0.19の二重buffer実装で確認し、コメントへ根拠を残す。
- entityを保持しないscratch Localは対象外とし、棚卸し表に理由を書く。

#### root coordinatorの固定phase

1. live apply開始後、root/plugin-local Messagesとrequest state、entity-bearing Resourceをresetする。
2. old persisted entityとstale UI/visual proxyをdespawnし、simulation cacheをclearして`WorldEpoch`を進め、`world.flush()`する。
3. **新しいDynamicWorldを書き込む前に**root coordinator自身がold RemovedComponents bufferを破棄する。Bevy 0.19の二重bufferを空にする`clear_trackers()`回数は一次ソースとtestで固定し、plugin hookへ委譲しない。
4. new DynamicWorldまたはrollback snapshotを書き込み、source/mirrorを含むruntime状態を正規化してpresentation shell/cacheをrehydrateする。
5. 最終`world.flush()`後にstale Entity検証を行う。phase 4以降では`clear_trackers()`を呼ばず、loaded/rehydrated componentのAdded/Changed trackerを維持する。

### 3.5 `ReservedForTask`互換

- v1 save allow-listから`ReservedForTask`を外す。
- runtime Producer、Query filter、item lifetime条件、designation flagを削除する。
- 型自体は`#[doc(hidden)]` legacy Reflect shimとしてv0対応期間だけ同じTypePathで残し、v0 registryへ登録する。
- v0 load後に全entityからshimを除去し、新しい予約/Relationship契約だけで動作させる。
- v1 saveを再保存した時点でmarkerはfileに含まれない。
- v0 supportを将来削除する時にshim型も削除する。本計画では型の物理削除を完了条件にしない。

## 4. 期待する影響

- save formatの判定をdeserialize前へ移し、未知versionや破損データがlive Worldへ部分適用される経路を閉じる。
- staging、rollback snapshot、rehydrate事前検証によりload中の一時メモリと処理時間は増えるが、preflight前のlive World不変とapply後のplayableなdegraded recoveryを優先する。
- 通常frameのhot pathには追加処理を置かず、性能影響をsave/load実行中へ限定する。
- schemaの4分類とlegacy shimの期限を明示し、型移動・削除時の互換性判断を機械的に監査可能にする。

## 5. マイルストーン

## M1: test可能なI/O境界と外部header

### 変更内容

1. `SavePath` Resourceを追加し、defaultだけが現行`SAVE_FILE_PATH`を使う。
2. `serialize_world_body(world) -> Result<String, SaveError>`をfilesystemから分離する。
3. `encode_save_file(header, body)` / `decode_save_file(content)`を純粋関数として追加する。
4. loadを`read_file`と`prepare_load_from_str`へ分ける。
5. magic無しv0、v1、future version、corrupt header fixturesを追加する。

### 主な変更ファイル

- `crates/bevy_app/src/systems/save/{state.rs,saving.rs,load.rs,mod.rs}`
- `crates/bevy_app/src/systems/save/format.rs`（新規）
- `crates/bevy_app/src/systems/save/error.rs`（必要なら新規）
- `crates/bevy_app/tests/fixtures/save/`（またはmodule内fixture）
- `docs/save_load.md`

### 完了条件

- [ ] unit testが`saves/world.scn.ron`を読み書きしない
- [ ] header/version/seedをDynamicWorld deserialize前に判定
- [ ] v0/v1/future/corruptを別errorへ分類
- [ ] filesystem atomic writeは一意temp pathでtest

### 検証

- format pure function tests
- temp path atomic write test
- `cargo test -p bevy_app@0.1.0 --lib`

## M2: 宣言的Save schema

### 変更内容

1. §3.2の4分類schemaを`save/schema.rs`へ実装する。
2. `register.rs`と`saving.rs`の手動型列挙をschema生成関数へ置換する。
3. `entities.rs`のroot marker収集をschema macroから生成する。
4. external registrationを含むregistry dataを検査する。
5. 全root markerの代表archetype matrixを作り、collect/extract/round-tripする。

### 主な変更ファイル

- `crates/bevy_app/src/systems/save/{schema.rs,register.rs,saving.rs,entities.rs,mod.rs}`
- `docs/save_load.md`
- `docs/invariants.md` I-P1

### 完了条件

- [ ] persisted resource/componentがregisterとallowの両方へ出力される
- [ ] reflect dependencyがregisterされallow-listには入らない
- [ ] external allowed componentのregistry dataが存在
- [ ] root markerの全代表archetypeが抽出される
- [ ] schema追加手順が1箇所に記載される

### 検証

- AppTypeRegistry test
- DynamicWorldBuilder extraction matrix
- representative body serialize/deserialize test

## M3: PreparedLoad・preflight・rollback

### 変更内容

1. `PreparedLoad { header, dynamic_world }`を導入する。
2. deserialize errorとstaging preflight errorを別型で返す。
3. staging World preflightはstatic registry/Reflect contractだけを検証すると文書化する。
4. live despawn前にrehydrate prerequisitesとseed/schema invariantをvalidateする。
5. current persisted worldをin-memory rollback DynamicWorldとして保持する。
6. live write失敗時はpartial load entities/resourcesを除去し、rollback snapshotを適用する。
7. M3では既存cache reset/normalization/rehydrateを`finalize_loaded_world`へ抽出し、success/rollbackの両branchをそこへ合流させる。M4で同entrypointを固定phase registryへ拡張する。
8. rehydrateをResult + idempotentにし、presentation shell重複を防ぐ。

### 主な変更ファイル

- `crates/bevy_app/src/systems/save/{load.rs,rehydrate.rs,entities.rs}`
- `crates/bevy_app/src/systems/save/transaction.rs`（新規）
- `docs/save_load.md`
- `docs/invariants.md`

### 完了条件

- [ ] deserialize/preflight/live apply/prerequisite errorを別testで再現
- [ ] preflight失敗前後でlive world同値
- [ ] injected live apply failure後にEntity-remapを考慮したpersistent graph同値、非保存runtimeは通常loadの初期状態、worldは次frameを継続可能
- [ ] rehydrate prerequisite失敗時はdespawn前に中止
- [ ] rehydrate 2回実行でshell重複なし

### 回帰テスト

- registered Reflect without ReflectComponentによるpreflight failure
- rollback fault injection: remap後persistent graph同値 + task/AI/UI正規化 + 次frame update
- seed mismatch
- representative Relationship/WorldMap round-trip

## M4: frame境界とload reset

### 変更内容

1. runtime M1/M2/M4完了を開始条件とし、request intakeはUpdateのInput/Interfaceから`SaveLoadState`へ書くだけにする。
2. exclusive save/load applyを`Last`の`SaveLoadApplySet`へ固定する。
3. Last内の他systemより後に実行するorderingを明示し、save/loadを直列化する。
4. §3.4のroot-owned`LoadResetRegistry`、root message typed macro、root facade登録を実装する。
5. `WorldEpoch`を`hw_core`へ追加し、entity-bearing Resource/Localのinventory表を作ってreset/epoch/retainを分類する。
6. §3.4の固定phase coordinatorでnew world適用前にold RemovedComponents bufferをflushし、new worldのAdded/Changedは次frameに観測可能なことをtestする。
7. runtime計画M4で現行rehydrateへ暫定配線したsource/footprint/index helperを固定phase coordinatorへ移す。保存済みobstacle bitmapをdurable semantic sourceから再構築し、transient placement reservation bitと不完全Move `Designation` rootを除去する。v0/v1へ同じ規則を適用する。

### 主な変更ファイル

- `crates/bevy_app/src/systems/save/{mod.rs,load.rs,reset.rs,state.rs}`
- `crates/bevy_app/src/plugins/messages.rs`
- `crates/bevy_app/src/plugins/`のroot facade登録箇所
- `crates/hw_core/src/`（`WorldEpoch`）
- `crates/hw_core/src/selection.rs`
- `crates/bevy_app/src/app_contexts.rs`
- `crates/hw_ui/src/`のentity-bearing state
- `crates/hw_visual/src/`のowner cache
- entity-bearing Localを所有するsystem
- `docs/save_load.md`
- `docs/events.md`
- `docs/architecture.md`

### 完了条件

- [ ] save/load applyがUpdate/PostUpdateへ登録されていない
- [ ] `Last::SaveLoadApplySet`で全producer後に1回だけ実行
- [ ] typed message resetに登録漏れtestあり
- [ ] leaf crateが`bevy_app::systems::save`へ依存せず、root facadeだけがregistry型を参照
- [ ] old Entityを持つselection/context/UI/visual/cache/Localが0件
- [ ] old RemovedComponentsを次frame readerが受信しない
- [ ] loaded componentのAdded/Changedは必要なrebuild systemが観測する

### 回帰テスト

- Interface中load request → Last apply ordering
- pending request before load → post-load applyなし
- selected/drag/pin/move/proxy/door-wait state reset
- old removal message drop + new Added observation
- v0/v1 live Move reservation fixture → durable blockerだけを再構築し、Door Open/Closed/Lockedを再適用して、予約bit/不完全Move Designationを除去

## M5: `ReservedForTask` runtime除去とv0 shim

### 変更内容

1. runtime M3完了を開始条件とし、§3.5のv0 shimを登録してv1 allow-listから除外する。
2. `DesignationOp::Issue.reserved_for_task`を削除する。
3. runtime insert/remove、Without filter、arbitration dirty reader、item lifetime条件を削除する。
4. v0 apply後にshim componentを全entityから除去する。
5. v0をv1として再saveし、shim type pathがbodyから消えるtestを追加する。

### 主な変更ファイル

- `crates/hw_core/src/events.rs`
- `crates/hw_logistics/src/types.rs`
- `crates/hw_logistics/src/item_lifetime.rs`
- `crates/hw_logistics/src/transport_request/`
- `crates/hw_familiar_ai/src/familiar_ai/decide/`
- `crates/hw_soul_ai/src/soul_ai/`
- `crates/bevy_app/src/systems/save/{schema.rs,load.rs}`
- `docs/logistics.md`
- `docs/tasks.md`
- `docs/save_load.md`

### 完了条件

- [ ] runtimeのReservedForTask参照はlegacy shim/loader以外0件
- [ ] v0 fixtureをload可能
- [ ] v0 load後runtime entityにshimなし
- [ ] 再saveしたv1 bodyにshim型なし
- [ ] AI candidate/item lifetimeが現行の予約/Relationshipだけで同じ挙動

### 検証

- v0 fixture migration test
- candidate/item lifetime regression test
- `rg` gate（shim/loader以外のReservedForTask禁止）

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| header導入で既存saveを読めない | ユーザーデータ損失 | magic無しをv0としてfixture固定、shimを1世代維持 |
| schema macroが型分類を隠す | 型追加困難 | 4分類を明示しregistry/extraction matrix test |
| staging成功後にlive apply失敗 | world破損 | persisted rollback + 共通reset/normalize/rehydrate + fault injection test |
| rollbackで非保存runtimeを完全復元できない | active task/AI/UI状態の喪失 | degraded recoveryをAPI契約化し、通常loadと同じ初期状態・playable性をtest |
| rollback後にpresentation shellが残る | 重複/ghost entity | commit前prevalidation、共通coordinator、rehydrate idempotence、partial entity tracking |
| load reset漏れ | stale Entity参照 | plugin registry + inventory表 + stale entity validation system/test |
| Local epoch対応漏れ | 次frame誤操作 | entity-bearing Localの`rg` inventoryと理由表 |
| clear_trackers回数誤り | old removal残留/Added消失 | Bevy 0.19 primary source確認 + representative App test |

## 7. 検証計画

### 各マイルストーン必須

- 変更Rustファイルを個別rustfmt
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test -p bevy_app@0.1.0 --lib`
- rust-analyzer workspace diagnostics 0件
- `git diff --check`

### 計画完了時

- `cargo test --workspace`
- v0/v1/future/corrupt fixtures
- failure layer/rollback/reset/order tests
- `python scripts/update_docs_index.py`

### 手動確認

1. 現行v0 saveをF9でloadし、警告後に正常復元。
2. v1 save/load後にSoul、Relationship、建物、資材、電力、WorldMapが復元。
3. future/corrupt saveをloadしても現在worldが維持。
4. load前の選択/drag/pin/move状態が解除される。
5. load直後に旧requestが新worldへ適用されない。

## 8. ロールバック方針

- M1〜M5を独立コミットにする。
- external header writer/readerは同一コミットに含める。
- schema移行中も旧手動リストとの二重正本期間を作らない。
- transaction commit前にrollback testをgreenにする。
- ReservedForTask runtime削除とv0 shimを同一コミットに含める。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `0%`
- 完了済み: なし
- 未着手: M1〜M5
- M1〜M3はruntime M0完了前、M4はruntime M1/M2/M4完了前、M5はruntime M3完了前に着手しない。
- `docs/proposals/hvac-plumbing-proposal.md`の既存変更は対象外。

### 次のAIが最初にやること

1. 現行save RONをfixtureとして匿名化/最小化し、v0 parser testを先に作る。
2. `bevy_world_serialization 0.19.0`のWorldDeserializer/write_to_world_with error境界を再確認する。
3. M1のpure format/I/O分離から開始する。

### ブロッカー/注意点

- versionをDynamicWorld Resourceだけに置かない。
- unregistered型は通常preflightより前のdeserializeで失敗する。
- staging Worldとlive Worldはinsert/apply条件が異なる。preflightを完全transactionと呼ばない。
- Bevy MessageRegistryのprivate entry列挙に依存しない。
- RemovedComponentsは通常Messages registryとは別buffer。
- UI node registryのUI Entityはsimulation loadで維持する。simulation Entity参照だけをresetする。
- v0 support中はReservedForTask型を物理削除しない。

### Definition of Done

- [ ] M1〜M5完了
- [ ] v0/v1互換とfuture rejectが自動test済み
- [ ] preflight前failureのlive World不変testと、apply後failureのdegraded recovery test成功
- [ ] post-load stale Entity検証成功
- [ ] `cargo check --workspace`成功
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`成功
- [ ] `cargo test --workspace`成功
- [ ] docs/index更新、計画archive済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-12` | `Codex` | 全体計画の自己レビュー指摘を反映して新規作成 |
| `2026-07-12` | `Codex` | 再レビューを反映し、reset ownership/phase、degraded rollback、runtime依存、v0/v1 obstacle正規化を確定 |
