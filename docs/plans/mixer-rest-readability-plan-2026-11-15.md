# ミキサーと休憩所の用途判読性を修正する

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | mixer-rest-readability-2026-11-15 |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-11-15 / 2026-11-15 |
| 作成者 | Codex |
| 関連提案 / Issue | 利用者の直接フィードバック / TAK-14 |

## 1. 目的

利用者が現行Mixerのアートを不採用、RestAreaを判読困難と評価した事実を受入条件とする。
通常縮尺でドラム・口・支持架台・車輪、休憩所の屋根・壁・閉じた入口を読み取れる描画へ修正する。
過去の技術passや独立採用を新フィードバックの解決へ読み替えない。

## 2. スコープ

新しい2種の制作原本・明示style、個別normal/far・回転姿勢の実画面比較を先に行う。
屋根の入口遮蔽が残れば一度で色調整仮説を打ち切り、新geometry/numeric契約へ進む。
論理占有・施工・入所Soul非表示・Dream・回転軸/速度・保存を維持し、他7種とBridgeを変更しない。

## 3. 現状とギャップ

Mixer g4は口縁が明るい同心円で、暗い台車と支持構造が一塊に見える。
Rest g4は紫灰の屋根と壁・入口カーテンが同色で、短い縫目だけが強く見える。
mode4は実際に有効。照明係数ではなく、object-localな図柄と用途を示す輪郭を修正する。

## 4. 実装方針

同目的branch `codex/tak14-non-bridge-release`、比較基点 `c4747ce8165b4e3695549c92197f7e6ac9f3a8d9`。
mainだけが編集・build/test・Git・原本操作を所有し、既存担当はread-only調査/独立reviewのみ。
旧illustrated-v1は不変再現できるよう、新illustrated-v2を明示選択する。
Mixerは口縁/胴/支持/車輪を分離する色面と太い接合線、非対称摩耗を部品UVに固定する。
Restは棟/軒、布の大きな継ぎはぎ、専用の暗い入口と枠・垂れ幕を描き分ける。
新原本・previewに旧art/numeric承認を流用しない。既定bindingは新しい採否が成立するまで維持する。
公開・push/mergeは本修正へ含めない。

## 5. マイルストーン

### M1: 図柄の修正と比較準備

- [x] 原画像/原本/generatorとshader経路を照合し、独立read-only調査を行う。
- [x] 新styleと拒否/旧再現/UV回転/部材判読性の回帰を実装する。
- [x] `37dc337c`のHelp No impact、contracts/tooling CI、storage、独立コードreviewを通した。
- [ ] 以下のgeometry試作にも新しい回帰と独立reviewを通す。

### M2: 実画面の用途判読性

- [ ] 新しい隔離原本を生成し、normal/far・Mixer四姿勢を独立比較する。
- [ ] 屋根が入口を隠す場合は新geometryを採用前に別numeric reviewする。
- [ ] 改訂した画像を利用者へ提示し、新しいart判断へ結合する。

### M3: 採用と終了

- [ ] 新しいart/numeric/candidate/release判断、切戻し・通常起動・必要な状態/性能確認を行う。
- [ ] 正本原本・仕様・採用画面を同期し、用途終了jobを整理してstorage checkを通す。
- [ ] 完了した一時計画を閉じて両索引を再確認する。

## 6. リスクと対策

全体tintや細い傷だけの反復を避ける。通常縮尺で入口と支持構造が読めるかを先に判定する。
旧原本・正式世代・receiptは上書きしない。試作・技術passから正式採用を推測しない。

## 7. 検証計画

該当Python toolingの実入力/拒否回帰、Help実経路の判断、`dev.py ci check --base <上記full-SHA> --mode auto`、
`git diff --check`、必要なnativeを登録済みdirect kitty→独立verify/seal/finalize/checkで行う。
Rust変更時はcheck/Clippy0/診断を追加する。外観が成立する前に全性能matrixを実行しない。

### 検証データ管理

正本は `docs/development-infra/validation-storage-workflow.md`。
consumer: `Mixer/Rest readability 2026-11-15`、owner: Codex。
primaryと既存Cargo cacheをこのフィードバック単位で維持し、新規targetを作らない。
新staging/jobはexact path・bytes・次用途・release_whenをprimary validationへ登録する。
採用/不採用の判断後に原本を正本へ保全し、用途終了jobを整理する。無応答はcloseにしない。

## 8. ロールバック方針

新正式世代まで旧g4 bindingを維持し、採用時はguard付きcanonical/runtime rollbackを確認する。
旧source/authorityを破壊して試作を表示する方法は使わない。

## 9. AI引継ぎメモ

現状: 原本と実画面を照合済み、実装開始。ユーザー不採用の解消が今回の目的。
最初に現diffと本計画を照合し、旧Orca統括や終了済みnativeを再送しない。
未完了を技術passへ隠さず、実際の新画像と次の判断を残す。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-11-15 | Codex | 利用者フィードバックに基づき2種の判読性修正を開始 |

### 配色比較の結果と次の形状試作

`37dc337c`のillustrated-v2原本は2種ともsource/GLB/UV/atlas/toolchainの登録済み独立verifyとfinalizeに成功した。
しかし独立外観reviewは両方ともchanges requested。Mixerの支持部と口の色分離、Rest入口の暗色化は改善したが、
Mixerの大きな板台座と円盤状の見え、Rest屋根の本/厚紙状の外形が残った。技術passをart採用へ変更しない。
通常画面へ公開せず、同系統の色調整はここで打ち切る。

Mixerは明示v4へ細い二本の実レールと支持脚を作り、共有yawだけを新しい数値候補へ分離する。
local Y軸・pivot・55度tilt・4秒/回転・Refining条件は維持する。連続全周sweepを新bodyへ再検査する。
Restは明示v3（Rest限定、Spa不変）へ連続したたわみ屋根と閉じた波打つカーテンを作る。
旧v2 fixture/原本/採否は保持し、新numeric/art採用は別に取り直す。

旧最初の生成wrapperは登録後・未起動reviewでassert guardを指摘され、child/出力なしを確認してabandoned/finalizeした。
通常if/raiseへ修正した別wrapperをPYTHONOPTIMIZE0/1/2で拒否確認し、fresh v2出力を生成した。旧登録は再送していない。

### Rest v3のUV照合補正

`aa053470`のRest v3はGLTF検査0errorsで出力されたが、indexed UVの照合でexit1となりinvalid/finalizeした。
576頂点中1点のUは元double `0.5202349856035131`、実f32は`0.520235002040863`で、旧round5のkeyが分かれた。
expectedへ実出力と同じf32格納→V反転→f32格納を適用すると576件全一致した。
許容幅やface privacy/windingを変えず、expectedの型変換順序を補正する。旧失敗をpassへ変更せずfresh sourceで再生成する。
