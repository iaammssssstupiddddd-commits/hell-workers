# 橋以外の建設物・正式ビジュアル導入

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `non-bridge-production-release-plan-2026-10-04` |
| ステータス | `In Progress` |
| 作成日 / 最終更新日 | `2026-10-04` / `2026-11-07` |
| 作成者 | Codex |
| 関連提案 | N/A |
| 関連Issue/PR | TAK-14 / 基盤統合PR #29 |

## 1. 目的

2026-11-07材料返却工程: 既存のゼロ材料caseを変えず、通常RestAreaの実搬入Woodを受動観測してから
同じF5/F9・二段階取消を行う別feedback caseを追加する。資材付与・進捗設定・AI割当は行わない。
実前状態の材料数と、新規の返却Entity IDs/種類/数量を厳密照合する。材料なし・施工完了・
同subject不一致・返却不足は拒否し、全予約/inventory清算や正式release承認へは昇格しない。
`material-refund-feedback-20261107-v1`は実Wood1の保存読込後、時間再開中のUI遷移で
通常施工が完了して対象を失いinvalid。停止中に対象詳細へ移動する手順へ修正した。
v2は実取消、owner2219v1／epoch1のcleanup、Wood2と返却Entity2411v18・880v6を観測したが、
読込Wood1の後に通常搬入が1個増え、旧verifierのloaded量との一致要求でinvalid。
save/load間の量・progress一致は維持し、取消時はsame-owner/epochの実raw履歴が
loaded量から単調増加し、最後の実材料数と返却ID数・種類が厳密一致する契約へ訂正した。
v3は配置後のSpace入力が時間再開を成立させずtimeoutでinvalid。新caseのみ既存HUDの
Normal（1x）／Pausedを一度操作して実状態を待ち、未知入力は再送しない。
v4は実搬入・保存読込・停止中の対象選択に成功したが、Normalを押して時間再開後、
release時にBevy cursorがnullとなりgesture guardが拒否。取消は送っていない。
4試行ともinvalidのままseal/finalizeし、それぞれの修正・原因調査用途だけを診断保持した。
限定コードは独立read-only review、55 focused tests、tooling gateを通過。profiling-only Rustの
材料snapshotはworkspace通常/profiling試験・両Clippy警告0で確認済み。非ゼロ返却native受入は未達。
次は入力ガードを緩めずfocus/cursorを切り分け、独立して実搬入branchのtransaction producerを追加する。

実搬入producerの実装方針: 手運びとwheelbarrowの既存Blueprint搬入成功branchでのみ、
actor/assignment/item/owner/epoch、実増分の前後量とfootprintをprofiling限定で記録する。
明示観測resourceがない通常起動は収集しない。item消費のdeferred適用とowner/material整合を
確認してからbounded receiptへ加え、未消費・foreign epoch・cap超過は観測失敗として閉じる。
状態差から搬入commitを推測せず、全予約清算・BuildWork・art/releaseを付与しない。
既存搬入・疲労・chain・予約解除・施工速度を変えず、拒否系単体試験、変更別検証、
Help No impact判断、storageとexact-source独立reviewを通してから新しい実機証拠へ結ぶ。

配送producerの手運び（通常終了・Build chain）／猫車実handler試験、bounded receipt拒否系、
Pythonのidentity/count/epoch/item照合を追加し、Soul AI 115件、通常/profiling workspace試験、
Memory/Tracy/RenderDoc構成、両構成Clippy警告0、tooling、Help No impactを確認した。
独立read-only reviewはコードと関連仕様を限定承認。rust-analyzerの既存Query呼出しの一時エラーは、
コード変更なしのfresh診断で0件へ戻った。変更別CIは既存依存advisory 2件で引き続き未通過。
fresh `tak14-rest-area-delivery-refund-feedback-20261107-v5`は実Wood配送4件を記録し、
保存・ロードWood1一致後、新epoch1のowner810v1へWood4まで通常配送、返却4IDとowner/request
cleanup・4tile walkability復元まで到達した。しかし旧Soul1845v0を触るdeferred commandのWARNで
厳格なgame-log検査が拒否し、invalidとしてseal/finalizeした（診断保持43,352,064 bytes）。
入力ガードは変更せず、この試行ではfocus/cursorを維持。警告を無視した受入や旧invalidのpass化はしない。
次はロード前Entityを保持する遅延会話cooldown等のproducerを実再現で特定し、修正・fresh実機を行う。
全予約/inventory・通常起動の正式asset・art/numeric/performance/release受入は未完。

次の限定修正は`hw_visual::speech::conversation::end_conversation`のcooldown遅延処理。
Bevy 0.19の独立DelayedCommandQueueはworld replacement後も期限まで残るため、
期限到達時にexact Entity generationの生存を読み直してからmarkerを除去するclosureへ変更する。
ログfilterや全遅延queue破棄は行わない。まず旧実装で置換後の実warning捕捉試験を失敗させ、
通常の期限、Virtual pause/resume、index再利用、実visual reset、中断時cooldownなしを回帰検証する。
会話時間・報酬・操作・保存schemaは維持し、Help判断・workspace検証・独立reviewとfresh nativeを再実施する。
旧実装はworld置換・pause/resumeの2試験で実WARNを再現、修正後は全4試験pass。
独立read-only reviewはこの限定修正を承認。fresh nativeと全体受入は別に確認する。
fresh `tak14-rest-area-cooldown-refund-feedback-20261107-v6`はsource
`31111610b5121cb963beb4aefd41727cde05415fdae63fcaee6cee73709390a4`で、
通常配送Wood0→1、保存読込Wood1、epoch1でWood1→2、owner2330v8の取消と
返却2102v10／2360v3各Wood1、request2098v9閉鎖・4tile通行復元を確認した。
Intel Arc/Vulkanの全6画像・原入力・rawを独立reviewし、登録verifierとstrict log検査pass、
seal/finalize済み。今回WARN/ERRORはなく、旧Soulへの遅延command警告は再現しなかった。
v5はinvalid履歴のまま保持する。v6の29,396,992 bytesは通常施工・全予約/inventory観測の
次工程比較だけをconsumerとして保持し、旧source/binaryの再検証依存は精算した。
通常/profiling workspace、Memory/Tracy/RenderDoc構成、両Clippy警告0、Help No impact、
rust-analyzer 0件、contracts/toolingを確認。全体CIは既存依存advisory 2件で未通過。
この限定feedbackを全予約/inventory・art/numeric/performance/release承認へ読み替えない。
原因修正とfresh v6の確認後、v5診断consumerを正式解除し、process不在・原本なし・
symlinkなしを確認して生成raw/画像/検証用saveだけを削除した（43,352,064 bytes）。
失敗metadata・採用コード・制作原本・最新v6証拠・通常Cargo cacheは保全した。

次の通常施工feedbackは既存`commit_completed_building`→`publish_building_completed`の
domain event記録を利用する。経路確認で元Blueprint・新building・kind・footprintのproducerが
既に存在すると分かったため、重複producerやゲーム処理は追加しない。
新しい`rest-area-completion` caseは通常配置後に実配送・施工を待つだけとし、完了event、
実Build task終了message、同epochの旧Blueprint消失・新ownerと全map占有をrawと画像で照合する。
状態差やUI通知だけを施工commitへ昇格せず、施工条件・速度・spawn・予約は変更しない。
全予約/inventory清算・正式asset/releaseとは分け、拒否系試験・Help判断・独立review・fresh実機を経る。
独立reviewで配送前Build完了を許す抜けを発見したため、最終配送以降のsample/time、
実ActiveTaskIdentityとの完了前一致を必須化した。元配送assignmentを保持するchainは維持し、
同sampleでのdomain callback→Lastのmessage読取順も別に検証する。関連59 tests pass。
初回`tak14-rest-area-construction-feedback-20261107-v1`は通常Wood5搬入・施工完了に到達したが、
verifierがRestAreaを誤って通行可能と期待して拒否したためinvalid封印/finalizeした。
既存`BuildingType::blocks_movement`とWorldMap完成処理は4tile通行不可が正しいため、
ゲームを変えず新caseの述語・fixtureだけを訂正し、通行可能を拒否するtestを追加する。
20,246,528 bytesの初回診断をfresh確認まで保持し、旧invalidをpassへ変更しない。
fresh `tak14-rest-area-construction-feedback-20261107-v2`はsource
`860333f9ad5043357e48a901ebdac833d59b66a99a07f1f6e3cbab20e158688d`、harness
`16199865666c335e2e14d8ba01470d7f84044a521bbfbf8015a82527dcd0384c`で限定pass。
Blueprint2233v0へ5個の実Woodを配送し、actor1863v0／起点assignment2234v0のBuild完了と
新building2283v42への4tile占有移管をsample206で確認した。domain event359→Lastのtask361、
実時刻は双方59.600192742。旧owner消失・全4tile通行不可、全3PNG/原入力/rawを独立review、
登録verifier・strict log検査pass、seal/finalize済み。Intel Arc/Vulkan、WARN/ERRORなし。
20,033,536 bytesは通常利用/Dreamと残lifecycleへの比較用途だけを保持し、旧source/binary依存は精算。
完成画像は現行fallbackの簡素な表示であり、正式art/numeric・全予約/inventory・性能・releaseは未承認。
通常/profilingのworkspace試験・両Clippy警告0、Memory/Tracy/RenderDoc構成、関連59 tests・tooling・
Help No impactを確認。rust-analyzerはcfg-disabled hintのみ、エラー0。全体CIは既存依存2件で未通過。

