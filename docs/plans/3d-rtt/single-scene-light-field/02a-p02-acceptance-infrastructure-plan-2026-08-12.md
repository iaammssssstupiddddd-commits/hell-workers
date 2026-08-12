# P02-A: TopDown presentation受入基盤計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-02a-p02-acceptance-infrastructure-plan-2026-08-12` |
| ステータス | `In Progress — automated evidence ready, actual-window scenario pending` |
| 作成日 | `2026-08-12` |
| 最終更新日 | `2026-08-12` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 関連計画 | [P00](00-baseline-gates-plan-2026-08-03.md)、[P01](01-single-scene-rtt-plan-2026-08-03.md)、[P02](02-topdown-presentation-plan-2026-08-03.md) |
| 直接依存 | P00 frozen `rtt-light-v1` contractとregistered current / P01 artifact。M2以降はP02の対応するproduction milestoneを入力とする。 |
| 後続 | P02 M5/M6のstage=`p02` evidence・formal/native受入、P06/P08のP02 compatible reference reader |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: P02 formal / native受入を実行できない。
- 背景: P00は`RLV1-P02-DOOR-DOMAIN`、`RLV1-P02-PRESENT`、`RLV1-P02-PERF`とstage=`p02`のexact gate集合を凍結済みだが、実行側のselector、artifact reader / writer、RenderDoc extractor、native launcherは`current`と`p01`までしか実装していない。
- 到達したい状態: P02のproduction subjectが、P00 / P01のhistoryを読み替えずに、Door、presentation、performanceの各gateをfail-closed artifactから再現可能に評価できる。
- 成功指標: `stage=p02`のformal recipeがaudit / behavior / Capture / Memory / RenderDocを正しいschemaで生成し、P01 RTT preservationとP02のexact gate集合を同時に検証できる。pixel / animationの確認は同じproduction fixtureを使う専用actual-windowシナリオで補完する。

## 2. スコープ

### 対象（In Scope）

- `current` / `p01` / `p02`の明示stage selector、stage×schema許可表、historical artifact reader。
- P02 Door / presentation metricのruntime producer、production fixture、behavior timeline、sidecar、bundle / gate extractor。
- P02のScene-only RenderDoc expectation、checkpoint / replay extractor、native acceptance recipe、Skillとadapter mirror。
- production indoor-light fixtureを使うP02 actual-window presentation scenarioと、Door / alpha-depth / foreground / animationのartifact判定。
- material pool cardinality、Soul shadow runtime停止、Render3d visibility toggleなど、frozen v1のgate外にあるP02必須視覚契約のfocused production test。

### 非対象（Out of Scope）

- Door、camera、Building presentation、billboard自体のproduction実装。各featureとそのsemantic sourceは[P02](02-topdown-presentation-plan-2026-08-03.md)が所有する。
- `rtt_light_migration_v1.json`、projection v1、P00 current / P01 registered artifactの値・意味・hashの変更。
- P04以降のfield / lifecycle metric、P06以降のGPU material / color metric。各stage ownerがP02-Aのextension pointを使って追加する。
- P02 formal candidateの性能・視覚不合格を値調整で吸収すること。P02 M6がproduction不合格を修復し、P02-Aはevidenceの成立を所有する。

## 3. 現状とギャップ

| 現状 | ギャップ | P02-Aで閉じること |
| --- | --- | --- |
| Rust / Python / nativeのstage allowlistは`current`と`p01`だけ | `--stage p02`が入力時点で拒否され、formal recipeを構成できない | selector、policy、native parser、self-testを同時に`p02`へ拡張する |
| P00 contractにはP02のgate / metricがある | P02固有metricを出すruntime inventoryとsidecarがない | source→artifact→gate rowを明示し、P02専用sidecarを追加する |
| RenderDoc expectationはcurrent / P01 resource table | P02のScene-only topologyとpresentation evidenceを抽出できない | P02を独立stage entryとして追加し、P01 RTT gateも同じcheckpointで再検証する |
| `visual_test`は独立アプリで2D / 3Dをともにspawnする | productionのexactly-one、Door、MainCamera、foreground、rehydrateを証明しない | game本体のfixtureを使うactual-window scenarioを追加する |
| frozen P02 gateはpool asset数とshadow system登録を直接測らない | pool漏れ / shadow実行がformal gateだけでは逃げる | これらはversion変更なしのfocused production testとしてhard gateにする |

