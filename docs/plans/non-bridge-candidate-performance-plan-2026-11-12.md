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

- [x] `building_art_acceptance.py`のgalleryに限りDPI 1/1.5/2を明示選択できるようにする。
  planで有限な厳密集合を検査し、argv・実Window DPI・physical extent・RTT target・owned PNGを照合する。
  Bevyの既存physical window 1280×720は維持し、DPIを掛けた架空の画像寸法を要求しない。
- [x] 既存context/UI recipeはDPI1の契約を維持し、bool/欠損した新契約/非有限/未知値/actual不一致を拒否する。
  旧planのDPI1互換と、通常起動・ゲームrule・release authorityの不変を守る。
- [x] Rustのstatic configとtyped art sessionでも同じ契約・requested DPIを照合する。
  新fieldは任意の対として旧sessionを保ち、新契約があれば完全なtyped sessionとgallery範囲を要求する。
  通常性能fixture、Spa page、context/UI、非profilingはDPI1のままにし、宣言とargvの不一致を起動前に拒否する。
- [x] Help実レビュー、変更別検証、独立exact review後、fresh actual-windowでDPI接続を確認する。
  対象群の共通gallery、High/Medium/Low×各DPI×normal/far、群別性能・独立release採否は別に閉じる。

### M6: 群別の厳密なload集合

- [x] 専用`building-group-paired-static-v1`とtyped `HW_BUILDING_GROUP_COMPARISON`を追加する。
  groupはM2/M3/M5だけ、fixedは先行群のReleaseApproved 0/2/4種、targetは当該群のIsolatedCandidate 2/2/4種。
  canonical locator・identity・凍結artifact pathを検査し、Bridge、重複、集合欠損、authority混在を拒否する。
- [x] 既存M6の全8種modeと通常releaseの2/4/8 guardを維持する。
  `M6Comparison`へ群別の実load集合を加え、request・loader snapshot・pool許可を同じ集合から導出する。
  controlにもfixedをloadし、targetのrequestと先読みはせず、AssetServerのactive path IDで検査する。
  受入性能をcontrolの追加観測費用で小さく見せないよう、両modeで同順序・全target pathを検索する。
  controlだけmanagedを拒否し、candidateは従来のpool/identity/consumer検証を維持する。
- [x] presentation検証をmode全体のboolからkindごとの完全identity一致へ変える。
  pool数0→2/2→4/4→8、mesh entity/copies 5→7/7→11/11→11、unique target mesh2→6/6→8/8→8を要求する。
  M5 foreground監視はM5 candidateだけ4×copiesで、fixed M2/M3のpart検査もcontrol側へ残す。
- [x] Pythonのview・manifest/provenance・oracle・group別contractと初期/実final照合を接続する。
  fixed/target inventoryとloaded集合を分け、target path省略・control先読み・kind/世代/bytes driftを拒否する。
  既存全8種の独立oracleを新しい任意集合へ緩和しない。

### M6-b: 正式群galleryの独立契約

- [x] 単種galleryやperformance scopeを拡張せず、新しいtyped group-art sessionと独立gallery contractを追加する。
  M2/M3/M5のexact target集合、先行ReleaseApproved集合、Door g7、nonce/PID/出力rootを拘束する。
  通常起動、既存art/ordinary/UI/Bridge/production/performance sessionとの併用は拒否する。
- [x] 全9種×4棟・15 Soulsと既存の論理状態・通常placement validatorを維持したart専用layoutを用意する。
  行anchorのpitchは元の5 tileを維持する（空き間隔ではない）。TILE_SIZE=32と専用窓のDPI2論理960×840を
  原本で照合し、元pitchでも対象群の範囲が収まるため不要な座標変更をしない。3 tile案は作業Soulとcompanionが重なる。
  DoorはM2/M3の対象群の直後へ入替える。Lampは最終行のまま、実Lamp位置からoff controlの非重複Yardを導出する。
  性能用layout・1280×720/DPI1/camera5は変更しない。専用1920×1680 clientとnormal/far cameraで
  High/Medium/Low × DPI1/1.5/2の全18 caseを検証する。対象全状態とDoorの実mesh AABB／image alpha boundsを
  実camera・RTT・DPIで投影し、UI非表示の可視領域内16 physical px margin、normalで最小辺4px/面積64px²、
  farで最小辺2px/面積16px²、3DのRTT側でも面積16 texel²を最低とする。数値だけで画風合格にはしない。
