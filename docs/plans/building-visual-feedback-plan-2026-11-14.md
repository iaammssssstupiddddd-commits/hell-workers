# 建物5点のビジュアル修正

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | building-visual-feedback-plan-2026-11-14 |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-11-14 |
| 作成者 | Codex |
| 関連提案 / Issue | TAK-14 の導入後フィードバック |

## 1. 目的

利用者の最新5指摘を実際のゲーム表示へ反映する。Tankの過剰な立体感、現場用に見えないMudMixer、RestArea内部のSoul表示、短いOutdoorLamp、SoulSpaの黄色い網状表示を解消する。

## 2. スコープ

- Tankは広い平面色と控えめな陰影の手描き表現へ寄せる。
- MudMixerは建設現場用の傾斜ドラム・架台・車輪のシルエットへ変更する。
- RestAreaは入所中のSoul描画を隠し、閉じた布張りの休憩小屋へ変更する。
- OutdoorLampは仮設脚・高いポール・頂部の投光器へ変更する。
- SoulSpaの黄色い網状表示のproducerを特定して除去する。
- 容量・利用・Dream・電力・施工・配置・save/load・Bridgeは変更しない。

## 3. 現状とギャップ

RestAreaは論理Soul本体のVisibilityを隠す既存仕様だが、独立したActorBillboard3dが追随しない。現行Mixerは箱型撹拌槽、Lampは短い骨支持、RestAreaは開放屋根である。既存世代の正式証拠は旧姿の履歴として保持する。

## 4. 実装方針

既存branch codex/tak14-non-bridge-releaseを再利用。比較基点は3364d1d88f879b956105ff5dd37738ae8f989dec。主担当が編集・検証・Gitを所有し、独立担当はread-only調査と最終reviewだけを行う。既存固定fixtureや旧承認を上書きせず、新しい候補・数値契約・世代を区別する。PR/push/mergeは本作業には含めない。Bevy 0.19の既存表示同期パターンを使う。

## 5. マイルストーン

### M1: 表示経路の修正

- [x] Soul本体のHiddenを独立ビルボードへ同期し、退出時に復帰する。入所/退出/owner消失の回帰test成功。
- [x] SoulSpaTile+GeneratePowerの黄色表示だけを除去し、domain指定と他マーカーの維持をtest。
- [ ] 生成・状態遷移・save/loadの回帰確認。

### M2: アートの改訂

- [x] Tank / MudMixer / RestArea / OutdoorLampの新候補を生成。generation2のexact preview4件を独立numeric reviewが採用可と判断。
- [x] 実GLB、全8静的部材との連続回転分離、部品quaternion、preview/catalog anchorを検証。
- [x] 4種のnormal/far feedbackと正式candidate画像を独立art/release reviewが採用。最終9種画面はM3で確認。
- [x] 4件の正式g2を正規promotion/installし、既定8件bindingを更新。隔離g1→g2→g1復帰と旧47ファイルのbyte inventory不変を独立確認。

### M3: 受入と同期

- [ ] Help実経路判断と恒久仕様更新。
- [ ] dev.py check、Clippy警告0、rust-analyzer診断0、変更別ci check。
- [ ] 同subject実機受入、独立read-only review、storage check。
- [ ] 結果を恒久仕様へ集約し、本計画を閉じて索引を更新。

## 6. リスクと対策

旧アセットの承認を新姿へ流用しない。候補生成やtechnical passを独立アート/数値承認へ読み替えない。feedback中は同じ作業場・Cargo cacheを保全し、正常な入力/表示guardを迂回しない。

## 7. 検証計画

python3 scripts/dev.py ci check --base 3364d1d88f879b956105ff5dd37738ae8f989dec --mode auto。実機はprimary validation coordinatorとnative acceptance Skillのfeedback/gallery経路を用いる。変更したSoul表示は入所・退出・ロード・owner cleanupを確認する。画像は通常/遠景で確認する。

## 8. AI引継ぎ

旧統括/Run/Driverは廃止済み。再開しない。既存正式9種導入は完了した旧姿の履歴であり、今回はその5点の修正である。未完項目を完了と報告しない。

## 実施中の検証記録

Tank/Mixerの通常倍率feedbackは実機pass。RestAreaの最初の試験は無関係なUI Imageの遅延追加をglobal pool差分が検出してinvalidとなり、結果を保持した。対象のmesh/imageの実resident依存へscopeを明示し、欠損依存・consumer handle・ready後のpart/material/pose不変を維持する補正を追加。独立read-only reviewでblockingなし、Python拒否系30tests成功。これは静止art確認のみでglobal poolや性能承認へ読み替えない。

## Help影響の実レビュー

No impact。`docs/help-screen.md`全文と既存providerを読み、通常建築→既定asset表示、RestingIn入退所/ロード→owner Visibility→独立ビルボード、Operational Spa→GeneratePower指定→表示producerを照合した。操作・成立条件・結果・文言・保存形式は不変。新しい外観・陰影と既存非表示仕様への復帰、Spaの不要な黄色い重ね表示だけを変更し、Help source/snapshotへ空変更を加えない。


### RSS差の追加診断

`32017b39`の24試行はCapture/Rust live/共有資源の増分は上限内だったが、Memory 4Nのprocess RSS中央値差139,260KiBが固定上限98,304KiBを超えたためinvalid。旧失敗をpassへ変更しない。
同binaryの各mode1回のpassive診断とarena2介入診断を独立検証した。匿名resident量は介入で減少したが、Rust liveも変わるため、driver固有のリークや空きarenaだけを原因と断定しない。
Linux/glibcでApp/worker初期化前に`mallopt(M_ARENA_MAX,2)`を通常起動にも適用する。codecと他platformは従来通り。
これは環境変数による試験限定の軽量化ではなく、通常runtimeの起動policyである。戻り値を検査しログを記録する。main前のarenaを撤去する保証はなく、CPU競合の悪化もあり得るため、環境介入なしの通常実画面と固定予算のfresh Capture/Memory全matrixを通るまで未採用とする。
