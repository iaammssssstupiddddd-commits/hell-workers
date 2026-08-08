# Track C2 セーブカタログ・手動スロット・オートセーブ 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `save-catalog-autosave-plan-2026-08-03` |
| ステータス | `Draft` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-08` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track C2） |
| 前提計画 | `docs/plans/archive/save-rehydration-registry-plan-2026-08-03.md`（C3完了済み）、`docs/plans/archive/building-deconstruction-plan-2026-08-03.md`（C1完了済み） |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: 単一save fileしか扱えず、退避・比較・世代付き自動復旧を安全に行えない
  - 現行は `saves/world.scn.ron` 1件だけを `SavePath` で参照し、試行前の退避、複数world状態の比較、
    世代付き自動復旧ができない。
  - fileの欠落、破損、future format、seed不一致はload実行後の通知でしか分からず、選択前に安全性を判断できない。
  - slot選択とsave/load requestが別Resourceになると、UI frameから`Last` applyまでにtargetが変わるraceを作り得る。
- 到達したい状態:
  - 3つの手動slot、load-onlyの既存default save、1〜5世代のautosaveをbounded runtime catalogで一覧できる。
  - F5/F9とPause menuは同じforeground catalog modalを開き、上書き確認、load確認、破損/非互換表示を行う。
  - operationとslotを不可分のtyped requestとして`Last::SaveLoadApplySet`へ渡し、既存atomic writeとrollbackをslot単位で再利用する。
  - autosaveはactive play時間、manual優先、bounded generation、同期save計測という明示契約で動く。
  - `RecoveryFailed`では通常transactionから切り離したC3のrecovery-only replaceを使い、別slotの再ロードまたは終了だけで
    fail-closed状態から復旧できる。
- 成功指標:
  - manual save/load/overwrite/missing/corrupt/unsupported/seed mismatchの全終端がslot label付きで識別できる。
  - pre-transaction failureと`ApplyRecovered`では現在worldが維持/復旧され、`RecoveryFailed`はpaused fail-closedになる。
    いずれの失敗でも別slotや旧default fileを上書きしない。
  - autosave generationが設定上限を超えず、manual requestと同frameに二重saveしない。
  - catalog open/closeでDynamicWorld deserialize、候補評価、A*、毎frame directory scanを行わない。

## 2. スコープ

### 対象（In Scope）

- typed `SaveSlotId`、`SaveLoadRequest`、`SaveStorageRoot`、runtime `SaveCatalog`とentry/status。
- 3固定manual slot、最大5 autosave generation、既存 `saves/world.scn.ron` のload-only legacy-path entry。
- bounded header inspection、file metadata、empty/current/v0/corrupt/unsupported/seed mismatch表示。
- F5/F9/Pause menuから開くSave/Load catalog modal、selection、overwrite/load confirmation、foreground capture。
- slot-aware atomic save、authoritative recheck、load preflight/rollback、terminal outcome/notification。
- file/parent directory durability、header/body streaming、crash temp非表示を持つplatform atomic-write adapter。
- user-local autosave enabled/interval/generation settings、active-play timer、manual優先scheduler。
- save所要時間のphase別計測とautosave default判断。
- v1 containerとworld schema evolutionの責務境界を恒久docsへ固定する。
- Help、恒久docs、temp-directory fixtures、実機受入。

### 非対象（Out of Scope）

- 任意slot数、フォルダ選択、ユーザー入力のslot名、rename/delete、cloud save、Steam Cloud。
- save file thumbnail、world preview、play time、任意metadata sidecar。
- background thread I/Oや複数frame snapshot。同期計測が閾値を超えた場合は別計画でsnapshot境界を先に設計する。
- 任意version間を自動変換する汎用migration framework。
- seedの異なるsaveに合わせてworld terrainを再生成してloadする機能。
- quicksave専用の確認なし上書き。初版F5はSave catalogを開く。
- autosave fileへのmanual save。manual UIからautosave generationを上書きしない。

## 3. 現状とギャップ

| 現行契約 | ギャップ | C2の対応 |
| --- | --- | --- |
| `SavePath`が単一pathを保持 | selectionとoperationを安全に束縛できない | `SaveLoadRequest { operation, slot }`へ統合 |
| v1 headerはversion/seedのみ | 一覧/破損判定APIがない | body非deserializeのbounded inspector |
| fixed `world.scn.ron` | 複数手動/自動世代がない | typed IDからcanonical filenameへ写像 |
| atomic temp+fsync+rename | empty/confirmed saveのTOCTOU契約がない | request-bound revision、apply再検査、Absent no-replace commit |
| load preflight/rollback | target labelがfile name依存 | request slot labelをterminal outcomeへ保持 |
| F9は単一file存在時にconfirm | slot list/corrupt status/selectionがない | foreground catalog modalへ置換 |
| saveは同期exclusive | autosave中のframe停止量が未計測 | serialize/write/totalのbounded timing取得 |

## 4. 実装方針（高レベル）

### 4.1 Slot IDとcanonical path

- neutralなtyped IDは `hw_core` に置き、root save ownerと`hw_ui::UiIntent`が同じ型を参照する。

```rust
pub enum SaveSlotId {
    Manual(u8),   // 1..=3
    Autosave(u8), // 1..=5
    LegacyDefault,
}
```

- constructorで範囲を検証し、任意文字列やpathを受け取らない。
- default storage rootは `saves/`、filenameは次に固定する。
  - `manual-1.scn.ron`〜`manual-3.scn.ron`
  - `autosave-1.scn.ron`〜`autosave-5.scn.ron`
  - `world.scn.ron`（既存file。load-only）
- path解決はroot ownerだけが `SaveStorageRoot + SaveSlotId` から行う。UI/ViewModel/notificationへ絶対pathを渡さない。
- testsは必ず一意temp directoryの `SaveStorageRoot` を注入し、実 `saves/` を読み書きしない。

### 4.2 Runtime catalog

- `SaveCatalog` はworld saveへ含めないruntime indexで、entry数は
  `3 manual + 5 autosave + optional legacy default`（最大9件）に制限する。
- `LegacyDefault` entryは`saves/world.scn.ron`が存在する時だけ生成し、欠落時にEmpty entryを水増ししない。
- scan triggerはcatalog open、save/load terminal outcome、settings generation変更、明示refreshだけとし、毎frame scanしない。
- inspectorは先頭16 KiBを上限として読み、separatorが上限内に無ければcorrupt headerとする。
  DynamicWorld bodyはdeserialize/cacheしない。entryは次を保持する。
  - slot IDとplayer-safe label
  - role（Manual / Autosave / LegacyDefault）
  - manual save可、scheduler write専用、load-only、inactive autosaveのcapability
  - exists/file size/optional modified time/`SaveFileRevision`
  - header statusとworldgen seed（読める場合）
  - last attempted load failureのruntime-only分類（fileが変わるまで）
- slotの場所と内容状態を一つのenumへ混在させない。capabilityは
  `can_manual_save / can_scheduler_save / can_load`の独立軸にする。Manualだけがmanual save可、active Autosaveだけが
  scheduler save可、inactive AutosaveとLegacyDefaultはload-onlyであり、`can_load`は内容healthから導出する。

| Content status | Load可否 | 表示 |
| --- | --- | --- |
| Empty | 不可 | Empty |
| CurrentV1 / same seed | 可 | Current format |
| LegacyV0Candidate | full preflight後 | Legacy save (validation required) |
| SeedMismatch | 不可 | Different world seed |
| UnsupportedVersion | 不可 | Newer/unsupported version |
| CorruptHeader | 不可 | Corrupt save header |
| Unreadable | 不可 | Save file unavailable |
| BodyInvalid（load試行後） | 再試行可 | Invalid save data |

- v1はheaderだけでseed mismatchを分類する。magic/headerを持たないfileはcatalogでvalid v0と断定せず
  `LegacyV0Candidate`とだけ表示し、load時のfull decode/schema/candidate preflightで初めて確定する。
  失敗時は`BodyInvalid`とslot label付きterminal outcomeへ流す。
- 既存内容のあるManual slotはcontent statusにかかわらず、明示overwrite confirmation後なら置換できる。
  ただしexisting targetのstable `SaveFileRevision`を取得できない場合はfail-closedで上書き不可とする。
  LegacyDefaultとAutosaveをmanual UIから上書きしない。

- modified timeはoptional domain値として保持し、初版UIは追加calendar crateを必須にせずboundedな相対表記
  （just now / Xm / Xh / Xd）を使う。表示中はcached timestampと現在時刻からlabelだけを更新し、directoryを再scanしない。
  metadata取得だけ失敗した場合は`Modified time unavailable`、file read自体が失敗した場合は`Unreadable`を表示する。
  file名やraw OS errorを本文へ露出しない。

### 4.3 Request、確認、transaction

- `SaveLoadState`の単純triggerを、operationとslotを同じ値に保持するone-shot requestへ置換する。

```text
Save {
  origin: ManualCatalog { dialog_session } | Autosave,
  slot,
  expected_target: Absent | Exact(SaveFileRevision),
}
Load { slot, dialog_session }
```

- UIが選択中slotを後から変えても、受理済みrequestのtargetは変わらない。
- `SaveFileRevision`はexists、file identity（取得可能なplatform）、length、mtime、bounded prefix fingerprintを値として保持する。
  optional metadataは欠落もrevisionの一部とし、existingなのに再検査可能なrevisionを作れないtargetへ`Absent`を代用しない。
- manual save:
  - Empty選択は `Absent` で発行する。
  - occupied選択はforeground confirm時のlength/mtime/bounded-header fingerprintを束縛した`Exact(revision)`で発行する。
  - ownerは`Last` apply時にrevisionを再検査する。Absent targetが出現、またはExact targetが変化/消失した場合は
    上書きせず`OverwriteConfirmationRequired` outcomeにする。
- autosaveもscheduler発行時のauthoritative target metadataから、missingなら`Absent`、existingなら`Exact(revision)`を束縛する。
  confirmationは不要だが、catalog snapshotを無条件置換権限に変えず、stale revisionは安全にterminal failureとする。
- save writerはbodyを一度serializeした後、同一directoryのexclusive temp fileへheader/separator/bodyを順次writeし、
  container全体をもう一つの`String`へ連結しない。fileをflushして`sync_all`した後にcommitする。
- `Absent` saveはOS/file adapterのatomic no-replace commit
  （同一filesystem hard-linkまたは同等のrename-no-replace）を使う。targetがcommit直前に出現しても失敗し、
  通常renameへfallbackして上書きしない。unsupported filesystemでは安全側のI/O failureにする。
- `Exact(revision)`はapp内のslot write lock下でrevision再検査後に既存atomic replaceを使う。
  app外の非協調writerによる再検査後のraceは初版対象外だが、app内selection/request競合は排除する。
- no-replace/replace成功後は対応platformで親directoryもsyncする。rename後のdirectory syncだけが失敗した場合は
  fileを元に戻せないため、`CommittedDurabilityUncertain`としてcatalogをrefreshしImportant warningを返す。
  crashで残ったtempはcatalog対象外とし、削除する場合も正規prefix・同一storage root・件数上限付きcleanupに限定する。
- loadはcatalog表示を信用せず、ownerがauthoritative read/header/schema/preflightを再実行する。
  failure時は既存transactionどおりlive worldを維持またはrollback復旧する。
- load failureのUI終端を分ける。
  - read/header/schema/candidate preflight failure: live replace前なのでselection/historyを維持し、Load catalogを開いたままentry状態を更新する。
  - `ApplyRecovered`: rollbackでEntity IDが変わり得るためentity-bound selection/scroll/pending targetをresetし、`Warning + Important`を出す。
  - `RecoveryFailed`: entity-bound UIを全resetし、gameplay/saveを止めたpaused fail-closed stateで`Error + Important`を出す。
    catalogから別slotのload再試行だけを許可する。
- C3 transaction coordinatorが所有する`SaveRecoveryMode::Healthy | RecoveryFailed`をUI/input gateの正本として再利用する。
  `RecoveryFailed`中は
  save/autosave/resume/world input/domain commitを全てgateし、foreground Load catalogとquitだけを許可する。
  別slotのrequestはfull read/decode/schema/staging/domain validationを通した後、C3のrecovery-only replace modeを使う。
  success時だけ`Healthy`へ戻して明示resume操作を再許可するが自動unpauseはしない。preflight/applyの再失敗では
  paused fail-closedとcatalogを維持する。
- outcomeはraw pathでなくslot ID/labelを持ち、A2 notificationのdedupe keyにもslotを含める。
- save success後とloadの全terminal outcome後にcatalogをdirty化し、次のrefreshでmetadataを更新する。

### 4.4 Modalと入力所有権

- F5 / Pause `Save Game` はSave catalog、F9 / Pause `Load Game` はLoad catalogを開く。
- AreaEdit active drag等、現行resolverがF5を禁止する境界は維持する。raw keyboard readerを追加しない。
- modal stateは少なくとも `Closed / SaveCatalog / OverwriteConfirm(slot) / LoadCatalog / LoadConfirm(slot)` を区別する。
- open request受理frameからpending captureを立て、表示後はfull-viewport `UiInputCapture`へ引き継ぐ。
  background world pointer/camera/UIを遮断し、Escは最前面confirm→catalogの順に閉じる。
- manual requestはmodalを閉じてから発行しない。`dialog_session`とslotをrequestへ束縛し、terminal outcomeまで
  同じSave/Load catalogまたは対応confirmがforeground captureを保持する。これによりmanual applyだけは
  「自分が所有するmodal/capture」を許可し、別modalやstale sessionを許可しない。
- world replacementでselection、scroll、pending slot、confirm stateをresetする。catalog自体はEntityを持たないが、
  load terminal後にauthoritative refreshする。
- save可否は一つのboolへ潰さず、共通baseとorigin別contextに分ける。
  - `WorldSnapshotEligibility`（共通base）: world replacement/save-load apply/root domain commit中でなく、
    AreaEdit/drag等の未commit gestureがない。request発行時と`Last` apply直前の両方で検査する。
  - `ManualCatalog`: common baseに加え、requestと一致するforeground Save catalog/confirm captureを要求する。
    Pause menu由来を含むpaused状態は許可し、modal保持自体を不適格理由にしない。
  - `Autosave`: common baseに加え、`Healthy`、unpaused active play、foreground modal/captureなしを要求する。
  - loadも同じsession ownershipとreplace/apply/domain commit gateを持ち、`RecoveryFailed`だけは専用Load catalogから
    recovery-only replaceを許可する。

### 4.5 Container format / world schema方針

- この方針はTrack C全体のC0判断として計画時点で先に確定済みであり、C2 M1実装までC1を待たせない。
  C1の`DeconstructionOrder` / target Relationship / `WorkType::Deconstruct`はv1 bodyへのadditive type/variant追加として扱い、
  new executableはold v0/v1を補完して読む一方、old executableによるnew v1のforward loadは保証しない。
- C2初版は既存container v1を維持する。slot ID、mtime、catalog status、autosave設定をDynamicWorld header/bodyへ追加しない。
- `format_version`はmagic/header/body separator等、containerをdecodeする規則のversionである。
- additive durable componentは現行どおり、旧v0/v1のmissing値を明示default補完し、旧実行ファイルからのforward loadは保証しない。
- durable typeの削除、rename、field shape変換などbody migrationが必要になった時点で、次の専用計画により
  container v2へ `world_schema_version` を追加する。v2 loaderはv1を明示migration inputとして扱い、
  unknown versionを推測でdeserializeしない。
- Catalogはcurrent/legacy/unsupportedを分類するだけで、unknown bodyを書き換えない。
- user表示名を将来追加する場合はheader v2またはsave本体と同じcommit protocolのsidecarを正本とする。
  C2初版では名前入力を導入しない。

### 4.6 Autosave

- user-local `GameSettings` に次を追加する。world saveへは含めない。

| Setting | 初期値 | 範囲 |
| --- | --- | --- |
| Enabled | false | bool |
| Interval | 10 active minutes | 5 / 10 / 20 / 30 min |
| Generations | 3 | 1..=5 |

- timerは`Time<Real>`のdeltaを、simulationがunpausedかつforeground modalなしのactive play中だけ加算する。
  game speedを上げてもwall-clock間隔を短縮しない。
- manual/autosaveは§4.4の同じ`WorldSnapshotEligibility` baseを使い、origin別contextだけを分ける。
  foreground Save catalogが必要なmanualをmodal一律禁止で自己否定しない。
- timerが満了後にineligibleになった場合はinterval値でsaturateして1件だけdueを保持し、eligibleへ戻った最初のframeで
  発行する。経過interval数をqueueへ変換せず、手動操作を追い越さない。
- generation設定は書込対象のactive autosave ID `1..=N`だけを決める。Nを減らしても既存の`N+1..=5`を
  自動削除せず、catalogへ`Inactive autosave`としてload-only表示する。Nを再び増やすとrotation対象へ戻す。
- producer順を固定し、`GameSystemSet::Interface`でmanual intentを先にone-shot requestへ変換し、autosave schedulerは
  その後かつ`Last::SaveLoadApplySet`より前に走らせる。既にmanual/load/requestがpendingなら上書きせずdueを保持する。
  queueを積まず、同時にpending requestは最大1件とする。busy/rejectされたmanual intentにはtyped outcomeを返す。
- active generation内ではmissing slotを小さいID順に選ぶ。全て存在し、全entryのmodified timeが信頼できる場合は
  最古→小さいIDで選ぶ。一つでもmtime unavailableならruntime round-robin cursorを使い、startupは最小ID、
  commit成功時（directory sync不確実を含む）だけ次IDへ進める。これによりmtime欠落時にslot 1だけを上書きし続けない。
- scheduler requestも`Absent/Exact`を束縛してautosave slotをconfirmationなしで置換する。失敗時はtight retryせず、
  次のfull intervalから再試行する。
- timer消費を次に固定する。
  - origin別eligibility不成立によるrequest/apply延期: intervalでsaturateしたまま保持し、terminal failureを出さない。
  - accepted autosaveの成功またはterminal failure（revision/serialize/write/sync/commitを含む）、manual save成功または
    `CommittedDurabilityUncertain`、load/rollback/recovery-only成功: 0へ戻し、次のfull intervalから数える。
  - manual save失敗やpre-transaction load失敗: timerを変更しない。
- autosave成功は`Success + ToastOnly`（historyへ残さずslot単位dedupe）、autosave失敗は`Error + Important`、
  `CommittedDurabilityUncertain`は実更新を明示する`Warning + Important`とする。manual save/loadの既存Important終端は維持し、
  定期成功で履歴を埋めない。

### 4.7 同期save性能・メモリgate

- current exclusive pathを維持し、snapshot/serialize、temp write+file sync+commit+directory sync、totalを固定長metricsへ記録する。
- representative small/medium/large fixtureを各3回warmup後、20回測定する。p95はnearest-rank
  `sorted[ceil(0.95 * n) - 1]`で算出する。
- M4でsave workloadを追加し、reference machine上で次を実行する。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py audit \
  --workload save --sizes small,medium,large --repeat 20 --preflight-runs 3 \
  --output /tmp/hw-c2-save-perf-20260803
```