## 4. 受入artifactと所有境界

### 4.1 凍結契約と互換性

- `rtt-light-v1` JSON、projection v1、P00 currentとP01のcanonical artifactは不変である。P02の都合で旧rowを削除、再解釈、default埋めしてはならない。
- `current`、`p01`、`p02`は明示stage tableだけから選ぶ。未実装の`p03`以降、unknown stage、stage / schema組合せ、reference locator欠落、mixed raw artifactはfail-closedで拒否する。
- P02に新しいraw fieldが必要な場合は、legacy CSVの意味を上書きせずP02専用sidecarまたは明示schema revisionへ追加する。readerはcurrent / P01の既存schemaを保持し、new readerで両registered artifactを再検証する。
- P02 formal合格後に初めてP02 referenceを登録する。P06 / P08は登録済みP02 referenceを読むconsumerであり、P02-Aがfuture stageを先回りしてallowlist化しない。

### 4.2 metric-to-evidence表

| P00 metric | production source | raw evidence / gate入力 | focused・negative検証 |
| --- | --- | --- | --- |
| `auto_attempted` / `auto_applied` | production root Doorとauto proximity | `door-state-v1` behavior timeline | rootにSpriteがないshellでauto transitionを壊す |
| `manual_attempted` / `manual_applied` | production root Doorとmanual intent | 同timelineのmanual-lock step | paused / invalid owner / state mismatchを個別に壊す |
| `mutation_requires_root_sprite` | `apply_door_state`のdomain inputとfixture topology | production-shell source assertion | root Sprite依存を再導入したfixtureを拒否する |
| `active_presentation_matches_semantic` | Door root、active child / `Door3dVisual` | presentation sync後のsnapshot | Closed / Open / Lockedのいずれかを不一致にして拒否する |
| `layer_2d_camera_count` / `layer_2d_pass_count` | actual camera / RenderLayers inventory | medium / gpu RenderDoc checkpoint | cameraまたはpassを1つ増やして拒否する |
| `duplicate_presentation_count` / `building_exactly_one_presentation` | Building ownerごとのactive 2D / 3D consumer分類 | P02 presentation sidecar、同checkpoint locator | 2D / 3D併存、presentation 0を各々拒否する |
| `soul_billboard_per_soul` | Soul owner数と`ActorBillboard3d` owner数 | numerator / denominatorを記録したsidecar | billboard欠落・二重spawnを拒否する |
| `familiar_3d_count` | 3D proxy marker query | scene root / presentation sidecar | proxyを1つ残したfixtureを拒否する |
| `state_and_bounce_probes_pass` | Tank / MudMixer state resolver、active presentation bounce | stable enum / markerを使うfixture probe | state / bounceのどちらかを外して拒否する |
| P02 p95 / p99 | P00 Capture all case | P01 compatible referenceとのgate row | reference stage / case / repeatを取り違えたbundleを拒否する |

`state_and_bounce_probes_pass`はmaterial type名に依存させない。P02はsemantic stateとactive presentationを、P06は最終`TopDownStructuralMaterial`を所有するためである。

### 4.3 観測境界とvisual補完

- Door metricのsnapshotは、P02が定義する`DoorPresentationSyncSet`相当のconsumer完了後に採る。mutation直後のpre-visual stateを成功証拠にしない。P04がmanual requestをN+1 pre-Visualへ移しても、このconsumer→observation境界を変えない。
- actual-window scenarioはproduction indoor-light fixtureを使い、固定camera、Wall前 / 後のSoul、alpha edge、selection / speech / effect、Closed / Open / Locked Door、Tank / MudMixer state / bounce、Familiar foreground、Render3d visible / hiddenを同じsubjectで採る。
- Door Openは単なる正方形primitiveの90度yawで済ませない。P02が非対称leaf / state別visualとorientation・pivotを定義し、actual-window artifactでClosedとの識別可能性を判定する。
- material pool cardinality、spawn / load / despawn後のasset増加なし、Soul shadow spawn / observer / projector registration 0、billboardを含むRender3d scene toggleはfocused production testsで検証する。これらはfrozen v1を変更しない補助hard gateである。

