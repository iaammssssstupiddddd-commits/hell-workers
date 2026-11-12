# 建築アート移行の静止基準シーン

`building-art-static` は表示基盤を変更する前の比較入力。新アートの採用や稼働中の生産性能を証明するものではない。
移行全体と未完条件は[移行計画](plans/3d-rtt/non-wall-floor-building-art-migration-plan-2026-09-19.md)が所有する。

`2026-09-20`のユーザー判断により、Bridgeは別件へ分離し、残る9種を先行する。
計測専用の川は作らず、通常地形・橋・配置ruleを変更しない。旧10種の無効な結果は基準に使わない。
以下の9種契約で静止参照を取得済み。M1-0全体のbaseline/budget確定とは区別する（経緯は移行計画§9）。

## 固定する条件

| 項目 | 契約 |
| --- | --- |
| fixture / native profile | `building-art-static-nine-v2` / `building-art-static-nine-reference-v2`、seed `20260920` |
| 対象 | Tank / MudMixer / RestArea / SoulSpa / WheelbarrowParking / SandPile / BonePile / Door / OutdoorLamp |
| 除外 | Bridge。配置・地形の問題は別途解決し、再合流時の専用基準と回帰確認が必要 |
| N / 4N | 各4棟 / 16棟、計36 / 144棟。実Soul 15 / 60体、Familiar 0体 |
| 支持物 | Doorごとに既存Wall 2枚とFloor 2枚。Tankごとに既存companion 2セル。対象建物数には含めない |
| 環境 | GPU Vulkan、X11、1280×720、DPI 1、RtT high、novsync、UI dialog/dashboard hidden |
| 時間 | Virtual Time停止、実時間warmup 30秒＋計測60秒、各3回、preflight 0 |
| カメラ | grid `(46,37)`、scale 5。Nと4Nで位置・縮尺を変えない |
| Door | 現行承認済みgeneration 7、Closed、東西支持壁 |

列は `x=8+5*ordinal`、建物種ごとの行は `y=8+5*row`。9種の座標・状態・cameraは旧案から維持する。
通常のplacement geometry・配置validatorを通し、地形を書き換えない。生成済みの実WorldMapを使う配置testも行う。
Nは4Nの最初の4列と一致する。Bridgeの混入と旧contract IDを拒否し、橋の成功を主張しない。
ordinalを4で割った余りに応じて、次の状態を繰り返す。

| 状態 | 0 | 1 | 2 | 3 |
| --- | ---: | ---: | ---: | ---: |
| Tank水量 / capacity 50 | 0 | 25 | 50 | 50 |
| Mixer実Refining task | 無 | 無 | 有 | 有 |
| RestArea実occupants | 0 | 1 | 5 | 0 |
| Spa実worker mask | 0 | 1 | 3 | 15 |
| Lamp通電 | 無 | 無 | 有 | 有 |

TaskWorkers / RestAreaOccupants / StoredItemsを直接偽造せず、workerのAssignedTask・WorkingOn・RestingIn、
水itemのStoredInからrelationshipを構築する。Spaは実tileのdurable parentと座標に従う。
列ごとのYardで配電し、1-worker Spaの列はLampのYardを分離する。

## 所有・実行順・失敗条件

- profiling featureかつ明示workload時のみ有効。通常起動の入力・建設・描画・保存・Helpへ接続しない。
- `Setup`で合法配置と実actor数を確認して既存Blueprint／Spa factoryへ渡す。
- カメラは準備時に一度だけ位置・`PanCamera.zoom_factor`・Transformの全scale軸を設定する。
  Bevy 0.19の操作処理が毎回zoom_factorからscaleを書き戻すため、Transformだけの設定は無効。
  準備後の入力・倍率変化を修復せず検査で拒否する。通常プレイの操作・倍率範囲は変更しない。
- `IndoorSettle`で通常の建設完了→worker設定→Spa activation→Mixer mirror→電力topology/output/allocationを順に実行する。
  Spaはfactory生成後、完成済みの搬入量と`Operational` phaseを一度だけ設定する。
  搬入量だけでは通常deliveryのphase遷移は発生しない。tileのDesignation/TaskSlotsは通常activationに任せる。
  完了ポップアップ・bounceだけは準備時に一度終了する。Virtual Time停止下で永続化させない。
  初期化完了後にタスク・水・人数・通電を再設定せず、完了演出の再出現も修復せず失敗扱いにする。
