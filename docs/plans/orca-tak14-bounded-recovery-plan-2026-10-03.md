# TAK-14統括の期限付き復旧計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `orca-tak14-bounded-recovery-plan-2026-10-03` |
| ステータス | Superseded — M1不合格・未適用の履歴を保持。通常Orca復帰・独自統括撤去へ方針変更 |
| 作成日 / 最終更新日 | 2026-10-03 |
| 作成者 / 実装責任者 | Codex main agent |
| 関連提案 | N/A |
| 関連Issue | 既存TAK-14、request `54cf1dd7-7485-5f49-a8e5-827773355096` |
| 関連計画 | [基盤計画・既存証拠](orca-system-hardening-plan-2026-09-29.md) |
| 工数計測開始 T0 | 2026-10-03 20:25:35 Asia/Ho_Chi_Minh（13:25:35 UTC） |
| **絶対停止期限** | **2026-10-03 22:25:35 Asia/Ho_Chi_Minh（15:25:35 UTC）** |

## 1. 目的

2026-10-03の利用者方針変更により、現行は[通常Orca復帰・外付け化計画](orca-normalization-and-coordinator-extraction-plan-2026-10-03.md)。
以下の復旧経路と120分上限は当時の履歴であり、現行の実行指示ではない。本計画を再開しない。

旧runtimeへ固定された統括を、既存の保存会話・pane・Runを維持して正常起動へ戻す。
成功は同じ会話の新turnと、そのturn内の正規workflowによる具体的次工程の操作で判定する。
Orca基盤全体の完成、TAK-14全体の受入、ゲームの実装完了は本計画の成功条件ではない。

本体配備後にruntimeが変わり、live EnabledLaunchの旧runtime固定によってDriverが失敗した。
mainは復旧部品を小分けに検証し、再開までの接続と総量評価を後回しにした。
本計画では先に全経路と変更量を確定し、部品の承認を復旧進捗へ読み替えない。

## 2. スコープ

### 対象（In Scope）

- mainによるcontrol helperの同目的修正と、既存固定read-only reviewerによるexact source review。
- 現所有・source・正式settlement・ACK・事前pidfd観測に基づく正式post-provider retirement。
- 同pane/PTY/incarnation、Run id/generation2を保持するappend-only succession、guard付きregistry更新。
- 実新launcherのFD/birth/ancestryに基づくenableと、既存`ui.launch`への接続。
- 関連tooling検証、Help実経路の影響判断、primary storage確認、実際の再開確認。

### 非対象（Out of Scope）

- Orca本体の追加機能・追加配備・予防的再起動、一般化した新復旧framework。
- 監視側によるゲーム編集/build/test/native受入。製品工程は既存統括の所有を維持する。
- 新Run、push/merge/外部公開、新たな課題取消、原本削除、認証情報出力。
- 生存中の担当/検証の中断、無条件Enter、本文再送、unknown effectの再実行、guard回避。
- 新規実装欠陥がない終了済み担当への安易な再配車。

## 3. 現状とギャップ

以下は13:22 UTC以前の観測であり、適用前に最小限再照合する。履歴を現在稼働と扱わない。

| 対象 | 観測・保全する識別子 |
| --- | --- |
| runtime / app | `2d5830cc-61d0-4f3f-9a17-00349be14e52` / PID3049512 |
| build | `0515a76b8512bda73db3866d90cd7b6007a4a21950f7bd299603d5a0da5069a0` |
| 統括terminal | `term_756e4b0f1b47a24bb2baa44b01480999` |
| incarnation | `fe677615-7fb8-462c-8b67-342c5b69b028` |
| 保存会話 | `01a0d377-feef-7520-bef5-50565ef282cb` |
| Run | `run_3236488f0dff` / consumer_generation2 / legacy0 |
| 旧launcher / provider / namespace init | 2087516 / 2103719 / 2103720（PIDだけで操作せずbirthとpidfd必須） |
| 旧activation | `7517dc0d-d14a-4746-936a-8ae37dd77c52` |
| 状態 | Driver failed、pane idle。11:42の新turnは限定読取のみ。製品工程未再開 |

既存準備の受入状況:

1. 原因特定: 完了。
2. process observerとpartial/full cooperative lease: 限定部品として検証・固定review済み。
3. 一回FD送信・RELEASED barrier・受信holder: 限定部品として検証・固定review済み。
4. 新runtimeのactual owner観測: 16 focused testsと固定review済み。上位起動経路への接続は未完。
5. 正式retirement、succession/registry CAS、current-enable解決、normal launch接続、実再開: 未完。

先の「3/8＝約38%」は工程件数を後から数えた値であり、工数や復旧完了率の証拠ではない。
本計画のM1〜M6はすべて開始時未完。既存部品を理由に達成点を先取りしない。

## 4. 実装方針と工数上限

### 上限の定義

- **mainと固定reviewerを合わせた追加実作業工数の上限: 120人分。** main最大105分、固定reviewer最大15分。
- **経過時間の上限も120分。** 計画作成、読取調査、実装、試験、修正、review、tool待機、適用、確認、報告を含む。
- 同時作業は合算して人分へ計上する。自動試験/tool待機は経過時間から除外しない。
- どちらかの上限・工程期限に先に達したら、新規実装・適用・再送を止める。heartbeatやcontext更新でT0をリセットしない。
- 追加問題を別名のtask/helperへ移して上限を迂回しない。延長は利用者の明示指示がある場合だけ。
- 120分は成功の予測ではなく実施可能性を判定する損失上限。未接続コードの量をまだ測れていないため、成功確率や完了時刻は保証しない。

### 順序と変更境界

M1で全経路の関数・変更ファイル・必要guard・fixtureを一枚に確定する。
M2はその一経路だけを実装し、最初に終了→移行→通常起動までの結合試験を通す。
個別部品を増やすだけの作業は禁止。予算に収まらなければ安全状態で中断する。

- CLI: `/home/satotakumi/.config/orca/linux-orca-cli-shim/orca`。
- control: `/home/satotakumi/orca/workspaces/hell-workers/orca-hardening-next/scripts/`。
- 想定変更: 一つのpost-provider transaction、既存enable解決/launcher接続、exact registry CASと関連tests。
  新ファイル名・既存ファイル変更箇所・source fingerprintはM1で確定する。
- 本体変更が必要と判明した場合はこの計画では適用せず中断。安易な旧build切替を復旧として扱わない。
- controlの同目的branchを再利用し、M1でHEADとdirty/untracked対象を記録する。primary現HEADは
  `80df44c471a964907fb0fe8b4d0c634d11f33f13`、branchは`codex/building-art-migration`。
  primaryの既存製品・文書差分は保全し、branch切替・commitは本計画作成では行わない。
- 公開許可なし。PR/CIのためにpushしない。ローカル同subject検証を選ぶ。
- Bevy/Rust変更なし。ゲームbuild/testを監視作業へ持ち込まない。

## 5. マイルストーン

配分は経過時間の最大値。遅延を後工程に積み上げず、絶対期限を守る。

| 工程 | 配分上限 | 累積期限（現地時刻） | 合格条件 | 不合格時 |
| --- | ---: | --- | --- | --- |
| M1 全経路・変更総量の確定 | 15分 | 20:40:35 | 正式終了からnormal launchまでの全call graph、対象files、guard、結合fixture、残工数を確定 | 経路/総量が確定しなければ適用せず中断 |
| M2 接続実装 | 40分 | 21:20:35 | 一続きの正常系fixtureが終了→移行→normal launchを通る | 上限までに通らなければliveへ触れず中断 |
| M3 関連検証・Help/storage | 20分 | 21:40:35 | 同source結合/拒否系・関連tooling・Help実判断・primary storage pass | 不合格を記録し中断 |
| M4 固定read-only review | 15分 | 21:55:35 | 全接続経路をexact SHAで承認、P1/P2残なし | 時間内未承認なら適用しない |
| M5 一回のguard付き適用 | 20分 | 22:15:35 | old positive exit、正式succession、新actual enable、同session normal起動 | unknownは保全、再実行せず中断 |
| M6 実再開確認・報告 | 10分 | 22:25:35 | 同session新turnとその中の具体的次工程操作、証拠と結果を記録 | 読取だけ/受付だけなら未復旧として報告 |