## 5. マイルストーン

## M1: `p02` selector・schema互換・historical readerを閉じる

### 変更内容

1. Rust `PerfRttLightSelection`、Python arguments / policy、native launcherを`current` / `p01` / `p02`の明示allowlistにする。
2. stage×schema許可表を1か所に集約し、current / P01 raw artifactを新readerで再検証する。
3. P02の未実装metricをcurrent / P01へfallbackさせず、P02 featureなしのsubjectは明確なmissing-evidenceで失格にする。
4. native `rtt-light` recipe、Skill正本、adapter mirrorをP02 stage選択へ同期する。P02 formal採取はまだ実行せず、recipe / self-testの成立だけを閉じる。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/perf_scenario/{config.rs,config/tests.rs}`
- `scripts/perf_tool/{arguments.py,policy.py,artifacts.py,fixtures.py,model.py}`とself-test / negative fixture
- `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`
- `.cursor/skills/hell-workers-run-native-acceptance/SKILL.md`、Codex / Gemini / Claude adapter mirror
- 実装後の`docs/performance-profiling.md`

### 完了条件

- [ ] `current`、`p01`、`p02`だけがstage selectorで受理される
- [ ] current / P01のregistered artifactをnew readerで再検証できる
- [ ] cross-stage、schema混在、missing reference、unknown lane / stageの各negative testがfail-closedになる
- [ ] native S1 recipeとformal recipeが`--stage p02`を構成できるが、P02 formal合格を偽装しない

### 検証

- `python3 scripts/dev.py check`
- `python3 scripts/dev.py cargo -- test --workspace`
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`

## M2: P02 Door / presentation evidence producerを接続する

### 変更内容

1. P02専用presentation inventory / sidecarを追加し、legacy `render_inventory.csv`の意味を変更しない。
2. production `attach_building_shell`、normal completion、rehydrate、initial spawn、fixture routeを同じpresentation分類で記録する。synthetic `Door + Sprite`だけをgate入力にする経路を禁止する。
3. behavior driverをDoor mutation→presentation sync→observationの順に固定し、Door six metricをtimelineから導出する。
4. camera / pass、duplicate / exactly-one、billboard ratio、Familiar proxy、state / bounceを同一medium / gpu checkpoint locatorへ束縛する。
5. Soul shadow runtime-zero、material pool、Render3d visibilityはP02 production focused testsとして併走させる。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/perf_scenario/{audit_checksum.rs,output.rs,renderdoc_capture.rs,behavior_driver.rs,workload_driver.rs,indoor_light_fixture.rs}`
- `crates/bevy_app/src/plugins/startup/perf_scenario.rs`
- `scripts/perf_tool/{artifacts.py,fixtures.py,model.py,rtt_light_bundle.py}`とself-test / negative fixture
- P02のDoor / presentation focused test owner

### 完了条件

- [ ] §4.2の全P02 metricにsource、raw field、checkpoint locator、positive / negative testがある
- [ ] Door gateはroot Door・root Spriteなし・active presentation consumerありのproduction topologyで採る
- [ ] all-building fixtureはP02 mapping / presentation routeと一致し、new gameとloadで別の期待形を持たない
- [ ] P02 metricのunknown / duplicated / missing evidenceがbundle validationで失格になる
- [ ] pool / shadow / Render3d toggleのfocused production testsがP02 feature不成立を検出する

### 検証

- P02 Door、presentation mapping、billboard / shadow、pool focused tests
- `python3 scripts/dev.py check`
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
- `python3 scripts/dev.py cargo -- test --workspace`
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`

## M3: RenderDoc・bundle・actual-window native recipeを閉じる

### 変更内容