- [x] groupの実load集合と既存pool/part/foreground検証を共用し、gallery専用の論理layout期待値を独立oracleへ持つ。
  旧単種/全8種/performance oracleは新gallery contractを拒否する。camera/quality/DPI/identity/状態の変化を修復しない。
- [x] owner付きPNG capture後のnonce/PID/probe SHA付きACKを専用rootへ一度だけ渡し、実final状態と通常exit0を要求する。
  JSONはgalleryだけで収集し、performance計測を主張しない。ACK欠損・古いnonce・変化・非zero・中断をfail closedにする。
  1つのsource/binary・共有immutable viewの下で18子processを逐次実行し、原本/画像を全件独立確認する。
- [x] Help実レビュー、品質gate、拒否系テスト、独立exact review後にcommitする。
  worst-DPIの単一接続caseはconnection-onlyとし、全matrixやreleaseの合格へ読み替えずfresh正式matrixへ進む。

### M6-c: 実client寸法の事前観測

初回group connection（ae32c940）はbuild後のX11/Vulkan起動で、producerの実window/DPI検査に
拒否された。ready/PNGなし、exit1、invalid封印/finalize済み。要求値を変えたり同試行を再送しない。

- [x] group runnerに、同じgame PIDの唯一のX11 clientだけを読む有界なgeometry観測を追加する。
  XGetGeometryとPID/frame/window状態属性だけを使い、XTEST/EIS・入力・resize・focus操作は行わない。
  ready前から寸法変化を有限の時刻付き診断へ残し、古いwindow/PID・未知送信を採用しない。
- [x] OS側の寸法とproducer拒否の関係をfresh connectionで確認する。geometry診断をPNGやDPI・
  camera・受入の代わりにはせず、1920×1680と既存投影の条件を緩めない。
- [x] Help No impact実レビュー、関連拒否回帰、同subject品質gate・独立review・storageを通す。

### M6-d: 専用窓の装飾分離と接続feedback

v2で同game PIDのclient1920×1662、frame上辺74pxを実測した。作業領域1736pxからframeを
引いた1662pxへ縮められていた。要求1920×1680は変更しない。v2もinvalid封印/finalize済み。

- [x] 完全なtyped GroupArtSessionが既にadmitされたWorldだけをframelessにする。
  通常・旧単種・性能・UIの窓装飾は不変。実Window.decorations=falseもDTO/oracleへ拘束する。
  native実測値付きの拒否を残し、準備後のresizeや不一致の自己修復はしない。
- [x] 反復接続だけに`--connection-only --feedback`を追加し、既存dev.py feedbackの
  dev/profile+profiling feature+incremental cacheを使う。clean source、同identity・ACK・画像検査は維持する。
  feedback_only=trueを明示し、feedbackの全18caseは禁止する。正式18caseとCapture/Memory性能は
  profiling buildで別に実施し、feedback画像を正式受入へ流用しない。
- [ ] 関連拒否回帰、同source品質・Help No impact・独立review・storage後、fresh接続を確認する。

### M6-e: 正式Capture実行binaryの独立保持

Memory buildは共有target/profiling/bevy_appを上書きする。一方、正式candidateの昇格時には
画像原本のverifierが実行binaryのSHAを再検査する。guard解除や旧binaryの復元で迂回しない。

- [x] build直後、正式group/個別artだけjob内の固定binaryパスへ上書き不可でコピーする。
  source/copy/sourceのSHA一致、通常ファイル・非symlink・canonical job root・実行属性を検証し、
  そのコピー自体を起動/codec照合/manifest参照する。feedbackは既存debug cacheを継続する。
- [x] verifyはjob内固定パスとSHAへ束縛し、共有targetがMemoryへ変わっても原本を再検証できる。
  source/assets/verifier固定は維持する。旧契約を黙って新契約へ変換しない。
