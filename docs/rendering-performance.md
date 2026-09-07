# レンダリングパフォーマンス

描画パイプラインごとの draw call 構造・バジェット・最適化方針をまとめる。

---

## 1. パイプライン構成

このゲームは 3 つの独立した描画パイプラインを持つ。draw call のカウントは各パイプラインで別枠になる。

| パイプライン | 内容 | draw call に影響するもの |
|---|---|---|
| **3D RtT** | 地形・建築物・Soul | `Camera3dRtt` の frustum 内にある 3D entity |
| **2D world** | 夢の泡パーティクル・前景スプライト | `Material2d` / `Sprite` を持つ 2D entity |
| **UI** | DreamBubbleUiMaterial・UI ノード | UI パイプライン（`UiMaterial` 等） |

### P00 current inventory（登録済みhistorical baseline）

single Scene RtT / indoor light migration前のcurrent構成は次である。下記inventoryはP00 canonical
RenderDoc formal legと照合済みであり、現sourceのstartup testではなく登録済みartifactが正本である。

| 項目 | current |
|---|---:|
| Camera3d RtT | 2（Scene 1、Soul mask 1） |
| world color target | 2（Scene 1、Soul mask 1） |
| Camera2d | 3（Main、Overlay、WorldForeground） |
| FHD High target | 各1920×1080、2 handleはdistinct |
| DirectionalLight entity | 2（標準1 active、extra 1 default disabled） |
| composite sampled texture | 2（Scene + Soul mask） |

### P01 Scene-only runtime inventory

現sourceはP01移行によりScene-onlyである。`RttRuntime`、Camera3d、world color target、composite sampled
textureは各1となり、Soul mask camera / target / proxy / material / layer / toggleは存在しない。P00互換CSVの
mask列は削除せず、P01 stageでは意味上の0を記録する。`p01_scene_only_rtt_startup_inventory_is_explicit`とvisual_testの
resize testは、Scene targetの再生成後にCamera3dとcomposite materialが同じhandleへrebindされることを検証する。

| 項目 | P01 source |
|---|---:|
| Camera3d RtT | 1（Scene） |
| world color target | 1（Scene） |
| Camera2d | 3（Main、Overlay、WorldForeground） |
| DirectionalLight entity | 2（標準1 active、extra 1 default disabled） |
| composite sampled texture / sampler | 各1（Scene、binding 1 / 2） |
| Soul mask target / camera / proxy | 0 |

### P02 TopDown presentation runtime inventory

P02 は Scene-only RtT を維持したまま、MainCamera を composite 後の唯一の `LAYER_2D` camera にする。structural Building は3D、foreground Buildingは2Dのどちらか一方だけが active presentationとなる。Soulは共有pool billboard 1 / owner、Familiarは2D前景のみである。

| 項目 | P02 source |
|---|---:|
| Camera3d RtT / Scene target | 1 / 1 |
| Camera2d | 2（Overlay、Main） |
| active `LAYER_2D` pass | 1 |
| Soul billboard / Soul | 1 |
| Soul GLB / shadow proxy / Familiar 3D proxy | 0 / 0 / 0 |

P02 固有値は legacy `render_inventory.csv` を読み替えず `p02_presentation.csv` と RenderDoc checkpoint の `p02_presentation` blockへ出す。formal gateは duplicate=0、全Building exactly-one、billboard ratio=1、Familiar3D=0、state/bounce probe=trueを同一 medium/GPU checkpointで評価する。

画像側の補完はproduction indoor-light fixtureの専用actual-window matrixが所有する。18 caseすべてでgame process所有の
X11 clientから、Door Open / Closed / Locked、Soul前 / 後、Bridge、Wall bounce active / rest、Foreground animation A / Bの
10 ready phaseを採取する。nonce / generation ACK付きsemantic sidecarは対象owner・ROI・render modeを束縛し、PNGはDoorの
状態差、Soul alpha/depth、Bridge visible / hidden、bounce、foreground差分を再計算する。BridgeはさらにRtT cameraとの
`RenderLayers`交差、期待mesh / material handle、asset registry在籍をsource側でfail-closedに確認する。desktop全体や独立
`visual_test`の画像だけではP02受入にならない。

この18 case profileはP02当時のsourceとlegacy structural mirror inventoryを含む凍結契約である。P08以降の
`Structural3d`はDoor / Tank / MudMixerもowner-linked 3D visual exactly one、子Sprite 0へ移行済みなので、現行sourceを
P02 selectorへ渡して期待表を緩和しない。現行の完成Wallの品質／DPI回帰は`wall-art-approved-candidate-matrix-v1`が所有し、
High / Medium / Low × DPI 1.0 / 1.5 / 2.0を9つの逐次X11 client captureとして検証する。historical P02は登録済み
immutable artifactのhashとlocatorをoffline再検証する。`perf.py`側の内部`--wall-art-matrix` authorizationは
Wall native helperだけが付与し、通常のsingle-case校正とformal densityの固定quality / DPI契約は変更しない。