1. P02 RenderDoc resource / checkpoint expectationをP01と別stage entryとして定義する。Scene-only topologyはP01 RTT gateを継続して検証する。
2. extractor / bundleに`RLV1-P02-DOOR-DOMAIN`、`RLV1-P02-PRESENT`、`RLV1-P02-PERF`のexact rowを実装する。P02ではfield-core / consumer-coreを実行・出力ともに拒否する。
3. `plan-rtt-light`、artifact validator、native S1 / formal dry-run self-testをP02に対応させ、source / binary / adapter / environment / referenceをfail-closedで照合する。
4. production fixtureを使う`p02-presentation-actual-window-v1`相当のscenarioを追加し、§4.3の固定状態、window/client ownership、screenshot / observation artifact、否定条件を定義する。
5. P02 actual-window scenarioはgeneric `visual_test`、headless run、root desktop screenshotを代替証拠にしない。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/perf_scenario/renderdoc_capture.rs`
- `scripts/perf_tool/{renderdoc_capture.py,renderdoc_extract.py,rtt_light_bundle.py,artifacts.py}`とself-test / negative fixture
- `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`
- `.cursor/skills/hell-workers-run-native-acceptance/SKILL.md`とadapter mirror
- 実装後の`docs/{performance-profiling.md,rendering-performance.md,visual_test.md}`

### 完了条件

- [ ] P02 checkpointはP01 RTT preservationとP02 presentation evidenceをともに抽出する
- [ ] P02 gate CSVはrequired row過不足、unknown metric、reference mismatch、schema混在をfail-closedにする
- [ ] native helper self-testはP02 S1 / formal command、exact gate set、current / P01 / P02 cross-read negativeを検証する
- [ ] actual-window scenarioはproduction fixtureのexpected observationをartifactで再検証できる
- [ ] actual-window scenarioをheadless / visual_test / root desktopへ置換すると不合格になる

### 検証

- RenderDoc runtime / replay / extractor / bundle positive・negative self-test
- `python3 scripts/dev.py check`
- `python3 scripts/dev.py cargo -- test --workspace`
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`

## M4: P02 formal readinessを再検証してP02 M6へhandoffする

### 変更内容

1. M1〜M3のself-test matrixをまとめ、current / P01 historical reader、P02 fixture / behavior / RenderDoc / bundle / native recipeを同一source revisionで再検証する。
2. P02 M1〜M4のproduction feature subjectを使い、formal recipeが全required legとexact gate rowを生成できることを確認する。actual formal attemptの採取と合否はP02 M6が所有する。
3. P02 M6、P06、P08が利用するP02 artifact contract、reference登録条件、failure triage（source / artifact / renderer / performanceの切分け）を記録する。
4. 実装された永続的な受入手順をperformance / rendering docsとSkillに同期する。player-visible変更を含むimplementation batchではHelp impact reviewを実際の経路から完了する。

### 完了条件

- [ ] P00 contract JSON / SHAとprojection v1が不変である
- [ ] registered current / P01 artifactがnew toolchainで再検証できる
- [ ] P02の各metricを壊すnegative fixtureが対応gateを必ず落とす
- [ ] P02 full formalを実行するための入力、command、artifact validator、failure判定が揃う
- [ ] P02 M6はこの計画のready stateを前提にのみformal / native candidateを採取する

### 検証

- `python3 scripts/dev.py check`
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
- `python3 scripts/dev.py cargo -- test --workspace`
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`
- `python3 scripts/dev.py verify`
- `git diff --check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| P02の都合でP00 / P01 artifactを読み替える | historical baselineが失われる | stage×schema許可表、registered artifact再検証、cross-stage negativeを必須にする |
| P02 metricがcomponent countだけでsemantic状態を見ない | Door / state visualの偽陽性 | production root fixtureとconsumer後snapshotをgate入力にする |
| RenderDoc resource形状がP01と同じためP02 entryを省く | presentation evidenceが採れない | topologyが同じでもP02 stage entryとmetric locatorを独立して持つ |
| visual_testをproduction証拠に流用する | exactly-one / camera / reload不具合を見逃す | actual-window scenarioはgame本体fixtureのみを受理する |
| P02 formalでRSSをhard gateと誤認する | frozen contractと矛盾する | P02はp95 / p99だけをhard gateとし、RSSは診断値。閾値追加はv2と双方rebaselineを要する |
| P04 / P06のfuture metricまでP02-Aが抱える | ownershipとstage拡張が曖昧になる | P02-Aはextension pointのみ、各future stage固有のproducer / gateを各計画が所有する |