次のCI復旧は依存の最小補正として分離する。`futures-util`は同0.3系列の保守版へ更新して
`pin-utils`経路を解消できるか確認する。`ruzstd`は0.9.1以上の修正版が必要だが、Bevy 0.19の
`bevy_image`は^0.8指定のため、エンジン全体の版変更・feature削除・advisory無視は行わない。
必要なら既存vendor方式でBevy 0.19のimage crateだけを正規原本から固定し、APIを変えず
decoder依存だけを更新する。license・原本hash・実差分・削除条件と拒否系decode試験を残し、
Help判断・通常/profiling検証・native画像読込・独立review・storage・変更別CIで検査する。
依存補正は美術承認や9種の正式releaseを代替しない。
修正前の実Bevy KTX2回帰試験では、正常1024 bytesとtruncated拒否はpass、
1KiB windowに対する1025 bytesの不正RLE blockを旧ruzstd 0.8.2が受理してredとなった。
巨大なbombを生成せず拒否境界を再現した。公式bevy_image 0.19.0の全Rust sourceを保持し、
optional ruzstd指定だけを0.9.1へ変更するvendor patchで補正する。出典・除去条件は
`vendor/bevy_image/PATCH.md`を正本とし、修正後の結果は別途確認する。
修正版ruzstd 0.9.1では同3試験がすべてpass。`dev.py deps`はadvisories/bans/licenses/sources
すべてokとなり、RUSTSEC-2026-0441/0344の2件を解消した。既存duplicateとyanked版
（clipboard-win 5.4.1、spin 0.10.0）のwarningは別扱いで残している。
`image-decoder-feedback-20261107-v1`はsource `97b343980d38…`、通常画面1PNGを
mainと独立reviewerが実見し、登録verifier・seal/finalize pass。Intel Arc/Vulkan、WARN/ERRORなし。
manifest `8f974eeeb5b3…`、PNG `3c0e16303f48…`。入力なしの描画と起動時KTX2 LUT読込に限定し、
全asset形式・art/numeric・性能・9種releaseを承認しない。最終review中の3,985,408 bytesだけを保持。
Helpは公式API/featureを維持する不正画像拒否・futures保守更新で、入力/UI文言/Help到達性/
gameplay/save schema不変のためNo impact。全体の変更別検証は引き続き実施する。
依存補正は`c0267a15`で保存済み。base `37c76a3dd04cc1c7baf91faa5779598f05fae3c6`からの
変更別CIはcontracts/tooling/deps/rust全groupを通過した。通常/profiling全試験、Memory/Tracy/
RenderDoc構成、Clippy警告0、tooling 538+208試験、`dev.py check`、primary storage pass。
最終比較が終了したためdecoder native holdを正式releaseし、生成物3,985,408 bytesを撤去した。
コード・出典・限定pass metadataは保持し、この旧jobを再検証する義務にはしない。

次工程は通常RestArea施工完了の後の利用・Dream放出である。準備によるcamera/pause以外は
既存入力と通常AIだけを使い、SoulのDream/疲労/idle/Visibility、RestingIn/予約/owner側occupantsと
DreamPoolをread-onlyで採取する。既存DreamTransferredVisualMessageのexact Soul/RestArea/amountと
同process/nonce/epochのbefore/afterを結ぶ拒否系試験を追加し、通常入所と実放出を実機確認する。
occupant/Dream/materialの注入、強制完了、gameplay変更、art/numeric/performance/releaseの代用はしない。
変更別検証・Help・独立read-only review・native seal/finalize・storageを次の同sourceで再実施する。
通常/profiling workspace試験・両Clippy警告0と全groupの変更別CIを通過した。
初回`tak14-rest-area-usage-feedback-20261107-v1`は通常Wood5搬入・施工・Soul1867v0の入所と
Dream9.4800758→6.8300657、Pool79.4395065→87.3095398を記録したが、最終UI checkpointより
raw writerが1.091728332秒手前で終了し、1秒以内の厳密照合でinvalidとなった。
invalidをseal/finalizeし、38,535,168 bytesを同checkpointのwriter競合調査に限定して保持した。
UI PostUpdate撮影後、Lastのraw記録が既存の同状態/時刻条件を満たすまで最大15秒待つ補正を行う。
許容幅を緩めず、未到達だけを待ち、foreign identity・時刻/関係破損・timeoutは即拒否する。
旧invalidを再解釈せず、拒否系回帰と独立review後にfresh batchで確認する。
補正後`tak14-rest-area-usage-feedback-20261107-v2`はsource `b77a1c8b1538…`、harness
`d2e2f1487740…`で登録verifierと別プロセスのverifyが限定pass、seal/finalize済み。
通常Wood5配送・施工後のRestArea2275v32へSoul1863v0が入所し、同世代・同入口座標の
Hidden/Restingを維持してDream4.1100345→1.4600365、Pool72.4896851→75.1398468を確認した。
実RestArea sourceのmessage総量と減少量、同epoch・停止中UI/rawの状態/時刻が一致する。
全5PNGをmainが実見し、strict log検査はWARN/ERRORなし。画像は現行の灰色fallbackであり、
美術・数値採用・性能・正式releaseの証拠ではない。36,794,368 bytesは退出/cooldownと
利用中save/loadの次工程比較だけを保持し、旧binary再実行依存は精算した。
通常利用は`5486abf6`で保存し、変更別CI全groupと独立code/画像reviewが通過した。
v1のwriter競合調査は完了したため診断consumerを正式解除し、生成物38,535,168 bytesを
撤去した。invalid metadata・採用コード・最新v2証拠・通常Cargo cacheは保全した。
次は独立した`rest-area-exit` caseで実Dream放出を最後まで待ち、同Soul/ownerのfinal message、
関係解除・可視復帰・正のcooldownと歩行可能な入口をraw/画像で結ぶ。材料/Dream注入・
強制退出・duration変更は行わず、既存利用caseと正式art/release未達を維持する。
退出verifierは独立reviewで見つかった残量/Poolの拘束不足を補正し、rawとUIを同時改変しても
不正残量・Pool加算なしを拒否する回帰を追加した。通常再生成は実cooldown経過と最大rate/
slow-stepからだけ許容する。関連68 tests、実Rust定数契約1件、変更別CI全group、
`dev.py check`、profiling Clippy警告0、Help No impact、rust-analyzerエラー0を確認した。
`tak14-rest-area-exit-feedback-20261107-v1`はsource `dd827e2d22be…`、harness `b76c0ece1f34…`で
実行と登録verifier・別verify・独立全6PNG/raw reviewが限定pass、seal/finalize済み。
Soul1867v0／epoch0の残Dream2.7300391に対し実RestArea放出22件は計2.73004065、
sample414のfinal時にDream0・関係解除・Visible/walkable・cooldown15を確認した。
撮影時は通常再生成Dream0.35／cooldown11.50、Pool84.98965→102.41963で全量以上の増加。
UI/raw境界の時刻は完全一致し、Intel Arc/Vulkan、WARN/ERRORなし。灰色fallback表示であり、
cooldown満了・利用中save/load・art/numeric/performance/releaseは未承認。
43,528,192 bytesは残lifecycle比較用に保持した。旧施工v2と利用v2の比較consumerを最新batchへ
移管・正式解除し、生成物20,033,536＋36,794,368 bytesを撤去。結果metadataとコード/原本は保全した。
施工完了v1の誤ったwalkable期待値はv2で修正・独立review・実機passとなったため、
旧v1 diagnostic consumerを正式releaseした。稼働process・symlink・原本がないことを確認し、
`target/native-acceptance/tak14-rest-area-construction-feedback-20261107-v1`の生成物
20,246,528 bytesを撤去した。invalid履歴と採用コード、最新v2の比較用途は保持している。

2026-11-07再開工程: 通常Blueprint取消が実際に発行したrefund Entity IDsと、
deferred適用後のowner/request/storage/map・関連actor状態を結ぶprofiling-only観測を追加する。
通常RestAreaの既存input recipeへ同nonce/PIDのrawを接続し、ゼロ材料予約の取消を実機確認する。
単体試験の非ゼロrefundと通常worldのゼロ材料取消を区別し、完全な予約/inventory清算、
SoulSpa専用取消、全9種のart/numeric/performance/release未達は維持する。
取消10件と通常/profilingのworkspace試験、Memory/Tracy/RenderDoc構成、両構成Clippy警告0を確認。
全体gateは既存依存のRUSTSEC-2026-0441（pin-utils）とRUSTSEC-2026-0344（ruzstd）で未通過。
例外追加や依存の無検証更新はせず、正式完了条件として残す。
初回`cancellation-witness-feedback-20261107-v1`はbuild成功後、ゲーム起動前のjson未importで
invalidとしてseal/finalize。設定生成関数を直接呼ぶ回帰テストを追加し、修正後はfresh試行で確認する。
fresh `cancellation-witness-feedback-20261107-v2`は5PNG/rawの独立reviewと登録verifierで限定pass、
seal/finalize/storage pass。Intel Arc/Vulkan、same nonce/PID118045、新epoch1のowner1834v1
（UI Entity ID8589932757）についてsample86で実applied witnessを確認した。
配送request1839v1とactor4件の参照解除、owner/map tombstoneと4tile walkability=trueを照合。
進捗0・材料0・refund0の実操作であり、非ゼロ返却や全予約/inventory清算の受入ではない。
次は同じ通常取消経路で非ゼロ実搬入材料の返却と全清算を検証する。9種の正式導入は未完。
この新証拠を次の比較用に保持し、用途を終えた旧RestArea zero-material証拠と初回build-only
診断をconsumer release後に削除した（計20,017,152 bytes）。制作原本・最新Tank/RestArea証拠・
通常Cargo cacheは保持。削除後storage checkは404,728,475,648 bytes、339 batch、未分類0でpass。

