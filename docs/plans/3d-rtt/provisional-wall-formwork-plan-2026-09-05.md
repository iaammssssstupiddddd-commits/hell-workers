# 仮設壁の木製型枠化計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `provisional-wall-formwork-plan-2026-09-05` |
| ステータス | `In Progress — M3b正式画像18/18、Capture 18/18、Memory 6/6合格／lifecycle・J1待ち` |
| 作成日 | `2026-09-05` |
| 最終更新日 | `2026-09-07` |
| 作成者 | `Codex` |
| 親計画 | [アセット作成マイルストーン](asset-milestones-2026-03-17.md)（Build-Aの仮設表現を追加改修） |
| 関連計画 | [ドアの本番ビジュアル化](production-door-art-plan-2026-09-05.md)、[完了済み本番壁計画](archived/production-wall-art-plan-2026-08-31.md) |
| 関連提案 / Issue / PR | `N/A` |

本書は実装追跡中。候補tooling・6 family・runtime接続を実装し、M3aのArtPreviewはユーザー承認済み。
subject `3c0f7c67`とgeneration 8の隔離candidateでM3b正式画像18枚も合格した。調査基点は `57a488e1`。
完了済み壁trackを再開せず、現在の石壁を基準に仮設段階の表現を改修する。

## 0. セルフレビューで修正した点

| 指摘 | 具体化した判断 |
| --- | --- |
| 未承認アートは現行candidate loaderへ入れられず、ゲーム内確認と承認が循環する | §4.4の隔離アートpreviewと正式candidateを分離。既存authorityの検査は緩めない |
| 専用scenarioをM3で作るのにM0でそのbaselineを要求していた | M0で計測fixture・profileを先に実装し、旧releaseでbaselineを封印 |
| 型枠の空隙・支柱数と240 trianglesの関係が未定義 | §4.1で部材構成・断面・family別上限を固定 |
| authoring、runtime projection、旧normal比較の契約が混同しやすい | §4.3でversion別inventory、role、review形式を明記 |
| joint受入を相手のM3完了条件へ含めると循環する | 単独M3の後に共通J1を設置し、両M4だけがJ1を待つ |
| 封印後のコード統合や検証用assetの上書きで先行証拠が無効になる | 最終preview・封印前に両M2をfreezeし、before／各単独／J1のasset viewを専用worktreeへ固定 |
| Memory・load反復・画像matrixの合格条件が曖昧 | §7でrun数、比較許容差、10回loadの観測点、PNGのphaseを固定 |

以下の寸法・予算は本計画で採用する実装契約であり、制作済み・実測済みという意味ではない。

## 1. 目的

- 解決したい課題: 木材を原料とする仮設壁の見た目を木製型枠へ合わせ、本設の石壁と形状・材質で識別できるようにする。
- 到達したい状態: 木材搬入 → 木枠の組立 → 泥塗り → 石壁の順が通常画面で読み取れる。
- 成功指標: 16接続mask、仮設と本設の混在、ドア隣接、タイル単位の完成、save/load後に同じ表示契約を満たす。

推奨は**木製型枠**。黒ずんだ木の支柱・横桟・粗い板・筋交いで構成し、恒久的な木造建物との
違いを出す。ここでいう型枠は現行の `Framing → Coating` を表す意匠であり、現実の型枠工法の
工程を追加する意味ではない。制作は型枠案に絞り、M1ではstraight・cornerの読みやすさを先に確認する。

## 2. スコープ

### 対象（In Scope）

- 型枠6形状と木材用texture、productionとfallbackの有限asset pool、段階別mesh/material切替。
- 通常壁サイト、legacy壁、Instant Build、cancel、撤去、save/load・rollbackの表示整合。
- 本設壁・仮設壁・ドア・設計図との接続、平面施工mask、完成bounce、depthの維持。
- asset manifest / loader / 検証profileの後継契約、実機受入、Help・仕様書の更新判断、release。

### 非対象（Out of Scope）

- 材料数量、作業時間、施工phase、耐久、移動、Room成立、遮光、save schemaの変更。
- 恒久的な木壁の新BuildingType、型枠の解体・再利用タスク、泥塗り途中の連続変形。
- 本設石壁の再デザイン、global shader / outline、PointLight、wall LODの追加。

## 3. 現状とギャップ

| 領域 | 調査で確認した現状 | 本計画の変更 |
| --- | --- | --- |
| 原料・工程 | 1 tileにWood × 1、StasisMud × 1。Framingで仮設Buildingを生成 | 木材段階を木の形状で表す。レシピは維持 |
| active表示 | release済み石壁6 GLBを仮設でも共有。仮設は同じalbedo、alpha 0.9のBlend、emissiveなし | 型枠専用6 GLB＋木材albedoへ変更 |
| 本設化 | `CoatWall`が各tileの`Building.is_provisional`をfalseへ変更。site全体完了は後続cleanupとbounceを担う | tileのフラグ変更でmeshとmaterialを一緒に交換 |
| 接続 | 16maskを6 family＋quarter turnへ解決 | 同じ接続意味・回転で段階別meshを選ぶ |
| 論理的性質 | 仮設は通路分離に参加するがRoom境界として不成立、Light Fieldは通光 | 見た目の隙間で未完成を示し、この論理を維持 |
| 検証 | resident 6 production mesh / 2 material、mesh上限72 trianglesを前提とする現行gateがある | 木枠追加用の新inventory・予算を検証器と同時に導入 |

主な根拠は `hw_core/src/constants/logistics.rs`、`hw_jobs/src/model.rs`、
`hw_soul_ai/.../task_execution/coat_wall.rs`、rootの `wall_construction/phase_transition.rs` / `completion.rs`、
`assets/wall_asset_set.rs`、`systems/visual/wall_presentation.rs`、`hw_visual/src/wall_connection.rs`。
`docs/building.md`の「警告色オーバーレイ」と、旧親計画の未着手表記は制作仕様の根拠にしない。

