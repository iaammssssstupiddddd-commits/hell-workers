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

### M7-c: 通常default起動の接続観測

- [ ] 非空compiled defaultだけの受動selected/activeログを実装し、空/明示/専用/perfでは無効にする。
- [ ] active/pending/failure/descriptor/residencyを区別し、既存ownerだけ観測する。生成・修復・承認しない。
- [ ] 正規native helperでoverride/session/perfなしの実binary/asset view/owned PNG・PID・ログを固定検査する。
- [ ] 拒否回帰、同subject品質・Help・storage・独立review後、正式inventory導入時に通常起動で確認する。

Help実レビュー: 受動auditは通常の状態・入力・設定・表示・保存・通知・Helpを変更せず、ログだけを生成する。
外側helperもactor/建物をseedせず、初期worldに実在しないconsumerを成功へしない。No impactとし、
アート/性能/全kind通常authority storyboardは別の正式工程を維持する。

### M7-b追補: 回転した状態からの保存／読込

`4a92da02`のcandidate refining v3は限定成功。386 samplesと5枚の所有窓PNGで
Idle→Active→Active paused→Active resumed→Idle、同owner/root/2 parts、Virtual 0.25倍、
72組の回転差分（承認axis/speedとの最大成分誤差約1.23e-7）を独立確認した。
manifest `92b8c75391beb3d347e5bd97cb8830329c505cef81d6ca789b0d7b4c10295dab`、
trace `3dfce3a45dfad5a8b630960258eb730edcf27cd74e89f705f316d07512dbd1e9`。
Intel Arc/Vulkan、WARN/ERRORなし。登録verifier・封印・finalize済み。
初回Idle画像にはConstruction Complete textが重なるため、无遮挡アート承認とはしない。
具体的consumer `M2 candidate state and lifecycle release review`の終了までv3を保持する。

未実行のsave-loadは旧fixtureのままだとIdle角度0の保存となり、角度resetを証明できない。
候補専用save-loadだけ同じquarter clock/real4秒の材料・Refine入力、6秒pauseを使い、resumeしない。
既存real12秒のSave/Load操作を維持し、成功Saveだけのraw prefixがActive/paused/非ゼロであること、
Loadまで同epoch/parts/Virtual停止、Load後は新epoch/root/parts・paused Idle/角度0を必須にする。
画像もActive paused→新epoch Idle pausedを要求する。通常プレイ・旧ArtPreview・move timingは不変。
この契約は`candidate-motion-quarter-speed-v2`であり、v3の旧refining契約を上書きしない。
Help実レビューNo impact: 明示profiling候補のseeded試験と検証器だけの変更で、通常の操作・文言・
保存／移動／生産rule・通知・Helpへの到達性は不変。

### M8-a追補: 旧基盤の資源量を補う独立観測（実装前）

旧`353fc4cd`のCapture/Memory計12回はtiming/native/RSS baselineとして保持するが、
Mesh/Image/materialのresident payload bytesを記録していない。PNG・source accessor bytesから推定して埋めない。
旧凍結source/cacheは変更せず、同commitから独立した資源観測用subjectを用意する。
追加は受動inventoryと境界出力だけに限定し、初期Ready後と測定終了後の二時点で採取する。
N/4Nの各1回を資源量補足として検証し、旧12回の性能値の置換や同一subjectとの偽装をしない。
新fallback側にも同じ定義を用い、実Mesh vertex/index payload、Image CPU payload、material shallow bytesと
各resident件数、root/part件数を比較する。GPU allocationやnative heap実測と混同しない。
旧補足のexact-source独立review・Help・storageを通した後、資源量の有限上限を固定してから新fallbackを測る。
時刻/Memory予算の提案（未採用）はp95 +1ms、p99 +3ms、native peak +16MiB、RSS +96MiB、
繰返しMAD≤10%。独立判断と根拠を記録するまでcandidate測定へ進めず、結果を見て上限を広げない。