2026-10-12の最新限定進捗: `89f40fe2`で通常Tank親Blueprintと2childの計画commit、
F5/F9のManual1保存復元、dialogなしのロード後画面をactual-window/rawの独立review・
登録verifierで確認した。次は通常計画取消のowner/付属保管所/map cleanupと実terminal receipt。
完成・正式asset・9種全体受入は未完。
`e9bc838f`の初回はF5直後のcatalog layout未確定（sample65、可視controls空）をクリック前に拒否し、
save/loadなしのinvalidとして精算・診断保持した。入力を再送せず、2つの実computed-layout sampleが
同じwindow／epoch／session／controlを持つまで最大3秒のpassive waitを追加してfresh subjectで再確認する。
`d58de249`のfresh実機は描画待ちを通過し、Manual1のSave成功（sample57、session1、497110 bytes）を
実見した。Loadはsample71/session2でInvalidData、epoch0のまま拒否。
原saveにはYard発行のDeliverToBlueprint requestとFamiliarのManagedByが混在していた。
Soul割当applyの`IssuedBy(=ManagedBy)`片側上書きが原因で、requestのproducer ownerを
deferred適用時に保持する修正を進める。原失敗・saveは診断保持し、load validatorは変更しない。
修正`ba1d8e3a`は変更別CI、Clippy警告0、workspace checkと独立reviewを通過。
fresh実機はSave以前にcameraのKeyW区間で停止した。sample22→23が0.505秒空き、
Yard投影が96→348pxへ移動してfeedback停止帯を通過した。通常worldやinputを変更せず、
明示profiling observerだけheld WASDを50ms間隔で読む。sample/event capは維持し、
hostは停止帯通過を即時拒否して元keyを一度releaseする。旧launchの再送・補正入力はしない。
`562a151b`のfresh実機では通常Save sample67/session1・Load sample82/session2がSucceeded、
epoch0→1、親1898v2→1984v1と2childの新ID、footprint・空材料・map占有の復元をrawで確認。
実Manual1は501865 bytes。ただしloaded.pngはLoad確認dialogが残り、独立画像reviewが拒否した。
cycleはinvalidとして精算し、原raw／save／画像を撮影同期修正の診断に保持した。
Load実receipt直後のcaptureをやめ、2つの後続Closed sampleと500ms settleをpassiveに要求し、
fresh画像を独立確認する。raw成功を画像成功・正式construction・releaseへ読み替えない。
fresh `tak14-save-tank-present-feedback-20261012-v1`は独立9PNG reviewと登録verifierで限定pass、
seal/finalize/storage check済み。Save sample67/session1、Load sample81/session2、capture sample84は
receipt後566ms・後続3sampleのClosed、新epoch1。原親2367v1→798v1、付属2childのID置換、
4tile footprint・空材料・進捗0・stockpile占有を保持し、旧実entity/map残留はない。
実Manual1は494407 bytes、SHA-256 `541c691df5a3ce70428244177c39238702a4e1bbc7dd26c45bebc91879998835`。
通常Intel Arc/Vulkan、WARN/ERRORなし。全world canonical serialization・完成・art/numeric/perf/
正式releaseではなく、次の取消／cleanup比較用途の限定raw/save/latest画像だけを保持する。
既存通常RestArea recipeのfresh試行は配置・Save/Loadの4checkpoint後、Task Dashboard取消の
Left押下でtargetが再生成され、pressed観測を失ってinvalid。取消receiptはない。
本文のdirty再描画をheld Left中だけ保留し、dirty要求をreleaseまで保持する修正を進める。
simulation snapshot・既存同対象release guard・live capability拒否を変更せず、fresh実機で再確認する。
修正`76209f80`は同sourceの全変更別CI（contracts/tooling/Rust）、profiling全target Clippy警告0、
workspace checkと独立read-only reviewを通過した。fresh `rest-area-gesture-feedback` v1/v2は
どちらもrecipe入力前の初期WM activationで拒否（owned `0x1600004`、focus `0x400003`）。
配置・保存・取消のcheckpointはなく、invalidとしてseal/finalize済み。第三の盲目的な再起動、
強制focus、旧launch再送はせず、通常取消の実機確認は未達として保持する。
通常Tankの保存復元修正で用途を終えた旧生成job6件（placement-yard、companion、saveの
initial/layout/owner/cadence）を、終了済み・利用processなし・追跡原本なし・symlinkなしと確認して
正式consumer release後に削除した。実測合計77,844,480 bytes。履歴結果と採用仕様は維持し、
最新Tank raw/save・RestArea取消診断・制作原本・通常Cargo cacheは保持、storage checkはpass。
初期activation拒否は、公開Shell状態のread-only観測でOverviewActive=true、画面ロックfalseと
切り分けた。通常の公開Properties.SetでOverviewだけを閉じてfalseをreadbackし、新規jobで
既存owned focus guardを通過。強制focusやtimeout調整はしない。
`1756c610`のfresh `tak14-rest-area-gesture-overview-feedback-20261012-v1`は通常配置・Manual1の
Save/Load・二段階取消・4tile復元を独立5PNG/raw reviewと登録verifierで限定pass、seal/finalize済み。
親8589932601→8589932382、epoch0→1、進捗0・材料未完の予約を保持。
frame562で新親/epoch1のCancellationRequested、frame576で予約なし・4tile owner=null/walkable=true。
取消ボタンは各press/releaseで同task payloadを維持し、ロード後画像に確認dialog残留なし。
完成材料のterminal refund、全actor/pool cleanup、art/numeric/performance/releaseは未達。

通常起動で橋以外の9種が正式ビジュアルになり、完成・状態・配置・移動・施工・カタログ・保存復元が一致する。
原移行計画の受入条件を維持し、橋だけを今回の完了対象から外す。技術previewと正式採用を区別する。

## 2. スコープ

Tank、MudMixer、RestArea、SoulSpa、Door、WheelbarrowParking、SandPile、BonePile、OutdoorLamp。
Wall/Floor、Bridge、gameplay・保存schema・配置条件の変更、廃止統括の再起動、外部公開は対象外。

## 3. 現状とギャップ

- 基点 `30477dab4cd091abfb2ee79cc1cb183e28c936b5`。通常起動はrelease bindingなしで従来fallback。
- Tank/MudMixerの未承認原本・previewは外部asset正本の `staging/imports/tak14-20261004` に保全済み。
- M3制作器とM5専用画像導入器は存在。primaryのbuilding manifestは未導入。
- 独立art/numeric採否、正式candidate受入、昇格・install・rollback、通常起動の既定接続が不足。
- production native toolingは通常開発のprocess/window/capture・host計測producerが不足。旧hostを代用しない。

## 4. 実装方針

mainがprimaryを直列編集し、branch `codex/tak14-non-bridge-release` を使用する。
候補を制作・比較し、独立レビューを経て正式assetへ昇格する。未承認assetを起動時に自動承認しない。
最終導入では承認済みの同一世代inventoryを既定起動へ接続し、明示overrideと失敗fallbackを検証する。
Bevy 0.19の既存loader/pool経路を使う。公開・push・mergeは本依頼の完了条件にしない。

## 5. マイルストーン

### M1: 原本・候補

- [x] 保全原本と現行loader/制作器を調査し、storage checkを実施。
- [x] M2の現行subject候補、M3両種、M5四種を制作・技術検査（未承認art_previewのみ）。
- [ ] 実機比較画像と数値契約を独立レビューし、exact identityに採否を記録。

### M2: 受入・正式導入

- [x] primary validation所有の通常world固定F12入力probeを拒否系試験・実機・独立reviewで確認。
  これはinput/raw輸送の技術境界だけで、正式recipe、domain lifecycleやreleaseの代用ではない。
- [ ] 通常開発の不足producerを実装し、拒否系試験と独立レビュー。
  - 11月7日: Blueprint取消の実返却ID・owner cleanup観測を実装/検証中。全lifecycle成功ではない。
- [ ] 同subjectのCapture→Memory、state/lifecycle、有限pool/cleanup、群別性能を検証。
- [ ] Door既存assetと全consumerの適合監査。
- [ ] 正式promotion/install/release/rollbackを検査。

### M3: 通常起動・完了

- [ ] 承認済みassetの既定起動経路を接続し、環境変数なしで9種の表示を確認。
- [ ] 原仕様・Help・asset記録を同期。橋は未完と明記。
- [ ] 同subjectの変更別検証、Clippy警告0、workspace test、独立レビューとstorage closure。

## 6. リスクと対策

未承認候補・古い技術passを正式採用に流用しない。原本を上書きせず新しい制作名を使う。
ゲームの論理を見た目都合で変えない。失敗・中断は正確に記録し、未検証範囲を残す。

## 7. 検証計画・データ管理