## 4. 実装方針

### 4.1 アートと形状

| 項目 | 初期方針・受入条件 |
| --- | --- |
| 材質 | 世界観の黒い木を基本に、低彩度の茶灰色、太い暗色線、粗い木目。板の割れ・天端・接合部が読める |
| 構成 | 支柱と横桟を主形状にし、板張りと筋交いで型枠を表す。均一な石目、紫の発光は持たせない |
| 視点 | productionの約59°正射影。正面だけでなく天端・上面に識別要素を置く |
| family | isolated / end / straight / corner / t_junction / cross。16個のGLBは作らない |
| 寸法 | tile 32 wu、高さ32 wu、中心anchorを本設と共有。接続端は本設の9.6 wu幅のportへ合わせ、装飾込み局所幅12.8 wu以内・cell内に納める |
| 隙間 | 木自体はOpaque、板間や骨組みの隙間はgeometryで表す。完成石壁の「全断面が連続した9.6 wu厚」の検査は木枠の空隙へ適用しない |
| 遠景 | 標準zoomで木材、最大zoom-outで仮設と本設の違いが分かる。微細な木目だけに識別を依存させない |

型枠の接続端で支柱が二重に太る、cornerで筋交いが交差して黒塊になる、施工maskを全面的に覆う、
という失敗をM1の比較対象にする。全体の半透明化は初期案に採用しない。Opaque化によるdirectional
shadowと論理的なLight Field通光は別の契約なので、実画面で混同しないことを確認する。

制作座標はGLBのlocal X/Zが水平、local Yが高さ、原点がcell中心かつ高さ中央とする。
Blender原本では1 tileを1 unitで作り、既存exportと同じ32倍を出力コピーへbakeする。
以下の数値はexport後のwu。geometry検査の許容差は0.01 wuとし、美術調整の余裕には使わない。

| 部材 | 固定寸法・配置 |
| --- | --- |
| 中心支柱 | X/Z各`[-2.4,2.4]`、Y`[-16,16]` |
| 各armの端半支柱 | arm方向距離s`[14.4,16]`、横断t`[-4.8,4.8]`、Y`[-16,16]`。隣cell側の半支柱と境界で接し、同じ体積を重ねない |
| 上下の横板兼横桟 | s`[2.4,14.4]`、t`[-4.8,4.8]`、Yを下`[-12.8,-9.6]`／上`[9.6,12.8]`に分ける |
| 筋交い | armあたり1本、横桟間を結ぶ幅2.4 wu・厚さ1.6 wuの角材。両端・回転後の頂点をarm/cellの外形内に納める |
| isolated | 接続portなし。X/Zの12.8 wu四方内に支柱2・横桟2・筋交い1の小型枠を作る。隣cellまで腕を伸ばさない |
| 表面 | 木材用512×512 RGBA albedo 1枚、alphaは1。木目・釘・細い割れをtextureへ焼き、追加mesh/materialにしない |

閉じた角材1本を12 trianglesとして、center＋1 armあたり4材で構成する。部材を単一meshへjoinし、
外から見える重複面・z-fightingを作らない。joinだけでは内部交差が解決しないため、接合部をGLB実bytesと
斜視renderで検査する。装飾込み240を超えたら部材・面を減らし、上限を自動的に引き上げない。

| family | arm数・標準方向 | 基本triangle数 |
| --- | --- | ---: |
| isolated | 0 | 60 |
| end | 1：N | 60 |
| straight | 2：N/S | 108 |
| corner | 2：N/W | 108 |
| t_junction | 3：N/S/W | 156 |
| cross | 4 | 204 |

N/S/W/Eは既存resolverのgrid方向に従い、GLBではNが-Z。各armのs=16面は全高32・幅9.6のportを
持ち、非接続面へportを作らない。旧石壁validatorの断面充填検査を流用せず、木枠用profileで端面の
接触、cell外への頂点0、各armに横桟間の実際の空隙があることを検査する。全16maskと各familyの
quarter turnで寸法を再評価する。空隙があるだけで遠景合格にはせず、§7のcomposite観測も必須とする。

### 4.2 段階別表示と所有権

- 旧「同一mesh / UV、materialだけ交換」を、**同一owner / topology、段階別mesh / material**へ改定する。
- `Changed<Building>`を既存`apply_wall_presentation_system`が消費し、各tileのmesh・material・表示状態を同じ更新で交換する。site全体完了を切替条件にしない。
- 完成時もownerとexactly-one `Building3dVisual`を保持する。quarter turn、owner transform、既存bounceを共通resolverで合成し、`MeshTag`はlogical grid anchorのままとする。
- Framing前の平面mask、資材待ち・塗布中maskは既存経路を維持する。泥塗り途中は木枠を保持し、tile完了時に既存石壁へ切り替える。
- 表示段階を保存対象へ追加しない。通常load・rollbackとも`Building.is_provisional`と復元済み接続から再構築する。
- rootはasset注入・domainとのadapter・orderingを所有し、`hw_visual`は描画専用型とpureな選択規則を所有する。新たなdomain直接依存をleafへ増やさない。
- 新APIは既存Bevy 0.19のmesh交換経路を参照し、必要な署名はrust-analyzer / docsrs / local registryで確認する。

### 4.3 asset setと性能契約

既存Wallのauthoring asset-setを**v3**、runtime `.wallset`を**version 2**へ拡張し、新generationを作る。完成6 GLB・albedo・emissiveの
既存bytesを保持し、型枠6 GLB・木材albedoを追加する。coreは計15 file、production meshは12、
materialは完成1＋型枠1、fallbackは最大1 mesh＋2 materialを予算とする。旧v2のexact 8 file契約は
互換readerで維持し、新roleを黙って旧schemaへ追加しない。runtime `.wallset`はauthoringとは別の
schema（現在version 1）なので、version 1のreaderも維持する。

