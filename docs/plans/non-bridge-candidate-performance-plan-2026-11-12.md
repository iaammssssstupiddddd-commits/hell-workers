# 橋以外9種の導入前性能比較

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | non-bridge-candidate-performance-plan-2026-11-12 |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-11-12 |
| 作成者 | Codex main |
| 関連提案 | N/A |
| 関連Issue/PR | TAK-14 / 非橋9種正式導入の継続 |

## 1. 目的

正式導入前の8候補と既存Door g7の混在費用を、同一source・同一adapterの
legacy-controlと比較する。現M6は8種のReleaseApprovedを要求するため、導入前比較には使えない。
候補のauthorityを変更せず、別profile/sidecar contractを持つprofiling専用入口を追加する。
本作業は全体の正式導入の一工程であり、通常world lifecycle・GPU配分量・release承認を代替しない。

## 2. スコープ

対象はmain所有のRust profiling admission、Python matrix/oracle、拒否系テスト、実機比較。
Bridge、既存release入口の許可緩和、ゲームルール、独立承認の作成、外部公開は対象外。

## 3. 現状とギャップ

8候補にはmanifest、art/numeric approvalと制作原本provenanceが存在する。
Door g7は別loaderで維持する。現loader/presentation候補許可は単一identityであり、
複数候補比較の明示許可が必要。正式導入済み比較は引き続きReleaseApprovedだけを受理する。

## 4. 実装方針

- 明示profiling sessionでexact8 IsolatedCandidate identityとcanonical locatorを検査する。
- production、ordinary-art、art-preview、Bridge、欠損/重複kind、別scopeの混在を拒否する。
- primary assetsを変更せず、全fallback bytesと8候補の検証済みbytesから専用asset viewを作る。
- raw sidecarの別contractを独立oracleで検証し、元M6のrelease-only oracleは維持する。
- Capture→Memoryを逐次実行。N/4N、各3隣接pair、中間repeatの順序を反転、30秒warmup/60秒measure。
- 数値budgetは測定前に理由と全metricを固定する。試験結果に合わせて緩和しない。
- 既存Bevy 0.19のstartup resource/loader capture方式を使う。新規外部APIを仮定しない。
- 同目的branch `codex/tak14-non-bridge-release`、比較基点
  `38c769f2d559abf46d5c1d3deb83413e7bd74621`。公開/PRは今回操作しない。

## 5. マイルストーン

### M1: 候補比較入口

- [x] exact8 admission / 別contract / asset viewと改変拒否を実装。
- [x] 独立read-only review、Help実レビュー、関連テスト、check/Clippyを実施（入口・準備補正は下記のfresh検証へ継続）。

### M2: 実機比較

- [x] 全candidate packaged-byte/provenanceをcodecで再照合し、測定前budgetを固定。
- [ ] 同subjectのCapture12・Memory12を登録launcherで実行・独立verify。
- [ ] 失敗を修正し新subjectで再検証。成功を正式release/art承認へ読み替えない。

### M3: 次工程へ接続

- [ ] 静止比較の結果・未検証範囲を恒久仕様へ同期。
- [ ] 通常worldの残lifecycle/性能とguard付きpromotion/install/rollback、通常起動導入を継続。
- [x] 旧preview入口と分離したM5採用候補planning4caseを同じ検証subjectへ準備し、fresh実機で確認。

### M4: 実測で判明したforeground検証と通常world観測基盤

- [x] M6のM5 owner/foreground既知DTOをexact key・identity・artifact・画像・親/TRS・layer・可視性まで独立検証する。
  既存galleryの4件/Lamp ordinal契約を保持し、1record検証を共通化する。
  M6では各kindのcopies件とordinal全域、Lamp ordinal%4の役割を要求し、validatorのhashもplanへ固定する。
- [x] M6 candidateのM5をSmall16/Medium64件の有界typed snapshotで毎frame照合する。
  JSON化は初期/Flushの区間外に置き、実最終観測からfinalを作る。初期cloneをfinalの代用にしない。
  candidate側の追加観測費用は増分に含む保守的比較と明記し、fresh pairで再測定する。
- [x] 未準備/初期後の画像・owner・親・pose・layer drift、偽final一致、未知key、欠損/重複、Medium Lamp役割を拒否する。
- [x] 通常worldのGPU観測をprofiling専用の別DTOへ追加し、device allocatorの瞬時allocated/reservedと件数・adapter/driverを記録する。
  Bevy0.19/wgpu29のsafe APIだけを使い、欠損/非Vulkan/None/算術不整合は明示unavailableとする。
  既存gpu_measured_bytes:null、performance_evidence:false、正式性能oracleとrelease権限は変更しない。
- [x] 原本API照合、Help実レビュー、変更別検証/check/Clippy、独立exact review後にcommitし、fresh nativeで接続を確認する。