M1/M2/M3/M5/M6はmain所有。M4だけ既存固定reviewerを再利用する。
各工程の開始/終了、使用人分、残予算、証拠、阻害要因を§9へ追記する。
工程時間を余らせても、停止期限の延長や対象外作業へ転用しない。

## 6. リスクと対策・即時中断条件

| リスク/条件 | 対策・停止判断 |
| --- | --- |
| old-runtime固定をread-only歴史扱いしてmutation guardを弱める | historical照合と新効果admissionを分離。guard回避案は不採用 |
| 未送信/unknown通知や正式取消を再実行する | 原operationとreceiptを保持。旧通知`7e40892f-ab1d-4efe-8200-c65ca5efb0d7`と取消`ca60adfa-1727-5885-804f-a9fca1fab976`は再実行禁止 |
| PID再利用/部分終了/存続FD alias | prearmed actual pidfd、birth/ancestry、全positive exit、RELEASED barrier、canonical OFDを一続きに検証 |
| 旧ownerの最終保存が旧guardで拒否される | separate正式retirementをraw child outcomeと区別。exit0/ACKを捏造しない |
| source/Run/pane/runtimeが変わる、稼働担当が見つかる | 変更・signal前に中断。再特定は読取だけ。予算内で解決しないなら報告 |
| 新権限・不可逆操作・仕様判断が必要 | この計画の許可では実施しない。根拠を示して質問。既存許可の再承認は求めない |
| 上限時にside effectが結果不明 | 新しいeffectを止める。既に投入した同operationの非変更照合と証拠保全だけ行う。続行/再送はしない |
| reviewerがbusy/不在、既存部品だけ承認される | 全接続の承認と区別。待機も予算に算入、期限内承認なしなら適用しない |

上限到達時の報告は「使用時間/人分、最終合格工程、未完の関数/guard、live変更とunknownの有無、保全状態」を必須にする。
同じ停止を監視だけで無期限に維持せず、予算消尽を一度通知し、この復旧の自動実装を停止する。
通常の実行許可と予算延長は別である。

## 7. 検証計画

- M2: 非ゲームfixtureでactual Unix peer、実FD継承、正/負のpidfd観測と同session normal launch接続を検証。
  模擬の成功をlive復旧の証拠にしない。
- M3: partial exit、unknown送信、fork/欠損chain、source drift、registry競合、偽peer/FD、二重適用拒否を検証。
- 関連Python unittest、Ruff、`git diff --check`。同subject base/head/dirty/untracked source fingerprintを記録。
- completion policyのlocal change-aware gateをM1で対象toolingへ確定する。
  要求群を黙ってskipしない。mixed sourceを分類できずゲーム実行が必要になる場合はscope conflictとして停止する。
- Help Skillで実producer/consumer経路を確認。developer-only IPC/lifecycleなら具体的No impactを記録し、古いtrailerを今回承認へ流用しない。
- primaryの`python3 scripts/dev.py validation check`。各開始/resumeでstorage workflowを読む。
- M4: fixed reviewは部品ではなくretirement→succession→enable→launch全体を対象とする。承認後sourceが変わったら無効。
- M5/M6: current runtime/build、元pane/incarnation、同Run/gen2、old exit、new birth/FD、通常ACK、fresh Driver heartbeat、同session task_started/turn、具体的次工程を照合。
- Rust/Clippy/workspace game tests/native受入は非対象（Rust/game変更なし、監視側実行禁止）。製品受入は統括workflowへ戻す。

### 検証データ管理

- 正本: primary `docs/development-infra/validation-storage-workflow.md`。
- owner: main。consumer: 本計画の同subject復旧・固定review。
- control workspaceとreview-active cache、保存会話/immutable原本を保全。新しい巨大job/build copyを作らない。
- 開始前の直近storage観測: pass、187batches、legacy0、675649040384bytes（13:07工程の検証時）。現在値はM3で再測定。
- 新出力が必要ならprimary validationへ登録し、終了時はseal/finalize/checkを行う。
- 削除済みpath: なし。原本削除は許可外。予算消尽時もunique code/会話/receiptは破棄しない。
- retained bytes/owner/consumer/next_action/release_whenは実測台帳を正本とし、本計画だけを理由に既存全出力を保持しない。