型枠のtriangle上限は**各240以下**とし、M1で実測する。
既存石壁の72以下を緩和する意味ではない。木目と釘などはtextureへ寄せる。runtimeは新schemaなら全12 mesh・
3 texture・2 material、旧schemaなら従来の6 mesh・2 texture（normal採用時3）・2 materialについて、topologyと
ready/identityを検証してからset全体を切り替える。旧releaseへ新inventoryを要求しない。
欠落・破損・未承認・世代不一致では既存の可視fallbackを使い、本番受入ではfallbackを0にする。

authoring、manifest validator、projection、allowlist sync、promotion receipt、loader、performance
sidecarを一つの変更契約として更新する。既存v2のreceiptや承認artifactは改変しない。
本設だけのdraw構造は6 familyのまま、型枠単独もOpaqueの6 family、混在は最大12のmesh/material
組合せを設計予算とする。これは実draw callの測定値ではなく、追加pass・shadowは別計上する。

| authoring / runtime | 必須coreとrole | readiness / 表示 |
| --- | --- | --- |
| v2 / v1（現行generation 4） | 既存8 fileとroleをそのまま保持。旧normal採用の9 file契約も削除しない | 現行は6 mesh＋2 texture＋2 material。旧normal採用時は3 texture。仮設も石meshを共有する旧表示互換 |
| v3 / v2 | 既存8 file＋`wall_formwork_<family>.glb` 6件（`mesh:formwork:<family>`）＋`wall_formwork_albedo.png`（`texture:formwork_albedo`） | 12 mesh＋3 texture＋2 material。仮設だけ型枠を選ぶ |

authoringの追加pathは`models/buildings/wall/wall_formwork_<family>.glb`と
`textures/buildings/wall/wall_formwork_albedo.png`。releaseは既存と同じ
`wall_sets/<GEN>/models/`と`wall_sets/<GEN>/textures/buildings/wall/`へ投影する。
asset-set IDとmutable locator `manifests/wall-production-v1.wallset`は維持し、filenameのv1を
schema番号として扱わない。新coreの順序、canonical JSON bytes、unknown role拒否はfixtureで固定する。
未知version、重複role、追加role、新旧inventoryの混入も拒否する。v2のnormalを省略して検査を通す
互換実装にはしない。旧schemaの仮設は従来のBlend、新schemaの木材はOpaque、既存Cuboid fallbackは
完成／仮設の単色pair（仮設alpha 0.9 Blend）のまま、とmaterial policyもversion別に固定する。

v3のsourceは`completed`と`formwork`の2組を持ち、各組にblend・geometry contract・tool provenance・
mesh reportを結ぶ。現行の単一blendを前提にしたvalidatorと`promote_asset_set.py`のpayload収集を
更新する。本設8 fileと原本・reportは旧generationからhash付きで再利用し、型枠は新原本・新geometry
fixtureで検査する。旧geometry fixtureや承認reportは編集しない。新generationを旧generationや
stagingなしで単独再検証できるよう、必要な原本・contract・report・licenseもpayloadへ同梱する。

旧`seal_wall_final.py`は過去のlit比較・`rejected_by_missing_mesh_tangents`を要求するため、
新v3用review分岐を実装する。新reviewには`opaque_formwork_approved`、`normal=not_used_by_design`、
候補hash、preview PNG hash、ユーザー判断を記録し、未実施の旧normal比較を作ったことにしない。

poolの検査は「保持handle数」「resident asset数」「active visualで使う数」を分ける。
steady readyは新production 12 mesh / 2 materialが上限、fallbackは最大1 / 2でresident 0も許す。
generation更新中は現行と次の最大2世代までを許し、切替・解放後のsettleで旧世代の強参照を残さない。
連続reloadを無制限に積まず、次の要求は最新1世代へ集約する。全ready前にtileを個別promoteしない。

### 4.4 共通の実行順とアートpreview

本節はDoor計画からも参照する共通契約。実装はmain agentが順に行い、調査・レビューのみ並行する。

1. **共通C0／各M0:** 新fixture・artifact schema・比較profileを作り、production表示を変えないclean subjectを封印する。そのsubjectで現在のreleaseのbeforeを採る。
2. **各M1→M2:** stagingで制作し、runtime接続・失敗系testを完成させる。未承認アートの確認には下記のpreviewだけを使う。両M2のコードを1つのclean candidate subjectへ統合・freezeしてからM3aへ進む。
3. **各M3a:** freeze済みsubjectでゲーム内previewの比較画像と候補hashを提示し、既存sessionに不足するアート判断を得る。その同じbytesとsubjectをfinal manifestへ封印し、通常の`isolated_candidate` projectionを作る。封印後に両M2をmergeして`runtime_subject`を変える順序にはしない。
4. **各M3b:** 同じfreeze済みsubject、preview無効の正式candidateで各単独native受入を実施する。画像・性能・Memoryの全gateを通して各単独M3を閉じる。個別の制作・探索previewは先行可能だが、最終承認previewと正式証拠は統合後のsubjectで採る。
5. **J1:** 両単独M3完了後、同じcommitの専用worktreeへ両候補をprovisionしjoint受入。単独計測時の非対象asset viewとJ1の両候補asset viewは別hashとして記録する。各M4はJ1を待つが、片方のM3が他方のM3を待たない。
6. **各M4:** release判断・昇格・通常release受入・close。J1以後にgeometry/表示resolver/対象assetが変わればJ1を再採取する。

正式jobはplan時だけでなく再検証時にもworktreeのasset viewを照合する。次の4条件を別々の
clean validation worktreeへ固定し、同じtreeへの逐次上書きで先行証拠を無効にしない。

| view | source commit | 固定するruntime asset |
| --- | --- | --- |
| before | 共通C0のみ | 現行Wall release＋現行Door fallback。両trackのbeforeを共用 |
| Wall単独M3 | 両M2統合後 | 型枠Wall candidate＋現行Door fallback |
| Door単独M3 | 両M2統合後の同じcommit | 現行Wall release＋Door candidate |
| J1 | 両M2統合後の同じcommit | 型枠Wall candidate＋Door candidate |