candidate save-load v1（`eb42e90a`）は実Save/Load成功と非ゼロActive pause→新Idle角度0を観測したが、
Save成功からLoadまで約0.1秒しかなく、0.1秒raw周期にSave-only prefixがなく検証器が正しく拒否した。
exit1/invalid封印・診断用途登録・finalize済み。この部分観測を受入へ流用しない。
`candidate-motion-quarter-speed-save-hold-v3`では候補save-loadだけ、実Save成功後に2 real秒待って
RequestLoadを一度送る。通常／旧ArtPreviewの即時Loadは維持し、待機中のpaused/Virtual/部品は修復しない。
厳密なraw条件と画像条件は維持する。Help No impact: 明示profilingの候補試験driverだけの変更で、
通常の保存結果・操作・表示・通知・Help経路を変更しない。


`8c97d605`のsave-load v2は限定成功。734 samples、Save-only20行で非ゼロActive/paused/Virtual固定、
epoch0→1後の新owner/root/parts・Idle角度0を確認し、以後の停止と旧parts消失を照合した。
4採用PNGを独立実見し、Vulkan WARN/ERRORなし、登録verify/seal/retain/finalize成功。
manifest `cbdb2301e8d2d7561ddc4bd172f69b67344f92e2f50c43c1524a438ab74c6184`、
trace `1bad6cd1f56bca4198fbb069f0c3ab622c3293d76eb388331c72e3e27fa461a0`。
同sourceのTank move v1も限定成功。509 samples、同body/waterとlinked storage2件の移動delta `[64,0,0]`、
Full水面Y22と移動後の固定を7採用PNG/rawで独立確認。manifest
`ca9c0c9cabed253811d8ce9b9c6074f3a56c03c279761021cd9783b591d15e87`、
trace `d60e21ecbbca799eb9da5852b2458670a1a792b57c2e34099d50c1771717db92`。
2件ともrecipe exit0と子gameの正常bounded-stop（-15）を区別し、アート・性能・release承認とはしない。
このexact旧sourceの限定結果を採用記録とし、後続sourceへ旧承認を流用しない。live旧verifierの再実行を
保持義務にせず、raw/PNG/binaryは現在のM2 candidate state and lifecycle release review用途だけに保持する。

Mixer move v1はraw移動を観測したが、startup Soul1868v0のWorkingOn/ActiveTaskIdentity警告1件でinvalid。
通常AI自体の不具合と断定せず、後からorderのManagedByを付けるfixtureと背景assignmentの競合も調べる。
警告拒否を維持し、新候補moveだけ`fixture-move-without-startup-familiars-v1`を明示する。
既存spawn入力HW_SPAWN_FAMILIARS=`0`で背景Familiarを作らず、assign_orderの専用Familiar/workerの
実ID・CommandedBy・MovePlant assignment/WorkingOn/ActiveTaskIdentity/ManagedByを観測・照合する。
startup Soulsと通常populationは維持する。Soul0はemergency spawnが補充するため、人口0とは主張しない。
旧ArtPreview、refining、save-load、通常起動と通常gameplayは変えない。No impactのHelp判断を維持する。
新fixtureのpassでも、旧警告の解消や通常population下の自然AI全般の健全性を証明したとは扱わない。

### M8-b: 欠測資源量を明示した基盤補足と群別予算

- [x] 元353fc4cd由来の別Git作業場を作成・review用途登録。旧凍結sourceは未変更。
  通常Orca CLIは接続不能のため、mainがローカルGitで分離した。agent/旧統括は起動しない。
  既存profiling cacheだけをreflinkし、現在の旧基盤資源補足用途と終了条件を登録した。
- [x] 初期Ready／実Flushの区間外だけ、CPU Mesh/Image payload、明示CPU backing欠測、
  descriptor texture bytes推計、material shallow bytesとhandle/pathを採る実装を準備。
  primaryのRust focused5件、独立Python oracle／M6接続のfocused検査は成功。完了承認は下記のfresh品質/review後。
- [ ] 新参照sourceの関連検証・Help/storage・exact独立review後にcommit。
- [ ] 正規launcherでN/4N各1回を新subjectとして取得・独立検証・封印/用途/finalize。
- [ ] 旧12回と新2回の出所を分離して資源上限を先に固定し、新fallback/groupのCapture→Memoryへ進む。