承認済みprovisional Wallは別の`wall-formwork-v1` / `wall-formwork-v1-farthest`が所有する。
`--perf-wall-formwork-acceptance`と`HW_WALL_FORMWORK_ACCEPTANCE=1`が揃ったschema 2の隔離candidateだけを許可し、
active handleが完成Wallではなく6つのformwork mesh集合に属することと、木材materialがOpaqueであることを
game process内で検査する。標準／最大zoom-outを各9 case採るため正式画像は18枚であり、completed-only galleryを
型枠合格へ読み替えない。最大zoom-outの型枠は最小6 px幅になるため、完成石壁の全列`3σ`契約とは分け、
中央値が`3σ`以上かつ地形と混ざるraster列を最大1列だけ許す。2列以上の欠落はfailとする。

本番Wallの正式frame比較は`wall-production-performance-v2`が所有する。凍結済み
`wall-density-v1`のlayout sidecarへ上書きせず、追加の`wall_density_presentation.json`で
計測開始時と終了時の表示状態を結ぶ。`production`は候補generation / manifest hashと
`ReadyToApply`、全targetのproduction一括収束を要求する。`fallback-control`は同じbinaryから候補認可を除き、
`CandidateDisabled`と全target fallbackを要求する。旧schemaではproduction 6 mesh、新しい型枠schemaでは完成6＋型枠6の
production 12 mesh、いずれもproduction 2 material、fallback 1 mesh / 2 materialのresident poolを要求する。
family / rotation分布と、完成mesh 72 tri・型枠mesh 240 triの上限が成立してからwarm-upへ進む。
binaryの`--perf-wall-presentation`とlauncherの`HW_WALL_PERF_PRESENTATION`は同値の二重鍵であり、通常runや
Wall art actual-window profileからは指定できない。completed / provisionalのN=96 / 4N=384を各3 run採り、
同一final binaryのfallback-control比p95 / p99中央値を各`+5%`以内で判定する。v2は各
`phase × size × run`のfallback-control / productionを隣接実行し、先行modeをpair間で交互にする。
全24 runの予定順と完了順を`capture-order.json`へ固定し、片側を全件先に測る時間帯バイアスを拒否する。
さらにv2は、比較可能な計測条件だったかを2点で検査する。各run直後に`frames / measure_secs`とp50を見て
表示フレームクロック（60 Hz）へ張り付いたrunを即座に拒否し、同一cellの3 runでp50中央値の最大/最小が`1.25`を
超えるsessionも拒否する。判定値はregimeとして`capture-order.json`へ封印し、独立verifyが生artifactから
再計算して突き合わせる。これはacceptance閾値ではなく前提条件であり、`+5%`は変更しない。
Wayland / Xwaylandのcompositorがwindowをpaceすると、GPUに余力（min frame time `7.60 ms`）があっても
提示が60 Hzで律速され、p95 / p99が壁ではなくcompositorを記述する。
production provisionalは半透明albedoとshared light fieldを保ちつつ、transparent passのtexture sampleを抑えるため
emissive textureをbindしない。completedだけが承認済みemissive textureを使用する。

本番Doorの静的密度比較は`door-density-v1`を使う。N=32 / 4N=128 Doorと64 / 256 completed support Wallを
同一layoutへ固定し、EW/NSとClosed/Open/Lockedを均等に近い分布で保持する。productionとfallback-controlは
同じbinaryを使用し、Doorだけをproduction 3 mesh＋共有1 materialまたはfallback共有1 mesh＋状態別3 materialへ
切り替える。`door_density_fixture.json`は開始／終了の同一性、候補identity、active/resident pool、3 production imageを、
`door_density_layout.csv`は全Doorと支持Wallのgrid・軸・状態を記録する。Captureはp95/p99の`+5%`、Memoryは
max RSSの`+5%`とpeak live bytesの`+4 MiB`を上限とし、Virtual Time停止中のReal Time計測として扱う。
subject `addc9004`の正式job `door-density-20260907T005953Z-fc28520e`はIntel Arc / Mesa 26.1.6 /
Vulkan / X11でCapture 12 runsとMemory 6 runsを完走し、独立verifyもpassした。production中央値は
fallback-control比で、Nのp95 / p99が`+0.714% / -0.830%`、4Nが`+1.613% / +1.714%`だった。
4N Memoryはmax RSSが`-6.601%`、peak live bytesが`+70,902 bytes`で、6比較すべてが上限内である。
このprofileは静止Door表示の比較であり、開閉操作やload成立の証拠には使わない。
旧v1 subject `991392b8`のIntel Arc / Mesa 26.1.6 / Vulkan / X11実測では、completed N / 4Nの
p95回帰が`+0.451% / +0.438%`、p99が`+0.910% / +0.540%`、provisionalのp95が
`+2.031% / +0.987%`、p99が`+1.134% / +0.369%`となり、全ケースが`+5%` gateを通過した。
24 runと4比較の独立verify対象は
`target/native-acceptance/wall-production-performance-20260902T173804Z-3f6bc903`である。
この値は`991392b8`の履歴passである。current final subject `ef5f2b8b`ではfresh job
`wall-production-performance-20260902T213112Z-837efcb8`が24 / 24 valid runを採ったものの、completed N p99が
fallback `9.905611 ms`対production `10.506062 ms`（`+6.062%`）でfail-closedとなった。load安定後の再測定
`wall-production-performance-20260902T221218Z-96e4b21b`も24 / 24 valid runを採ったが、completed N p95が
`9.171164 ms`対`14.011493 ms`（`+52.778%`）で再失敗した。後者のproduction Nではrun 2 / 3に約8.3 / 9.9秒の
連続stutter区間があり、p50は`+3.957%`、completed 4N p95 / p99は`-0.078% / +0.468%`だった。
単一外れ値として合格扱いせず、この2 jobはinvalidのまま保持する。
generation 4での再採取は2 job続けて計測条件側の問題で無効になった。
`wall-production-performance-20260903T131432Z-78414216`はcontrolが1808〜3363 frames、productionが
2198〜5269 framesと両modeが別regimeで走っており、completed Nの`+190.461%`はこの非対称性の産物である。
`wall-production-performance-20260903T164122Z-3b424e6f`はseq 4以降の全runが3601 frames / 60.02 fpsへ
paceされていた（同一caseのrun-001は8041 frames / p50 `7.43 ms`）。いずれも上記のregime検査で拒否される。
本番Wallのframe-time判定は、有効regimeで完走した`3f6bc903`（generation 2、全8行`+5%`内）と、
generation 4が同一material / texture / draw経路のままtriangleを216〜240から24〜72へ減らした差分を根拠とする。
wall geometryやmaterial経路を増やす変更を入れる場合は、この根拠を流用せずv2 profileで新規採取する。