各treeの全asset-view hash、非対象assetの一致、candidate identityをjobへ結ぶ。previewとreleaseの
viewも正式jobのtreeへ上書きしない。追加treeの作成前に実使用量と必要なbuild容量を確認し、容量不足を
別targetや証拠の途中削除で回避しない。各treeは固有の`target/`を使い、build/gameは完全逐次実行する。
再試行でtreeを増やさず、両trackのcloseまで保持して再検証し、capsule保存後に本track所有分だけを整理する。

現行`project_wallset.py`、syncのmanifest mode、runtime loaderは`final/art_approved`を要求する。
その検査を緩める代わりに、**新しいprofiling専用アートpreview adapter**を両assetに共用する。
これは未実装の追加機能であり、既存candidate commandへpending manifestを渡して実行できるという意味ではない。

- 入力は外部stagingの技術検査済みallowlistと実bytes hash。main tree/canonical外の検証worktreeだけへ、専用のprovision処理でそのallowlistをコピーする。
- `profiling` build、専用preview scenario、起動時の明示opt-in、repo/asset root/候補hash一致を全て要求する。通常起動・release projection・通常同期のauthorityは変更しない。
- productionと共通のmesh/material選択処理へ有限handlesを注入するが、状態は`ArtPreview`と明示し、`Production`や`release_approved`を報告しない。新adapterはauthoring検証に実際に使うtoolingとして保持する。
- window画像・sidecar・job manifestに`evidence_kind=art_preview`を記録する。正式受入・性能比較・promotionのvalidatorはこのkindを拒否する。
- 最終アート判断はこの具体的なゲーム内比較画像に対して行う。承認後に正式candidateを別jobで再受入し、preview成功を正式passへ読み替えない。

### 4.5 編集責務と着手単位

| 単位 | 主な対象（新規名は実装予定） | 終了時に確認する契約 |
| --- | --- | --- |
| W0: geometry / schema / preview・計測入口 | `tools/blender_ai_workflow/fixtures/`、同`scripts/validate_wall_glb.py` / `validate_asset_set_manifest.py` / `project_wallset.py` / `seal_wall_final.py`、root `plugins/startup/perf_scenario/` | 旧schema互換、新role拒否系、beforeのfixture固定、previewと正式受入の区別 |
| W1: 制作 | 新`create_wall_formwork_scene.py`、外部`staging/` | 6 family、240以下、512² albedo、完成8 fileのhash不変 |
| W2: asset / 表示 | `crates/bevy_app/src/assets/wall_asset_set.rs`、`systems/visual/wall_presentation.rs`、`hw_visual/src/visual3d.rs` | 旧/新inventoryのreadyとtile単位mesh/material切替 |
| W3: lifecycle / ordering | root `plugins/visual.rs`、`systems/save/rehydrate/`と`systems/save/transaction.rs`のtests | reset→topology再構築→表示→Transform propagation。bounceと表示更新の二重適用なし |
| W4: 受入 / close | 新formwork profileとsidecar、§7のdocs | current subject・exact assetを結んだ証拠、Help判断、capsule |

施工domainの`coat_wall.rs`や`wall_construction/completion.rs`は回帰経路として使う。
本件の表示切替のために、task完了条件や搬送producerの責務を移さない。

## 5. マイルストーン

### M0: 共通C0・寸法・baseline基盤

- 変更内容: §4の型枠寸法・source分割・schemaをfixtureへ落とし、共通preview入口と§7のbaseline用scenario/profileを実装する。
- 対象: §4.5のW0、本計画、Doorと共有するpreview/provision/比較helper。
- 完了条件:
  - [ ] 木材→泥塗りの実経路、tile完了、legacy、loadの対象一覧を確定。
  - [ ] 成立条件が異なる旧半透明と新Opaqueの比較項目、baseline subject、計測予算を事前に固定。
  - [ ] ドア計画M0と9.6 wu port・型枠隣接時の見た目を共有。
  - [ ] 新profileが旧releaseを採取でき、現在のproduction表示を保つclean before subjectとfixture/harness hashを封印。
  - [ ] previewを正式受入・promotionへ渡した失敗系、旧schemaのnormal有無、2 sourceのpayload収集を検証。
- 検証: 新fixture/profileのself-test→旧releaseのfresh baseline。旧Cuboid fallback対照をbeforeと呼ばない。

### M1: 型枠アートと後継manifestの制作

- 変更内容: stagingで6 collection、6 GLB、共有木材albedo、寸法/triangle/provenance reportを制作。まずstraight・cornerで方向性を確かめ、全familyへ展開する。
- 対象: 外部asset rootの`staging/`、`tools/blender_ai_workflow/`、`scripts/sync_external_assets.py`。
- 完了条件:
  - [ ] Blender scene・Khronos・GLB実bytes・全rotationのport検査が合格。
  - [ ] 本設と型枠の比較boardで識別性を確認。Door候補の完成は本M1の前提にしない。
  - [ ] 旧v2互換と新inventoryの失敗系を検証し、完成石壁のhash不変を確認。
- 検証: 変更したPython toolingのfocused test。色評価は既存の正常なOCIO経路を使用する。

### M2: production経路への接続

- 変更内容: 段階別asset選択、atomic切替、fallback、rehydrateとsteady-frame更新抑止を実装。
- 対象: `crates/bevy_app/src/assets/wall_asset_set.rs`、`systems/visual/wall_presentation.rs`、必要な`hw_visual`表示型、startup handles / plugin wiring。
- 完了条件:
  - [ ] material-only testをmesh/material同時切替とowner/topology維持のtestへ改定。
  - [ ] 2 tile siteの片方だけCoat完了、legacyの完了、asset遅延・失敗、通常load・rollbackが同じ規則を満たす。
  - [ ] 完成bounce、mask、RenderLayers、MeshTag、cleanupを保持し、安定frameで不要なasset生成・表示書込みをしない。