primaryの `dev.py validation` で制作/実機batchを登録、独立verify、seal、finalize/checkする。
M3最初の原本候補batch: `nonbridge-restarea-authoring-20261004-v1`。
現行RestAreaはv3（無地入口レビュー後の描線）、SoulSpaはv2（施工＋16mask）。どちらも技術候補で正式採用ではない。
小型4種は固定256px canvasの専用原画生成器を追加。worldのcrop/rescaleや既存item画像の変更はしない。
M5 v1のBonePileは板に見えるため不採用とし、関節と軸の形状を補正して再生成する。
RestArea actual-window v2は画像取得後、AdapterInfoログ不足でinvalid。失敗を保持し、子専用INFOログへ補正。
候補計画のready返却欠落とpackaged-byte verifyの承認schema検査を修正し、独立read-onlyレビューを受けた。
関連40件のunit test成功。これらはordinary lifecycle/性能/art/releaseの完了を意味しない。
RestArea actual-window v3はIntel/Vulkan・exact identity・静止inventoryのtechnical verify pass。
ただし遠景scale 5の画像は採否に不十分なため、明示profiling-onlyの近景／通常／遠景viewを追加して比較する。
M5 v2 BonePileは関節と湾曲により独立read-only比較で板状の指摘を解消。人の正式art承認ではない。
小型4種のruntime packageをfresh隔離rootで検証済み。SoulSpaは候補名不一致の出力前失敗をinvalid記録後、
正しい既存原本名による別batchでpackage検証済み。原本や製品assetは上書きしていない。
M2 albedoのface normal由来の焼付け明暗を除去。materialの色差だけを残し、normal反転でpaint bytesが
変化しない回帰テストを追加した。旧候補を新しい美術・数値承認として流用しない。
Help実経路レビュー: 未承認staging制作・byte検査・明示profiling比較cameraのみの変更で、通常起動、
player操作、文言、gameplay、save schema、Help到達性は不変（No impact）。正式導入時は改めてレビューする。
現行のTank水面tintとatlas塗装も未導入のArtPreview候補に限る。開発用panelはprofiling条件下だけで、
通常起動のasset解決、操作、表示文言、状態遷移、保存形式、Help到達性には変更がない（No impact）。
Tank/MudMixerのv2原本とruntime packageを検証。Tankの水面は面全体に連続する2本の波線に修正し、
三角形ごとの放射状筆跡を除去した。原本・数値・ゲームの水位判定は別契約のまま維持する。
RestArea/Tank/SoulSpa/Parkingの近景、Sand/Bone/Lamp/MudMixerの通常zoom実機比較はtechnical passで
独立verify・用途登録・finalize済み。普通の利用・精製・採取・配電lifecycleを実行した証拠ではない。
Lampは通常zoomで素材・籠の判読が弱く、追加の拡大比較が必要。SoulSpaの全16mask、
最遠・異なる照明、実際のDream／配電／回転・保存復元・owner cleanupは未受入。
offline verifierへ先行9種の明示non-bridge scopeを追加。10種fullの条件を維持し、
9種でも異なるsourceの累積比較・両instrumentの全leg・全計測値を要求する。不足producerは未実装。
package importの孤立テストで露呈したnative helperの相対importを補正し、CLIの既存入口は維持する。
利用者の継続指示に基づき候補の方向性は統括が判断する。独立art/numeric承認や正式releaseとして記録していない。
まとめたPython関連92テストとprofiling m2 probeのRust3テスト成功。通常／profilingのClippy警告0。
profiling既存警告の条件式・test配置だけを整理し、Bridgeの挙動・制作は変更していない。
通常validationのlive直接child/親出生・command・sourceを照合するread-only入口を追加。
これは未接続の所有確認のみで、ordinary-world producerや不足計測を実装済みとはしない。
入口の独立read-onlyレビューで起動時の子公開raceと未検証plan再hashを修正、拒否系10件成功。
街灯v2の追加近景はtechnical passだが、独立画像レビューで骨の節・籠の内部判読不足が残った。
縮尺の経路は正常なためM5 v3原画で曲がった骨支持と太い籠の帯・2区画を再制作した。
32wu canvas・中心接地点・off/on同一alpha・内部のみ点灯を維持し、上端余白も回帰検査。
原画と街灯runtime v2を登録済みbatchでbyte検証、用途保持・finalize済み。
街灯v2の通常距離 `nonbridge-lamp-normal-20261004-v3` もactual-window technical passだが、
籠が数pxの横帯に潰れることを実画像で確認。縦長籠へ修正した原画v6、runtime v3、通常距離
`nonbridge-lamp-normal-20261004-v4` を独立byte/actual-window verifierでtechnical pass、用途保持・finalizeした。
画像の独立read-onlyレビューでは点灯状態と縦長輪郭の改善を確認した一方、通常距離の骨・錆鉄素材は
判読不足。正式art採用は未承認で、遠景・暗い背景・密集配置・実配電は未検証。
街灯v3の近景 `nonbridge-lamp-near-20261004-v3` もtechnical passしたが、独立画像レビューでは
籠が笠、骨支柱が短い脚に見えたため正式art候補として不採用。旧v6原画とruntime v3の専用出力は
比較終了後に正式holdを解除し、復元可能なゴミ箱へ移動した。原画generatorと画面証拠は保全する。
原画v7では骨の曲がり・関節と、背景が実際に見える鉄籠の隙間へ形状を再設計。
`nonbridge-m5-originals-20261004-v7` と `nonbridge-lamp-runtime-20261004-v4` は
独立byte verifyのtechnical passであり、後者の identity は `art_preview`、manifest
`8cc4fec0d4bed81f3f0b47c44082c6b307a0411f76377805aef5c9a5c323323e`。
同identityの近景 `nonbridge-lamp-near-20261004-v4`、通常距離
`nonbridge-lamp-normal-20261004-v5`、最遠 `nonbridge-lamp-far-20261004-v4` を
Intel/Vulkanの実ウィンドウで独立technical verify後、用途登録・finalizeした。
近景/通常距離の独立read-only画像レビューでは開いた籠・曲がった骨支持・off/on差が残り、
正式art評価へ進める候補とした。ただし静止profiling画面に限る。最遠の独立画像レビューでは
種類・off/onを画像だけで確実に判読できない。この `far` はcamera scale 2であり、
Bevy 0.19 `PanCamera` の製品既定最大scale 5ではない。最大倍率側の比較と、選択した
Outdoor Lampの既存情報パネルにある種類・Supply状態の実操作経路を確認する。
既存UI実装とHelp本文の存在だけでは代替経路を受入済みとしない。明暗・重なり・低品質、配置preview、実配電、
art/numeric正式採否は未完了。
同一identityの最大zoom-out scale 5 `nonbridge-lamp-overview-20261004-v4` はIntel/Vulkanの
実ウィンドウでtechnical verify passした。画像では街灯とoff/onがほぼ判読できないため、
情報パネルの建物種別とSupply状態を実選択で確認する受入が必要。既存の
`append_building_model` / `append_power_consumer_model` は値を構築するが、
最遠での選択可能性や実配電との同期をこの静止画像から推定しない。
原画の出力先を生成前に `chdir` した試行はexit 125で成果なしと確認し、invalid seal/finalize済み。
同一候補の比較入力holdが後続の正規planを保守的に拒否したため、各plan前に対象holdのみ正式releaseし、
出力を保持して再登録、plan完了後に保持用途を戻した。旧passや空の試行を採否へ数えていない。
変更別 `dev.py ci check --base 30477dab4cd091abfb2ee79cc1cb183e28c936b5 --mode auto` は
街灯v7と情報パネル回帰テスト後のsubjectでcontract/tooling/Rust群pass。
これはこの検証結果を計画へ追記する直前のsourceであり、追記後の最終差分は再検証する。
Bridgeの整形だけの差分は除外した。情報パネルが同一OutdoorLamp entityの種類と実際の
PowerSupplyState変更を表示する回帰テストを追加したが、最遠の実ウィンドウで選択できる証拠ではない。
同sourceの独立read-only実装レビューはblocking findingなし。ただし実機操作、art/numeric採否、
ordinary lifecycle／性能、正式releaseは未受入であり、実装レビューから承認を推定しない。
既存static画像はその撮影時subjectの比較用であり、後続のtooling差分を検証したnative受入には流用しない。
通常worldの最初のhost観測器 `building_production_local_probe.py` を追加。
primary検証batchの直接child／登録planとclean sourceを照合し、隔離cwdの通常起動から
所有window、raw sample/event、子process RSS/fdを採る設計。入力なし・正式identityなしの技術probeで、
全lifecycle、Capture→Memory、GPU/native/アプリ全体handle、正式美術・数値・releaseを受け入れたものではない。
拒否系unit testと現subjectの実機観測・独立reviewを経るまでM2のproducer条件は未完とする。
同観測器は拒否系5テストと独立read-only reviewで阻害なし。正規batch
`nonbridge-ordinary-probe-20261004-v1` はsource `ee9410363d4e8f7bd144506a64ac3fb3a71527dd`で
Intel Arc/Vulkanの所有X11画面、raw 3 sample／0 event、子process RSS/fdを技術verify passとして
seal/finalizeし、比較用途をstorage holdへ登録した。画像は通常開始時点で建設物がなく、
操作・建設・配電・save/load・release identityは観測されていない。0 eventをlifecycle passへ読み替えない。
Door g7はprimaryの既存locator `21075f6b5c6e4412ccc17632ecc3b44bcb619bb901a06062fa0905c0be303002`、
receipt `95ba6a19494d84825db54075cf8735d2024d18e6983dc54343cdc71c19a046ad`と
6 core fileの保存済みhashが現時点で一致。現行HEADのghost/catalog focused test 4件もpass。
この静的監査は通常配置・load後・pool cleanup・性能の実機受入を代替せず、M2のDoor監査を閉じない。
8種の保存済みactual-window画像を独立read-only監査したところ、全て`art_preview`で正式採用は未決。
Tank近景の空状態は開発パネルに遮蔽され、水位差も弱い。SoulSpaの骨素材／全mask、
Lamp最遠の種類・状態と情報パネル経路も重点未達。明示building-art session中だけの
開発パネル非表示を補正し、同一新subjectでTank近景を再観測する。通常プレイヤーUIは変更しない。
`c606733989c0f21b6fba4dea7138190f5f873409`の明示art-previewでは、保存済みdebug表示が
再適用されてもDevPanelだけをLayout前に隠す実装・回帰テスト・固定read-only reviewを通過。
Tank近景 `tak14-tank-near-c6067339-20261004-v1` と通常倍率
`tak14-tank-normal-c6067339-20261004-v1` はIntel/Vulkan実window・同manifest
`87997feeafa4c99dddeab227d0e8e60bfc371d8572e208f0266a652abccc9596`で独立verify pass、
seal/finalize/storage check済み。近景の空槽遮蔽は解消し4標本を確認できた。
ただし通常倍率ではPartial(25)／Full(50)の水位差を判読しづらく、近景の木部材・鉄帯と
空槽内面の三角形状も正式画風採否には不足。静止画像は背景・preview・実給排水・性能を含まない。
この候補の正式art/numeric採用とreleaseは保留し、まず通常倍率で状態差が読める
表現／確認経路と内面描線を修正し、fresh identity・同subject画像で再評価する。
通常倍率の判読不足を受け、Tankの既存`TankPartial`状態にのみ低水位の陰を示すmaterial tintを
追加した。水量・高さ・asset identityは変えず、世代交換／owner cleanupの既存回帰を維持する。
これは候補補正であり、変更後のactual-window再撮影と独立採否までは未受入。
`29fcf958b0f661e8edc56979cff10c472ce6ffd9`の通常倍率feedback
`tak14-tank-tint-feedback-29fcf958-20261004-v1`は、Intel/Vulkan実windowの静止画像を
独立verifyしtechnical passとしてseal/finalizeした。Partial水面がFullより暗く、以前の
同倍率画像より区別しやすい。ただしfeedback-onlyであり正式art、数値、releaseの権限はない。
同sourceのart-preview試行`tak14-tank-normal-29fcf958-20261004-v1`は、recipeが旧証拠の
`target/profiling/bevy_app`を上書きするビルドを始めたため、そのcodec SHAが変わる前に
該当ビルド工程を停止しinvalidとしてseal/finalizeした。画像やpassは存在せず、診断ログだけ保持。
旧2件のcodec SHAは不変。正式previewは別worktreeの専用targetで作る。
独立画像レビューでは状態差の改善を確認したが、暗色が水質差に見える可能性、鉄帯と空槽底の
面割れが残る。数値fixture・geometry・水量を変えず、槽底を連続した同色・world基準の木目に、
外壁だけを控えめな錆鉄帯にするpaint候補と拒否系テストを追加した。原本の新規生成・近景／通常
比較と独立採否まで正式候補ではない。
`b8dbce220e18ecf39e80fa5d04d185ae014925cd` でTankのpaint候補を新規生成し、
原画とruntime packageのbyte/UV/geometry/state検証をtechnical passとして保持した。
runtime identityは `art_preview`、manifestは
`22ff091af4af76527361c9b6c6da8c224c792a9fd81e58e7b3a3a9cd4755329f`。
専用worktreeの初回actual-windowはignored assetの不足でinvalidとし、失敗記録を保全。
primaryと同一内容のasset viewを復元したfresh batchでは通常倍率と近景がIntel/Vulkanの
所有X11 window・独立verifierでtechnical pass、用途登録・finalize・storage check済み。
独立画像比較では槽底の放射状面割れが減り、Partial/Fullの色差も増した。一方、近景の
PartialはhoverのMove tooltipとselection強調に重なり、鉄帯のface境界に細い切れ目と
太さのばらつきがある。背景・照明・遠景・実給排水とstate transitionも未受入。
従ってこれは静止技術証拠であり、独立art/numeric承認、正式候補、promotion/installではない。
近景のhover重なりは通常worldの美術欠陥と混同しない。明示profilingのbuilding-art静止
sessionだけhover更新を抑え、通常起動・通常inputは不変のまま同identityをfresh sourceで再撮影する。
`64f69762c3edbbd3147bac725e8f8792af97c078` の明示ArtPreviewでhover由来の
Move tooltip・selection強調がないTank近景をfresh subjectのIntel/Vulkan実windowで取得し、
独立verifierのtechnical pass、用途登録、finalize、storage checkを完了した。
独立read-only画像評価ではEmpty/Partial/Fullの区別と槽底の改善を確認したが、
外壁の錆鉄帯にUV面境界の切れ目と幅の不揃いが残り、正式art採否は不可。
静止草地画面であり、異なる背景・照明・遠景、実水量遷移、numeric freeze、releaseは未検証。
この指摘に限って外壁の2帯を同じ高さ比率の面へ塗り、UV境界外に描線を延長する候補を
`534fd9b30c6bd5050bde567faae40affb92ec110` で補正した。12面の両側で帯がgutterへ届く
拒否系回帰テスト、変更別CI、Help No impactを確認した。新規のTank原本
`nonbridge-tank-authoring-20261005-v4` とruntime package
`nonbridge-tank-runtime-20261005-v4` を同sourceから生成・独立検証し、technical passとして
seal/finalizeした。runtime manifest identityは `art_preview`、manifest SHA-256は
`edc65d14fe5f97b8953ccdd1487b62b1fcb51d9112915968d5994c2edf29f9cb`。
数値freeze、独立art approval、production candidate、promotion authorityはいずれもない。
専用worktreeと同一codec/identityのIntel/Vulkan所有X11 windowで近景、通常倍率、遠景、最大ズームアウトを
それぞれfresh batch `tak14-tank-band-near-534fd9b3-20261005-v1`、
`tak14-tank-band-normal-534fd9b3-20261005-v1`、
`tak14-tank-band-far-534fd9b3-20261005-v1`、
`tak14-tank-band-overview-534fd9b3-20261005-v1` として撮影した。4件とも登録済み独立verifierで
technical pass、用途登録、finalize、storage check済み。独立read-only画像評価では、近景・通常倍率に
帯の重大な断絶は見えず、Empty/Partial/Fullも色差を伴って判別でき、正式art採否へ提出する
候補には進められる。遠景の草地上でも三状態の色面と桶の外形は判別できるが、木目・水位高低差・
鉄帯の連続性や素材感は判定不能。一方、通常倍率の鉄は細い灰色線で錆の質感が弱く、最大ズームアウトでは
建物が十数pixel程度のためPartialとFullを確実に区別できず、鉄帯も判定不能。黒背景の静止画から
通常地形の最遠判読性は証明できない。これらは静止ArtPreview比較に限り、通常worldの
給排水遷移、異なる背景・照明・重なり、低品質設定、native性能、数値freeze、正式art採否、
release/install/rollbackは引き続き未受入。
通常world固定F12入力probeの初期7 batchは全てinvalidとして独立にseal/finalizeし、診断保持と
primary storage checkを完了した。v6 `tak14-ordinary-input-probe-b885f623-20261005-v6` は
所有X11 windowへの押下・解放receiptを残したが、製品raw 49 sampleには対応する入力イベントがない。
focus待ち・tap順の調整は同じ欠落を再現したため打ち切った。v7
`tak14-ordinary-input-probe-69aa99d1-20261005-v7` は原 `KeyboardInput` メッセージも記録したが、
XTest F12 receiptに対して48 sampleで原メッセージ・ButtonInput状態イベントがともに欠けた。
Xwayland上流の入力配送を疑い、次の技術probeは既存の同意付きWayland portal輸送へ固定する。
所有windowと同意、製品rawの順序を新sourceで確認する。これは9種のlifecycle、正式recipe、
art/numeric承認、releaseを完了させるものではない。
新source `dcac0f7541b26ec788b3bee93973e0448c5eac96` の
`tak14-ordinary-portal-probe-dcac0f75-20261005-v8` はportal同意と固定F12輸送receipt、
製品rawの原メッセージ・ButtonInput状態遷移を5 sample／4 eventで確認し、独立verify pass、
用途登録、finalize、primary storage checkを完了した。これは入力配送境界の技術証拠で、
通常worldの施工・保存・cleanupや9種の正式受入を代替しない。次はこの成立経路を用いた
通常world Capture/Memory producerとhost instrumentを別工程で実装・受入する。
現行の正式raw verifierは新しい原 `KeyboardInput` イベントを拒否していたため、厳密な
key/state/repeat/window payloadを持つ入力観測としてだけ追加し、domain witnessには流用しない。
拒否系fixtureと独立reviewを経ても、通常worldのhost admission／capture ACK／instrument
producer欠落は残る。補正前のcollectorはX11 XTest固定であり、portal輸送の実証だけでは正式driverにならない。
本補正ではoffline production planをportal専用v2へ更新し、collectorを固定同意journal・
所有window・focus・raw ACKのportal輸送へ接続する。grantをaction logへ残して独立検査するが、
host admission、actual-window capture/ACK、外部GPU/native/RSS/application handle計測器は
別途必要であり、このコード補正だけでは正式campaignを開始できない。
同一Tank preview identity `edc65d14fe5f97b8953ccdd1487b62b1fcb51d9112915968d5994c2edf29f9cb` の
近景・通常・遠景・最大ズームアウト4枚を独立read-only美術再評価した。近景の槽底・鉄帯の大きな断絶は
解消し、通常倍率でEmpty／Partial／Fullを区別できるため、追加の形状変更は現時点では必須でない。
ただし最大ズームアウトではPartial／Fullの判別が不確実で、黒背景の静止画像は通常地形との分離を
証明しない。正式art採用は `changes_requested` とし、実操作による既存情報UIの確認経路、異なる明暗・
壁／Soul／隣接設備との重なり、低品質表示、完成と配置previewの接地・縮尺、実状態遷移を同identityの
追加証拠で評価する。技術画像の再評価を数値freeze、lifecycle、release承認へ転用しない。
現行source `0b4738dd8f831bbdfdc981fb140903ddfd89f870` のRestArea通常倍率
`tak14-restarea-normal-0b4738dd-20261005-v2` は、同じ未承認ArtPreview identity
`364c1ad44187d074fe4bc7251f3a919a40659cf49da59187662ce733e7179860` を
Intel/Vulkan実ウィンドウで撮影し、独立verifierのtechnical pass、用途登録、finalize、
storage checkを完了。独立read-only画像比較では入口開口と支柱の形は読めるが、
屋根が板または皮革に見え、縫い目・継ぎ布・皺と木支柱との素材差が通常倍率で不足。
正式art採用は `changes_requested`。配置geometry・利用状態・保存契約を変えず、
布atlasの色面と少数の世界座標基準の継ぎ目・皺だけを新候補へ補正する。
この撮影は静止比較のみで、利用者/Dreamの実遷移、異なる照明・背景、最遠・重なり、
低品質、preview、numeric freeze、ordinary lifecycleやreleaseを証明しない。
原画補正 `77e5bd2870261c6489bb92cc983024b84913fb34` は布色を木材から分離し、
世界座標基準の継ぎ布・2本の横断継ぎ目・少数の皺／縫い目を追加。geometry、入口、利用状態、
SoulSpaには触れていない。変更別contracts/tooling CI、回帰7件、Help No impact、
独立read-only実装レビューはpass。新原画 `tak14-restarea-authoring-77e5bd28-20261005-v1` と
runtime package `tak14-restarea-runtime-77e5bd28-20261005-v1` は独立byte/geometry verify、
用途保持、finalize済み。新manifest `db0f535611de1140cc83dd82f08128052da82e5d8dd2642d77f3075d5fae2522`
は依然 `art_preview` で正式authorityなし。同identityの実画面通常倍率
`tak14-restarea-normal-77e5bd28-20261005-v1`、近景
`tak14-restarea-near-77e5bd28-20261005-v1` はIntel/Vulkan所有windowで独立technical verify pass、
用途保持・finalize・storage check済み。独立read-only画像比較では旧候補の茶色板／皮革感は解消し、
淡い補修布、暗い木支柱、継ぎ目と入口が両倍率で読めるため、この2 viewの追加paint補正は必須でない。
正式art採用は未実施。情報UI、明暗／重なり／低品質、配置preview、
実際の利用者・Dream状態遷移、数値freeze、ordinary lifecycle／性能、promotion/install/rollbackは未受入。
同manifestの遠景 `tak14-restarea-far-35a35f5a-20261005-v1` と最大ズームアウト
`tak14-restarea-overview-35a35f5a-20261005-v1` も現行clean headのIntel/Vulkan実windowで
独立technical verify pass、用途保持、finalize、storage check済み。独立画像評価では遠景で淡いテント屋根・
入口・支柱が隣の四角い設備と区別でき、追加paint補正は不要。一方、最大ズームアウトでは
種類と入口、支柱、利用状態を初見で確実に読むには小さすぎる。屋根を誇張する前に、
通常worldの実操作で既存選択・情報UIが建物種別とResting／Dream状態を示す代替経路を確認する。
黒いviewport余白の原因や通常起動での挙動、明暗・重なり・低品質・配置preview・実利用状態は
これらの静止画像からは未検証。正式art/numeric採否とreleaseは保留する。
RestAreaの通常world経路は既存分類で `Temporary`、配置域はSiteではなくYard。
通常生成worldのSiteから探した初回テストは配置成立せず、分類とproduction validationを照合して
Yardへ修正した。terrainやzoneを受入用に書き換えず、実際のYard内で既存判定が通る
2×2候補だけを受動観測するUI caseを追加中。プレイヤーの配置、予約、保存復元、取消を
実ウィンドウ入力で確認する工程であり、完成建物のResting/Dream、最大ズームアウトの
情報パネル、正式asset表示、性能やreleaseの代用にはしない。
初回バッチ `tak14-restarea-planning-2aa4edef-20261005-v1` は通常worldの配置予約、
保存・再読込を実操作で観測したが、再読込直後に可変top barの OpenEntities 位置が
pointer移動中に変わり、hover guardで終了した。exit 1／invalidとして封印・診断保持済み。
取消の実入力と独立verifyは未完了であり、部分観測を正式lifecycle受入へ昇格しない。
UI driverは移動だけを最大3回再取得し、実際にhoverを観測するまでpressしない補正を実施。
修正後のsource `3038dee7a347bf37d49f2d6a9ac26273c195890a` でfresh batch
`tak14-restarea-planning-3038dee7-20261005-v2` を起動し、実ウィンドウの5 checkpoint
（空き→配置→保存→読込→取消）を取得。Yardの2×2 Blueprint予約、保存後の同一対象復元、
取消後のowner解放を固定独立verifierがpassし、用途保持・finalize・storage check済み。
対象は完成前のBlueprintであり、Resting／Dream・数値／art採否・正式導入・性能を証明しない。
同一MudMixer `art_preview` manifest `4007d3fb4ebb93b9c6a4ce73aec3a9ac1e53736f8e19fc71211dd74ca45b094a` の
近景 `tak14-mud-mixer-near-86a0953a-20261005-v1` と遠景
`tak14-mud-mixer-far-86a0953a-20261005-v1` は、現行clean subjectのIntel/Vulkan所有windowで
独立technical verify pass、用途保持、finalize、storage check済み。独立read-only画像評価は
回転部の十字輪郭とIdle／Refiningの静止色差を認める一方、固定槽が緑の平箱に見え、
木・鉄・泥、槽の内側と架台の区別が近景でも不足すると指摘した。正式art判断は
`changes_requested`。数値geometryやゲーム状態を変更せず、槽内面・木の縁と外板・鉄架台・
回転部をalbedoの大きな色面で分け、fresh候補を同一条件の実画像で再比較する。
遠景の小さな内部線は承認根拠にせず、通常worldの実精製時だけ回転する状態、
明暗・背景・低品質・配置preview、numeric freeze、ordinary lifecycle／性能、正式releaseは未受入。
`cf265dd2a12315be818bb553da86ccf4de0e0afa` ではMudMixer既存body/rotorの部材別albedoを
変更し、数値geometryと状態ruleは不変。変更別contracts/tooling CI、Blender回帰204件、
Help No impact、独立read-onlyコードレビューはpass。fresh原画
`tak14-mud-mixer-authoring-cf265dd2-20261005-v1` とruntime package
`tak14-mud-mixer-runtime-cf265dd2-20261005-v1` は独立byte/UV/geometry verify、
用途保持、finalize済み。manifest `5fdf119b668270d6d898fce5613aa2e9e9e4b58f23b293060df69356253f0d28`
は依然 `art_preview`。同identityの通常倍率
`tak14-mud-mixer-normal-cf265dd2-20261005-v1` と近景
`tak14-mud-mixer-near-cf265dd2-20261005-v1` はIntel/Vulkan所有windowで独立technical verify、
用途保持、finalize、storage check済み。独立画像比較では旧「緑の平箱」は解消し、木枠・暗い内面・
十字攪拌部、近景の鉄支持部を区別できる。一方、暗い底面は泥か木床かを判読できない。
現行数値原本に泥の独立した面・状態がなく、`docs/building-art-direction.md` の容器とは別の
水・泥表面契約を満たしたとは言えない。泥の状態・面の採否を原本側で決めるまで
正式artは `changes_requested` とし、色面だけから泥の存在や実精製中の回転を推定しない。
最大zoom-out、明暗／背景／低品質、配置preview、実精製と保存復元、数値freeze、
ordinary lifecycle／性能、promotion/install/rollbackも未受入。
`281a6fc50111c79963348558854fd3780240f9ee` の2本の泥汚れ仮説は、
fresh原画 `tak14-mud-mixer-authoring-281a6fc5-20261005-v1`、runtime候補
`tak14-mud-mixer-runtime-281a6fc5-20261005-v1`、通常倍率actual-window
`tak14-mud-mixer-normal-281a6fc5-20261005-v1` で検証した。byte/UV/geometryと
実機technical verifierはpassしたが、前候補との独立画像比較では泥と木床の判読性が
実質改善しなかった。この描線仮説は不採用とし、同種の描線・明度調整を重ねない。
制作器は前の部材別albedoへ戻し、候補のtechnical passを正式art/数値承認へ昇格しない。
独立泥面が必要なら数値原本と状態契約の別途改訂・受入が必要。
次の比較用として旧v1原本を保全し、`building-mud-mixer-v2.geometry.json` と
`building-m2-v2.numeric-candidate.json` に固定の薄い泥残留面をbody内へ加えた。
槽底上面Y=12と攪拌部の全角度最下端Y=14の間に泥面Y=12.25〜12.75を置き、
床と0.25wu、攪拌部と1.25wu、内壁X/Zと各2wuの離隔を検査する。
body/rotorの2role、外形・接地点・preview anchor、回転・Refining条件、保存／gameplayは不変。
これは泥量や新しい充填状態ではなく固定の残留面という比較仮説であり、
Pythonの算術・拒否系検査だけでは数値freezeにならない。M2制作器に明示`--revision v2`を
加え、旧v1は既定のまま固定し、新候補の原本・GLB・UV・runtime previewを別identityで
生成できるようにした。独立実装レビューでaccessorだけでは泥面未描画を拒否できないと分かったため、
実際のindexごとにface所属・有向周縁・逆向き一対の内部対角線を照合し、
泥面のindex削除／誤接続／逆巻きを拒否してから
離隔値を出す。`fee69559e958fe36dd22c0cb7a88e307f1ed8cf0` のclean sourceから
原画 `tak14-mud-mixer-v2-authoring-fee69559-20261005-v1`、preview package
`tak14-mud-mixer-v2-runtime-fee69559-20261005-v1` を別identityとして制作し、
index付きGLB・UV・離隔・byteと独立verifierがpassした。bodyは132三角形、rotorは36三角形、
manifest `43b1fe0f75b7176870f9d72663a5c75119d0e81989ba49be0ab96a73697177f1` は
`art_preview`、`numeric_freeze=false`、`art_approved=false`、`production_candidate=false`。
同identityの通常倍率 `tak14-mud-mixer-v2-normal-fee69559-20261005-v1` と近景
`tak14-mud-mixer-v2-near-fee69559-20261005-v1` はIntel/Vulkan所有windowで
独立technical verifier pass、用途保持・finalize・storage check済み。v1との独立画像比較では
槽底と別の連続面は両倍率で前より読めるが、木枠と近い黄土色の平面で「泥」の素材判読は
まだ不確実。近景では紫の状態表示が右2標本に重なるため、泥色の観察は左2標本を重視する。
この静止技術比較を独立art/numeric承認に読み替えない。次の色面仮説は、木材とは異なる
低彩度の濁色と広い不均一な流れを、独立mud面だけに与えること。微細描線を増やすのでなく
通常倍率・近景の同条件画像で素材の分離を一度検証する。実精製中の動作、通常world状態、
full lifecycle／性能、正式releaseは未受入。
`bda34aadc43815f531e5b4a55e5da48bb98c1b31` では独立mud面だけを低彩度の濁色と
広い不均一なflowへ補正。数値geometry、rotor、旧v1、通常gameplayは不変。
fresh原画 `tak14-mud-mixer-v2-flow-bda34aad-20261005-v1` とruntime package
`tak14-mud-mixer-v2-flow-runtime-bda34aad-20261005-v1` を独立byte/UV/index verify後に保持・finalizeした。
manifest `faa969b7af23d93644d7e58cf86c000e35c3113d2c89ecd7f7105d0958fcb876` は
`art_preview` のままで、art/numeric/production authorityはない。
同identityの通常・近景・遠景 `tak14-mud-mixer-v2-flow-{normal,near,far}-bda34aad-20261005-v1` は
Intel/Vulkan所有1280×720 windowで独立technical verifier pass、用途保持・finalize済み。
独立画像レビューでは通常・近景の木製槽底との混同は解消し、遠景scale2でも木枠・泥面・
十字攪拌部と隣接種を区別できた。追加paint調整はここで止める。紫色表示は静止状態差だけで
実精製の証拠ではない。最大scale5、低品質、明暗・重なり、配置preview、実状態遷移、
正式art/numeric採否、ordinary lifecycle／性能、release/install/rollbackは未受入。
次の画像検証のため `building_art_acceptance.py plan --quality high|medium|low` を追加する。
指定argvだけでなく実window CSVの開始／終了品質、Scene解像度・scale、所有window寸法、
Vulkan/X11/adapterを既存window verifierで検査する。旧planはhighとして扱う。
静止性能参照はhigh限定を維持し、lower qualityは明示building-art sessionだけで許可する。
品質指定は検証子processだけの制御で、通常起動やプレイヤーの設定・Help契約を変更しない。
追加経路の実装レビューと現subjectの低品質画像取得までは、低品質受入を完了扱いにしない。
`e234fbe0aa2b80a98d1baacd59d0838b30a4459c` の低品質normalとhigh最大scale5を
同manifestで撮影。`tak14-mud-mixer-v2-flow-{low,overview}-20261005-v1` は独立technical
verifier pass、具体用途保持・finalize済み。low画像SHA
`81748907ad80948d7fe162c93fd832dc5bfea51bb29722a66a9559d242b5201b`、overview画像SHA
`25fffea7babf76914a3f4d425179e5bac81c297a7f4a5f72e78498ca01709e94`。
独立画像レビューではlow normalの木枠／泥／rotor／紫の静止差と隣接種の識別は維持され、
追加paint調整は不要。ただしoverview scale5は素材・種・状態を確実に読めず、画像可読性は未達。
細かなpaint追加で解決したとせず、既存選択・情報UIによる種類・状態の確認を実操作で検証する。
いずれも通常worldの精製・停止・再開、明暗・重なり、正式art/numeric採否やreleaseの証拠ではない。
次の状態観測に向け、`building_m2_acceptance.py plan --mixer-revision v2` を追加する。
Tankはv1、MudMixerは明示revisionと数値原本hashをkind単位で固定し、manifestと実provenanceの
一致をverifyする。旧field欠落planはv1のみと解釈し、hashからv2を推測しない。
top-levelの旧`numeric_candidate_sha256`は共通v1基点で、v2の根拠ではない。検証結果の
`numeric_candidates`を各kindの根拠とし、inspectも同じrevision拒否規則を使う。
unknown revision、Tank v2、v2 hash欠落・v1への取り違え、manifest／provenance不一致を拒否する。
これは未承認feedback候補の状態観測準備だけで、数値freezeや正式art/releaseを付与しない。
補正`6836e916f49dcfefc84d403208cc317395c61acd`のfresh refining batch
`tak14-mud-mixer-v2-refining-6836e916-20261005-v1` はRust producerのv1固定hashで拒否されinvalid。
ログの `M2 probe descriptor does not bind the exact numeric candidate` を診断保持しfinalizeした。
同batchの再起動・結果の読み替えはしない。producerへ明示`HW_M2_ACCEPTANCE_REVISION`を渡し、
Tank v1/MudMixer v1/v2だけを許可、選択hashとdescriptorを照合する補正を行う。
raw traceにも同数値hashを残し、v2 verifierはhash欠落・v1取り違えを拒否する。
通常起動や精製システムは不変。修正後のfresh subjectと独立実機verifyまで精製観測は未達。
producer補正`2fd6bb381cafb525898def9393ad2e43d600b851`のfresh batch
`tak14-mud-mixer-v2-refining-2fd6bb38-20261005-v1` は160秒観測後、renderer/GPU identity
欠損でinvalid。rawはv2 hash・Idle/Active/pausedを記録したが、actual-window logにAdapterInfoがなく
正式観測passにはしない。旧失敗と別の診断保持・finalizeを行った。
M2 runnerの子processへ既存nativeと同じ`RUST_LOG=info,wgpu=error`を明示し、rendererの証拠を
採取する補正を行う。renderer欠損／不一致の拒否は維持する。inventory単独もrawの数値hashを
照合し、v2 traceのhash欠落やv1取り違えを拒否する。修正後のfresh実機verifyは未達。
補正`5468dbbc928f48993986b362e7929021d6e553c0`のfresh
`tak14-mud-mixer-v2-refining-5468dbbc-20261005-v1` と
`tak14-tank-water-5468dbbc-20261005-v1` はIntel Arc/Vulkan所有1280×720 windowで
各160秒観測、登録済み独立verifier pass、比較用途保持・finalize・primary storage check pass。
MudMixer rawは1288 samples、同一v2数値hash・ArtPreview identityでIdle/Active、17 paused samples、
rotor姿勢変化と再開を記録。独立画像レビューでは木枠・泥・rotorと紫のActive表示差が読め、
追加paint欠陥は見当たらない。一方、owner itemはrawの33 samplesにあり、画像では独立した形が
明瞭でなく、item/rotor重なりの可読性は未達。Idle画像にはConstruction Complete表示の重なりがある。
checkpoint 1の撮影前raw index39はpaused=falseだが画像は既に一時停止表示で、checkpoint 2も
停止画像である。現在の撮影前sample bindingは画像の撮影時点を固定しないため、画像だけで
停止前後の運動を証明しない。撮影前後のraw区間と描画時点の整合を確認する補正が必要。
Tank rawは955 samples、同一v1数値hash・ArtPreview identity、同一ownerでEmpty/Partial/Fullを記録。
通常倍率のcheckpoint画像では空の槽底、暗めの低水面、明るい満水面が見えるが、独立画像採否と
撮影時点は別取得である。独立画像レビューでも3状態と木枠／水面の分離を確認し、追加paint修正は
不要と判断した。raw index3/39/75は同じownerのEmpty/Partial/Fullで、水面Hidden→高さ12→高さ22と
画像変化が整合するが、完全な同時刻証拠ではない。EmptyのConstruction Complete表示は上縁・内面を
遮り、その部分の美術評価は未達。水のfixture投入やMudMixerのfixture task/state操作は通常入力の証拠ではない。
両passはfeedback技術観測だけで、正式art/numeric freeze、ordinary-input給排水・精製lifecycle、
全leg/性能、promotion/install/rollbackや通常起動の本設導入は引き続き未受入。
2026-10-12再開: 撮影前sampleだけへの結合を修正する。M2 runnerは撮影後のraw prefixを再取得し、
前後でsampleが進み、全区間でstate/pause/camera/consumer表示・identity・fixture owner・root owner・
world epochが不変な画像だけをcheckpointと最終画像へ採用する。途中で変わって戻る区間も拒否する。
不安定なcheckpoint画像は理由付きで診断保持し、同名で上書きせず最大48試行／24採用画像へ制限する。
最終画像も最大8試行の別pathへ撮り、拒否画像と理由を上書きせず保持する。
session/prefix改変は再撮影で隠さず失敗させる。新verifierは撮影後indexがない旧画像を受入証拠として拒否する。
これはrawの安定区間への結合であり、renderer frameと完全に原子的な撮影や通常入力の証拠ではない。
旧passはその時点の限定結果として維持する。fresh実機観測と独立画像評価を終えるまで本補正の実機結果は未達。
Help実経路: 子processの撮影・raw検証のみの変更で、通常起動、player input/UI、gameplay、保存、Helpは不変（No impact）。
撮影補正`9b1d518d5d9c07447fe73e777ddf2863478b3bec`のfresh 30秒batch
`tak14-mixer-capture-interval-9b1d518d-20261012-v1`は`native bounded stop/capture failed`でinvalid。
拒否画像と前後indexを保全し、同sampleの最終撮影213/223/232を拒否したことを確認、診断保持・finalizeした。
rawの公開間隔より撮影が短いため、撮影後のstrictly newer sampleを最大2秒だけ待つ補正を追加する。
timeoutでは不採用を維持し、prefix/session改変、区間内の状態変化やowner交替は引き続き拒否する。
待機補正`4e2a39635cfadee28a3a7fe4acf6d38ffd7e2c91`のfresh 30秒batch
`tak14-mixer-capture-interval-4e2a3963-20261012-v1`は最終画像を採れたが、unpaused Active画像が
状態切替を跨ぎ`required leg state/pause client captures missing`でinvalid。診断保持・finalize済み。
observerは0.1秒ごとに採るsampleを1秒おきにしか公開していなかったため、検証専用M2 observerで
各due sampleをそのまま公開する。サンプル／entity cap、失敗時のfail-closed、通常起動の無効化、
performance_evidence=falseを維持し、精製ルールやfixtureの材料・task・pause時刻は変えない。
公開間隔補正`85c7574f14d8da720758fe31fc6b95c0ade3c43e`のMudMixer/Tank fresh batchは
技術verifier pass・seal/finalize済み。ただし独立画像レビューでMudMixer Idle raw4..5の画像は
UIだけで中央viewportがclear色のままと判明し、Idleの視覚証拠として不採用とした。
paused Active46..49とunpaused Active62..65は木枠・紫表示・rotorが見え、rawの停止／進行と限定整合する。
中央160x160が単色の画像を採用・verify双方で拒否し、元画像・理由を保持して既存上限内で再撮影する。
この最小blank検査は対象の描画完了や美術採否を保証しないため、fresh画像の独立比較は必須のまま。
最終補正`0619cd65a741e64e082a049fbd10d31ef62d90fd`のfresh 30秒batch
`tak14-mixer-capture-ready-0619cd65-20261012-v1`と
`tak14-tank-capture-ready-0619cd65-20261012-v1`は登録済み独立verifier pass、seal/finalize済み。
同source `0d20a4d2d8fb744832076ff8a292568eb4638a962b6d1cb5f010432a4591fed8`、
同codec `9d846a9b7df7c804dfaf29eb9b01d0960e4d87ca836af0be9bd66872b0b60e84`で逐次実行した。
独立画像レビューでもMudMixer Idle5..6、unpaused Active37..38、paused Active43..45の3画像に
terrainと対象を確認し、同owner2187v0／epoch0と停止・進行の限定整合を確認した。
Tank Empty6..8／Partial39..41／Full78..80も対象が見え、同owner2186v0／epoch0で
水面Hidden→高さ12→高さ22と画像変化が整合する。startup blankはなく、撮影補正の限定受入を閉じた。
ただしConstruction Complete表示の上縁遮蔽、MudMixer紫表示下の泥とitem重なりの判読、
通常入力lifecycle、正式art/numeric freeze・性能・promotion/install/rollback・通常起動導入は未受入。
修正の独立コードレビュー、Python tooling 437件／asset tooling208件、変更別CI contracts/tooling、
observer変更時のworkspace test・Clippy・dev.py checkは成功。HelpはNo impact。
診断が終了した本修正の旧4jobは結果・制限を本節へ集約して整理し、最新2jobだけをM2候補画像比較用途で保持する。
出力root: primary `target/native-acceptance/nonbridge-restarea-authoring-20261004-v1`。
owner: main / consumer: M3候補の独立採否・現行subject導入。次工程: 原本・画像を比較しruntime候補を生成。
release_when: 採用原本のasset正本への集約と候補フィードバック終了後。旧batchのpassを新subjectへ流用しない。
検証: `dev.py check`、Clippy、workspace test、`dev.py ci check --base <基点full SHA> --mode auto`、
必要なactual-window/native性能受入、Help実レビュー、`git diff --check`、storage check。