- `PostUpdate`のvisibility確定後、owner/占有/支持物/companion/状態/cameraを検査する。
  各kindをk棟として、現行の共有mesh 2種・対象3D root `5k`・対象foreground `4k`と、実resident handle・可視性を要求する。
  対象は`9k`、支持Wall/Floorを含むBuilding全数は`13k`。Bridgeのmeshを対象poolへ計上しない。
  準備完了前のasset待ちは許すが、完了後の不一致・消失はエラー終了。
- 共通run validatorでは現行ActorBillboard描画方式を選び、旧Soul proxy/mask/shadowとFamiliar proxyは0を要求する。
  旧stageの期待数は変更しない。この検査は旧proxyの不在検査であり、Soulの全billboard可視性の証明ではない。
- `building_art_static.json`に初期状態と同じ不変条件を最後まで満たした証拠を出す。
  Python側は独立した固定layout/state期待値、厳密JSON、SHA-256を検査する。自己申告したhashだけでは通らない。
- 現行表示専用の初期参照であり、後続candidateのroot/part数をこのlegacy検査へ無理に合わせない。
  新表示比較を追加する際は論理fixtureを維持し、presentation期待値を別契約として検査する。

## 正式な起動と原本検証

clean commitを用意し、[保存管理](development-infra/validation-storage-workflow.md)に従ってprimary coordinatorへbatchを登録する。
`verify_command`は `python3 scripts/building_art_static_acceptance.py verify --job-root @job_root` を指定する。

```bash
python3 scripts/dev.py validation plan --spec /absolute/batch.json -- \
  python3 scripts/building_art_static_acceptance.py plan --repo /absolute/hell-workers
```

planが返すkittyコマンドをそのまま実行する。既定native Skillの排他lease、開始時RAM 10 GiB／stage時8 GiB、
persistent storage guardを利用する。Capture N×3→Capture 4N×3→Memory 4N×3の逐次実行で、同時ゲームは1個。
subject/source/helper/runtime asset viewを固定し、各sessionの原本CSV・ログ・actual adapter/window・状態sidecarを再検証する。
Memoryはallocation CSVとGNU timeから再計算し、Captureと別binaryであることを確認する。
集計値は各指標の3回の中央値とMAD。static合格を稼働時・美術・release合格へ読み替えない。

独立したtoolingテスト:

```bash
python3 -m unittest scripts.tests.test_building_art_static
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --lib --features profiling building_art_static
```

## 静止参照の実測（2026-09-20）

- clean subject `353fc4cd85b7eaf88737ec36bafa909841289f08`、source
  `84cf072b6615ba0e26eedde41c0786abfbf538f7a0ead7c25f9f44c0e068857b`。
- native v7でCapture N×3 / 4N×3、Memory 4N×3が全て有効。登録済み独立verifierとsealも成功。
  実adapterはIntel Arc (MTL)、Mesa 26.1.8、Vulkan / X11 / 1280×720 / DPI1 / high / immediate。
- Capture binary `0fbecc9281f0283c383df395744d72b100b91246d12e17b823eae9210d4790a9`、
  Memory binary `f108fb4d44dcb1a93e637a48ce00fdbaf2ddd9332f91e7d2afeef0c7357a98bb`。

各3回の中央値 / MAD。これは測定成立の証拠であり、性能予算への合格判定ではない。

| 指標 | N=36 | 4N=144 |
| --- | ---: | ---: |
| Capture p95 (ms) | 8.629081 / 0.001453 | 14.379702 / 0.774638 |
| Capture p99 (ms) | 9.117201 / 0.008431 | 57.037860 / 1.923346 |
| Memory peak live (bytes) | 未測定 | 671,045,869 / 770,472 |
| Memory max RSS (KiB) | 未測定 | 1,338,420 / 6,740 |

4Nのtail latencyを平均値で隠さず、raw分布と環境ばらつきの校正を予算決定へ含める。
原本は`target/native-acceptance/building-art-static-20260920T054051Z-f4e2aa28`（2,105,344 allocated bytes）。
ownerはbuilding-art-migration、用途は提示結果レビューとM1-0の予算校正、終了／置換／打切り時に解放する。
元sourceでの独立検証は済ませており、旧verifier再実行のためにprimaryやbinaryを凍結し続けない。
新旧表示基盤の比較は未実施。静止参照は表示実装の開発入力として使い、正式な性能判定の前に予算を確定する。