- 検証: focused Rust tests、rust-analyzer診断、`python3 scripts/dev.py check`、Clippy。

### M3: アート判断・正式candidateの単独受入

- 変更内容: M0で整備したprofileでM3aのpreview比較・候補封印、M3bの正式candidate受入を順に実施する。
- 対象: root `plugins/startup/perf_scenario/`、native acceptance helper、`scripts/perf_tool/`の対応validator。
- 完了条件:
  - [x] M3aのゲーム所有window ArtPreviewを独立verifyし、木製型枠の方向性をユーザーが承認。候補manifest `f11eb5ef…`、PNG `86a5d643…`、回答「OKです」をapproval artifact `6250d7bd…`へ結んだ。
  - [x] M3b正式画像を同一subject・同一candidateで標準9 case＋最大zoom-out 9 case採取し、両jobを独立verify。詳細は§7.3。
  - [ ] §7の正式画像・状態・性能の各gateが成立し、木製型枠の最終candidateを確認できる。
  - [ ] 新inventoryの検証が実resident handlesに一致し、旧completed-only galleryを仮設受入に代用していない。
  - [ ] `art_preview`ではない正式candidateの証拠を封印し、単独M3を完了。相手のM3完了を前提にしない。
- 検証: `hell-workers-run-native-acceptance` Skillの専用scenario・direct `kitty` launcher・独立artifact verify。

### J1: Doorとの共通受入

- 前提: 両計画の単独M3完了。Wall→型枠のreceipt発行やDoorのreleaseはまだ不要。
- 変更内容: 同じclean subjectへ両候補をprovisionし、両軸の型枠―Door―本設、Door連続、corner近傍、片tile完成、支持壁撤去・復元、pause中loadを1つのstoryboardで確認する。
- 完了条件: 標準High/DPI 1の1 processで全phaseのstate・mesh/tag・画像を結ぶ。両asset generation/hash、subject、job IDを両計画へ同じ値で記録する。
- 本節はshared J1を所有する。Door側で同じjobをもう一度実行せず参照する。quality特有の失敗を見つけた場合だけ該当caseを追加する。

### M4: release・文書同期・close

- 変更内容: 検証済み新generationのrelease手順、実画面比較、manifest・receipt・同期差分を提示し、canonicalへ昇格する。
- 対象: `docs/building.md`、`docs/art-style-criteria.md`、`docs/assets_workflow.md`、`docs/rendering-performance.md`、親計画。Room・照明文書は表示説明に影響する部分を確認する。
- 完了条件:
  - [ ] session内のアート／release承認を確認し、不足する最終判断だけを完成済みの候補とともに提示。
  - [ ] 共通J1がvalidで、両assetの変更後の組合せを指している。
  - [ ] 通常起動のrelease authorityで受入。Help影響レビュー、全品質gate、証跡capsuleとworktree整理が完了。
  - [ ] 恒久仕様へ転記後に本計画をarchiveまたは削除し、両索引を再生成。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 石壁用の形状検査を流用 | 型枠の空隙が不正として扱われる | 共通port/anchorと材質固有の断面検査を分ける |
| site完了だけで切替 | 泥塗り済みtileも木のまま残る | `CoatWall`のtile単位フラグを表示の唯一の段階入力にする |
| 旧pool数・透明passのgateを維持 | 実装が正しくても受入不能／誤認 | version付きの新profileでinventoryとOpaque予算を検証 |
| 木枠の隙間が細すぎる | 遠景で完成壁と区別不能 | 上面・天端・大きな空隙を優先し、最終compositeで確認 |
| ドア側も共有loaderを変更 | 実装競合・互換性破壊 | 本計画M0で共通の必要範囲を決め、main agentが順に編集 |

## 7. 検証計画

| 層 | ケース・合格条件 |
| --- | --- |
| topology | 全16mask×仮設/本設、全quarter turn、混在列、Wall/Door/blueprint追加削除、cancel後に正しいfamily・port |
| lifecycle | Framing前→木枠→片tile完成→site完成、legacy、Instant Build、撤去、paused load、rollback、asset fallback→ready。owner重複・消失なし |
| gameplay回帰 | Wood/StasisMudの消費、通路分離、仮設Room不成立、仮設通光、本設遮光を既存ruleで確認 |
| actual-window | 新galleryに16mask、混在列、施工mask、Soulの前後depth、状態遷移を配置。High/Medium/Low × DPI 1/1.5/2、標準zoom・最大zoom-outで確認 |
| 性能 | 仮設96・仮設384・混在384（仮設192＋本設192）の3 case。§7.2のbefore/after比較で各p95/p99中央値+5%以内 |
| resource | Capture後にMemoryを逐次実施。asset poolは有界、load反復でvisual・mesh・materialの残存増加なし。draw測定が必要な場合だけ別RenderDoc採取 |

画像はゲーム所有windowとphase・nonce/ACK・subject/source/asset hash・実adapter/backendを結ぶ。
headless、Blender render、`visual_test`だけでは実機合格にしない。既存Wall galleryはcompleted-only、
旧P02は凍結済みなので新profileを実装する。性能のfixture条件を変える場合はbaseline側も同一条件で採る。
未取得のMemoryを「性能合格」に含めない。

### 7.1 ケースと観測点

新profile名は`wall-formwork-v1`とし、標準／最大zoom-outの正式画像profileを実装済み。既存`wall-density-v1`やcompleted-only galleryの
凍結fixture・過去sidecarを変更せず、新しい状態別inventoryを持つ。