## 8. ロールバック

kind単位の正式rollback記録で復旧し、loader失敗時の従来fallbackを維持する。

## 9. AI引継ぎ / Definition of Done

現在はM1。旧統括・Run・終了済み担当の再配車は禁止。採否は独立レビュー、製品編集・検証・Gitはmain所有。
参照: 原移行計画、building-art-direction.md、building-asset-sets.md、assets_workflow.md、validation-storage-workflow.md。

- [ ] 9種の通常起動表示と全受入を確認。
- [ ] 正式identity・採否・release/rollback・恒久文書を同期。
- [ ] Help/native/変更別検証/独立review/storageを完了し、追加差分なし。
- [ ] 不要jobを整理し、保持用途を引継いで計画をarchiveまたは撤去。

## 10. 更新履歴

2026-10-04: 利用者の「橋以外で最後まで」指示により先行9種の実作業を再開。storage pass、専用branch作成。

2026-10-12継続: 9種の正式導入を完了対象として残す。独立候補監査は完成形の部分評価のみ可で、
全候補の正式art承認は未達。通常worldのArchitect開閉を実入力・実UI状態・所有画像で収集する
固定scenarioを追加する。ゲームを変更せず、MenuState／PlayMode／TaskModeをprofiling observerへ読取り追加する。
defaultのF12 probeは維持し、別profileだけB→EscapeでHidden→Architect→Hiddenを検査する。
独立コードレビュー、fresh実機／画像review、Help No impact・変更別CI・storageを通すまで新経路は未受入。
これはcatalog開閉の収集基盤であり、正式identity、配置・完成・全lifecycle／性能、releaseの代用ではない。