- artifactにはcommit、OS/filesystem、CPU、fixture entity数/body bytes、process peak RSS delta、各phase sample、percentile算法だけを残し、
  save bodyや絶対save pathを残さない。
- memoryの構造gateとして、serialized body以外のfull-size container `String`/`Vec<u8>`を作らず、header/bodyをtempへ
  sequential writeし、I/O bufferを64 KiB以下に固定する。body bytesとpeak RSSは回帰artifactへ残すが、OS依存RSSだけで
  pass/failを決めない。
- C2 autosaveの完了条件は、reference machineのlarge fixtureで`total p95 <= 100ms`かつ`max <= 250ms`とする。
  超える場合はdefault offのまま「合格」とせず、immutable snapshot境界のfollow-up計画を作成してautosave milestoneを
  blockedにする。閾値を満たしてもdefaultを自動でonにせず、初期値offは独立したproduct判断として維持する。
- catalog openはheader prefixだけを読むため、save body sizeに比例するdeserializeを行わないことをcounterで固定する。

### 4.8 設計判断

| ID | 判断 |
| --- | --- |
| C2-D01 | manual slotは3固定、autosave物理slotは5固定・active世代は1〜5、legacy default pathはload-only |
| C2-D02 | F5/F9は確認なしquick操作でなくforeground catalogを開く |
| C2-D03 | operationとslotを単一requestに束縛し、Lastまでselection raceを作らない |
| C2-D04 | catalogはbounded header scanで作りDynamicWorld bodyを読まない |
| C2-D05 | manual/autosave requestへAbsent/Exact revisionを束縛し、apply再検査とatomic no-replaceで未確認overwriteを防ぐ |
| C2-D06 | 初版container v1を維持し、slot metadataをworld/headerへ保存しない |
| C2-D07 | body migrationが必要な最初の変更でcontainer v2 + world_schema_versionを別計画化 |
| C2-D08 | autosaveはactive real time、manual優先、queueなし、mtimeが全て信頼できる時だけoldest、それ以外はround-robin |
| C2-D09 | autosave初期値offはproduct判断。性能gateは機能完了条件でありdefault on/offとは分離する |
| C2-D10 | background化より先にimmutable snapshot境界を設計する |
| C2-D11 | manual/autosaveは同じauthoritative baseを二段階確認し、origin所有modalだけをmanualで許可する |
| C2-D12 | v1 additive/forward-incompatible方針はTrack C0で確定済みとし、C1をC2 M1まで待たせない |
| C2-D13 | autosave成功はToastOnly、失敗はImportant。manual save成功時もautosave timerをresetする |
| C2-D14 | confirmed overwriteの非協調external-writer CASは初版対象外。AbsentだけはOS no-replaceでfail-closedに保証する |
| C2-D15 | `RecoveryFailed`は明示root stateとし、C3 recovery-only replaceの成功時だけ解除する |
| C2-D16 | save fileはheader/bodyを順次writeしfile+directory syncする。2本目のfull container bufferを作らない |
| C2-D17 | manual intent producerをautosave schedulerより先に固定し、pending requestを後発producerが上書きしない |
| C2-D18 | accepted autosaveの全terminal failureはfull interval backoff、eligibility延期だけdue保持とする |

