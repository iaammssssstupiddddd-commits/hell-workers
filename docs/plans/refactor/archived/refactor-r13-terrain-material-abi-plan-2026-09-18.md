# R13: Terrain materialのuniform・binding宣言の共有 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r13-terrain-material-abi-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R13 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P3 / 中〜大 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 共通WGSL型・binding宣言、Rust base material helper、RenderDoc import/snapshot転送と実descriptor検証を実装。地形・P08の必要受入と終了整理を完了。 現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: LOD別Rust extensionと4 shaderに重複するuniform/binding宣言の同時更新負担を減らす。
- 到達したい状態・成功指標: uniform layout・binding番号・importが一貫し、3 LODとprepassの描画およびresize再bindが不変で、Indoor Lightの現行sampling停止を維持する。

## 2. スコープ

### 対象（In Scope）

- Terrain ABI宣言の共通WGSL moduleとRust側descriptor構築
- source契約テスト・fingerprintと現行production GPU受入

### 非対象（Out of Scope）

- LOD別sampling/blendの統合、shader全体の分岐化
- Indoor Light sampling再有効化やbinding削除
- 歴史的性能baseline再構築、全DPI/performance/Memory matrix

### 主な変更対象（現行ファイル）

- [crates/hw_visual/src/material/terrain_surface_material.rs](../../../../crates/hw_visual/src/material/terrain_surface_material.rs)
- [assets/shaders/terrain_surface_material.wgsl](../../../../assets/shaders/terrain_surface_material.wgsl)
- [assets/shaders/terrain_surface_material_lod1_lite.wgsl](../../../../assets/shaders/terrain_surface_material_lod1_lite.wgsl)
- [assets/shaders/terrain_surface_material_lod2.wgsl](../../../../assets/shaders/terrain_surface_material_lod2.wgsl)
- [assets/shaders/terrain_surface_material_prepass.wgsl](../../../../assets/shaders/terrain_surface_material_prepass.wgsl)
- [crates/bevy_app/src/plugins/startup/perf_scenario/renderdoc_capture.rs](../../../../crates/bevy_app/src/plugins/startup/perf_scenario/renderdoc_capture.rs)
- [crates/bevy_app/src/plugins/startup/visual_handles.rs](../../../../crates/bevy_app/src/plugins/startup/visual_handles.rs)（3 extensionの初期構築）

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