| ID | setup / 操作 | 観測する結果 |
| --- | --- | --- |
| W-G01 | 仮設・本設それぞれ16mask、混在の接続pair、Soul前後を同じgalleryへ配置 | family/quarter turn、port、材質、depth。標準と最大zoom-outを別checkpointで撮る |
| W-L01 | 2 tileサイトを通常Framing→片方だけCoat→残りCoat | 仮設2→仮設1/本設1→本設2。各tileのowner/visual ID不変、切替は同じpresentation更新内 |
| W-L02 | legacy Coat、Instant Build、Framing前cancel、仮設後cancel、完成壁撤去 | 現行domainの存続/削除結果に表示が追従。cancelを一律despawnとして期待しない |
| W-L03 | 混在世界をpause中save/load、失敗loadのrollback、通常loadを10回反復 | rehydrate直後は可視fallback、次のPostUpdateでtopology/assetsが揃えばproductionへ復帰。意図的な1 frame待機を追加せず、各ready後のowner/visual/pool数が初回と一致 |
| W-A01 | 木枠GLBを1件遅延/欠落、texture hash不一致、新旧inventory混入 | 全Wallが可視fallback、正式受入はfail。全ready復帰時だけ一括切替 |
| W-A02 | 新→旧v2→新generation、切替要求の連続発生 | 正しい旧表示互換、最大2世代、settle後に旧参照を残さない |

`Last`で行うworld置換はPostUpdateより後なので、loadした同じframeのpropagationを要求しない。
表示再構成はpauseに依存しない。sidecarはframe番号、world epoch、phase、logical owner grid、stage、
topology、表示mode、generation、active/resident/retainedの各数、MeshTagを記録する。
load後のEntity IDは保存前との一致を要求せず、そのworldのownerとのexactly-one対応を検証する。

### 7.2 計測matrix・予算（Doorもこの規則を使用）

- beforeはM0の新計測基盤だけを足した現行release、afterは対象実装を足したclean subject。両者のsource hashは異なってよいが、fixture・harness・非対象asset・seed・実adapter/backend・window・present modeは一致させる。
- 比較許可差分は対象productionコード・asset inventory・対象authoring/projection toolingに限定して台帳へ列挙する。計測器やlayoutが変わったらbeforeも採り直す。旧cross-subject helperの全asset-view一致条件をそのまま使わない。
- 画像はHigh/Medium/Low × DPI 1/1.5/2の9 process。各process内で標準/最大zoom-outを別nonce/ACK checkpointとして採り、最低18 PNG。状態galleryを同じ画面へ含め、quality×個別状態の直積を作らない。
- lifecycleはHigh/DPI 1で各track1 storyboard process。10回loadと失敗系はここまたはfocused auditで検証し、性能measureには混ぜない。
- CaptureはHigh/DPI 1、Vulkan/X11、同一実adapter、novsyncで30秒warm-up/60秒measure、各case・各presentation3 runs。Wallは3 case×2 presentations×3＝18 runs。Doorは2 case×2×3＝12 runs。
- 各runのp95/p99をraw framesから計算してから3 runの中央値・MADを比較する。各中央値+5%以内。失敗runを都合よく除外せず、再測定理由と全attemptを残す。
- 本設のみの追加性能caseは共通renderer/material実装を変更した場合に実施する。本設8 fileのhash不変とgallery回帰は常時必須。
- MemoryはWall混在384、Door128の最大caseについてbefore/after各3 runs、同じ30/60秒。Capture検証後に別Memory binaryを逐次build/runする。計12 Memory runsで、Memoryのframe timeは性能比較に使わない。
- Memory合格値は各caseの`max_rss`中央値+5%以内、`peak_live_bytes`中央値+4 MiB以内、allocator accounting error 0。新規に採用する予算であり、現在の実測値ではない。GPU memory量の証明とは区別する。
- load反復の安定性はMemory CSVの単一peakから推測せず、10回それぞれのready/settle後のowner・visual・保持asset数を専用sidecarへ記録する。
- 正式比較・candidate受入はgallery18＋lifecycle2＋Capture30＋Memory12＋J1 1＝63 process。before画像はbefore Captureへ同梱し、release確認は両trackの9 caseずつで18 process、初回アートpreviewは各track最大2 processとして、初回計画は最大85 game起動を見込む（build/self-test・失敗再実行は別記録）。RenderDocはdraw/passの計測が必要な場合だけ追加し、過去RtT formal matrixを再開しない。

Wall性能fixtureはseed `20260901`、1280×720、camera scale 5を使う。既存densityの幾何配置だけを
新contractへ固定し、ordinal iの中心はgrid `(2+5*(i%20), 2+5*(i/20))`、maskは`i%16`とする。
target数96 / 384、mask反復6 / 24、接続用Door blueprint数192 / 768。connectorはtopologyにだけ
参加し、旧／新previewの性能差が混ざらないようroot・pulseを含むvisible presentationは0に固定する。
混在384は`(i/16)%2==0`を仮設、それ以外を本設とし、各maskで仮設12 / 本設12を保証する。
全ready後もVirtual Timeをpauseし、Real Timeで定常表示を測る。新fixture/schemaにはこのfreeze方針を
明記し、旧densityのdraw gateや旧半透明の期待値を変更しない。

正式画像の人による判断はstyle・材質・状態の読みやすさを扱う。自動ROIはownerと投影位置の一致、
接続部の背景の露出、frame間の変化、シルエットの連続性を検証する。固定RGB値で木材を合格させない。
performance比較は新profileのbefore/afterだけで行い、gallery・previewの短時間frame値を混ぜない。

### 7.3 M3b正式画像の証跡（generation 8）

subject `3c0f7c673e65946584e6e20fe97648dc8f54334b`へ、承認済みbytesをgeneration 8の
isolated candidateとして固定した。authoring final manifest SHA-256は
`244b522deea6f9b3744360a77ce90c94cff179220fd10def4690a2a2e7db04d0`、asset-view fingerprintは
`ad63f1c334c91335851d06aa1b64be76b7ce08da903e53ab425c4f7938e4cc98`である。両jobは
source fingerprint `649e0b255dfa5c2a8393b906eaf27294ee6536e9ffef0a3bc4da3d3d50bf778a`と
harness fingerprint `a0d88a6dd4d77757844a6f910737e8ad8be833eca2a83366111e573c90bd037d`も一致する。