- Bevy 0.19 APIでの注意点:
  - `Time<Real>`と`Time<Virtual>`のpause/speed意味はBevy 0.19一次情報で確認し、timer ruleをunit testする。
  - foreground modalのInteraction/captureは既存LoadConfirm/Help/Settingsのproject patternを再利用する。

## 5. マイルストーン

## M1: Slot model、catalog、format/schema方針

- 着手条件: Track C3とC1が完了し、C1のadditive orderを含むcurrent v1 round-trip fixtureがgreenである。
- 変更内容:
  - typed slot ID、canonical filename、storage root、role/health/capability/revisionを分けたcatalog entry、bounded header inspectorを実装する。
  - current/v0 candidate/corrupt/unsupported/seed mismatch/missing fixtureを追加する。
  - v1 container維持とv2導入条件を`save_load.md`へ固定する。
- 主な変更ファイル:
  - `crates/hw_core/src/save.rs`
  - `crates/bevy_app/src/systems/save/{catalog.rs,format.rs,state.rs,mod.rs}`
  - `docs/save_load.md`
- 完了条件:
  - [ ] 任意path/stringからslotを作れず、範囲外IDをrejectする。
  - [ ] scan entry数とheader read bytesが上限内に収まる。
  - [ ] bodyをdeserializeせず全header statusを分類し、magicless fileをvalid v0と断定しない。
  - [ ] `can_manual_save/can_scheduler_save/can_load`とcontent statusが直交し、legacy v0 candidateのseedを誤分類しない。
  - [ ] relative mtime label更新でfilesystem scanせず、metadata/read failureにplayer-safe fallbackが出る。
  - [ ] legacy default fileが存在する時だけentryを追加し、変更せずload候補に表示する。