M0対productionのcross-subject比較は、M0 `35f1f6e3`へDoor connector fixture修正だけを載せた
派生subject `0241d3b9`を使う。density helper / Rust fixture / JSON contractのSHA-256はそれぞれ
`104584a6…` / `26200bab…` / `7b32f4e0…`でproduction側とbyte-identical、asset viewも
`ebeb81af…`で一致する。`wall-cross-subject-performance-v1`は両job自身の凍結verifierを再実行し、
絶対pathである`BEVY_ASSET_ROOT`だけを正規化して、他のrequested environment、actual adapter、matrixを
厳密比較する。`0241d3b9`対`991392b8`の履歴比較ではcompleted N / 4Nのp95が
`-22.313% / +0.852%`、p99が`-37.737% / +0.816%`、provisionalのp95が
`-13.160% / +0.924%`、p99が`-37.360% / +0.681%`で全件passした。sealed capsuleは
`wall-m5-cross-subject-0241d3b9-vs-991392b8`である。後述のproduction RenderDoc checkpoint v2が
source fingerprintを更新したため、この値は経路成立を示す履歴証拠として保持し、最終M5判定は新subjectの
production sessionから再比較する。

production WallのRenderDoc checkpointはschema 2を使い、単一fallback Cuboidのindex数ではなく、
active mesh index数集合とindex数ごとの期待instance合計、active mesh数、presentation mode、candidate identityを
記録する。extractorは同じcolor+depth passから集合に属するdrawだけを抽出し、index/instance分布とowner総数が
checkpointへexact一致しないcaptureを拒否する。launcherの`--candidate`はcandidate generation / manifest / asset viewと
`--perf-wall-presentation production` / `HW_WALL_PERF_PRESENTATION=production`を対にする。これにより6 familyのうち
同じindex数を持つmeshを正しく集約しながら、fallbackや無関係drawをproduction draw-groupへ数えない。
`ef5f2b8b`のfresh job `wall-renderdoc-20260902T200533Z-c3f9b66d`はcompleted Nでactive mesh 6、owner 96、
draw group 6、index別instance `648:30 / 720:66`をexact一致させたが、completed 4Nの1回目replayがGPU fence待ちで
600秒timeoutとなった。build cache済みの再試行`wall-renderdoc-20260902T204607Z-30b0d6c5`も同じ箇所でtimeoutし、
kernel logにGPU hang / resetは記録されなかった。両jobはinvalidのまま保持する。
このtimeoutはGPU hangではなくharness側の`REPLAY_TIMEOUT_SECONDS = 600`不足だった。~550 MB captureのreplayは
N=96で約300秒、4N=384で828秒を要するため、上限を2400秒へ広げてから4 caseが初めて完走した。
subject `080bee95`のjob `wall-renderdoc-20260903T175737Z-4df5356f`はcompletedがN / 4Nとも
`draw group 6`、rendered instanceが96 / 384でcheckpointed ownerと一致し、`D_N <= 6` / `D_4N <= 6` /
`D_4N = D_N`を満たした。provisionalは`69 / 286`である。同一subjectでcandidate認可を外したfallback対照job
`wall-renderdoc-20260904T004450Z-e1ab0458`は全4 caseが`draw group 1`であり、半透明の分解が1 meshでは起きず
6 meshで起きることを示す。不透明passはmesh単位でbatchされて密度に依らず6 drawで頭打ちになるのに対し、
半透明passは深度ソート順に描くため隣接する壁のmeshが変わるたびにbatchが切れる。
そこでprovisional predicateを`D <= ceil(K * (M - 1) / M) + 1`（M=6、上限はN `81` / 4N `321`）と
`D_4N / D_N <= 4 * 1.10`へ導出し直した。前者はmesh identityと無相関な順序での期待切断数を上限にしたもので、
これを超えることはmergeが成立していないことを意味する。旧`D_4N <= 4 * D_N + 6`の定数余裕6には導出がなく、
mergeが期待より効いている286を282で弾いていた。
本番Wallの受入profileは4つある。`wall-art-approved-candidate-matrix-v1`（隔離candidateの標準zoom 9 case）、
`wall-art-approved-candidate-farthest-zoom-v1`（同じ9 caseを最遠zoom-outで撮り、直線specimenの連続性を判定）、
`wall-renderdoc-v1`（draw group構造）、`wall-art-released-generation-v1`（昇格後に通常起動と同じ
release authorityで撮る9 case）である。最後のprofileは`--candidate` opt-inを使わず、gallery viewだけを
`HW_WALL_ART_GALLERY=1`で要求し、projectionが指すreceiptをrepo実体とgeneration receiptへ突き合わせる。
subject `45af5355`のjob `wall-art-20260905T115042Z-9572a891`が9 / 9 case validで、全caseが
`authority=release_approved`、generation 4、production 96 / fallback 0、`fallback_mesh_resident=false`である。