新sidecarの初終は実handle集合・各count/payload全体を照合し、別processではidを除いたpath/payload multisetで
N/4N plateauを検査する。GPU-only ImageのCPU bytes nullを0-byte textureとして扱わず、
GPU descriptor推計をallocator実測に置き換えない。MeshのCPU抽出済み・未知format・overflowは拒否する。
群別予算には`mesh_vertex_index_payload_bytes`と`declared_texture_payload_bytes_estimate`も明示必須とし、
旧budgetの自動流用や結果を見た上限緩和はしない。旧reference repeat3は維持し、補足だけrepeat1を明示する。
新helperは元reference manifestとN Memory session SHA、別sourceのdriver/source/assetsを固定する。
Help No impact: 明示profilingのperformance観測・検証だけで、通常playerの入力・表示・保存・rule・
通知・Help入口は変えない。compile済み既定release一覧は空のまま、未正式8種を導入済みとはしない。

旧treeのidentity/harnessは、承認済みのsource-local helperをisolated/read-only Python子process
（-I/-B）で取得・前後再照合する。primaryの現catalogを旧treeへ要求せず、不足する現19pathをコピーしない。
受動2path＋CLI2path＋拒否系testのexact scope・clean・元helper SHAを実行前に検査し、primary側のcontroller/oracleと全verifier harness SHAは
別にplanへ固定する。この境界を変更したfresh helperでのみ補足を実行する。

### M7-b追補: 短いMoveの完了identity

Mixer move v2は新しい背景Familiar0の条件で警告なし・実移動ありだったが、0.1秒周期で
瞬時のMove assignmentを取得できずinvalid封印/finalizeした。周期だけのpredicateを実完了identityで補う。
候補のみ実orderを保持し、既存OnTaskCompletedの受動receiptをactor/owner/order/map/deltaへ結ぶ。
完了receiptをfixtureから発行せず、任意worker/別order/重複/欠測を拒否する。新scopeでfresh試行を行う。
既存の通常gameplay、警告拒否、撮影条件は不変で、Helpへの追加影響はない。
旧基盤資源の分離workspace関連テストはこのM2補正と独立して正規Cargo guardで逐次検証する。

refiningの旧v1/v2はv3で原因を解消・採用済みのためdiagnostic用途を正式解除した。
唯一asset・使用process・残consumerなしを確認したexact2rootのみTrashへ移動し、
作業場から2,451,869,696 allocated bytesを削減。原本/最新証拠/codec/cacheは保持し、物理空き量とは区別する。

### M8-c: 基盤対照の時間的な隣接条件

- [x] canonical計画§7は別source対照にも適用されると独立read-only照合。旧12runを校正へ分離する。
- [x] primaryのplain-static performanceへ、M6と同じReady/Flush初終inventoryを接続する。
  art_session/group_artと通常起動は除外し、測定中の列挙・JSON化を追加しない。
- [ ] ゲーム非起動plan/guard付きbuild準備/verifierと拒否系試験をexact品質検証・独立read-only review後にcommitする。
- [ ] 資源補足後、新control測定前に全有限budgetとMAD/gapを独立照合して固定する。
- [ ] 既存分離referenceとprimaryのCapture両binaryを事前固定し、N/4N×3対を隣接/中間反転で実行する。
- [ ] Capture全12caseの正式精算後にMemoryをbuild/bindし、同じ契約の12caseを実行する。
- [ ] 台帳とrawによる順序/環境/identity/binary/payload/budgetの独立検証を閉じ、群別導入へ継続する。

各caseは既存coordinatorのexact repo/root登録とlauncherを使い、次のcaseまでに封印/用途/finalizeする。
単一batchへ別repoを混ぜず、親子batchやactivity FD継承の例外を作らない。
二段階planは元source/assets/budget/matrixを不変とし、Memoryで条件や閾値を変更できない。
pair間は照合と精算だけ、別build/測定は禁止。No impact: profiling診断だけでplayerの契約は不変。
source別buildも正式batchとし、同lease内のimmutable copyとexact recipe/source/plan/log/hash receiptを
成功ledgerへ束縛する。同一binaryの別source偽装を拒否し、Memory build開始前からCapture完了を要求する。
登録時のcase verifier/stage SHAへ予算を固定し、事後変更や期間前から重なった別試験も拒否する。
prepare-build/prepare-caseの標準kitty planを既存validation planで包み、返されたcoordinator launcherだけを実行する。