## 8. ロールバック方針

- live適用前の中断: 候補コードを未適用のまま保全し、現app/pane/Run/registryへ触れない。
- live適用後: immutable retirement/succession/unknown receiptを保持。古いenable/registryの書戻しや原operation再送は禁止。
  一回効果を取り消したことにはせず、正規経路が証明できなければ状態を保全して中断する。
- build rollbackだけでruntime pinが復元する証拠はなく、安易に実施しない。
- 本体起動が別途正当に必要になった場合も`launch-supervised`のみ。ただし本計画は追加本体再起動を実施対象にしていない。
- Gitによる一括reset/checkout、既存dirty差分の削除は禁止。

## 9. AI引継ぎ・工数台帳

### 現在地

- 本計画のM1〜M6達成: 0/6。復旧は未完。既存準備のSHA/試験は関連計画末尾にある。
- CurrentRuntimeOwnerの固定review SHA:
  production `6f72f503002d90e1b6e0195f96e2fefc78b23207a88bdb2d38d88ccece9d7834`、
  test `874c2f540018f050a6a7a7c95e51fe705c94134f4a0050b240812ea4ac2be57f`。
- 先の試験/レビューは限定部品だけ。全接続承認・実再開の証拠はない。

| 工程 | 開始 | 終了 | main人分 / reviewer人分 | 合否・証拠・残予算 |
| --- | --- | --- | --- | --- |
| M1 | 20:25:35（計画作成含む） | 20:40:53に期限超過確認 | 経過15分18秒、main保守的計上16人分 / 0 | 不合格。15分期限内に結合fixture・総工数を確定できず、M2へ進まない |
| M2〜M6 | 未開始 | 未終了 | 0 / 0 | 達成の先取り禁止 |

### 次に最初にすること

1. 本計画はM1不合格で中断。残りの全体予算があっても工程期限を迂回してM2へ進まない。
2. 自動実装・live retirement・起動・再送は実施しない。automationのPAUSEDを保持する。
3. 続行には工程・予算の明示的な変更が必要。既存復旧権限の再承認とは区別する。

### M1実施可否判定（2026-10-03 13:40:53 UTC）

**No-go。限定部品はあるが、復旧を完結する上位transactionがない。**
`orca_ui_coordinator.py`の通常起動入口にはpost-provider handoffの受信経路がなく、
既存`recover-settled`はpre-provider専用。通常のsave/enable経路は旧runtime guardを維持している。
既存部品だけの接続やguardの緩和では、正式終了・新所有・通常起動を証明できない。

調査で特定した未接続経路と候補変更境界:

| 順序 | 未完の処理 | 候補対象・必要な証拠 |
| --- | --- | --- |
| 1 | 正式post-provider retirement | 新しい上位transaction。現在のsource/所有/settlementと事前pidfd、単一の終了効果、全対象positive exit。raw exit0と区別 |
| 2 | 新launcherへの実FD移行 | 同paneの通常入口への受信flag追加とactual Unix connect/accept。peer birth/argv/ancestry、RELEASED barrier、full lease所有 |
| 3 | append-only successionとregistry CAS | 新しい正式記録処理。親hash/一意chain/旧原本保全/競合拒否、同Run/gen2保持。台帳の直接書換えでは代替しない |
| 4 | current-enableの解決と新所有admission | `orca_recovery_enable.py`とlock所有証明の接続。kernel flock PIDはFD受信後も送信者を指し得るため、単なるPID guard解除は禁止 |
| 5 | 同会話のnormal launch | `orca_ui_coordinator.py`の通常launchへ接続し、full leaseを適切に解放。実FD/birth/current runtimeに基づく新enable |
| 6 | 結合fixture・実再開確認 | 上記全経路とpartial exit/unknown/fork/source drift/CAS競合/偽peer/二重適用拒否。実環境は新turn・具体的次工程まで必要 |