Wallのsave/load表示にはactual-window証跡がない。rehydrate後にfallbackで可視化されること、world replace後に
一括でproductionへ戻ることは`rehydrated_wall_starts_in_visible_fallback_at_its_world_position`と
`wall_presentation.rs`のfocused testが担保する。画像側の証跡が必要になった時点で、save/load phaseを持つ
actual-window scenarioを追加する。

Door production候補のruntime poolは3状態mesh＋1 shared material、albedo 1＋EW/NS preview 2で、owner数に比例してcloneしない。候補galleryはClosed / Open / Locked×EW / NSの6 ownerを通常の`Building3dVisual` consumerへ通し、production 6 / fallback 0、状態別mesh、解決軸、candidate identityを`TransformSystems::Propagate`後のsidecarへ記録してからX11 client captureを許可する。このgalleryは`evidence_kind=art_preview`であり、通常起動や正式performance baselineの証拠には使わない。

導出後のsubject `48743206`のjob `wall-renderdoc-20260904T153035Z-e91b1bb9`は`status=valid`で封印され、
独立verifyも`pass`となった。completed `D_N = D_4N = 6`、provisional `69 <= 81` / `286 <= 321`、比`4.14 <= 4.4`で、
4 caseとも`presentation=production`、rendered instanceは96 / 384である。

最遠zoom-outの視認性は`wall-art-approved-candidate-farthest-zoom-v1`が所有する。
`--perf-wall-art-zoom farthest`と`HW_WALL_ART_ZOOM=farthest`の二重鍵が揃った受入計測時だけ、gallery camera scaleを
`PanCamera`最大zoom-outの`5.0`にする。通常起動、formal density、standard zoom matrixの契約は変更しない。
判定はmask `0011`のE-W直線specimenの投影バンドで行い、固定色ではなく同一ROIの地形行から平均と標準偏差を取り、
各列の最暗画素が地形より`3σ`以上暗いことを全列へ要求する。バンド幅は投影されたcell幅から導く（specimenは
互いに接続しない1 cellで、camera scale 5では約6.4 pxしかない）。subject `eebeba69`のjob
`wall-art-20260904T180842Z-eb5a5925`は9 / 9 caseがvalidで独立verifyもpassし、最弱列のsigmaはHigh
`5.09 / 6.78 / 22.40`、Medium `13.10 / 9.93 / 13.34`、Low `6.37 / 9.57 / 8.53`である。

current final subjectのactual-window matrixは
`wall-art-20260902T210312Z-2ab3b1c9`である。High / Medium / Low × DPI 1.0 / 1.5 / 2.0の9 / 9 caseが
Intel Arc / Mesa 26.1.6 / Vulkan / X11でpassし、独立offline verifyも9 screenshotを再検証した。全caseで
production 96 / fallback 0、distinct mesh 6 / material 1、connector visual 192 hidden / 0 visibleであり、全PNGの目視でも
品質／DPI固有の欠落、断線、黒抜け、fallback混在はない。

### P06 shared Light Field runtime inventory

P06はPoint／Spot Lightや追加shadow map／local-light passを生成せず、P01の単一Scene RtTとP02のTopDown presentationを維持する。CPU fieldは1つのlinear RGBA8 `Image`へrevision単位でuploadされ、Terrain 3 pipelineとstructural 1 pipelineはそれぞれtexture／samplerを1組だけbindする。Wall／Doorのper-instance sampling anchorは`MeshTag`にあり、material handle数はLamp数・Building数に比例しない。