| profile / job | 結果 | manifest SHA-256 | 9 PNG相対path/hash一覧のSHA-256 |
| --- | --- | --- | --- |
| `wall-formwork-v1` / `wall-art-20260907T031245Z-d7fea076` | 9/9 valid、独立verify pass | `0aa1e081067137fea21057047e91d3e7f169aaa5dd449e150141130d4d33575d` | `612c8ecb95541d999321c68773663763f8f1ef2271666f3a14cf7442bbe839b9` |
| `wall-formwork-v1-farthest` / `wall-art-20260907T032953Z-d83fa1ba` | 9/9 valid、独立verify pass | `5db4a2ee8cbcfe4b9daec74dc835e024d595e60c63434f72eacae02a4f8e79b8` | `39a227eaa4c88ae4a8cf620288a3467a5b82f13d3ae1fe5ffe177a07362c45b5` |

先行するcompleted-only補助回帰`wall-art-20260907T022328Z-41b24ffd`（subject `e010355f`）も
9/9 validだが、仮設型枠の証拠には算入しない。subject `e163a067`の最遠job
`wall-art-20260907T030652Z-01574e9d`はLow / DPI 1.0の最弱直線列が`2.74σ`となりinvalidである。
6 px幅のうち5列は`3σ`以上だったため、骨組みが消失したのではなく片側のterrain-mixed raster edgeと診断した。
型枠だけを「中央値`3σ`以上、terrain-mixed列は最大1列」に分離し、2列欠落を拒否するself-testを追加した上で、
generation 8の両jobを最初から採り直した。失敗jobはtrack完了まで保存する。

この証跡が閉じるのは§7の正式画像だけである。W-G01の混在gallery・Soul前後depth、W-L01〜L03、
W-A01〜A02、J1、release受入は引き続き未完とする。Capture / Memoryは§7.4で別に閉じた。

### 7.4 M3b性能・Memoryの証跡（generation 10）

subject `8bb004e7985dc5428348d111b416dbf958d128f2`とgeneration 10のisolated candidateを固定し、
Intel Arc / Vulkan / X11、1280×720、High、DPI 1、immediate presentで正式job
`wall-formwork-density-20260907T151655Z-35eef90a`を実行した。Capture 18 / 18とMemory 6 / 6がvalidで、
独立`verify`もpassした。candidate manifest SHA-256は
`734a2a06940287ea63047f4aaa4e57a8c32aa61cd64899d74f20959e0a8e29d1`、asset-view / source / harness
fingerprintは順に`becb0697cfe6d2ead21e841cd510830e65863ba66de872607b85cdb5b893e378`、
`2b31468034a4f08f1705ae71bf46512e8b1dcc6bb5b14e2cc265b56e6f6ad4f2`、
`04eed7d50369517e9c54eac745fb1d8a201ee9f587cb7deaef9245ba5db60140`である。

| case / metric | fallback中央値 | production中央値 | 差 | 結果 |
| --- | ---: | ---: | ---: | --- |
| provisional 96 / p95 | 9.426943 ms | 9.248062 ms | -1.898% | pass |
| provisional 96 / p99 | 10.260355 ms | 9.943983 ms | -3.083% | pass |
| provisional 384 / p95 | 16.241659 ms | 16.219445 ms | -0.137% | pass |
| provisional 384 / p99 | 17.506656 ms | 17.449946 ms | -0.324% | pass |
| mixed 384 / p95 | 16.157502 ms | 16.182783 ms | +0.156% | pass |
| mixed 384 / p99 | 17.391257 ms | 17.434654 ms | +0.250% | pass |
| mixed 384 / max RSS | 1,430,088 KiB | 1,339,400 KiB | -6.341% | pass |
| mixed 384 / peak live | 665,926,330 bytes | 665,867,492 bytes | -0.009% | pass |

manifest / Capture result / Memory resultのSHA-256は順に
`26416748d862a24359b34a6536f2b5c97e551c301afcf930f6a801ecdc8ca0a2`、
`259db705b8db17117e226354d8639af5c77365aa45c5f614341a805ddd8e279a`、
`24fab503fabe8093a11a826610394baea75636a7520745d8c8126047def9017a`である。
最初のgeneration 9試行は非対象Door locator欠落を実行開始後に検出してinvalidとなった。この経路をplan / run / matrix /
verifyのbuild前preflightへ移し、generation 10で最初から採り直した。失敗jobもtrack完了まで保存する。

native実行はSkillのdirect `kitty` launcher、repository lock、RAM/disk preflightと逐次buildを使う。
本計画の新profileには`plan / status / verify / self-test`を実装してから利用し、未実装のcommandを
既存helper名へ読み替えない。statusは15〜30秒ごとに確認し、source/asset drift・timeout・画像欠落はfailとする。

実装中は`python3 scripts/dev.py check`と
`python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`。
実装完了時は`python3 scripts/dev.py verify`（workspace testsを含む）、`git diff --check`、
Help impact review Skillを実施する。仮設の見分け方が変わるため、Helpの実際の建築説明を読み、
Update required / 理由付きNo impactをその時点で判断する。

## 8. ロールバック方針

- 新generationは旧releaseから独立して保持し、失敗時は検証済み旧pointer / projectionへ戻せるようpreimageを記録する。
- schema後継readerの旧v2互換を先に検証する。productionでは部分的に新旧meshを混ぜない。
- セーブの変換は不要。Rustの差戻しが必要なら履歴と全差分・並行作業を確認した上で対象変更だけをrevertする。
- 無効artifactはtrack中保持し、close時に必要な封印記録を残して本trackの作業場を整理する。

## 9. AI引継ぎメモ

### 現在地