変更候補は少なくとも通常launcher・enable解決・新しい上位transaction/記録処理と結合tests。
実FD移行後の所有証明、記録とregistry更新の順序、正常起動への渡し方を同時に整合させる必要がある。
40分のM2枠に収まる実装総量、完成した結合fixture、同subject gateの選択を期限内に確定できなかった。
未確定な行数・工数・成功確率を推測の百分率で埋めない。**M1は達成扱いにしない。**

実見したsource基準:

- control HEAD `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、branch `codex/orca-hardening-next`。
  既存多数のdirty/untrackedを保全。今回M1でproduction/testコードの追加編集なし。
- CLI status/terminal show/run-currentでruntime/build、同terminal/incarnation、同Run/gen2を再確認。
- Driverのfailed状態と旧runtime pinは未解消。snapshotは別の旧runtimeであり、稼働証拠に採用しない。
- loop原本の単純なtop-level jq結果はnullだったが、envelope構造を未解決のため状態消失とは判定しない。
- 新signal、retirement、registry更新、新launcher起動、通知再送: **すべて未実施**。

停止は復旧成功ではない。M1〜M6合格0/6、実再開0。成果は不足接続の特定と、期限によるlive変更の抑止。
終了記録・索引確認に要した時間も最終報告へ加算し、M1期限超過を隠さない。
13:42:25 UTCの終了記録時点で総経過16分50秒、main保守的計上17人分、reviewer0人分。
索引checkとdiff checkはpass。文書全体gateは既存node_modulesの4リンク欠損でfailのまま。

### 必読

- 本計画、関連基盤計画の末尾、primary `docs/development-infra/orca-development.md`。
- `orca-cli` / `orchestration` version-matched skillとrecovery/legacy契約。
- primary validation-storage workflow、Help Skill。
- controlの`orca_ui_coordinator.py`、`orca_recovery_enable.py`、post-provider観測/leases、runtime binding、FD transport。

### 最終確認ログ

- 本計画文書: `dev.py docs --write`はindex生成後、既存未追跡node_modulesの4リンク欠損でexit1。
  `node_modules/debug/README.md`のexamples/node/{app,stdout,worker}.js、
  `node_modules/nan/README.md`のkkoopaであり、今回文書による欠損ではない。依存物を削除/修正しない。
  `update_docs_index.py --write/--check`はplans/proposalsの両索引pass。両索引をreviewし、今回追加の計画行を確認。
  `git diff --check`pass。全体文書gate/CIがgreenとは報告しない。
- primary `dev.py validation check`pass、187batches、legacy0、675649232896bytes、原本削除なし。
- 既存automation `orca`は観測時PAUSED。promptへ本計画・同T0/絶対期限/中断条件を反映し、PAUSEDを保持した。
  notification設定・cadence・対象threadは変更せず、新しい自動実行を開始していない。
- このturnのscopeは計画・索引・監視の予算制約のみ。ゲーム/Orca実装コードやlive所有を変更していない。
- 既存限定部品: 関連124tests、CurrentRuntimeOwner16focusedがpass。全接続の成功ではない。
- CI run: なし。push禁止。
- mainによる新しいlive retirement/registry変更/起動: 未実施。
- 未解決: 正式retirement/current-enable/registry CAS/normal launch接続と実再開。

### Definition of Done

- [ ] M1〜M6を証拠付きで達成し、両予算内。
- [ ] 固定review承認と同subject tooling検証、Help実判断、primary storage確認。
- [ ] 原本・既存成果・担当・同会話・同pane・同Run/gen2を保持。
- [ ] old positive exitとnew actual ownership、通常ACK/fresh Driverを確認。
- [ ] 同session新turnと、そのturnの具体的な正規次工程操作を確認。
- [ ] 最終使用人分/経過時間、live effects/unknown、成果と未達を報告。

未達で上限に達した場合はCompletedではなく中断として記録し、新しい自動修正を止める。
TAK-14全体完了と本計画の復旧完了を区別する。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-10-03 | Codex main | 120人分・120分の上限、15分実施可否gate、工程期限と実再開DoDを制定 |
| 2026-10-03 | Codex main | M1期限超過を確認しNo-go。未接続経路と候補変更境界を記録、M2/live適用を中断 |
