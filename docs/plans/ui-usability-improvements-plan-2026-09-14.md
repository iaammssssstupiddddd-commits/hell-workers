# UI操作性改善の残る受入計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `ui-usability-improvements-plan-2026-09-14` |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-09-14 / 2026-09-17 |
| 作成者 | Codex |
| 関連提案 | [UI実装の横断レビュー](../proposals/ui-usability-audit-proposal-2026-09-13.md) |
| 関連Issue/PR | N/A |
| 現在地 | U01〜U32と地図中心UIは実装済み。現行UIはユーザー承認済み。本計画には未実施の操作・表示・理解度評価だけを残す |

## 1. 目的

地図中心UIの実操作、狭い画面・高DPI、初見での理解を検証する。
2026-09-17の承認は実装と今回のフィードバック対応の区切りであり、未実施の受入をpassに変えない。

現行の正本は [地図中心のUI](../ui-world-first.md)、[状態](../state.md)、[一覧](../entity_list_ui.md)、[仕事](../task_list_ui.md)、[詳細](../info_panel_ui.md)、[選択](../world-selection.md)。旧レイアウト、途中の不具合、入力接続の試行ログを現在の仕様や残作業として扱わない。

## 2. スコープ

- C01〜C08の未実施操作、再設計後の共通枠・範囲編集・ガイドの実入力、長文・高DPI、初見評価。
- 同じ条件の回帰テストと実入力を区別し、既存の成功・拒否経路を維持する。
- 任意拡張O01〜O05、新規ゲームdomain、全面翻訳、正式な性能改善の主張は今回の受入範囲に含めない。

## 3. 現状とギャップ

| 項目 | 確認済み | 残る確認 |
| --- | --- | --- |
| U01〜U32 | 実装・Help・回帰テスト。初回checkpointは `c6adac8a` | C01〜C08全体の実入力受入 |
| 再設計前のOS入力 | portal経由で6条件・84 checkpoint。Tasks末尾、最小化、Settings末尾、通知、Help/ガイド開閉 | 再設計後の操作へ流用しない。保存/ContextMenu/Zone等の不足シナリオ |
| 地図中心UI | 初回7場面×6条件の描画、その後の管理6条件、最終範囲編集/入口18条件 | 共通枠・固定・取消・keyboard focus・大件数の実操作 |
| 小型管理・エリア | 個人情報と文字サイズを保持、通常のエリア表示を復元 | 長名・8使い魔×各8魂の比較/配属操作 |
| 作業範囲 | 通常/展開/明示対象、全10操作の描画、開始/終了/拒否の回帰 | 左クリックの詳細から開始、ドラッグ、履歴、保存枠、終了のOS入力 |
| 任意ガイド | 選んだ休息所の搬入/施工/正式完成、対象同一性・消滅の回帰 | 実操作での完走、床の養生工程を含む初見課題 |
| 表示 | 1280×720 / 1920×1080 × UI倍率0.85/1/1.25 | 実OS高DPI、長い対象名、非空Mode案内と範囲メニューの同時表示 |