### M8-d: source-local CLIの単発leg契約

資源補足v1は旧CLIのrepeat3必須条件により、repeat1が実機起動前に拒否された。
driver exit1/child exit2をinvalid封印・診断用途登録・finalize済み。旧凍結353と旧12runは保持する。
新参照の受動Rust2pathに加え、perf_tool/arguments.py、argument_validation/density.py、
拒否系test1件だけをmain所有のreview対象へ追加する。描画・ゲームルール・既存guardは変更しない。

- [x] 明示`--building-static-single-leg`を両sourceへ実装。flagなしrepeat3、flagありrepeat1を区別する。
- [x] 別workload/command、M6/asset-root/RtT混在、時間/seed/actors/画面/hidden逸脱を拒否する。
- [x] 両sourceのfocused拒否検査と実CLI dry-run成功。単発自体の受入権限は与えない。
- [ ] exact独立review・関連CI/Help/storage後に両sourceをcommitし、fresh資源補足へ進む。
- [ ] 基盤pairedの全24case・隣接反転・3対・事前budgetを維持して実行する。

### M8-e: 実測で見つかった初期資源とrenderer log

補足v2は実ゲームreturn0でも、adapter INFO欠落によりperf verifierがinvalidを返した。
rawには未対応packed LUTと、初終の匿名画像1件の退役があり、両方を診断用途として保持・精算した。
初終不一致を許容せず、待機フレーム数の調整でも埋めない。

- [x] 補足recipeとpaired exact argvへrenderer INFOのenvを固定し、log検証を維持する。
- [x] 一次APIで確認したRgb9e5Ufloat=4 bytes/voxelだけを独立oracleへ明示追加し、48³/mipの回帰を追加。
- [x] 両sourceへ同じ受動RTT Ready条件を実装。active targetの実descriptor/viewportと旧scene自然退役を待つ。
  強参照追加・画像削除・計測中の再Ready化は行わない。初回待機/Readyのlabel/handle/extentを残す。
- [x] primaryの自然drop処理を含むRust7件、Python拒否/packed形式を含むfocused28件成功。
- [ ] exact独立review・両source関連CI/Help/storage後にcommitし、fresh v3で因果と初終/N4Nを確認する。

旧匿名画像は寸法だけでRTTと断定しない。新gateの実際の待機理由と結果をfresh raw/logで確認し、
同じ変化が残ればこの仮説を打ち切って実ownerを調査する。v2の時間値は性能合格へ使わない。

M7-bのMixer move v3は登録verifier、4採用画像/rawの独立確認、封印/保持/finalizeを完了。
同identityの実MoveCompleted・owner/root/parts・`[64,0,0]`変位を確認した。
旧v2 invalidは維持し、新passを通常AI・art・performance・releaseの承認へ読み替えない。


### M8-e: 境界quad共有と旧基準の既知失敗

v3補足はSmall/Medium両actual-window・adapter・初終一致を確認したが、N/4NのMesh85→103、
payload120,376→123,112 bytes（18×152）でinvalid封印・診断保持・finalizeした。
sourceではSite/Yard境界が各1 quadを生成し、fixture境界数7→25と一致する。旧rawに所有対応がないため
個別handleの帰属確定とは主張せず、新sidecarで全Mesh2d/3dと境界markerの所有を直接記録する。

- [x] primaryだけ境界quadをsystem-local強handle1つへ共有。個別material/scale/色・gameplayは維持。
- [x] 別sourceのreferenceには受動所有観測だけ追加。旧凍結source・描画実装は変更しない。
- [x] 旧referenceの7/25 exclusive境界quadと残る資源を分類し、全量・既知失敗をpaired結果にも保持する。
- [ ] 拒否系・ECS・両source品質・Help/storage・exact独立reviewを閉じる。
- [ ] fresh補足v4で実所有と初終一致を照合し、現行sourceのglobal plateauと改善量を正式pairedで確認する。