## 現在の限界と無効試行の履歴

- 旧10種の履歴: `2026-09-20`、clean subject `9a46751af0a92671c22718dcbc7fa6f0700ca550` のnative v2で、
  small Captureの3回すべてが準備完了前に `Bridge/0` の `(8,65)` を `NotRiverTile` として拒否した。
  `hw_world/src/river/channel.rs`の現行生成は幅2〜4、Bridgeは2×5全セルがRiverであることを
  `hw_ui/src/selection/placement/validation.rs`で要求する。旧`RIVER_Y_MIN/MAX`は生成済み地形の保証ではない。
  layout単体testは数・重複・状態を検査するが、実mapgenとの成立を証明していなかった。
- 同旧試行ではX11 window生成とIntel Arc/Vulkan adapterのログは得たが、window/readiness原本がなくrenderer受入には使えない。
  有効なframe-time測定は0、medium CaptureとMemoryは未実行。基準値や性能改善率を出さない。
- 幅5の計測専用川という案は不採用。Bridgeの問題は残り9種の進行条件にしない。
  9種の基準を全10種の基準へ読み替えず、橋の再合流時に別の比較・受入を追加する。
- v3のcamera初期化不一致を修正したv4は、準備完了に至らずsmall初回が300秒でtimeout。
  後続試行を中断して調べた結果、Spaの初期phaseがConstructingに残る不備を確認した。
  修正後のv5は準備を通過しsmall初回の30/60秒計測を終えたが、共通validatorが旧Soul proxy各15体を要求して不合格。
  現行方式を明示した診断再計算では他の全検査が通った。dispatchを修正し、旧方式と旧proxy混入の拒否を回帰test済み。
- v6はCapture6回・Memory後続2回が有効だったが、Memory初回でcamera scaleが5.0→4.9に変わり全体不合格。
  Bevyのzoom入力経路と整合するが入力原本がないため原因は断定しない。同一source・条件のv7では全9回で再発なし。
  無効試行を合格へ読み替えず、上表はv7だけから集計した。計測windowへの入力を避け、条件変化の拒否は維持する。
- 回転、Dream粒子、稼働中の状態遷移、搬送・生産継続、save/load、preview、GPU draw-call詳細は未測定。
- Mixerは初期Refining状態を停止保持する。入力や進捗を毎frame補充・巻戻しして稼働負荷を捏造しない。
- 不備のある稼働計測はユーザー指示で破棄した。静止参照を稼働時の性能証拠にはしないが、稼働fixtureの再実装や予算確定を表示実装の着手条件にしない。
- Help impactは **No impact**。明示profiling CLIの検査経路だけで、通常プレイヤーの操作・成立条件・表示意味は不変。

## 導入前の9種混在比較

`scripts/building_m6_acceptance.py plan --candidate-roots <canonical JSON>` は
`building-pre-release-paired-static-v1` を選ぶ。JSONはDoorを除く8種のkind→candidate rootで、
全candidateのmanifest/dependency、制作原本、art/numeric approvalを既存codec/pipelineで検証する。
`HW_BUILDING_M6_CANDIDATES` は明示profiling static専用で、通常起動、他のart/production session、
Bridge、ReleaseApproved/ArtPreview混在、kind欠損/重複、canonical以外のlocatorを拒否する。
loader登録時にexact identity集合を固定し、candidate側だけpoolへのrequestとpresentation許可を持つ。
controlにも同じ8候補を要求するがロードしない。Door g7のlocator・依存bytesは両側で変更しない。

primary assetsを変更せず、全fallback bytesと候補のimmutable artifacts/locatorsから専用viewを作る。
入力元の全file inventoryとview、numeric validator/contract inventoryとart driverの実hashを
実行中・独立verify時に再照合し、途中の変更を拒否する。
sidecarは旧正式導入後M6とは別の`building-art-pre-release-nine-v1`で、旧oracleはこのcontractと
IsolatedCandidateを受理しない。part poseはRust f32へ厳密投影し、Tank waterは数値承認済みの
partial/full Yを論理water数から選ぶ。小数誤差の一律許容やFullとPartialの同一視はしない。