| 項目 | P06 source |
|---|---:|
| Light Field image / live handle | 1 / 1 |
| canonical logical payload / padded staging | 40,000 / 51,200 bytes |
| receiver material pipeline | Terrain LOD1、LOD1-lite、LOD2、TopDown structural |
| receiver binding / pipeline | texture 1 + sampler 1 |
| Point／Spot／shadow map／local pass増分 | 0 / 0 / 0 / 0 |

`TopDownStructuralMaterial`は既存section discard、wall build progress、alpha／prepass、directional shadow stylingを保持した`ExtendedMaterial`である。productionのWall、Door、Floor、Bridge、Tank、MudMixer、RestArea、SoulSpaは有限共有handleへ移行済みで、Soul／Foreground2dはLight Fieldをbindしない。

P00のmeasurement contractはfrozenの`rtt-light-v1`である。canonical contract hashは
`ba5d6bf7320426b441465df8fae42d6ff80820748ce55e0edf0dbba409dc755a`、fixture hashは
`a688d564f8f50c2fdcdbe49dca7625b2cb05d01f8555378215fb8ba89b553eed`である。stage別projection義務と
gate expected row、resolved window backend / effective present modeの開始・終了検証、formal attempt
validatorは実装済みである。P05 evidence定義を補完したadditive改訂では、fixture / threshold不変を条件に旧hash
`121a365ac3349cd4fa7890ab3069f0392098ced17e0d47f920095a1490c2ba11`をP04までの履歴entryだけにexact pinする。
旧attemptのraw inventory、locator、SHA ledgerは保持し、P05以後へ旧hashを許可しない。以後の非additive変更は同じv1を編集せず新generationを追加する。

P00 canonical current baselineはsubject `10763a4da6bfbe0b480971fb85c474e6ff7a5f86`、attempt
`9e813f24-0f7b-47f5-8a8d-e3ff34775370`として登録済みである。Intel Arc / Vulkan / X11、1920×1080、DPI 1.0、
High、immediate presentを記録し、audit / behavior / Capture / RenderDoc / Memoryの全legをoffline verifierと
baseline registry verifierで再検証した。raw artifact 884件のdirectory SHA256は
`a9f4927186fe7c8c5f009583fd645cd7963b6ad36398e9d614fbcbbff1f8a6aa`である。

P01 canonical Scene-only candidateはsubject `29a4a719e9fe92b10618f36ce548c4bb5a4c7e80`、attempt
`8bc82f04-10ac-4903-89b6-89011dacdada`として登録済みである。同じcontract / fixtureとIntel Arc / Vulkan / X11環境で、
audit / behavior / Capture / RenderDoc / Memoryの全5 leg・18 caseを再検証した。gate ledgerは123 / 123 row pass
（`RLV1-P01-RTT` 9 row、`RLV1-P01-PERF` 20 row）で、raw artifact 884件のdirectory SHA256は
`68e470e51cf30f7659bb87eb1893235758d49f2c9d7a1e8f881f0e5f2a9f7502`である。

| case | frame p50 / p95 / p99 (ms) | max RSS (KiB) | allocator peak live (bytes) |
|---|---:|---:|---:|
| small / cpu | 14.701 / 21.441 / 24.271 | 1,338,744 | 669,949,156 |
| small / gpu | 28.567 / 36.939 / 40.385 | 1,519,344 | 687,635,837 |
| medium / cpu | 17.846 / 24.675 / 27.604 | 1,388,468 | 696,767,330 |
| medium / gpu | 30.272 / 38.812 / 42.531 | 1,475,668 | 732,618,413 |
| large / cpu | 23.772 / 30.750 / 34.008 | 1,425,912 | 739,108,778 |
| large / gpu | 33.679 / 42.488 / 46.635 | 1,527,076 | 801,151,582 |

P02 subject `6ea0bf99391b1660607537304a3764f380a10eac` / attempt
`54d85a63-e237-4501-a0d0-33c1d0a29f3b`はfrozen v1 formalの履歴として保持する。actual-window profile v1は
画像predicateとraw artifactの改竄再検証が不足するため、review remediation後のP02完了証跡には使わない。直後の表は
この履歴測定値であり、現subjectのperformance結論ではない。

| case | P02 frame p50 / p95 / p99 (ms) | P01比 p95 / p99 |
|---|---:|---:|
| small / cpu | 14.218 / 20.828 / 23.624 | -1.97% / -0.39% |
| small / gpu | 21.427 / 30.843 / 35.826 | -10.24% / -8.60% |
| medium / cpu | 17.582 / 24.267 / 27.221 | +3.36% / +3.11% |
| medium / gpu | 22.427 / 29.765 / 32.775 | -6.81% / -5.93% |
| large / cpu | 23.292 / 30.665 / 33.831 | +4.54% / +3.42% |
| large / gpu | 26.025 / 34.236 / 37.935 | -15.87% / -14.99% |