- 検証:
  - `cargo test -p hw_core save_slot`
  - `cargo test -p bevy_app@0.1.0 --lib systems::save::catalog`
  - `cargo test -p bevy_app@0.1.0 --lib systems::save::format`

## M2: Manual slot transactionとcatalog modal

- 変更内容:
  - origin/dialog session/slot/revisionを束縛したone-shot requestをsave/load dispatcherへ接続する。
  - revision-bound overwrite recheck、atomic no-replace/replace、file+directory sync、streaming header/body、
    slot label outcome、catalog dirty/refreshを追加する。
  - Save/Load catalog、selection、confirm、Esc/capture、Pause/F5/F9導線を実装する。
  - missing/corrupt/unsupported/seed mismatchを操作前にdisableし、load試行時のbody invalidをterminal notificationへ流す。
  - C3の`SaveRecoveryMode`/recovery-only replaceをcatalogへ接続し、RecoveryFailed中の入力allow-listと再ロードを実装する。
- 主な変更ファイル:
  - `crates/bevy_app/src/systems/save/{state.rs,saving.rs,load.rs,format.rs,catalog.rs,atomic_file.rs,mod.rs}`
  - `crates/hw_ui/src/{intents.rs,setup/pause_menu.rs,setup/dialogs.rs,interaction/}`
  - `crates/bevy_app/src/interface/ui/interaction/handlers/save_game.rs`
  - `crates/bevy_app/src/interface/ui/notifications.rs`
  - `crates/bevy_app/src/input_actions/`