- [ ] owner/用途/実測bytes付きで正式採否の現在の利用者だけ保持し、採否/比較終了後はコピーも整理する。
  同source校正→候補測定の継続workunitはevidence用途で保持し、不要な共有target依存を作らない。
- [ ] source中途変更、既存destination、symlink、SHA不一致、Memory後のcopy検証を回帰し、
  Help No impact、同source gate、独立reviewとstorage後に正式matrixへ渡す。

### M7-a: 通常起動の既定release inventory

- [ ] 既存ReleaseBinding/validate_bindings/request_releasesを再利用し、compile時の既定JSONを選択する。
  実release/独立decision/guard install完了までは空配列を維持し、authorityを新しく作らない。
- [ ] 明示envは完全置換し、非UTF8/空文字/不正JSONを未指定へ読み替えない。専用sessionと
  enabled performanceでは既定を混入させず、profilingでも通常起動は既定を使う。
- [ ] 非空defaultは既存完全集合・ReleaseApproved・canonical locatorの検査を必ず通す。
  rollbackは旧locatorと対応する旧binary/default inventoryを一組で復旧する。
- [ ] 空request0、明示優先、専用/性能分離、candidate/preview/不正locator拒否、Help・品質・
  独立reviewを終え、独立release判断後にpipeline.bindingsの正規出力だけをdataへ採用する。

### M7: 群別性能と順次導入

- [x] 旧9種参照の不足N Memoryは、旧source/helperを編集せず、既存perf CLIのN×3を別batchで測る。
  primaryの限定`verify-memory-small`で、固定353fc4cdの元verifier・既存6 Capture/3 Memory原本を再検証し、
  同じMemory binary、Nのlayout、実adapterとexact sourceを要求する。正式起動はprimary登録→kitty→execute、
  old成功の流用やmatrix書換えをせず、補足3 runだけの結果として扱う。
- [ ] 安定したcontrol baselineを取得し、群別の全metric有限budgetと根拠をcandidate測定前に固定・独立reviewする。
  全8種budgetを盲目的に流用せず、target実inventory、共有資源plateauとbaselineを用いる。
- [ ] N/4N、Capture→Memory、3隣接pair・中間順序反転、30/60秒を維持する。
  部分集合の既知noise/budget不合格は早期に検出し、未知結果や旧passを再利用しない。
- [ ] 正式group galleryと当該群の残lifecycle証拠を揃え、独立release decisionからguard付きpromotion/install/rollbackへ進む。
  M2を先に正式導入し、M3、M5へ進む。最後に既定起動・Door監査・9種累積と文書/Help/storageを閉じる。

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
`cffd9fe3`のfresh DPI1.5接続は同sourceの実窓・PNG・rawを独立reviewし、登録verifierのpass封印/finalizeも成功した。
これはTank単種High/normalの接続確認だけであり、残る正式群gallery・性能・releaseはM6/M7へ進める。

### Definition of Done

- [ ] 候補比較と独立verifyが成功し、同subjectの検証/Help/reviewを記録。
- [ ] 永続仕様へ結果を移し、不要job整理とstorage確認、索引を更新。
- [ ] 未完の通常world/正式導入工程を保持して継続し、本比較を全体完了としない。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-11-12 | Codex main | 導入前比較を正式導入後M6から分離する計画 |

### 群gallery実装のHelp実レビュー（2026-11-12、commit前）

No impact。新group-artの1920×1680、UI非表示、配置と専用ACK終了はprofilingかつ明示した
小型静止fixture・完全sessionでのみ到達し、通常起動はこのsessionを拒否する。
既存performance配置/終了経路はsession不在時に不変。通常プレイのInputAction/UiIntent、操作、文言、
建設・保存・世代切替の成立条件、通知・Help入口を追加/変更しない。
新しい技術画面のpassは性能、アート、正式導入の承認を与えない。Help本文とsnapshotは変更不要。

### Frameless/feedback変更のHelp実レビュー（2026-11-12）