2026-10-12実機: `tak14-architect-input-da3da1c4-20261012-v1`はinvalidとして正式seal/finalize。
B/Escapeと同epochのHidden→Architect→Hiddenは実rawで確認したが、TaskMode単体resourceの
読取りはnull、catalog画像にはpanelが未表示だった。失敗画像/rawを具体的診断用途で保持する。
通常操作が参照するTaskContextへ観測先を補正し、panel表示・layoutの2連続sample成立を撮影前に
要求する。失敗をpassへ読み替えず、修正後のfresh sourceで再検証する。9種正式導入は未完。

`tak14-architect-input-50ae73ff-20261012-v1`はexit0でも独立画像reviewでchanges_requested。
開いたpanelは表示されたが、raw Hidden直後のafter画像にpanelが残ったためinvalidを正式記録。
閉鎖側にも2連続Hidden／layout非表示と撮影後fresh sampleまでの安定区間を要求する。
Rustの観測先補正は成立しており、この追加補正はPython撮影driverと拒否系testだけで行う。

`tak14-architect-input-a47fa8ab-20261012-v1`は開閉の画像が成立したがbeforeが全面黒だった。
独立画像reviewで3画像全体はchanges_requested、invalidとしてseal/finalizeして診断保持。
既存M2のnonblank検査を開始・開・閉の全画像へ適用し、開始はfresh Hidden区間を拘束する。
blankだけを8秒・16回以内で再撮影し、拒否画像・理由を保持する。別batchの再launchではない。