- 進捗: M1/W2の中核を実装。schema v3候補、runtime wallset v2、6 family GLB、Opaque木材albedo、tile単位mesh/material切替、ArtPreview authorityが動作する。M3aはgeneration 5の技術候補とゲーム所有window PNGに対してユーザー承認済み。generation 8の正式画像18/18、generation 10のCapture 18/18とMemory 6/6、および各独立verifyが合格した。通常releaseはgeneration 4のまま。
- W-L01中核: 2 tileを仮設2→仮設1/本設1→本設2へ遷移させ、phase別mesh/materialとowner / visual Entityの同一性、visual root非増殖をproduction回帰テストで固定した。native storyboardの採取は未完。
- 次の作業: 同じcandidate bytesを使い、W-G01の混在gallery・Soul depthとW-L01の実機storyboard、W-L02〜L03 / W-A01〜A02のstateful storyboardを実装・採取する。Door側の単独gate後にJ1、M4へ進み、性能jobのMemory値をload反復のasset-pool安定性へ読み替えない。
- ドアはM0の共通port確定後に制作を並行可能。Rust・tooling編集はmain agentが順に行い、joint受入は両方のruntime接続後。
- 240 triangles・core 15 file・§7の予算は採用した新設計値。現在のreleaseの実測・アート承認値として扱わない。

### 参照必須ファイル

- `docs/building.md`、`docs/room_detection.md`、`docs/art-style-criteria.md`、`docs/assets_workflow.md`、`docs/blender-setup.md`、`docs/rendering-performance.md`。
- `crates/bevy_app/src/systems/visual/wall_presentation.rs`、`crates/bevy_app/src/assets/wall_asset_set.rs`。
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/coat_wall.rs`、`crates/hw_visual/src/wall_connection.rs`。
- `.codex/skills/hell-workers-run-native-acceptance/SKILL.md`、`.codex/skills/hell-workers-review-help-impact/SKILL.md`。

### 最終確認ログ

- 計画作成: `2026-09-05`。Rust / runtime asset変更なし。
- セルフレビュー: `2026-09-06`。§0の指摘を計画へ反映。実装・アセット制作・native起動は行っていない。
- 文書gate: `python3 scripts/dev.py docs --write`で索引を同期後、`docs --check` / `git diff --check`がpass。
- Help gate: `No impact`。性能受入基盤・非対象asset preflight・証跡文書の追加だけで、通常の入力、ゲームプレイ、表示ロジック、UI文言、Help coverage、release authorityは不変。理由付き`check_help_impact.py`がpass。
- 最終検証: `wall_formwork_density_acceptance.py self-test / verify`、`scripts/perf.py self-test`、`python3 scripts/dev.py check`、全workspace testとClippy `-D warnings`を含む`python3 scripts/dev.py verify`がpass。
- ブロッカー: Wall final封印・正式画像・性能・Memoryは解消。Wallのstateful lifecycle、混在gallery / Soul depth、Door単独残件、J1が未完であり、Wall単独M3全体はまだ完了扱いにしない。

### Definition of Done

- [ ] M0〜M4と共通J1完了、木枠と石壁の識別性・全lifecycle・ドアseamが成立。
- [ ] Help影響判断と恒久文書更新が完了、check / Clippy / verify成功。
- [ ] release authorityによる実機受入と最終候補の承認記録がある。
- [ ] 各jobのmanifest、比較CSV、承認PNGを小さなcapsuleへ保存し、hashをclose文書へ記録。
- [ ] 本trackで作ったvalidation worktree・branchを廃棄。`git worktree list`、削除前後の`du -sh`、回収容量を記録し、他sessionの作業場に触れていない。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-09-05` | `Codex` | 木製型枠を推奨し、段階別mesh・後継asset契約・tile完成・実機受入を計画化 |
| `2026-09-06` | `Codex` | セルフレビュー。部材寸法と三角形予算、2 source/schema互換、承認前preview、baseline前倒し、J1、数値gate・10回loadを具体化 |
| `2026-09-06` | `Codex` | M1/W2中核のtooling・型枠6 family・runtime schema v2・段階別表示・ArtPreview経路を実装。通常releaseは未変更 |
| `2026-09-06` | `Codex` | ユーザーがM3a ArtPreviewを「OKです」で承認。候補・実機job・PNG hashをapproval artifactへ封印し、final seal / isolated-candidate projection toolingを追加。通常releaseは未変更 |
| `2026-09-06` | `Codex` | Door generation 5もArtPreview承認済みとなり、Door M2統合とDoor専用approval/final/candidate toolingを共通freezeへ追加。両trackとも正式M3b前で通常releaseは未変更 |
| `2026-09-06` | `Codex` | clean統合subject `49c43e0f`でDoor generation 6 finalを封印し、isolated candidateの実機visual legを通過。Wall finalと両単独M3b残件、J1前のため通常releaseは未変更 |
| `2026-09-07` | `Codex` | subject `3c0f7c67`・Wall generation 8で正式画像profileを標準／最大zoom-out各9 case実行し、18/18 validと独立verifyを記録。stateful lifecycle・Capture・Memory・J1・releaseは未完 |
| `2026-09-07` | `Codex` | 正式性能ランナーの旧6 mesh固定をschema別exact inventoryへ修正。schema 2でも完成6件は72 tri、型枠6件だけ240 triを適用し、性能採取前のresident再検証を実装 |
| `2026-09-07` | `Codex` | 凍結済み旧density契約と分離した`wall-formwork-density-v1`、仮設／本設の混在384、Capture 18＋Memory 6の逐次・隣接counterbalance runner、raw artifactまで辿る独立verifyを実装。実機値は未取得 |
| `2026-09-07` | `Codex` | subject `8bb004e7`・Wall generation 10でIntel Arc / Vulkan / X11のCapture 18・Memory 6を正式採取し、24/24 validと独立verifyを記録。generation 9で判明した非対象Door欠落をbuild前preflightへ移した |
| `2026-09-07` | `Codex` | W-L01の2 tile仮設→混在→本設遷移をproduction回帰テストへ追加。phase別mesh/materialとowner / visual Entity同一性、root非増殖を固定し、native storyboardは未完として分離 |