review remediation後のcanonical P02 candidateはsubject
`c3515a40543026a588592682889a08d67cbaeff9`、attempt
`9ff336ef-1312-4248-b0bf-bb454111decc`である。Intel Arc (MTL) / Mesa 26.1.5 / Vulkan / X11で、actual-window
v9（schema 7）18 / 18とAudit / Behavior / Capture / RenderDoc / Memoryを再検証した。formal gate ledgerは128 / 128 row
pass、raw artifact 932件のdirectory SHA256は
`a6b76b64ca8df601d051abeb15589ffd464fd1bd41dfe61c9f25fa734f6a0e72`である。P01比のframe hard gateは全12 rowがpassし、
positive maximumはmedium / cpuのp95 +2.52%、p99 +1.96%である。

| case | P02 frame p50 / p95 / p99 (ms) | P01比 p95 / p99 |
|---|---:|---:|
| small / cpu | 14.411 / 20.727 / 23.212 | -2.44% / -2.13% |
| small / gpu | 21.891 / 30.907 / 35.829 | -10.05% / -8.60% |
| medium / cpu | 17.653 / 24.071 / 26.917 | +2.52% / +1.96% |
| medium / gpu | 22.350 / 29.823 / 32.759 | -6.63% / -5.98% |
| large / cpu | 23.487 / 29.990 / 33.258 | +2.24% / +1.67% |
| large / gpu | 26.245 / 34.124 / 37.606 | -16.15% / -15.73% |

frame値はCapture leg、RSS / allocator値はMemory legの正本であり、相互に代用しない。P01以降は同じcontract /
fixture / adapter matrixとstable projectionで比較する。

P00 RenderDoc runtime checkpoint schema v3とextraction schema v2は、historical current inventoryをGPU replayで厳密化する。composite drawはfragment
descriptor set 2のScene texture / sampler `(1, 2)`、Soul mask texture / sampler `(3, 4)`を同じ1 drawで
使うことを要求する。抽出はVulkan subpass transitionを正しく分割し、全drawに散らばったsampler数では代用しない。
canonical formal captureではVulkan 18 render pass、212 draw、516 attachment record、1,996 binding record、
composite draw 1、Scene target attachment / binding各1、Soul mask target attachment / binding各1を実測した。
compositeのScene texture / sampler `(1, 2)`、Soul mask texture / sampler `(3, 4)`も同一drawで一致する。raw RDCは
697,940,813 byte、SHA256は`5b33c53d0f81da746f92136edc1f1fe2143a97db89369a2ad70ba20654823f42`である。
RenderDoc binary SHA256は`5d0ac3accba20db0d9071ea036a770e1b236884335927121b64f4e873c9efb2f`、App APIは
requested 1.6.0 / returned 1.7.0、capture / replay processはすべてexit 0かつorphan 0だった。

P01 canonical RenderDoc captureは14 render pass、163 draw、369 attachment record、1,780 binding record、
composite draw 1を実測した。tracked world color resourceはScene targetだけで、compositeのfragment set 2は
Scene texture / sampler `(1, 2)`を各1回使用し、Soul mask target / attachment / binding / sampleは0である。
raw RDCは695,577,267 byte、SHA256は
`de501ede816213662e86eca2c63983667a0b87dd7ad5700e3e86b80b5337cfbf`である。

P02 canonical RenderDoc captureは11 render pass、88 draw、147 attachment record、1,217 binding record、
composite draw 1を実測した。tracked world color resourceはScene targetだけで、compositeのfragment set 2は
Scene texture / sampler `(1, 2)`を各1回使用する。同じcheckpointでworld `LAYER_2D` pass 1、duplicate
presentation 0、全Building exactly-one、Soul billboard ratio 1、Familiar 3D 0、state / bounce probe trueを検証した。
raw RDCは705,337,616 byte、SHA256は
`38561b2ad8a9146aef6b80c269295b9d6cc49c5478dc807a316856d393f03120`である。

RenderDoc leg は通常の `profiling` output を使わず、`profiling-renderdoc` feature と同名の専用 Cargo profile
で build した capsule を使う。専用 profile は `profiling` を継承しつつ debug assertions を有効にし、native build の
RAM peakを抑えるため LTO を無効化して codegen unit を 16 に固定する。これは `wgpu-hal 29.0.4` が debug assertions
無効時に RenderDoc bridge を無効化するためであり、Capture / Memory
の通常性能 profileへこの条件を波及させない。

---

## 2. draw call の基本規則

### 発生条件
- **画面内（frustum カリング後）** のエンティティのみ draw call を生成する
- 画面外・VRAM キャッシュ済みのアセットは draw call に含まれない

### 自動インスタンシング（Bevy）
同一の `Handle<Mesh>` かつ同一の `Handle<Material>` を持つ entity は自動バッチされ、インスタンス数に関わらず **1 draw call** になる。

### バッチが壊れる条件