`tak14-architect-input-32b0bd72-20261012-v1`はfresh sourceで3画像がnonblank、通常worldの
開始／Architect表示／閉鎖が実画像と同epochのrawに整合し、独立画像review・登録verifierで
限定technical pass、seal/finalize済み。固定B/Escapeは実入力であり、建物選択・配置は未実行。
このjobの画像/rawだけを次の収集producerの対照として保持し、primary sourceやbinaryは凍結しない。
次はprofiling observerに実際の建設cardの可視・clip済みphysical rectとBuildContextを
read-only追加する。既存UiIntentと実layoutから対象を解決し、hidden ancestor／未layout／
重複・過大集合を拒否する。座標の推測、worldや施工結果のseed、Bridge対象化はしない。
その後、所有windowの実pointer操作と選択後のauthoritative stateを同区間で拘束する。
選択観測の準備だけではordinary placement/lifecycleや正式導入を完了扱いにしない。
独立reviewで変形UIのrect不整合を指摘され、finite translation＋identity matrixだけを受理する
最小補正と拒否testを追加。原MouseButtonInputのbutton/state/windowも状態由来入力と分離する。
原因解消済みの旧3診断jobは正式consumer解除後に生成物だけを整理し、du 11,964,416 bytes減。
invalid metadata・コード・原本・現行比較証拠は保全し、primary storage checkはpass。
観測追加`b94e88a3`は独立read-only review、profiling観測15 test、変更別CIのcontract/tooling/Rust、
通常・profiling Clippy警告0、Help No impactとdev.py checkを通過した。
次の選択feedbackは既存debug cacheを利用する別profileとし、正規validationの所有／source／
plan／driver／binary hashを維持する。9種の実cardだけを固定集合として扱い、pointerの製品ACK、
原MouseButtonInputのprimary window、選択後modeを検査する。通常worldをseedせず、正式採否を付与しない。
`tak14-equipment-tank-feedback-2be539bb-20261012-v1`はクリック前にcard inventory空でexit1、
invalid seal/finalize済み。実Architect PNGには12 cardが表示された。Bevy 0.19の
`ComputedNode::resolve_clip_rect`と実`Overflow::scroll_y()`から、Visible横軸の正規±∞を
観測器が拒否する前提誤りを確認。外向き無限clipを有限node／viewportへ交差させ、
NaN・内向き無限・反転・空・変形を引き続き拒否する。値調整やUI変更では解決しない。

