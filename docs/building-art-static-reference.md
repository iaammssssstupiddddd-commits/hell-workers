# 建築アート移行の静止基準シーン

`building-art-static` は表示基盤を変更する前の比較入力。新アートの採用や稼働中の生産性能を証明するものではない。
移行全体と未完条件は[移行計画](plans/3d-rtt/non-wall-floor-building-art-migration-plan-2026-09-19.md)が所有する。

`2026-09-20`のユーザー判断により、Bridgeは別件へ分離し、残る9種を先行する。
計測専用の川は作らず、通常地形・橋・配置ruleを変更しない。旧10種の無効な結果は基準に使わない。
以下の9種契約で再検証・基準取得する（正式結果は移行計画§9へ記録）。

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

## 現在の限界

- 旧10種の履歴: `2026-09-20`、clean subject `9a46751af0a92671c22718dcbc7fa6f0700ca550` のnative v2で、
  small Captureの3回すべてが準備完了前に `Bridge/0` の `(8,65)` を `NotRiverTile` として拒否した。
  `hw_world/src/river/channel.rs`の現行生成は幅2〜4、Bridgeは2×5全セルがRiverであることを
  `hw_ui/src/selection/placement/validation.rs`で要求する。旧`RIVER_Y_MIN/MAX`は生成済み地形の保証ではない。
  layout単体testは数・重複・状態を検査するが、実mapgenとの成立を証明していなかった。
- 同旧試行ではX11 window生成とIntel Arc/Vulkan adapterのログは得たが、window/readiness原本がなくrenderer受入には使えない。
  有効なframe-time測定は0、medium CaptureとMemoryは未実行。基準値や性能改善率を出さない。
- 幅5の計測専用川という案は不採用。Bridgeの問題は残り9種の進行条件にしない。
  9種の基準を全10種の基準へ読み替えず、橋の再合流時に別の比較・受入を追加する。
- 9種版も正式値は未取得。v3のcamera初期化不一致を修正したv4は、準備完了に至らずsmall初回が300秒でtimeout。
  後続試行を中断して調べた結果、Spaの初期phaseがConstructingに残る不備を確認した。
  初期phase修正後のnative結果を得るまでは、9種版のrenderer・性能合格を主張しない。
- 回転、Dream粒子、稼働中の状態遷移、搬送・生産継続、save/load、preview、GPU draw-call詳細は未測定。
- Mixerは初期Refining状態を停止保持する。入力や進捗を毎frame補充・巻戻しして稼働負荷を捏造しない。
- この静止参照だけではM1-0は閉じない。稼働fixtureと基盤budgetの確定前に新しい表示基盤を導入しない。
- Help impactは **No impact**。明示profiling CLIの検査経路だけで、通常プレイヤーの操作・成立条件・表示意味は不変。