### M5: 正式galleryのDPI条件

- [ ] `building_art_acceptance.py`のgalleryに限りDPI 1/1.5/2を明示選択できるようにする。
  planで有限な厳密集合を検査し、argv・実Window DPI・physical extent・RTT target・owned PNGを照合する。
  Bevyの既存physical window 1280×720は維持し、DPIを掛けた架空の画像寸法を要求しない。
- [ ] 既存context/UI recipeはDPI1の契約を維持し、bool/欠損した新契約/非有限/未知値/actual不一致を拒否する。
  旧planのDPI1互換と、通常起動・ゲームrule・release authorityの不変を守る。
- [ ] Rustのstatic configとtyped art sessionでも同じ契約・requested DPIを照合する。
  新fieldは任意の対として旧sessionを保ち、新契約があれば完全なtyped sessionとgallery範囲を要求する。
  通常性能fixture、Spa page、context/UI、非profilingはDPI1のままにし、宣言とargvの不一致を起動前に拒否する。
- [ ] Help実レビュー、変更別検証、独立exact review後、fresh actual-windowでDPI接続を確認する。
  対象群の共通gallery、High/Medium/Low×各DPI×normal/far、群別性能・独立release採否は別に閉じる。

## 6. リスクと対策

候補authorityの通常経路への漏出は、profilingと明示session・全identity検査・旧入口の拒否試験で防ぐ。
静止試験の過大解釈は別contract・accepted=false・promotion_authority=falseと未検証範囲で防ぐ。
copyしたassetsの欠損/改変は開始時と独立verify時の完全inventory照合で拒否する。

## 7. 検証計画 / データ管理

primary `dev.py validation` のstorage checkを開始/終了時に実施。
同一baseからの変更別CI、check、default/profiling Clippy0、workspace test、Python回帰、Help実レビュー、
独立exact-source reviewの後にcommitしてnative sourceを凍結する。rust-analyzer診断も確認する。
batchは実行前にowner/consumer/roots/verifierを登録。既存8候補・codecのholdを維持し、
新jobは比較と独立reviewが終わるまで保持する。結果は既存仕様へ集約し、終了したconsumerをreleaseして
不要生成物を回収可能な手段で撤去し、bytesとstorage結果を記録する。原本・唯一の成果は削除しない。

## 8. ロールバック

新profiling sessionを指定しなければ旧動作。primary assets/authorityは変更しない。
実装差分は本比較のcommit単位で扱い、既存の未完成果を巻き戻さない。

## 9. AI引継ぎ

