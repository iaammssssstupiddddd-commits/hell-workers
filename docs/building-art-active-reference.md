# 建築アート移行の稼働基準シーン

`building-art-active` はBridgeを除く9種の既存表示で、生産・搬送・Dream粒子を観測するprofiling専用workload。
**実機の成立と性能budgetは未確定**。静止v7の結果とは別契約であり、未取得の稼働性能を保証しない。
全体の順序は[移行計画](plans/3d-rtt/non-wall-floor-building-art-migration-plan-2026-09-19.md) M1-0、
実施状況は[稼働参照計画](plans/3d-rtt/building-art-active-reference-plan-2026-09-20.md)が所有する。

## 固定条件

- fixture `building-art-active-nine-v1`、native profile `building-art-active-nine-reference-v1`、seed20260920。
- [静止v2](building-art-static-reference.md)と同じ9種・配置・初期建築状態、通常地形、Door承認世代7。
- Smallは36棟・29 Souls・2 Familiars、Mediumは144棟・116 Souls・8 Familiars。
- 4列当たりRest6名、Spa7名、生産用16名。ordinal % 4が2/3の列に各8名＋使い魔1体。
- 各使い魔のTaskAreaは列Yardと同じ境界。許可はRefine / Haul / HaulToMixer /
  HaulWaterToMixer / WheelbarrowHaul。GeneratePowerや川からのGatherWaterは許可しない。
- Mixerの砂・岩・水は各1から開始。既存Tankの水50と専用bucket、既存SandPile・駐車場の
  wheelbarrowを使用。各稼働列に岩30個、泥専用の容量10×10セルの通常備蓄を追加する。
  備蓄は列x+2、y11〜20。初期化後の資源生成は通常の生産・採取だけ。
- Vulkan / X11 / 1280×720 / DPI1 / high / novsync / hidden dialog/dashboard。
- カメラgrid(46,37)、scale5。通常Virtual Timeでwarmup30秒＋測定60秒。3回ずつ、preflight0。

## 実行と証拠の所有

静止側の実factoryによる配置→完成→関係構築→GPU readiness検査までは共用する。
その後、稼働用の使い魔・追加Soul・岩・備蓄を一度だけ初期化し、capture開始時に時間を再開する。
再割当はauto_refine→Familiar policy→通常task execution、補充はauto_haul→通常搬送、
搬出はtask_area_auto_haul→通常Stockpile受入を使う。通常システムの容量・速度・疲労・乱数を変更しない。

`active::observe` はUpdateの通常ゲーム処理後、capture driver前に読み取る。測定境界の直前snapshotを
起点に、実際に `StoredByMixer` を持って生成された泥のEntityを追跡し、同じEntityの
`StoredIn` がその列の備蓄を指したときだけ到着に数える。消滅や輸送開始だけでは到着にしない。
砂/岩/水の搬入は、在庫の増減と実生産の消費数から算出する。初期在庫だけでの1回生産は合格しない。
sidecarは `building_art_active.json`。静止sidecarとは混在不可。

独立Python validatorは以下を要求する。

- 初期レイアウト・状態・個数・hashの独立oracleとの一致。
- 各稼働Mixerで泥15個（3回）以上、測定前中後20秒ごとに1回以上の生産。
- 各Mixerに砂・岩・水の搬入があり、泥5個以上が実備蓄へ到着。
- Mixer稼働率が0超1未満。観測frame数は測定frame CSVの行数と一致。
- 観測中のRest6/Spa7名（4列当たり）を維持。カメラ・pauseの変化は失敗。
- Dreamのworld/UI粒子は両方非ゼロを観測し、最大数と全frame合計を残す。
  world上限は4列当たり96（生産Soul16×5＋利用中Rest2棟の5＋11）、UI上限は通常の128。
  この上限は負荷逸脱の拒否であり同負荷の保証ではない。seedだけで同一負荷とは主張しない。

負荷条件は性能改善率の予算ではない。比較候補の実装前に、得られた分布・負荷差・MADから
比較許容差と性能budgetを別途確定する。不成立runを後から条件緩和で合格にしない。

## 起動・検証

検証済みのclean commitで、保存管理workflowに従ってcoordinatorへ登録する。
verifierは `python3 scripts/building_art_active_acceptance.py verify --job-root @job_root`。

```bash
python3 scripts/dev.py validation plan --spec /absolute/batch.json -- \
  python3 scripts/building_art_active_acceptance.py plan --repo /absolute/hell-workers
```

返されたdirect kitty launcherだけを使う。Capture Small×3→Medium×3→Memory Medium×3を逐次実行。
原本から環境・source/assets/binary・frame値・負荷・native allocator/RSSを再検証し、失敗で停止する。
Memory結果をCaptureのframe-timeへ混ぜない。seal/finalize/storage check後に結果を報告する。

```bash
python3 -m unittest scripts.tests.test_building_art_active scripts.tests.test_building_art_static
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --lib --features profiling building_art
```

Help影響はNo impact。明示profiling CLIからだけ到達し、通常プレイヤーの入力・操作・建築/物流ルール・
表示の意味・出荷assetは変更しない。Help本文/snapshotの空更新は行わない。