- 完了条件:
  - [ ] Empty save、confirmed overwrite、loadが選択slotだけを操作する。
  - [ ] paused状態のowning Save catalog/confirmからmanual saveでき、別modal/stale dialog sessionからはrejectされる。
  - [ ] UI表示後からcommit直前までにfileが出現してもAbsent saveはno-replace失敗し、未確認overwriteしない。
  - [ ] confirmation後にrevisionが変わったExact saveは再確認を要求する。
  - [ ] existing targetのstable revision取得不能時は`Absent`へ降格せず、manual/autosaveとも非変更failureになる。
  - [ ] temp fileはfile sync後だけcommitされ、対応platformではdirectory syncされ、catalogはcrash tempを列挙しない。
  - [ ] directory syncだけのpost-commit failureは`CommittedDurabilityUncertain`となり、未保存と誤表示しない。
  - [ ] pre-transaction failureではcurrent world不変、ApplyRecoveredではrollback復旧、RecoveryFailedではpaused fail-closedになる。
  - [ ] RecoveryFailed中はsave/autosave/resume/world commit不可で、別slot load/quitだけが可能。valid recovery-only load成功時だけ解除され、resumeは再許可されるが自動実行されない。
  - [ ] recovery-only loadのpreflight/apply再失敗ではfail-closedを維持し、さらに別slotを試せる。
  - [ ] 全load failureでtargetを含む全slot fileは不変である。
  - [ ] modal open request frameからworld pointer/camera/background UIがcaptureされる。
  - [ ] terminal outcomeとnotificationがslot label/operation/resultを正しく示す。