8候補のpackaged bytes/art/numeric provenanceを現行debug codecで再照合済み。
session/loader/pool、matrix/oracle、拒否系試験、測定前budgetを実装した。
初回contracts/tooling/rust CI相当gateはpass（Python800・制作229・default/profiling全workspace test・
全profiling feature check・Clippy0）。独立reviewのf32/Tank状態比較指摘は修正済み。
最後のprovenance validator hash固定を追加し、最終差分の再検証とexact review後にcommit/nativeへ進む。
`802d9d0f`で最終contracts/tooling/rust gate（Python801/制作229）、dev.py check、
profiling Clippy0と独立reviewが完了した。正式Capture buildから687,149,856Bのimmutable codec
（SHA256 `601b0fd770b29d00cef29fc47c21e8c8353f0315fad80c40812be37f0d0646ce`）を保持登録し、全8候補/Doorを再照合済み。
初回`tak14-nine-candidate-performance-802d9d0f-20261112-v1`は出力root生成前にexit1。
CLIが`native.activity_locked`へrepo/job_rootを渡していない不備で、測定は0回だった。
invalid封印/finalize・root不存在を記録済み。working art CLIと同じplanからの引継ぎを追加し、
入口回帰と変更別gate・独立review後にfresh batchへ進む。旧失敗を再送・成功化しない。
入口修正`15d35daf`のv2はゲーム起動まで進んだが、最初のcontrolで常駐資源変化を検出してexit1。
初回snapshotでReadyを公開する実装であり、AdapterInfoも既定performance filterに抑制されていた。
有効計測0回・invalid封印/finalize、jobは新しい起動安定性比較だけのdiagnostic用途に保持した。
次はM6の30連続安定観測→Readyと明示ログ、after-ready変化の値付き拒否を実装して再検証する。
再ビルドをまとめるため、独立draft reviewと合成10testを終えたM5採用候補planning4caseもこの段階へ反映する。
旧preview/generation/入力/完成拒否は維持し、適用後のRust/Python・Help/Skill/docs・固定reviewを新たに実施する。
配布Skill正本の同期後、full gateの依存監査で既存rustix1.1.3への
[RUSTSEC-2026-0486](https://rustsec.org/advisories/RUSTSEC-2026-0486.html)を検出した。
公開修正版1.1.6へ限定更新し、同版の必須制約`libc ^0.2.182` / `linux-raw-sys ^0.12`に伴って
lockfileはlibc0.2.183・linux-raw-sys0.12.1も更新する。変更package versionはこの3件だけで、
Cargoが互換range内のWindows依存解決も更新した。denyのignore・監査条件は変更しない。
同じ最終sourceで依存監査、全必要品質群と実機を再検証する。旧codecは形式検査専用の明示identityとして保持し、
新subjectのゲームbinary/性能証拠へ流用しない。
`d8a8b7fb`で依存修正版、`47e48c40`で準備安定性/M5候補入口を保存した。
contracts/tooling/deps/rust全群、Python804・制作229、default/profiling全workspace tests、各feature check、
default/profiling Clippy0、dev.py check、独立exact-source review、storage checkが成功した。
同head/source `0216714f9037b8873c690f45c604e500f65484ea6a0876c66dec29f4786a67dd`のM5候補4caseは
各6画像/21入力を完了し、登録verifierによるseal、独立画像/raw reviewとfinalizeが成功した。
Sand/Bone/Parking/Lampのsample/event数は順に30/80、32/73、31/68、35/83。
catalog/ghost/Blueprint/選択だけの限定証拠で、自然完成・利用・配電・保存・性能・releaseを証明しない。
性能v3はcontrolとcandidateの30/60秒計測を完走し、準備直後の資源変化は再発しなかった。
ただしcandidateのM5 owner/foreground追加DTOをM6 oracleが拒否しexit1、campaignはinvalid封印/finalize済み。
論理state/layout値は一致しており、未知fieldを無視せず上記M4で完全検証・継続観測を補う。
旧失敗の再封印や受入条件緩和を行わず、同じ候補/測定前budgetのfresh subjectへ進む。
既存のproduction runnerはnormal-development admission・ordinary-world capture/ACK・host instrument producerが
未実装と宣言する。旧Orcaや架空receiptで補完しない。全体受入は継続中。
`ea3c8077`のfresh v4は最初のCapture control/candidateを完走し、foregroundと実finalも検証されたが、
asset manifestがsession manifest変数を上書きしてbinary取得に失敗した。invalid封印/finalize済み。
参照の分離と全case verifier回帰で修正し、既存rawは診断比較だけに使う。
GPU DTOのfresh接続は同headのLamp候補planningで31sampleすべてavailableとなり、独立画像/raw reviewとsealが成功した。
この接続試験は正式性能受入ではない。

原移行計画§7に従い、基盤・群別・最終累積比較を分離する。現在の全候補増分比較だけで3種を完了としない。
通常AIの継続生産・給水・搬出や代替active fixtureを表示移行の事前gateへ追加しない。
群固有の表示・状態遷移・lifecycle・性能条件を満たした群から独立導入し、全9種完成まで先行群を未releaseに留めない。
v5（`f2fdf9ea`）はN側6測定と4N側の最初のpairを完走したが、N controlのp95 median/MADが
9.796273/1.290815ms、p99が17.823617/6.172078msとなり、固定10% MAD上限を超えた。
既知の不合格のままMemoryまで続けず、所有identityを照合したrecipeへ一度だけSIGINTを送った。
既存finallyで子process群を終了し、exit -2を確認してinterrupted封印/finalize済み。Memory未実行、予算は変更しない。
10秒区間の集計ではcontrol3の冒頭20秒に遅延が集中したが、環境要因の原因は未確定。
次の計測中はmainのビルド・storage整理・worktree作成も並行させず、独立して進められるDPI入口を先に完成させる。
基盤fixture-only `353fc4cd`は編集しない専用worktreeと保持用途を登録し、正規native計画まで準備済み。
現在環境での基盤測定・不足するN Memoryは未実行であり、旧測定の合格を流用しない。
未起動の基盤実行計画はprimary storage gate前にabandoned/finalizeし、workspaceの比較用途だけを継続保持した。
`bff4440e`のDPI1.5試行はRustのstatic configのDPI1制限で描画前に拒否された。
probe/PNGはなくinvalid封印/finalize済み。Python入口だけの完成を実機接続と扱わず、
typed art sessionへ契約とDPIを渡し、Rust admission・実Windowまで同じ拘束を接続してfresh試行へ進む。

### Definition of Done

- [ ] 候補比較と独立verifyが成功し、同subjectの検証/Help/reviewを記録。
- [ ] 永続仕様へ結果を移し、不要job整理とstorage確認、索引を更新。
- [ ] 未完の通常world/正式導入工程を保持して継続し、本比較を全体完了としない。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-11-12 | Codex main | 導入前比較を正式導入後M6から分離する計画 |