No impact: 境界の表示寸法/色、入力、配置成立条件、保存、通知、Help到達経路は不変。
旧基準の重複を許容する分類を現行側へ適用せず、候補のperformance/art/release承認を代用しない。
M8-dのv3では初終安定まで確認したが、RTT診断markerをrun.logでは未確認。旧匿名画像の直接帰属は未確定のまま残す。


### M8-f: 同一検証操作内のCapture証拠の共有

resource v4は両sourceのうち旧referenceの2runで初終・実所有・残資源一定を独立verify/画像以外のraw読取・
seal/保持/finalizeした。manifest `a2be883d0d8b60893a780dc127c92690c01dc0d971d0909ce0575efd1aaf3cb4`。
旧境界7/25 quadのglobal plateau失敗は保持し、予算をcurrent測定前に固定した。
prepared v1の両Capture buildはpass/finalizedだが、計測caseはまだ起動していない。

事前のCapture check_stage実測は15.979秒。Memoryでは同じCapture全原本を複数回再検証する呼出しがあり、
prepare＋sealで固定180秒gapへ影響するため、検証項目を保ったまま同一呼出し内だけ結果を共有する。
privateなproofは完全verifyで作り、stage/path/plan SHAへ拘束する。CLI/保存JSON/次processからは受け取らず、
standalone build verifyは完全検証を維持する。Memory最終結果でも同じproofから全cross-stage比較を行う。
新helper/新計画へ既存receiptを書き換えず、同じCargo cacheからfresh build batchを正規に拘束し直す。
予算・MAD・gap・原本検証・art/release条件は変更しない。Help No impact（host検証処理のみ）。


### M8-g: 古い精算済み履歴の終了上限

v2のCapture12 caseは個別verify/seal/finalizeまで成功したが、全体判定は旧3件のfinished_at欠落で
`other validation interleaved`として失敗した。旧2件は9月26日、1件は11月11日に正式精算済みで、
verified_at/finalized_atは今回の開始前、owner/child identityは解除済み。Memoryは未実行。
旧12runを新しいdriverへ流用せず、失敗を診断用途で保持し、補正後のfresh plan/subjectへ進む。

command終了時刻があれば従来どおり使用する。欠ける場合だけ、既知の非成功result・非zero integer exit・verified→finalizedの
時刻整合・キーが存在して明示Noneの解除済みidentityを持つ正式finalized履歴について、finalized_atを保守的な終了上限として扱う。
本来の終了時刻は作らず台帳も書換えない。今回interval内までの上限、unknown、非finalized、live identity、
欠測/逆順の時刻は引き続き拒否する。除外した履歴と上限の出所はstage結果にも残す。
予算・180秒gap・生データ・独立検証の条件は不変。Help No impact（hostの履歴判定のみ）。


開始時刻の欠測も一律除外しない。実行時刻が欠測の正式精算済み非成功履歴だけ、登録created_atを開始下限とし、
exit未設定・検証/精算時刻・ownershipを別契約で確認する。exit欠測だけで未起動とは断定しない。未精算/unknownの開始欠測は拒否する。
9月の1件にはold-boot child identityが残るため、現在kernelのboot ID/btimeを二重読取りし、全nonnull identityが
別bootと証明できるときだけ、max(正式精算時刻, 現boot開始)を保守的終了上限にする。PID不在から推測しない。
同boot/不正identity/boot観測欠測は拒否し、boot証拠と旧identityもauditへ記録する。


開始/child記録がない正式精算済み履歴も発見した。欠測を無条件に除外せず、古いcoordinatorの履歴は
正式closeが現boot前であることとkernel境界を要求してboot時刻を終了上限にする。現coordinatorの履歴は
完全なregister→seal→result→finalizeの保存transition（追加のstart/child/recoverがない）と時刻整合を要求する。
後者の上限はfinalize保存event時刻。child PIDだけが残る不整合や不完全WALは拒否する。
いずれも実終了時刻や未起動を断定せず、保守的区間と正の根拠をauditへ残す。
