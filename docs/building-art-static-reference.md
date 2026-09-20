# 建築アート移行の静止基準シーン

`building-art-static` は表示基盤を変更する前の比較入力。新アートの採用や稼働中の生産性能を証明するものではない。
移行全体と未完条件は[移行計画](plans/3d-rtt/non-wall-floor-building-art-migration-plan-2026-09-19.md)が所有する。

## 固定する条件

| 項目 | 契約 |
| --- | --- |
| fixture | `building-art-static-v1`、seed `20260920` |
| 対象 | Tank / MudMixer / RestArea / SoulSpa / WheelbarrowParking / SandPile / BonePile / Door / OutdoorLamp / Bridge |
| N / 4N | 各4棟 / 16棟、計40 / 160棟。実Soul 15 / 60体、Familiar 0体 |
| 支持物 | Doorごとに既存Wall 2枚とFloor 2枚。Tankごとに既存companion 2セル。対象建物数には含めない |
| 環境 | GPU Vulkan、X11、1280×720、DPI 1、RtT high、novsync、UI dialog/dashboard hidden |
| 時間 | Virtual Time停止、実時間warmup 30秒＋計測60秒、各3回、preflight 0 |
| カメラ | grid `(46,37)`、scale 5。Nと4Nで位置・縮尺を変えない |
| Door | 現行承認済みgeneration 7、Closed、東西支持壁 |

列は `x=8+5*ordinal`、建物種ごとの行は `y=8+5*row`。Bridgeだけ既存川の `y=65..69` を使用する。
通常のplacement geometry・配置validatorを通し、地形を書き換えない。Nは4Nの最初の4列と一致する。
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
- `IndoorSettle`で通常の建設完了→worker設定→Spa activation→Mixer mirror→電力topology/output/allocationを順に実行する。
  初期化完了後にタスク・水・人数・通電を再設定しない。
- `PostUpdate`のvisibility確定後、owner/占有/支持物/companion/状態/cameraを検査する。
  現行の共有mesh 3種・対象3D root `6N`・対象foreground `4N`と、実resident handle・可視性を要求する。
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

- 回転、Dream粒子、稼働中の状態遷移、搬送・生産継続、save/load、preview、GPU draw-call詳細は未測定。
- Mixerは初期Refining状態を停止保持する。入力や進捗を毎frame補充・巻戻しして稼働負荷を捏造しない。
- この静止参照だけではM1-0は閉じない。稼働fixtureと基盤budgetの確定前に新しい表示基盤を導入しない。
- Help impactは **No impact**。明示profiling CLIの検査経路だけで、通常プレイヤーの操作・成立条件・表示意味は不変。