| 条件 | 結果 |
|---|---|
| entity ごとに `materials.add(...)` でハンドルを生成している | 1 entity = 1 DC |
| 状態変化のたびに material を clone/mutate している | variant 数 × DC |
| `RenderLayers` が異なる | レイヤーごとに分離 |
| `AlphaMode::Blend` と `Opaque` が混在 | 透過パスと不透過パスで分離 |

---

## 3. 現行の draw call 構造（3D RtT パイプライン）

### 地形

| 要素 | DC 数 | 備考 |
|---|---|---|
| `TerrainSurfaceMaterial` (LOD1) | 1 | 49 chunk が同一ハンドルを共有 |
| `TerrainSurfaceMaterialLod1Lite` (LOD1-lite) | 1 | 同上 |
| `TerrainSurfaceMaterialLod2` (LOD2) | 1 | 同上（LOD 切替で一方だけが有効） |

LOD 切替閾値（hysteresis）:

- `LOD1 -> LOD1-lite`: `tile_rtt_px < 22px`
- `LOD1-lite -> LOD1`: `tile_rtt_px > 25px`
- `LOD1-lite -> LOD2`: `tile_rtt_px < 14px`
- `LOD2 -> LOD1-lite`: `tile_rtt_px > 16px`

### 建築物（現行プレースホルダー）

| 要素 | DC 数 | ハンドル管理 |
|---|---|---|
| 壁（完成） | 1 | `Building3dHandles.wall_mesh` + `wall_material` |
| 壁（建設中） | 1 | `wall_provisional_material`（別マテリアル） |
| 床・ドア・設備 | 各 1 | 種類ごとに 1 ハンドル |

`Building3dHandles`（`startup/visual_handles.rs`）が全ハンドルを Resource として保持し、
entity はこれを clone して参照するため、インスタンス数が増えても DC 数は変わらない。

### キャラクター（Soul）

| 要素 | DC 数 | 備考 |
|---|---|---|
| billboard mesh | frame variantごとに最大1 DC | 全Soulが同一 Rectangle meshを共有 |
| billboard material | 最大8 variant | frame切替は共有handle差替えで、entityごとのmaterial生成なし |
| GLB / shadow proxy | 0 | production spawnとsystem登録を停止 |

---

## 4. LOD0 建築物の draw call バジェット（将来）

### 前提

| 変数 | 値 |
|---|---|
| RtT 解像度 (High/FHD、DPI 1.0) | 1920 × 1080 |
| tile_rtt_px（LOD0 仮定、DPI 1.0） | 32 px |
| 1 world unit（orthographic scale 1.0） | 1 logical px = `Window DPI × RtT quality` physical RtT px |
| カメラ仰角 | 59°（VIEW_HEIGHT=150, Z_OFFSET=90） |

RtT Camera3d の `ImageRenderTarget.scale_factor` は `Window DPI × RtT quality` であり、
`world_to_viewport` の logical target px は LOD 観測時に同じ倍率を掛けて physical `tile_rtt_px` へ戻す。
したがって上の面積・triangle 概算は DPI 1.0 / High の基準値である。

### 可視ピクセル数（59° Camera3d投影係数）

| 面の向き | 投影係数 |
|---|---|
| 水平面（上面、world Z方向） | sin(59°) ≈ 0.857 |
| 垂直面（前面、world Y方向） | cos(59°) ≈ 0.514 |

これはCamera3dのRtT内でfragment数を見積もる係数である。後段compositeの縦補正
`hypot(150, 90) / 150 ≈ 1.166`は表示上の縦縮みを戻すが、Camera3dが生成済みのfragment数は
増減させないため、下記GPU予算へは掛けない。最終画面上の形状寸法を求める場合は、縦補正後の
`screen_y = -world_z + 0.6 × world_y`を使う。

設備の推定可視面積：

| 設備 | 仮定高さ | 可視面積 |
|---|---|---|
| 1×1 (Tank 等) | 1.5 tile | ~1,670 px |
| 2×2 (MudMixer 等) | 1.8 tile | ~5,410 px |

### Triangle バジェット導出

micropolygon 下限（1 tri ≥ 4 px²）× カリング率（可視率 35%）から設備のGLB total triを逆算し、
制作予算は端数とsilhouette用余裕を上へ丸める。Wallは単純面積ではなく接続silhouetteとshared texture主体の
専用計画値を使う。

| 建築物 | 可視 tri概算 | 逆算値 (÷0.35) | 制作予算 |
|---|---:|---:|---:|
| 壁 1×1 | [本番壁計画](plans/3d-rtt/archived/production-wall-art-plan-2026-08-31.md)で判定 | — | **150〜250 tri目標 / 350 tri hard cap** |
| 設備 1×1 | ~420 | ~1,200 | **~1,300 tri** |
| 設備 2×2 | ~1,350 | ~3,860 | **~4,100 tri** |

### 20 種類での draw call 数

Trellis 等で生成した GLB を種類ごとに 1 ハンドルで管理すれば:

```
20 種類 × 1 DC/種類 = 20 DC（全建築物合計）
```

建設中/完成の 2 状態を別 material handle にしても 40 DC。
現代 GPU では問題ないレベル。