TerrainSurfaceUniformsはfull/lod1_lite/lod2/prepassに重複し、Rustの3 extensionにもfield/bindingが重複する。既存テストはIndoor Light binding維持・sampling停止、旧soul projector/section cutの除去を検証している。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/hw_visual/src/material/terrain_surface_material.rs](../../../../crates/hw_visual/src/material/terrain_surface_material.rs)
- [crates/bevy_app/src/plugins/startup/perf_scenario/renderdoc_capture.rs](../../../../crates/bevy_app/src/plugins/startup/perf_scenario/renderdoc_capture.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- 最初にuniform field型/順序/align/size、binding番号、visibility、各shader importとevidence fingerprintの対応表を作る。既存ABIを変更しない。
- ABI宣言だけを共通WGSL importへ移し、LOD固有shader/MaterialExtension型を維持する。Rust側は同値のbase material構築だけを小さいhelperへ寄せ、大きなmacroを作らない。
- prepassは共通uniform型だけをimportし、binding 100の宣言を維持する。本passのtexture/sampler群を持ち込まない。Rustの3種類のAsBindGroup型とextension構築入力は維持する。
- RenderDocReceiverShadersのload配列と現在2個固定のimport snapshot、render側pipeline登録へ新WGSL moduleを追加する。ロード完了とsnapshot転送までを同じ変更単位にし、文字列上のimport変更だけで完了しない。
- 独立verifierがproductionと同じ期待値生成を使わないようにする。ソース配置移動で壊れる文字列検査は意味の契約へ直すが、sampling禁止等の失敗検出を弱めない。
- 新しいimport/ShaderType/AsBindGroup APIを使う前に既存0.19実装またはdocsrs/local registryで確認する。load/bind/resizeのschedule所有は維持する。

### 具体設計と変更境界

| 配置・API（新規名は案） | 共有するもの / 保持するもの |
| --- | --- |
| terrain_surface_types.wgsl | TerrainSurfaceUniformsの型宣言だけを置く。field順序/型/ABIを保持。 |
| terrain_surface_bindings.wgsl | 本passのbinding 100〜134の宣言を置き、full/lod1_lite/lod2からimport。sampling/blend処理は各shaderに残す。 |
| prepass | typesだけimportし、自身のbinding 100を保持。本passのtexture/sampler宣言を含めない。 |
| terrain_base_material() -> StandardMaterial | 3つのmake関数の同値base構築だけを共有。3 AsBindGroup型・全field・rootのLOD別None/Someを保持。 |
| RenderDocReceiverShaders / StableRenderDocCheckpoint | 新moduleをload一覧へ追加。receiver_import_shadersを固定2要素から一覧へ移す案とし、ロード完了→snapshot→render側set_shaderまで同時移行。 |

現行uniformの基準はalign16、size160、LUT開始offset32、ready80、shadow params96、indoor params144。M1でRust metadataとWGSLから再確認し、固定期待表へ記録する。
fragmentの4 receiver契約とIndoor Light bindingは維持する。sampling停止や旧projector/section cut除去のtestを弱めない。
binding維持はRust material layoutとGPU descriptorの契約であり、停止中の画像がshader reflectionや`onlyUsed`集合に含まれることを要求しない。

### 実装単位と移行順

1. **R13-A / M1:** ABI/binding表と不正fixture、既存RenderDoc import依存を固定。
2. **R13-B / M2:** 新2module・3本pass/prepass import・Rust base helper・RenderDoc転送経路を一つの整合した差分として移行。
3. **R13-C / M3:** 既存LOD probe/pipeline residencyでcompile/bindingを確認し、現行P08 closureへ不足する可視LOD/prepass/resizeだけ補う。過去の全性能matrixを再実行する計画に拡大しない。

### 依存関係と着手順

先行計画なし。R14のconfig/RenderDoc契約編集と同時に進めない。R12とhw_visual登録を変更する場合も直列化。受入batch中はRust/shader/harnessを凍結する。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: ABIと証拠依存を固定

- 変更内容:
  - 3 extensionと4 shaderの対応表、source hash consumerを棚卸しする。
  - 既存Light Field sampling停止・binding・shadow/style契約の回帰を確認する。
- 変更ファイル: R13-A: hw_visual material/terrain_surface_material.rs test、4 WGSL、既存shader契約test、renderdoc_capture.rs のsnapshot調査。
- 完了条件:
  - [x] 移す宣言と維持する値/意味、更新が必要なverifierが明確。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: 宣言と構築の局所共通化

- 変更内容:
  - 共通WGSL moduleを追加し、宣言/importだけを移す。Rust側はterrain_base_material()で3つのmake関数のbase構築だけを共有し、extension構築入力は維持する。
  - LOD別アルゴリズムとextension型、bind番号は保持する。
  - prepass向けuniform型と本pass向け資源宣言を分け、RenderDocのimportロード・snapshot転送・pipeline登録を更新する。
- 変更ファイル: R13-B: 新規 terrain_surface_types.wgsl / terrain_surface_bindings.wgsl（案）、4 shader、terrain_surface_material.rs、renderdoc_capture.rs とrender側pipeline転送。startup/visual_handles.rs はLOD別None/Someを確認し、公開constructorを変えなければ編集不要。
- 完了条件:
  - [x] Rust gateとshader契約testが成功し、独立検証が壊れたABIを検出できる。
  - [x] 新しいimportの欠損/未ロードを検出し、全receiverへsnapshotが届く。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: 3 LOD/prepassとproduction GPUを受入

- 変更内容:
  - 既存RenderDocのLOD1-lite/LOD2 probeと全receiver pipeline常駐検査を再利用する。専用caseは各LODの可視出力・prepass・resize後の再bindの不足分に限定する。
  - 現行P08 bounded closureでproductionのbinding/epoch/revision契約を確認し、限定caseで不足を補う。
- 変更ファイル: R13-C: 既存RenderDoc LOD probe/P08 helper・独立verifier。不足する可視LOD/prepass/resize caseだけ補う。
- 完了条件:
  - [x] 実adapter/backendを記録した3 LOD/prepassとbounded production GPU証拠が揃う。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 描画の意味・可視条件・設定が不変か確認しABI整理だけならNo impact。見た目や設定の変化を発見した場合は調整を混ぜず別判断する。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/rendering-performance.md](../../../../docs/rendering-performance.md)
  - [docs/indoor_lighting.md](../../../../docs/indoor_lighting.md)
  - [docs/performance-profiling.md](../../../../docs/performance-profiling.md)
  - [docs/cargo_workspace.md](../../../../docs/cargo_workspace.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功。workspace RA診断は上記MCP制約を記録。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| WGSL import/layout変更で一部LODだけ壊れる | 全3 LOD/prepassを明示的に生成・描画する。 |
| 検査を実装と共通化して同じ誤りでpass | 期待値/拒否判定は独立に維持。 |
| sampling停止を未使用コードと誤認 | 現行compatibility testを必須にする。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| full/lod1_lite/lod2 | 全variantでshader import/compile/bindingが成功し描画不変。 |
| prepassと本pass | uniform layout/geometry契約が一致。 |
| resize・実scale factor | target再生成後もreceiver bindingsが有効。 |
| Indoor Light/旧projector/section cut | sampling停止と既存除去契約を維持。 |
| 意図的な不正bindingのテスト入力 | 独立validatorが誤りを拒否する。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R13-T1 `terrain_uniform_layout_matches_fixed_abi_test` | Rust ShaderType metadataとWGSL field宣言を独立の固定期待表へ照合。 | 全fieldの順序/型/offset、align16/size160が一致。production descriptorから期待値を生成しない。 |
| R13-T2 `terrain_binding_validator_rejects_drift_test` | test入力でfield順交換、binding133→132の重複、共通module欠損をそれぞれ作る。 | 各不正入力を拒否し、正常full/lite/lod2/prepassを受理。 |
| R13-T3 RenderDoc import回帰 | 新2moduleを含むロード済みshader snapshotをrender側へ渡し、既存LOD probeを実行。 | 全importが揃い、各pipelineがcompile/bindできる。固定長由来の欠落なし。 |
| R13-T4 native/GPU | 現行P08の入口で3 LOD・prepass・resize/実scale factorを必要範囲で確認。 | production source/asset/binary一致、描画とreceiver契約が維持され、sampling停止検査も成功。 |

T1/T2の意図的な破損はtest用入力だけに適用する。診断用shaderや一時sampling有効化をproductionへ残さない。

### focused command（実装時に実行）

```bash
python3 scripts/dev.py cargo -- test -p hw_visual terrain_surface_material
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --features profiling-renderdoc renderdoc_capture
```

test filterは実装時に実際の出力件数を確認し、0件実行を成功根拠にしない。
既存filterに入らない新testは正確なmodule/test名をここへ追加する。

### 完了gate

```bash
python3 scripts/dev.py docs --write
python3 scripts/dev.py docs --check
python3 scripts/check_help_impact.py
python3 scripts/dev.py check
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
python3 scripts/dev.py verify
python3 scripts/dev.py validation check
git diff --check
```

`verify`がworkspace testを含むため、成功後に理由なく同じ全testを繰り返さない。
上のコマンドは実装完了時の要件であり、計画作成時の実行済み記録ではない。
Help No impactのローカルoverrideはSkillに従い具体的理由を設定し、将来の差分を事前承認しない。
Help更新時はmanifest/provider/coverageと生成したexact snapshotを同じbatchに含め、全差分を読む。

### 実画面・性能確認

native Skillの現行P08 plan-rtt-light --level closure --stage p08をprimary coordinatorで利用する。既存LOD1-lite/LOD2 probeとreceiver pipeline常駐検査でcompile/bindingを確認し、M3の専用caseは各LODの可視出力・prepass・resize後のbindingの不足分を補う。probeの存在だけを可視出力の証明にはしない。旧P02 matrixをcurrentへ流用せず、実adapter/backend/scale factorとresizeを記録する。歴史的formal matrix・Memory・Tracyは要求しない。

実機確認を実施する場合は[Native Acceptance Skill](../../../../.cursor/skills/hell-workers-run-native-acceptance/SKILL.md)を読み直す。
primary `dev.py validation plan --spec ... -- <既存helperのplan>`へ登録し、返されたno-prompt kitty launcherだけを使う。
fixtureが対象を覆わなければ先に専用caseと独立verifierを用意する。汎用smokeの成功を個別要件の証明にしない。
Capture/Memoryを含むrecipeは逐次実行し、read-only observer・source/asset/binary一致・timeoutを検証する。
通常の修正feedbackでは同じdev cacheを再利用し、headless・feedback・正式受入・性能結果を区別する。

### 検証データ管理（完了記録）

正本は[検証データ管理](../../../development-infra/validation-storage-workflow.md)。2026-09-19に全batchの独立verifyまたは失敗理由をsealし、不要jobを削除してfinalize／storage checkを完了した。

| 対象 | 結果・整理 |
| --- | --- |
| terrain診断 | a/b/c/dはseal後に整理。`target/native-acceptance/ui-usability-feedback-20260918T172432Z-9a6c9c07`は5,459,968→0、同prefix`20260918T173157Z-fc367197`は2,297,856→0、`20260918T173440Z-f5c0d24d`は2,080,768→0、`20260918T185818Z-18c6c5f7`は2,084,864→0 bytes |
| P08診断・再受入 | candidateの`target/native-acceptance/renderdoc-foundation/`以下、a `4ed5f3d9-4ddb-43f7-b4ed-a92beb9636f7`は1,625,620,480→0、b `6231ad06-a353-475e-9678-76d5a4a41f51`は1,630,638,080→0、c `11795dd6-744d-4559-8e9d-0ee455fa02e0`は1,625,980,928→0 bytes |
| 限定replay | 同namespaceの`refactor-field-diagnostic-20260919-a/b`は151,552／110,592→0、`refactor-scene-extent-20260919-c`は20,480→0 bytes。一時スクリプトも撤去 |
| 計測上の容量 | 各job削除直後のdf空き差は0 bytes（Btrfs）。duのallocated bytesと共有extentの空き増加は同一視しない |
| 最終候補 | `/home/satotakumi/projects/hell-workers-validation/refactor-r01-r14-92a7d87235e6`、clean subject `a0487a35c09ed444dc588902ed198c9532f4c685`、40,952,061,952 allocated bytes |
| 残る具体的用途 | owner `refactor-r01-r14`、consumer `refactor-r01-r14-review`。2026-09-19の実装提示に対する確認・修正で同じcandidateとtargetを再利用する。実装consumerは解除し、review consumerへ引継ぎ済み |
| release_when | 提示したR01〜R14のレビューと必要な修正が承認または明示終了し、利用者がなくなった時。R04/R08/R12の入力確認にはprimary dev cacheを使用する |
| 正本 | code・assets・仕様はprimary。primary HEAD/indexは変更していない。通常Cargo cacheは別の保守寿命で維持 |

## 8. ロールバック方針

WGSL宣言移動とRust構築helperを分離する。問題が出た層だけを戻して既存ABIを維持し、変更subjectには新jobで再受入する。旧pass流用はしない。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了）

共通WGSL型・binding宣言、Rust base material helper、RenderDoc import/snapshot転送と実descriptor検証を実装。地形・P08の必要受入と終了整理を完了。

P08 aはshader未参照画像を取り落とす検証器がinvalidにし、bはGPU検査通過後にsource_checks受渡し漏れでinvalidとなった。実descriptor＋名前＋画素照合とメタデータ受渡しを修正し、新subjectのcで全受入を確認した。samplingは再有効化していない。

| 検証 | 最終結果 |
| --- | --- |
| focused回帰 | ABIの固定layout・binding・negative fixture、import/snapshot転送、sampling停止、Light Field descriptor／pixel拒否の9 Python回帰が成功。 |
| 全体gate | 最終変更後の`dev.py verify`と`dev.py check`が成功。Python 200件、Blender 151件、通常／profiling workspace tests、feature別compile、通常／profiling Clippy警告0 |
| rust-analyzer | workspace診断はMCPのnull応答で取得不可。compile／tests／Clippyで代替確認し、RAの診断取得成功とは報告しない |
| native | terrain-materials dは3 LOD・診断NormalPrepassの4 fragment pipeline、native DPI 2.0、1920×1080→1280×720→1920×1080のresize/rebind、6画像の目視とpixel比較が成功。P08 cは独立verify／二重replay／Scene実寸法1920×1080が成功。 Intel Arc Graphics (MTL)／Vulkan／X11／Mesa 26.1.8 |
| Help | 本計画自体はNo impact。通常の操作・表示の意味・入力条件を維持する。全14件ではMove失敗時の保持・タスク表示・建設工程の3 entryを更新し、exact snapshotをレビュー・検証済み |
| 保存整理 | 全jobをseal／整理／finalize済み。storage check成功、candidate/cacheだけreview-activeとして上記consumerへ引継ぎ |
| 対象外・未検証 | 歴史的baseline再構築・全性能／DPI matrixは対象外。R04/R08/R12のOS許可待ち実入力受入は別の現行計画で管理 |

P08 cのsource fingerprintは`2f3cc42ecb89c75ef0785a96a3e4bfb478733dfed6d72dc30956cad4ca4e8ee3`、harnessは`a017d1a25a78c3189ae515fa094eed09a41bd68176ae03e5a7be324dba3a7b1f`。
CPU/GPU/Soul/Roomのepoch 0／revision 1、Scene 1920×1080、Light Fieldの期待pixelと22件の実bindingが一致した。
詳細な受入結果は[描画仕様](../../../rendering-performance.md)と[性能計測仕様](../../../performance-profiling.md)を正本とする。

### Definition of Done

- [x] milestoneと必要な回帰／native受入を完了
- [x] 仕様・Helpの意味判断と対応を完了
- [x] check・Clippy警告0・verifyに成功し、RA診断取得の制約を記録
- [x] 不要job／capsule／診断スクリプトを整理しstorage checkに成功
- [x] 実装consumerを解除し、最終candidate/cacheを明示review consumerへ引継ぎ
- [x] 具体的な削除path・実測bytes・保持用途を記録
- [x] 計画をarchiveし、両索引を再生成

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-18 | Codex | R13の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: prepassのuniform限定import、RenderDoc snapshot受渡し、既存LOD probe再利用と不足受入を明記。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: Rust metadataと4 WGSLからABI/binding固定表を作り、RenderDocのload→snapshot→set_shaderで追加moduleが通る変更箇所を列挙する。 実装は未着手。 |
| 2026-09-19 | Codex | 共通ABI・回帰・品質gateとterrain診断cを確認。最終sourceのcandidateへ更新し、追加診断とP08 closureを継続。primary HEAD/indexは不変。 |
| 2026-09-19 | Codex | 最終回帰・全体gate・P08 cの独立受入を完了。job整理とreview cacheの引継ぎを記録しarchive。 |