`c21b916a`のfresh feedbackでは9 cardの観測が成立した。一方、移動前はゲーム外の
X11 cursor／製品cursor nullで、相対motion要求[663,563]に対し製品cursor[596,575]。
クリック前にtimeoutでinvalid、正式seal/finalizeして座標経路の診断用途で保持する。
経験的なoffset補正や到達条件の緩和は行わない。既存pointer／keyboard consentで
RemoteDesktopのConnectToEISを一度だけ使う限定equipment profileを追加する。
libei senderは単一seat・一意なactive device・単一virtual region・同monitorのphysical
extent/scaleだけを受理し、pause/removal/monitor変更は拒否する。EIS接続後のNotify混用、
入力再送、未知regionの推定は禁止。原inputと製品の厳密なcursor ACK・選択state、
独立画像review・登録verifierは維持する。実機受入前であり9種のreleaseは未完。
初回独立コードreviewはmonitor変更時の保持入力cleanupを指摘。releaseだけを通常guardから
分離し、原deviceの押下記録を解放してcloseする補正とpause／revocation拒否系testを追加する。
`f16d3e87`のfresh EIS実機は接続・device取得後、physical region一致guardで入力前拒否。
input_receiptsは空、invalid seal/finalize・診断保持済み。拒否にactual rect/scaleとexpected値を
含めるだけの観測補正を追加し、同じ拒否条件を維持して次のfresh実測で単位を確定する。
`8709e085`のfresh入力前実測はEIS logical rect=(0,0,1440,900)、scale=2、X11 physical
root=(2880,1800)を記録してinvalid精算。一次libei APIのlogical pixel契約に従い、
declared scaleとexact logical extentが一致する時だけphysical targetをscaleで割る。
任意倍率の逆算や到達ACKの緩和はしない。半logical pixelも丸めず、ゲーム側のexact
physical cursor ACKを維持する。独立review／fresh実機前で9種の正式導入は未完。

`8b5db4f6`のfresh EIS feedbackはphysical cursor [663,563]、原Left入力、
Tank選択、Escape解除をrawで確認した。closed capture直前の2 sampleのうち
先行sampleがHidden/BuildingPlaceだったためstrict verifierが拒否し、invalid精算済み。
待機側だけを2連続Normal／task None／選択解除済みへ補正する。撮影interval verifierは
変更せず、別subjectのfresh撮影と独立reviewで確認する。正式アート／releaseは未承認。

`ffded26d`では選択・解除rawとclosed intervalは成功したが、独立画像reviewは
catalog.pngのpanel未表示を拒否。exit0を採用せずinvalid精算して診断保持した。
layout readyとOS撮影の表示時点は同一とは限らない。限定equipment profileの撮影に
theme正本のdefault card色とraw rectによるpanel存在確認を追加し、最大8回の撮影だけを
試す。hover／tooltipを除く7 card以上を要求し、失敗画像を保全する。入力再送なし。
これはpanel存在の補助gateであり、文字／アート採否や独立画像reviewの代用ではない。

`b9ac0823`のfresh Tank実機はカタログ実表示／Tank配置説明／Escape閉鎖を独立画像reviewで
確認し、原MouseButtonInput・同window/epoch・選択stateと登録verifierもpass。限定UI選択技術証拠。
選択時の紫cardは移動後cursor下のBonePile hoverであり、Tank採否やselection色の証拠にしない。
原因解消済み6診断jobはconsumer解除・使用process不在・原本不在を確認し生成物だけ削除。
du合計24,985,600 bytes減、台帳metadataと採用コード・原画・最新証拠は保全、storage pass。

次のordinary placement producerはUI処理が実際に得た成功／拒否／Tank companion待機を区別する。
profiling観測sessionがある時だけbounded receipt queueを有効化し、実owner/footprint、
grid/kind、原primary windowとreal timeを記録する。Tank親の仮生成を成功と扱わず、
companion成功時だけ親子の同時commit、失敗時はrollback済み拒否を記録する。
SoulSpaの4tile ownerも実spawn結果に拘束する。ordinary input rawとは別のdomain結果として
結合し、state deltaから成功を推測しない。ゲーム配置条件／spawn順／saveを変えず、
材料配送・cancel/refund・move・save bytesやhost metricsまで受入済みとしない。

次の限定feedbackは`architect-select-place-cancel --kind Tank`。原UI選択後の単発world入力と
Rejected／CompanionPending、Blueprint不変、Escape解除だけを確認する。committed ownerや
正式lifecycle成功を許可しない。producer／入力probe hashとfresh subjectを固定し、
実機raw／画像の独立reviewと登録verifierを通す。正式9種導入の未完条件は維持する。

`5c48a6de`の実機は原Left入力と同frame/timeのTank／NotInYard拒否、Blueprint不変、
Escape解除を実見。ただしbefore画像の地形未描画を独立reviewが拒否し、batchはinvalid精算。
非blankだけではactor／光で偽陽性になるため、normal startupの固定map内viewportに
正本ClearColorが過半残る画像の補助拒否を追加する。撮影だけ有界で再確認し原失敗を保持する。
storage subject fingerprintとnative plan.sourceは別digest体系であり同一hashと記録しない。

`1b529faa`のfresh実機は未描画startup原画像を拒否・保全し、地形描画後の開始から
カタログ／Tank選択／NotInYard拒否／Escapeまで全5画像とrawを独立review、登録verifierで
限定pass精算しstorage pass。正式配置・親子commit・9種releaseは未受入。
成功経路は既存Yardの位置と現カメラprojectionを実観測して通常操作で到達する。
`placement_context`は最大32の実Yard boundsと単一active MainCamera projectionを読み取るだけで、
新Yard・ownerのseed、カメラ強制移動、入力・受入authorityを追加しない。missing projectionはnull。

既存Yardへの次の限定recipeは`architect-select-place-yard-cancel --kind Tank`。
実target window／logical viewport／WorldMap grid対応を追加し、必要なcamera移動を原WASDで
各軸1回・4秒以内、解除まで観測する。Yard trial点はmap生成を変えず実投影と同一contextに拘束。
Yard内の最初のTank clickはCompanionPendingだけを検査し、Blueprint commitはしない。
正常配置の次の親子commit・占有・材料配送・完成・move/save/cleanupと性能・正式releaseは残す。

`7fc5697b`のfresh実機では通常D→W入力で既存Yardへ到達し、grid `[73,60]`で原Leftと
同sample37／time `7.633592636`／window `69v0`／epoch0のCompanionPendingを取得。
全6owned画像と拒否startup原画像・rawを独立reviewし、登録verifierで限定pass封印／finalize、
primary storage pass。Blueprintは空のままであり、正式導入の承認ではない。
次の`architect-select-place-companion-cancel`は実pending stateとBucketStorage root／map占有を
読み取り、通常2回目のクリックによる親Blueprint・2childの計画確定だけを検査する。
既存world／材料／actor／建設結果のseedはせず、完成・性能・art／numeric・releaseは未達のまま保持する。