Capture→Memory、各N/4N×3隣接pair（中間repeatは逆順）で計24 run、warmup30秒/measure60秒。
M6ではtarget/資源の完全snapshotが30frame連続で一致してからworkloadをreadyにする。
準備中の欠損や変化では安定回数を0/1から数え直すだけで、worldや資源を修復しない。
ready後の資源変化は従来どおり失敗とし、before/after値を診断へ残す。
専用runnerは`RUST_LOG=info,wgpu=error`を要求/記録し、performance既定filterに隠れる
実AdapterInfoを観測可能にする。親shellのlog抑制は引き継がない。
M6 candidateのM5 foregroundは、Small16件/Medium64件の実owner・sprite・画像handle・色・canvas・
owner/local/global pose・anchor・layer・可視性をready以降も毎frameのtyped値で照合する。
計測区間内でforeground JSONやartifact/identity文字列を生成せず、初期とFlushの区間外で完全DTOを作る。
最終sidecarはMeasure→Flush後のPostUpdateで取得した実最終値を要求し、初期cloneで代用しない。
candidate側の追加typed観測費用も増分に含む保守的比較であり、純粋なrenderer費用だけとは呼ばない。
独立oracleはM5の既知owner/foregroundをexact keyで検査し、全ordinal・一意owner/sprite・Lampの
ordinal%4のoff/on・製品のpile/structure Z・白色・TRS・layer0・画像/artifactを照合する。
galleryの4件契約は共通1record検証の呼出側に残し、M6の16件/64件へ暗黙拡張しない。
runnerは独立foreground verifierのSHAをplanへ固定し、packaged manifestのdescriptor/artifactにも再照合する。
`scripts/perf_tool/contracts/building_pre_release_budget_v1.json` は測定前の増分上限で、
p95 +2ms、p99 +10ms、native live +64MiB、RSS +128MiB、mesh accessor +128KiB、
image CPU +16MiB、mesh +7、image +22、material +32/shallow +64KiB、反復MADはmedianの10%以内。
candidateは7GLB/22PNG（RGBA展開9,699,328B）を持つため、個数は実inventoryから固定した。
N→4Nのresident共有資源は同じ値にplateauすることを別に要求する。これはabsolute gameplay性能や
GPU配分量のbudgetではない。試験結果を見て上限を緩和せず、超過は原因調査と修正対象とする。

導入後の旧`building-m6-paired-static-v1`は引き続き全8種ReleaseApprovedを要求する。
どちらのstatic比較もaccepted=false/promotion_authority=false。通常world lifecycle、稼働性能、
独立アート採否、promotion/install/rollback、通常起動への正式導入は別途完了させる。
2026-11-12時点で新比較の実機結果は未確定。
初回v1はCLIのrepo/root引継ぎ欠損、v2は比較元の起動直後の資源変化でinvalid。
v2は正常な計測window/summaryとAdapterInfoがなく、有効な性能結果は0回のままである。
失敗を再送・成功化せず、入口と準備安定性・ログ契約を修正してfresh batchへ進む。
v3（`47e48c40`）はcontrolとcandidateの30/60秒計測を完走し、起動直後の資源変化は再発しなかった。
しかしcandidateはM5のowner/foreground追加DTOへ未対応のoracleに拒否され、全体はinvalid。
計測値は診断用であり、有効なcandidate比較やbudget passへ読み替えない。継続foreground照合と
実finalの欠落も補正対象とし、同じ候補・事前budgetの新sourceで全matrixを実施する。
v4（`ea3c8077`）では最初のCapture N control/candidateが共に有効となり、M5の継続foreground・
実final検査を通過した。その後のcampaign verifierがasset manifestでsession manifest変数を
上書きし、binary識別子の抽出で停止した。全体はinvalid封印/finalize済みで、Memoryは未実行。
参照変数の分離とcase verifier全体を通す回帰を追加し、旧結果を合格化せずfresh matrixで確認する。

この同source比較は基盤・群別・最終累積比較の代用ではない。群別では対象群だけを切り替え、
他群を同じ世代で固定する。基盤の出発点は有効な9種fixtureの`353fc4cd`であり、
旧10種のinvalidな`9a46751a`ではない。旧参照と現在はMesaも異なるため、古い数値を直接比較しない。
通常AIの継続生産・給水・搬出や代替active fixtureを表示移行の事前gateにはしない。
当該群の表示・状態遷移・lifecycle・資源・性能受入を満たした群から独立に正式導入する。