- 検証:
  - `cargo test -p bevy_app@0.1.0 --lib systems::save`
  - `cargo test -p hw_ui save_catalog`
  - `cargo test -p bevy_app@0.1.0 save_catalog`

## M3: Autosave settings、scheduler、generation rotation

- 変更内容:
  - user-local settingsと互換default、UI controls、active-play timerを追加する。
  - context-aware eligibility、Interface後のmanual優先arbiter、single pending、mtime/round-robin rotation、
    全terminal failureのbackoff、load後resetを実装する。
  - clockを注入可能なpure schedulerへ分離し、長時間sleepなしでrotationをtestする。
- 主な変更ファイル:
  - `crates/bevy_app/src/systems/settings/`
  - `crates/bevy_app/src/systems/save/{autosave.rs,catalog.rs,state.rs,mod.rs}`
  - `crates/hw_ui/src/setup/settings_panel.rs`
  - `crates/bevy_app/src/interface/ui/interaction/handlers/settings.rs`
  - `docs/settings.md`
- 完了条件:
  - [ ] default off、10分、3世代が旧settings.ronへ補完される。
  - [ ] paused/modal/manual pending/AreaEdit/drag/world replacement中はrequestを発行しない。
  - [ ] owning Save catalog中のmanual saveは許可される一方、同じframeのautosaveはmanual requestを上書きしない。
  - [ ] 満了中のineligible期間が長くてもdueは1件だけで、eligible復帰後に1回だけ発行する。
  - [ ] 5→10→20→30分と1〜5世代の全境界が正規化される。
  - [ ] generation上限を超えず、全mtime可ならoldest+ID、mtime欠落ならcommit成功時advanceのround-robinでstable rotationになる。
  - [ ] generation縮小で既存fileを削除せずinactive load-onlyとして保持する。
  - [ ] revision/serialize/write/sync/commitのどのaccepted failureでも毎frame retryせず、eligibility延期だけdueを保持する。
  - [ ] autosave成功/試行失敗/manual成功/committed-uncertain/load成功のtimer消費と通知retentionが契約どおりである。
- 検証:
  - `cargo test -p bevy_app@0.1.0 autosave`
  - `cargo test -p bevy_app@0.1.0 settings`

## M4: 性能gate、互換fixture、Help、恒久docs

- 変更内容:
  - save phase timing、body bytes、peak RSS deltaをbounded metricsへ追加し、small/medium/largeを測定する。
  - manual/autosave/legacy v0/v1、B1〜B3 durable values、C1 DeconstructionOrder、corrupt/future/seed mismatch、
    rollback/recovery-onlyを横断確認する。
  - Help impact reviewと恒久docsを同期する。
- 主な変更ファイル:
  - `crates/bevy_app/src/systems/save/{saving.rs,metrics.rs}`
  - `scripts/perf.py`（既存runnerへ最小のsave workloadを追加する場合のみ）
  - `crates/bevy_app/src/interface/ui/help_content/`
  - `docs/{save_load.md,settings.md,state.md,notifications.md,help-screen.md,architecture.md,events.md,invariants.md}`
- 完了条件:
  - [ ] timing artifactがbody/pathを保持せず、serialize/file sync/commit+directory sync/totalとbody bytes/RSSを比較できる。
  - [ ] serialized body以外のfull-size container bufferがなく、I/O bufferが64 KiB以下であることをstructural testで固定する。
  - [ ] 3 warmup + 20 sampleとnearest-rank p95が固定コマンドで再現できる。
  - [ ] largeでp95 100ms/max 250msを満たす。超過時はC2を完了扱いにせずsnapshot follow-upを作りautosaveをblockedにする。
  - [ ] threshold結果にかかわらず初期値offを維持し、default変更を性能測定へ暗黙連動させない。
  - [ ] legacy/current/invalid fixtureのplayer-visible statusとterminal resultが一致する。
  - [ ] Help provider/manifest/coverage/exact snapshotが新workflowを説明する。