## 7. 検証計画

- M1〜M4の各完了時にfocused / negative tests、`python3 scripts/dev.py check`、Rust変更時のClippyとworkspace testを実行する。
- `scripts/perf.py self-test`はselector、schema reader、fixture、behavior、RenderDoc extractor、bundle、native command builderを含む。
- P02 M6のactual formalは`hell-workers-run-native-acceptance` Skillのno-prompt launcherだけを使う。headless audit、dry-run、software renderer、RenderDoc screenshotだけをactual-window / GPU / allocator evidenceにしない。
- player-visible production変更を含む実装batchの完了前に、repository `hell-workers-review-help-impact` SkillでUpdate required / No impactを実経路から決定する。

## 8. ロールバック方針

- P02 selector / readerを戻す場合もregistered current / P01 readerは維持し、P02 raw artifactをcurrent / P01として受理する曖昧なfallbackを残さない。
- P02 production featureのrollbackはP02側の独立commitで行い、frozen contract、baseline index、P00 / P01 artifactを改変しない。
- formal不合格時はgate row、raw evidence、runtime source、RenderDoc topology、actual-window observationの順に切り分け、candidate結果でcontract閾値を緩めない。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `80%`
- 完了済み: M1 selector / historical reader、M2 P02 sidecar / Door timeline、M3 RenderDoc checkpoint / exact bundle gate、native `p02` S1 / formal command生成とnegative self-test。
- 未完了: production fixtureを使う専用actual-window matrixと、そのartifact validator。これを閉じるまでM3 / M4およびP02 M6を完了扱いにしない。

### 次のAIが最初にやること

1. clean committed P02 subjectを確定する。
2. production indoor-light fixtureのHigh / Medium / Low × DPI 1.0 / 1.5 / 2.0 × Render3d visible / hidden actual-window artifactを採取・検証する。
3. S0 / S1 prerequisite後にP02 formalを採取し、P02 M6へhandoffする。

### ブロッカー / 注意点

- P02 full formalをP02 M4 production feature前に採取しない。
- P04はP02 M1 Door correctnessだけを直接依存とする。P02-A full formalをP04の新規entry blockerにしない。
- P06はP02 formal artifactが登録されるまでP02 compatible performance referenceを要求しない。

### 最終確認ログ

- 最終 docs gate: `2026-08-12` / `docs --write / check pass`
- 最終 Rust / native gate: `2026-08-12` / `verify pass。S1 valid: Intel Arc (MTL) / Vulkan / X11、Audit 3/3、Capture 18/18、Memory 18/18、source unchanged。formalはclean committed subject待ち`

### Definition of Done

- [ ] M1〜M4の受入基盤が完了
- [ ] current / P01 historyを再検証し、P02 selector / schema mismatchをfail-closedにする
- [ ] P02 Door / presentation / perf metricのsource-to-gate対応とnegative testsが揃う
- [ ] P02 actual-window scenarioがproduction fixtureで再検証できる
- [ ] P02 M6がformal/native candidateを採取できるready stateへhandoff済み
- [ ] 実装に追従する性能・描画・Skill docsが更新済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-12` | `Codex` | 修正後S1を再実行し、Audit / actual-window Capture / native Memoryの全legをvalid確認 |
| `2026-08-12` | `Codex` | S1 CaptureでOpen Door presentationの同frame検証とP02 GPU legacy proxyの旧期待値を検出。fixture settle境界とstage-aware validatorを修正 |
| `2026-08-12` | `Codex` | S1初回auditでstatic Door mutationとfixed-step P02 sidecar不許可を検出。producer/schedule境界を修正してfocused audit 3/3 validを確認 |
| `2026-08-12` | `Codex` | production fixtureのP02 presentation topologyとDoor / load behavior（各3回）をheadless artifactでvalid確認 |
| `2026-08-12` | `Codex` | P02 selector、sidecar、Door / presentation / perf gate、RenderDoc / bundle / native command生成を実装。actual-window scenarioを残課題として明示 |
| `2026-08-12` | `Codex` | P02 reviewで判明したstage=`p02` formal / native受入基盤の不足を独立計画として作成 |