最終描画・品質結果は [仕様書の検証結果](../ui-world-first.md#検証結果と残る受入)に集約。
XTest送信成功でもpointerが動かなかった調査は過去の記録であり、その後portal入力の到達は確認済み。同じ許可確認を繰り返さず、既存no-prompt launcherを使う。

## 4. 進め方と不変条件

1. 再開時にprimaryのGit差分と既存recipeを読み、未実施の最小シナリオから始める。
2. native Skillと[検証データ整理規則](../development-infra/validation-storage-workflow.md)に従い、primary `dev.py validation`でsubjectと用途を登録する。
3. fixture準備後は「矩形/epochの観測→OS入力→production結果の観測→client画像→ACK」。driverによるInteraction/scroll/確認済み状態の直接代入を実入力の代替にしない。
4. owner PID/window/focus/nonceとsource/harnessを照合し、喪失・timeoutで止める。入力を重複送信せず、OS側の成立条件を先に切り分ける。
5. 不具合修正時は入力→成立条件→結果→表示を追い、Help影響レビュー、生成器によるsnapshot更新、必要な回帰を同じ単位で実施する。

選択・詳細固定・カメラ移動、既存domainの書込制限、captureとEscの所有、epoch/revisionの適用直前照合を維持する。
保存request後のrevision照合と、表示時snapshot/Loadへの照合は保証範囲が異なる。後者の拡張はO03。

## 5. マイルストーン

### M1: 基本操作の残る受入

- [ ] 下表C01〜C08の全手順を現行UIで確認し、既存限定checkpointとの差分を記録する。
- [ ] 200仕事・64通知・8×8配属の先頭/中央/末尾へ到達し、操作先・camera・pinが一致する。
- [ ] 停止中に許可された計画だけが変更され、再開時の再送やload後の旧対象参照がない。

### M2: 再設計の操作と表示

- [ ] 管理→詳細→戻るで検索/絞り込み/scrollを保持し、明示「現地へ」だけcameraを移動する。
- [ ] 固定A/選択Bの一時詳細・戻る・閉じる、参照消滅、world置換を確認する。
- [ ] 使い魔の左クリック→詳細上部から作業範囲を開始し、表示対象を編集する。pause許可、capture拒否、適用済み範囲を残す終了を確認する。
- [ ] 範囲の通常/展開、全10操作、空履歴の説明、対象変更時の折りたたみを確認する。
- [ ] 要対応→fresh Blocked、12建築カード、補助表示と常設エリア、通知未読と問題件数の区別を確認する。
- [ ] 6条件と実高DPIで長い日本語、tooltip全文、非空Mode案内＋範囲メニュー、終了ボタンの可視性を確認する。
- [ ] modal focusの循環/復帰、IME、Esc一段処理、重なり候補、配置に漏れないpanを確認する。

### M3: 初見理解と終了

- [ ] 任意ガイドを採取→選んだ休息所の木材搬入→施工→完成まで実操作する。消滅・取消・skip・loadも確認する。
- [ ] 初見参加者5名を目安に「床を一つ完成」「担当の確認」「止まった仕事の理由」「必要な範囲の表示」「未確定/確定済の取消」を観察する。
- [ ] 停止中か・対象の役割・停止理由・次の確認先を4名以上が各10秒以内に説明できることを設計目標とし、実測結果と混同しない。操作数、迷った入口、誤解した語も記録する。
- [ ] 品質gate、Help/仕様同期、使用終了データの整理後、本計画を閉じる。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| 描画fixtureを操作成功と誤認 | OS入力、回帰、描画、初見観察の結果を別に記録 |
| UI倍率を高DPIと誤認 | 実OSのscale factorと物理/論理寸法を記録 |
| 新デザインを旧passで受け入れる | 現行sourceで再検証する。旧84 checkpointは歴史的な限定結果 |
| 診断が長引く | ユーザー観測を強い制約にし、変化のない仮説を一度で終了 |
| 性能改善を過大評価 | 描画面積・短いsmokeから速度改善を主張しない。必要時だけ別の正式比較を計画 |

## 7. 検証計画

### C01〜C08

seed `20260914`、save/settingsはjob内に隔離し、プレイヤーの保存データを使わない。
以下は確認条件であり、全行が実入力passという意味ではない。

| ID | 回帰で固定する条件 | 実入力の手順と期待結果 |
| --- | --- | --- |
| C01 / U02 | Entities/Tasks × 展開/最小化の4状態、検索有無、capture中pressの破棄。minimizeとvisibilityをproduction順序で複数frame実行 | Tasks→最小化→同タブ展開、最小化中のタブ変更、検索後のタブ往復。本文の描画/クリック領域は最大1つ、TasksにSoul検索は出ず、検索解除後の元状態へ戻れる |
| C02 / U07 | TimePluginの手動real advanceでPause/1x/2x/4x。delay未満は非表示、delay/fade後は表示。実時間step1個以内の差 | 停止の伝播後に未hoverの説明付きbuttonへpointerを入れる。unpauseを挟まず表示され、別buttonへの移動も同じ待ち時間。入力/表示時刻とframe時間を記録し、速度間の差は描画2frame分以内 |
| C03 / U06 | Settingsのheader/footerがscroll body外、Scrollbarの対象がbodyであること | 1280×720 / 1920×1080 × scale 0.85/1/1.25の6条件で末尾とCloseへ到達。実際のSettings操作でscaleを1.25へ変更後、元へ戻す。長い日本語fixtureで文字/操作がclipされない |
| C04 / U03 | 異なるkeyの重要通知64件、Line/Pixelの標準ScrollArea observer、追記/追い出しのanchor/clamp | 通知buttonを押す→wheelで最古へ→thumbで最新へ。先頭/中央/最古の固定IDを画像と表示modelで一致確認。camera transform/projection・world選択は不変。Pixel observerのpassを実trackpadの証明にはしない |
| C05 / U04 | 空slot直接保存、確認のowner/session、保存request後→書込直前のrevision変更拒否、requestの多重発行抑止 | occupied slotを同じ座標でdouble-clickしてもtransactionは0。確認画面を撮影し、明示Confirmで1回だけ実行。Back→再open後の旧session Confirmは回帰側で拒否。Load/Recoveryの各ownerを確認 |
| C06 / U01 | 0/1/20/21/200件のpage範囲・clamp、filter/sort reset、対象失効、world reset | 未配属で自動進行しない取消可能な手動指定200件を使い、1/5/10ページの先頭・末尾を選択/取消。最後のpageで件数減少後も空pageへ取り残されない。行数≤20、pin対象と操作先、READ_ONLY拒否も確認 |
| C07 / U05 | resolverのmodal→ContextMenu→mode/menu順、close consumerの早期return、古いopen要求破棄 | 通常Familiarの命令/目的地を記録し、右クリック→Escでmenuだけ消える。Helpを重ねた場合は最初のEscでHelpだけ閉じる。連打・同frame inputを回帰側でも確認 |
| C08 / U08 | Stockpile/Yard各plan、条件変更、cursor喪失、capture、逆方向/0面積の正規化、release reset | Stockpile: Yard外、複数Yardに跨る有効範囲、16セル中4不可、全不可。Yard: 有効拡張、Site/他Yard重なり、変更なし。採用セル集合または新boundsを照合し、失敗後に枠がcursorへ追従しない |


C03は6条件、その他は1920×1080 / scale 1を基本とし、C01/C04/C05/C06は1280×720 / 1.25でも繰り返す。
高DPIは別に実測し、実trackpad/Waylandの未検証も結果へ残す。

### 既存の検証入口

| 入口 | 用途 |
| --- | --- |
| `.codex/skills/hell-workers-run-native-acceptance/scripts/ui_usability_acceptance.py` | no-prompt launcher、描画/実入力feedback、独立verifier |
| `scripts/native_ui_input.py` | OS入力bridge、PID/window/focusの照合と終了処理 |
| `crates/bevy_app/src/interface/ui/native_acceptance.rs` | profiling専用fixtureと受動observer |
| `scripts/tests/test_native_ui_input.py` / `test_ui_usability_acceptance.py` | 別対象/古いnonce/timeout/欠落画像などの拒否 |
| Task Dashboardの既存native recipe | CPU/割当の検証。pixel layoutやOS入力とは別 |

必要な操作がrecipeにない場合だけdriver/verifierを拡張する。実入力が成立しない条件は不成立として記録する。
Native性能を測る場合はCapture→Memoryを逐次実行し、正式比較では既存のwarm-up/measure規則と事前の許容値を守る。

### 品質と保存

修正した契約のfocused test、`python3 scripts/dev.py check`、Rust診断、`python3 scripts/dev.py verify`、`git diff --check`を実行する。
`verify`のworkspace test/Clippyを変更なしに重複実行しない。索引変更時は `python3 scripts/dev.py docs --write`。

承認済み実装の旧jobは終了し、結果と代表画像は仕様/デザイン資料へ集約する。未実施の受入を理由に旧raw一式を保持しない。
再開時に新しいbatch・owner・consumer・path・実測bytes・next_action・release_whenを登録し、最終closeでconsumerを解除する。通常primary Cargo cacheは別の保守対象とする。

## 8. 任意拡張とロールバック

| 後続候補 | 今回の基本範囲から分離するもの | 再開条件 |
| --- | --- | --- |
| O01 | Task仮想scroll、即focus互換設定 | ページングの実利用で移動負担が確認され、可変高/activationの方式と計測条件を追加計画へ記録したとき |
| O02 | Stockpile方針コピー/preset、通知の見た行だけ既読 | 誤適用防止の差分確認、履歴更新中の既読規則を定義したとき |
| O03 | save名・人口・ゲーム内日付・サムネイル、確認表示時snapshotとLoadのrevision照合 | metadataの有界読取、旧形式互換、書込失敗とサイズ上限を定義したとき。revision拡張は表示対象と実際に読み書きするbytesの一致・変更時の再確認手順を定義したとき |
| O04 | 全面日本語化、文字のみ拡大、演出量設定 | 用語辞書/viewport検証と利用者評価から対象範囲を決めたとき |
| O05 | 停止中のBlueprint/Floor/Wall/Zone/Haul/Dream/配属/取消、範囲policy | 副作用・index/mirror・繰返し適用・取消/loadを各owner単位で受け入れる計画を追加したとき |


これらは後続候補で、基本受入の完了条件へ追加しない。戻す場合は `git log --oneline -5` と対象差分・並行作業を確認し、入力/Help/仕様を同じ単位で整合させる。全体resetや未確認のcheckoutをしない。

## 9. AI引継ぎメモ

- 実装と今回のデザインレビューは承認済み。C01〜C08全体、再設計後のOS入力、高DPI、初見評価は未完。
- まず本計画、現行仕様、native Skill、validation storage workflowを読む。新しい実装を最初から始めない。
- `check` / `verify` / Clippy警告0 / Help exact/coverage / 変更Rust診断は実装最終版で確認済み。結果の範囲は仕様書参照。
- 2026-09-17の文書整理では経過ログと旧仕様を除き、未実施条件を本計画へ一本化した。
- code編集をsubagentへ委譲しない。他作業のDoor/Wall/HVAC、通常Cargo cacheを変更しない。
- 現在の新しいnative batchはない。次回着手時に必要な利用だけ登録する。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-14 | Codex | U01〜U32の計画とC01〜C08を具体化し、実装・限定操作検証に着手 |
| 2026-09-15 | Codex | 整列/密度/案内遮蔽を改善しcheckpoint。地図中心UIの再設計へ |
| 2026-09-16 | Codex | 地図中心UI、管理小型化・エリア復元、範囲編集の入口と小型化を実装・描画検証 |
| 2026-09-17 | Codex | ユーザー承認後、仕様・設計理由・結果を正本へ移し、本計画を残る受入へ限定 |