### インスタンシングを壊さないための運用ルール

1. **GLB ロード時に種類ごとに 1 ハンドル**を `Res` に格納し、entity は clone して参照する
2. **状態変化は `commands.entity().insert(MeshMaterial3d(handle.clone()))` で差し替える**
   - `materials.get_mut(handle)` で mutate しない（他のインスタンスのバッチも壊れる）
3. **per-entity material clone は禁止**（現行の `soul face` は必要性があるため例外）

---

## 5. 2D パイプライン：夢の泡パーティクル（要対応）

### 現状の構造

world-space の夢泡は `Mesh2d + DreamBubbleMaterial` で描画する。
現在は以下の共有構造に整理されている。

| 要素 | 現在の構造 |
|---|---|
| mesh | `DreamBubbleHandles.circle_mesh` を全粒子で共有 |
| world material | `DreamQuality × alpha bucket` の 24 ハンドル共有 |
| shader time | `DreamBubbleMaterial.time` ではなく `globals.time` を使用 |
| alpha 更新 | `Assets::get_mut` ではなく `MeshMaterial2d` の handle 差し替え |

`DREAM_PARTICLE_MAX_PER_SOUL = 5` なので、
50 体睡眠時のアクティブ粒子数上限は依然として 250 だが、
material asset 数と per-frame material mutation はこの上限に比例しない。

### 改善方針の概要

`(DreamQuality × alpha_bucket)` のマテリアルプール（24 ハンドル）を `Resource` に保持し、
粒子は bucket が変わったときだけ handle を差し替える。

alpha bucket の運用:

- `bucket 7` は現行と同じ `alpha = 0.85`
- `bucket 0` は `alpha = 0.0`
- `bucket 0` に入った粒子は不可視のまま slot を占有しないよう早期 despawn する

期待できる効果:

- mesh asset 数: 粒子数依存 → 1
- world material asset 数: 粒子数依存 → 24 固定
- world-space per-frame `Assets::get_mut`: 粒子数依存 → 0

注意:

- transparent 2D mesh は sorted phase なので、draw call は shared handle だけでは決まらない
- 同じ mesh / material を共有していても、Z 順で隣接したものしか batch されない
- したがって **24 ハンドル = draw call 上限** ではない

UI 側の `DreamBubbleUiMaterial` は world-space 版と同様に `time` フィールドを削除し、
`@group(0) @binding(1) var<uniform> globals: Globals;` で `globals.time` を shader 内で直接参照する方式に変更済み。
これにより per-frame の `Assets::get_mut` 呼び出しがパーティクル数に比例して発生していた問題を解消した。

UI material は `velocity_dir` のような粒子ごとの時間変化uniformを持たず、alpha × mass × color の
`8 × 4 × 2 = 64` 個の共有handleだけを使う。粒子側はbucketが変わったときだけ
`MaterialNode` のhandleを差し替えるため、粒子数に比例するmaterial asset生成・mutationは行わない。

`TaskAreaMaterial` も同様に `time` フィールドを削除し `globals.time` を使用するよう変更済み
（`mesh2d_view_bindings::globals` 経由、`@group(2)` マテリアルバインドへの毎フレーム書き込みを排除）。

→ world-space 泡の draw call 最適化詳細は `docs/plans/dream-bubble-perf-2026-04-09.md`

---

## 6. テクスチャキャッシュ（draw call と独立した懸念）

draw call 数とは別に、GPU オンチップテクスチャキャッシュ（数 MB）のスラッシングが
フラグメントシェーダーのスループットを落とす場合がある。

| 対策 | 内容 |
|---|---|
| テクスチャアトラス化 | 同素材グループを 1 枚の大テクスチャにまとめる |
| テクスチャ共有 | 同種の建築物は同一テクスチャハンドルを使う |
| 解像度の適正化 | LOD2 では使われないテクスチャは 256px 以下でよい |

Trellis 生成 GLB は各モデルが独立テクスチャを持つため、
多種同時表示時はキャッシュミスが増える。20 種程度なら許容範囲内。

---

## 7. フラグメントシェーダーコスト（参考）

draw call ではなくピクセルコストの観点。LOD0 (32px/tile, FHD) での概算。

| カテゴリ | 占有 px | テクスチャサンプル/px | フレーム総サンプル |
|---|---|---|---|
| 地形 LOD1 | ~1,760,000 | ~15 | **~26M** ← 支配的 |
| 地形 LOD2 | ~1,760,000 | ~4 | ~7M |
| 建築物 PBR | ~250,000 | ~5 | ~1.25M |
| 建築物 Unlit | ~250,000 | 1〜2 | ~250k〜500k |

建築物の fragment コストは地形 LOD1 の 1/10 以下であり、
Trellis 生成の高ポリゴンモデルを使っても fragment 面での影響は軽微。
ポリゴン数の増加は頂点シェーダーコストに影響するが、
50k tri × 30 インスタンス = 1.5M tri/frame は現代 GPU で問題ない。