- 検証:
  - 上記§4.7のsave performance workload
  - `python3 scripts/check_help_impact.py`

## M5: Native acceptance、full gate、archive

- 変更内容:
  - no-prompt native acceptanceでmodal表示/capture/manual workflow/catalog statusを確認する。
  - autosave rotationはclock-injected integration testを正本とし、実機で10分待つ受入は行わない。
  - full workspace gate、docs index、計画archive、親提案進捗を同期する。
- 完了条件:
  - [ ] native V1〜V5が合格する。
  - [ ] full workspace gateが成功する。
  - [ ] C2計画をarchiveし、Track C親提案を更新する。
- 検証:
  - `python3 scripts/dev.py verify`
  - `python3 scripts/check_help_impact.py`
  - `git diff --check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| slot selectionとrequestが別 | 誤slot上書き/load | operation+slotをimmutable one-shot requestへ統合 |
| UI snapshotを信用 | TOCTOUで未確認overwrite | request-bound revision、Last再検査、AbsentのOS no-replace commit |
| no-replace非対応で通常renameへfallback | empty slotへ現れたfileを上書き | safe I/O failureとし置換renameを禁止 |
| catalogがbodyを読む | modal openが重い/メモリ増 | bounded header prefixだけを読むcounter test |
| raw path/nameをUIへ渡す | path traversal/情報漏洩 | typed ID→root canonical filename、player-safe label |
| autosaveとmanualが競合 | 二重snapshot/通知 | manual優先、pending最大1、queueなし |
| foreground modalを一律ineligibleにする | Save catalogからmanual saveが永久に実行不能 | 共通base+origin contextに分けowning dialog sessionだけ許可 |
| autosave成功をImportantへ記録 | 定期toast/history spam | successはslot dedupe付きToastOnly、failureだけImportant |
| pause中もtimer進行 | 操作中に意図しないsave | active real timeだけ加算しmodal/pauseでgate |
| 未確定gesture中のautosave | area selectionの途中状態を意図せずsnapshot | manualと共通のbase eligibilityをrequest/applyの両方で再検査 |
| generation縮小でsaveを暗黙削除 | 復旧世代を意図せず失う | inactive load-only entryとして保持し、自動削除しない |
| 同期saveが長い | frame freeze | phase計測、default off、閾値超過はsnapshot先行follow-up |
| header+bodyを2本目のStringへ連結 | large saveで一時メモリがほぼ倍増 | serialized bodyをheaderと別々にstream writeしbuffer上限をtest |
| serialize失敗だけtimerを消費しない | autosaveが毎frame再試行する | accepted autosaveの全terminal failureをfull-interval backoffへ統一 |
| mtime欠落時に常に最小IDを選ぶ | autosave-1だけが上書きされる | 全mtime信頼時だけoldest、それ以外はruntime round-robin |
| rename後のdirectory sync失敗を通常failure表示 | 実際はfile更新済みなのに未保存と誤認 | committed-but-uncertain outcomeとauthoritative catalog refresh |
| RecoveryFailedで通常rollback候補を要求 | 壊れたlive worldからsnapshotできず再ロード不能 | C3の専用recovery-only modeをLoad catalogからだけ使用 |
| metadataのためv1 headerを場当たり拡張 | format責務が曖昧 | 初版はfs metadata、v2条件を先に文書化 |
| corrupt slotを一覧から消す | 復旧判断不能 | error entryを保持しload不可/再試行状態を明示 |

## 7. 検証計画

- unit/integration:
  - typed ID/path mapping、bounded scan、optional legacy、mtime/round-robin ordering、all status classification。
  - request target race、revision recheck、atomic no-replace/replace、file/directory sync、crash temp ignore、
    missing/read/format/schema/rollback/recovery-only failure。
  - dialog session ownership、paused manual save、capture/Escape/world replacement/recovery mode reset。
  - autosave clock、manual precedence、rotation、generation変更、全failureのtimer consumption、notification retention、failure retry。
  - v0/v1、B1〜B3 durable value、C1 mid-order round-trip。
- native acceptance（実装時は `hell-workers-run-native-acceptance` のno-prompt launcherを使用）:
  - V1: F5/Pause Save catalog、empty slot、save、occupied slot overwrite confirm。
  - V2: F9/Pause Load catalog、slot選択、load confirm、異なるmanual slotのround-trip。
  - V3: legacy/corrupt/unsupported/seed mismatchのlabel、disabled状態、通知。
  - V4: modal open request frameから背景pointer/camera/UIが動かず、Escがconfirm→catalogを順に閉じる。
  - V5: pre-transaction failureではcurrent world/selection/historyを維持し、ApplyRecoveredではentity-bound UIをreset、
    RecoveryFailedではsave/resume/world操作不能かつLoad catalog/quitだけになり、別slot成功後に明示resume可能、再失敗で
    fail-closedを維持することを確認する。
- autosave:
  - generation rotationはfake clock/temp directoryで自動確認し、実時間待機を受入項目にしない。
  - actual windowではSettings表示と変更反映、1回の明示triggerによるstatus更新だけを確認する。
- 性能:
  - catalog scan bytes/count。
  - serialize / file sync / commit+directory sync / total、body bytes、peak RSS deltaのsmall/medium/large測定。
  - full-size container bufferがserialized bodyの1本だけで、I/O bufferが64 KiB以下であるstructural check。
  - steady stateでdirectory scan/save timing処理0。
- 完了時:
  - `cargo fmt --all -- --check`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `python3 scripts/dev.py verify`

## 8. ロールバック方針

- M1 catalog model、M2 manual UI/transaction、M3 autosave、M4 metrics/docsを独立commitにする。
- manual slot filesは全て現行v1 bodyのため、C2 UIを戻してもdata自体は専用pathに残る。
  旧単一実装で読む場合はfileを自動移動せず、明示import手順を用意する。
- legacy `world.scn.ron` はC2から上書き/削除しないため、roll backしても既存saveを維持する。
- autosaveに問題があればsetting default/featureをoffへ戻し、manual catalogを維持できる。
- v2 migrationはC2初版で開始しないため、format rollbackを伴わない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1〜M5
- 前提状態: Track C3とC1は完了・archive済み。C2はM1から開始可能。

### 次のAIが最初にやること

1. M1でtyped slot/path mappingとbounded catalog fixtureだけを実装し、UIへ進む前にstatus分類を固定する。
2. M2でrequestへslotとAbsent/Exact revisionを埋め込み、単一`SavePath`差替え方式を残さない。
3. C1のdurable `DeconstructionOrder`、runtime task cleanup、C3のrehydrate契約をsave/load回帰対象として維持する。

### ブロッカー/注意点

- UI選択ResourceとSavePathを別々に読んでrequest targetを決めない。
- Absent saveを通常renameでcommitしない。no-replace非対応時はfail-closedにする。
- confirmed overwriteはconfirmation時revisionをrequestへ束縛する。
- manual saveをforeground modal一律禁止にしない。owning dialog sessionだけをeligibility例外として許可する。
- catalog scanでDynamicWorld bodyをdeserializeしない。
- absolute path/raw OS errorをnotificationへ出さない。
- legacy default fileを自動移動・上書き・削除しない。
- autosaveをbackground化する前にimmutable snapshot境界を設計する。
- autosave成功をImportant historyへ蓄積しない。
- `RecoveryFailed`から通常transactionを再利用せず、C3 recovery-only mode以外でlive resetしない。
- headerとbodyを結合した2本目のfull-size Stringを作らない。
- production変更後は必ず `hell-workers-review-help-impact` Skillの判断を完了する。

### 参照必須ファイル

- `docs/save_load.md`
- `docs/settings.md`
- `docs/notifications.md`
- `docs/plans/archive/save-load-hardening-plan-2026-07-12.md`
- `docs/plans/archive/save-rehydration-registry-plan-2026-08-03.md`
- `crates/bevy_app/src/systems/save/`
- `crates/bevy_app/src/interface/ui/interaction/handlers/save_game.rs`
- `crates/hw_ui/src/{intents.rs,setup/pause_menu.rs,setup/dialogs.rs}`

### 最終確認ログ

- 最終 `cargo check --workspace`: `未実施（計画作成のみ）`
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: `未実施（計画作成のみ）`
- 最終 `cargo test --workspace`: `未実施（計画作成のみ）`
- 未解決エラー: `N/A`

### Definition of Done

- [ ] M1〜M5が完了
- [ ] manual/legacy/autosave slotの全statusとterminal resultが自動確認済み
- [ ] Absent no-clobberとExact revision再確認が競合fixtureで保証される
- [ ] pre-transaction/ApplyRecovered/RecoveryFailedのworld・UI終端が区別される
- [ ] RecoveryFailedから別slotの成功でのみ復帰し、再失敗中はsave/resume/world操作がfail-closedである
- [ ] autosave generationとmanual precedenceが固定される
- [ ] accepted autosave全failureのbackoffとmtime欠落時round-robinが固定される
- [ ] save性能・メモリgateを満たし、default offを独立判断として維持する
- [ ] Help/docs/native V1〜V5が完了
- [ ] `python3 scripts/dev.py verify`が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-03` | `Codex` | role/health/capability分離、revision-bound no-clobber save、失敗種別別UI終端、共通SaveEligibility、active-time autosave、再現可能な性能gate、v1 additive方針をC2実装契約として確定 |
| `2026-08-03` | `Codex` | 自己レビューでorigin別eligibility、autosave Absent/Exact、file+directory sync、streaming memory境界、mtime fallback、全failure backoff、RecoveryFailed再ロード経路へ修正 |
| `2026-08-04` | `Codex` | 前提Track C3の実装・archive完了を反映。C2はDraft/0%とC1完了待ちを維持 |
| `2026-08-08` | `Codex` | 前提Track C1のM1〜M5完了・archiveを反映。C2はDraft/0%のままM1から開始可能へ更新 |