No impact。mainのWindow装飾は、検証済みtyped GroupArtSessionを持つprofiling Worldだけfalseとし、
通常起動・旧gallery・性能・UIセッションでは従来trueを維持する。feedbackはconnection-onlyのhost
build選択だけで、入力・操作・保存・gameplay・通知・Help入口を変えない。正式18case/performanceへ
feedbackを流用しない。一般利用者向けHelp source/snapshotの更新は不要。

### Frameless接続の限定結果（2026-11-12）

`1a235c6b`のHigh/DPI2/normal connection-only feedbackはactual1920×1680・decorations=falseを確認。
Tank/Mixer/Door各4、計12 ownerのgeometry、owned PNG、PID/nonce/probe→ACK→実final、exit0を独立照合した。
stable60→158、Intel Arc/Mesa26.2.6/Vulkan、WARN/ERRORなし。登録verifierでpass封印/finalize済み。
manifest85efe7b102b1affd6694cac8425012942f971b1b9c8ed80c9e848ddd241df176。
feedback_only/connection_only=trueで、正式18case・性能・美術採否・releaseの完了ではない。

### 正式実行binary保持のHelp実レビュー（2026-11-12）

No impact。正式art/groupのhost runnerが、実際にbuildした同bytesの実行binaryをjob内へ固定して
そのコピーを起動・検証する。ゲームのコード/設定/入力/表示/保存/通知/Help経路は変えない。
feedbackは従来の共有debug cache、旧契約は固定の共有cacheのlive検証を維持する。
任意pathのcodec実行前にpath/権限/SHAを検証し、通常起動の配布authorityや承認は作成しない。

### 既定release接続の実装とHelp判断（2026-11-12）

空のcompiled inventoryを既存ReleaseApproved admission/Startup requestへ接続する。
明示overrideの非UTF-8/空/不正を拒否し、専用受入・有効performanceでは暗黙一覧を選ばない。
現一覧は空なので通常の入力・表示・保存・通知・成立条件・Help入口は不変、今回はNo impact。
実際の各群導入時には正式receiptから一覧を更新し、通常起動表示と同subjectのHelp影響を再確認する。
アート・数値・性能・正式releaseの承認はこの配線で与えない。
既定JSONだけの変更もHelp gateのproduction対象に追加し、一覧更新を古いNo impactへ便乗させない。

### M7-b: M2 candidateの不足state/lifecycle表示

- [ ] 旧ArtPreview受付を維持し、新candidate専用scopeでMixer refining/save-loadとTank/Mixer moveだけをadmitする。
- [ ] exact provenance/driver/source、所有PNG、body/rotor TRS・Virtual回転・pause/Idle、Load reset/cleanupとcompanion moveを独立検査する。
- [ ] 旧/新scope混同、flag/authority/UI混在、body/clock/axis/parts/撮影順序の拒否試験を通す。
- [ ] Help/品質/storage/独立review後に同subjectの4本を実行し、既存Tank persistence/constructionと合わせてM2独立採否へ渡す。

Help実レビュー: 新しい明示profiling候補sessionでのみ既存domain fixtureとread-only observerを使う。
通常起動では有効化されず、既存input/UiIntent、gameplay保存/移動/生産rule、通常ラベル/通知/Help入口は不変。
No impactとし、プレイヤーHelp source/snapshotは変更しない。技術storyboardは独立アートやreleaseを承認しない。

M7-b初回はprivate overlayのnumeric承認欠落による既知exit1。状態rawは進行したが受入ではない。
原本をdiagnostic保持・invalid/finalizeし、既存approval4件のexact copyと全inventory検査、
candidate専用の開始/Move入力終了時だけのpointer parkingを追加してfresh sourceで再実行する。
通常/旧ArtPreviewの入力・UIは変えないため、この補正もHelp No impact。

v2は最初のActive0.56秒の画像がpause境界を跨いで不採用となった。要求画像列を緩めずinvalid/finalize。
candidate refiningだけの明示quarter-speed clockとreal6/9秒pause/resumeで撮影時間を確保し、
実clockも独立検査する。通常/旧preview/他legを変えないためHelp No impactを維持する。
