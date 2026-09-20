# 9種建築物の通常稼働参照

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `building-art-active-reference-plan-2026-09-20` |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-09-20 / 2026-09-20 |
| 作成者 | Codex |
| 関連提案 | [移行計画](non-wall-floor-building-art-migration-plan-2026-09-19.md) M1-0 |
| 関連Issue/PR | N/A |

## 1. 目的

静止参照では証明できない通常の再割当・給水・原料搬入・泥搬出を、既存表示のまま計測する。
Bridgeは別件。地形・建築ルール・通常AIを変更しない。静止v2を稼働証拠へ読み替えない。

## 2. スコープ

- 対象: profiling専用fixture、読取専用の負荷証拠、独立validator、native helper、基準取得。
- 非対象: 新モデルの公開、表示基盤の実装、Bridge、AI/物流の仕様変更、美術承認。

## 3. 現状とギャップ

静止v7はCapture6 / Memory3が有効。Mixerは1回の初期作業しか持たず、Virtual Time停止中である。
通常経路は `mud_mixer_auto_refine_system` → Familiar割当 → Refine完了、
`mud_mixer_auto_haul_system` → 水/砂/岩搬入、`task_area_auto_haul_system` → 備蓄への搬出。
泥は通常のStockpileAcceptanceで受入可能。fixtureはこれらのconsumerを置換しない。

## 4. 実装方針

- 同目的branch `codex/building-art-migration`、比較基点 `daf6dfed`。push/PRはしない。
- `building-art-active` を独立workloadとして追加。静止fixtureの配置・工場・初期表示検査を共用する。
- small: 各種4個、29 Souls、2 Familiars。medium: 各種16個、116 Souls、8 Familiars。
  4列当たりRest6 / Spa7 / 生産労働者16。稼働2列それぞれに8名と使い魔1体を置く。
- 稼働列の使い魔は列Yardと同一TaskAreaを持ち、Refineと搬送だけを許可する。
  Tank初期50水、Mixer初期砂1/岩1/水1。有限の岩30個と泥専用備蓄を列内に用意する。
  砂は既存SandPileとWheelbarrowParking、給水は既存Tankと専用bucketを利用する。
- 初期化は一度だけ。稼働中は在庫補充、作業の再挿入、疲労/Dream/カメラの復元をしない。
- warmup30秒＋測定60秒、通常Virtual Time。静止と時間・負荷が違うため横比較しない。
- 証拠は測定期間の生産数、搬入数、備蓄到着数、Mixer稼働率、Rest/Spa人数、Dream粒子を記録する。
  単なるis_activeでは合格しない。各稼働Mixerで3回以上生産、砂/岩/水の搬入、泥5個以上の到着、
  測定の前中後20秒区間で生産があることを要求する。稼働率は0超1未満。
  Rest/Spa人数は初期数を維持し、Dream粒子は非ゼロを観測する。乱数ruleは変更しない。
- これらはfixture健全性条件であり、性能budgetではない。初回校正が不成立なら原因を調べ、
  同じrunの条件を緩めて合格にしない。契約変更は新versionで新規測定する。
- Bevy 0.19の既存fixture/SystemParam/schedule実装を参照する。観測は通常処理後、capture境界前。

## 5. マイルストーン

### A1: fixtureと検査

- [ ] 一度限りの初期化、通常ドメイン処理、観測sidecarを実装。
- [ ] config/負荷検査の拒否test、静止v2の回帰testを実行。
- [ ] Help実経路レビュー、docs更新、check/Clippy/CIを実行してcommit。

### A2: 実機の稼働校正

- [ ] validation coordinatorからno-prompt launcher、同一clean subjectでCapture→Memoryを逐次実行。
- [ ] 負荷証拠・source/asset/binary同一性・全runの独立検証。失敗は失敗のまま記録。
- [ ] 同環境の静止/稼働の分布とばらつきから候補実装前のbudgetを別途freeze。

## 6. リスクと対策

- 搬送距離/疲労で生産停止: 通常taskと所有者・対象を追い、値を毎frame固定して隠さない。
- 製品の消滅と到着の混同: resource identityとStoredInの到着を数え、消滅を搬出成功としない。
- 測定中の入力/カメラ変更: 検出して失敗。自動修復しない。
- ばらつき: 全runの負荷値を残す。負荷不成立runを性能比較に混ぜない。

## 7. 検証計画

`dev.py check`、`dev.py cargo -- clippy --workspace --all-targets -- -D warnings`、
rust-analyzer診断、`dev.py ci check --base daf6dfed726cf733c4d07b880a109837f47bb469 --mode auto`、`git diff --check`。
nativeは対応Skillのcoordinatorと正式buildを使う。検査通過後commitしてsourceを固定する。

### 検証データ管理

正本は `docs/development-infra/validation-storage-workflow.md`。各validation開始時に再読する。
primary cacheは既存review consumerで再利用する。静止v7 rawは既存budget-calibration evidence consumerの
ため保持する。active用batch IDは `building-art-active-nine-20260920-v1`、ownerはbuilding-art-migration、
consumerはbuilding-art-active-reference-v1。同名review holdでprimary checkout/cacheを継続利用する。
開始前target実測296,821,587,968 allocated bytes。batch specをGit common directoryへ用意済み、
jobはclean commit後のplanで作成する。まだ実機は未起動。最終報告前にstorage check。

## 8. ロールバック方針

新workloadだけを無効化できる。静止v2と通常プレイを維持する。並行変更を取り消さない。

## 9. AI引継ぎメモ

- A1実装中。A2未着手。M1-0全体未完。新runtime基盤へ進む前に稼働参照とbudgetが必要。
- 読む場所: 移行計画、静止参照記録、本計画、`perf_scenario/building_art_static/`、native Skill。
- 完了後は有効契約/結果を恒久docsへ移し、本計画を閉じて索引を更新する。
