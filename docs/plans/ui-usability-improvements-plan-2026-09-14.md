# UI操作性改善 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `ui-usability-improvements-plan-2026-09-14` |
| ステータス | In Progress |
| 作成日 | 2026-09-14 |
| 最終更新日 | 2026-09-15 |
| 作成者 | Codex |
| 関連提案 | [UI実装の横断レビューと操作性向上案](../proposals/ui-usability-audit-proposal-2026-09-13.md) |
| 関連Issue/PR | N/A |
| 調査基準 | primary repository、HEAD `a4051f9b`。提案書作成後もproduction差分なし |
| 現在地 | U01〜U32実装済み。表示密度・文字/記号の整列・tipsの非遮蔽を修正し、全verify（session 67698）と通常/メニュー各6条件の描画確認が完了。同じ1920×1080/1.25でSoul表示数5体を維持。ユーザー依頼により現状をcheckpointコミットし、UI全体の情報設計を再検討する。再デザイン前の84項目操作passと最新の入力なし描画確認を区別する。全C01〜C08受入は未完了 |

### 現在の残作業

| 区分 | 状態と次の作業 |
| --- | --- |
| 実装 | U01〜U32は実装済み。ガイド本文scroll/固定終了ボタンと、ModeText上端に合わせた一覧高さ制約を追加 |
| コード検証 | 最新修正の回帰・Help exact・全verify・Clippy 0・docs/storageはpass（session 67698） |
| 限定実操作 | 1280×720 / 1920×1080 × UI倍率0.85/1/1.25の84 checkpoint通過。Tasks最終ページ、最小化/復元、Entities、停止中Tooltip、Settings末尾/開閉、通知最古/開閉、Helpからガイド表示/終了 |
| 実画面の残り | 高DPI、長文/後続ガイド、C01〜C08の未実施シナリオ（保存の二重確定防止、ContextMenu取消、Zone確定など）。今回のnavigation/layout確認を全受入へ拡張しない |
| 検証環境 | XTest原因調査は終了。既存portal Notify経由で6条件の入力を確認済み。最新画像とprimary cacheをフィードバック用に保持 |

## 1. 目的

### 記号と文字の整列・メニューによる案内遮蔽（2026-09-15）

- 提供画像の作業アイコン/文字の上下ずれと、ZonesメニューがModeTextを隠す状態を修正する。既存の表示密度を維持する。
- Soulセルに明示的な中央揃え、本文に16px行高を指定し、数値もUIフォントへ統一。サブメニューはModeTextの実測上端から4px空けて上へ置き、残るviewport高さでscrollを制約する。UI倍率1.25の配置回帰を追加。
- Help影響は追加差分No impact。表示値/操作/ショートカット/前提は維持し、spawnとviewport補正だけを変更する。6条件の一覧描画と、Zonesを開いた別fixtureで案内の非遮蔽を確認する。検証は既存入力なしlauncherと同じcacheを使用する。
- 初回撮影ではセルの左寄せだけだと作業名が1文字ずつ折り返されたためinvalidとした。本文Nodeへ幅100%を指定し、再撮影で水平表示・整列を確認した。行高/余白の再拡大は行わず、1920×1080/1.25で5体、1280×720/1.25で2体の表示を維持。
- `ui-alignment-20260915-2`と`ui-menu-guidance-20260915-1`は各6条件・全12枚を目視し、独立verify/pass seal/finalize済み。Zones表示時にもModeText全文が読める。入力なしfixtureの描画確認であり、クリックや高DPIの受入ではない。
- 最新画像は`target/native-acceptance/ui-usability-feedback-20260915T142713Z-c9488a1d`（15,106,048 bytes、hold `ui-alignment-current-images`）と`target/native-acceptance/ui-usability-feedback-20260915T144606Z-4c8a202b`（15,060,992 bytes、hold `ui-menu-current-images`）。owner `ui-usability`、consumer `ui-usability-implementation`、次の用途は整列/メニュー配置へのフィードバック、解除条件はフィードバック終了または新画像への置換。primary build cacheは継続使用。
- 比較終了した`target/native-acceptance/ui-usability-feedback-20260915T001117Z-d8c57be9`と`target/native-acceptance/ui-usability-feedback-20260915T141034Z-5d0db0b8`はhold解除・process参照なしを確認して撤去。duは計30,150,656→0 bytes、df空きは前後とも677,144,928,256 bytes（並行書込み/Btrfs共有extentによりdu差と一致しない）。
- 撮影subjectはsource `d26420ace457eff454ab06095505966509978a9239f73931c16fcb043d0d9d21`、harness `266d032bd3a3fba34f3ac1067e8b682570c862244e3f7c124b14681ab8cf36a5`。撮影後はClippyの配置規則に従ってtest moduleだけを末尾へ移動し、描画コードは変更していない。最終checkと対象2ファイルのrust-analyzer診断0、Python回帰8件、native helper群self-test、Skill quick_validate、rule checkはpass。
- 最終`dev.py verify`（session 67698）は終了0・All quality gates passed。通常/profiling tests、追加feature check、Clippy警告0、Help exact/coverage、docs/storageを通過。途中のメモリguard停止では検査を省略せず、解析backendのidle解放後に全体を再実行した。最終storage checkはpass・未分類0。

### 一覧の表示密度の再調整（2026-09-15）

- ユーザーの「2段化で画面内の情報量が減った」という観測に対応する。56px最小高に上下padding12pxと行間4pxを重ねていたため、Soul行は標準で約72pxを占有していた。
- 名前14px・数値12pxと全項目を維持し、最小高32px・上下padding各1px・行間1pxへ変更。使い魔の増減操作を見出しと同一段へ戻し、中央の文字だけを列内で折り返す。セクション下の余白も8pxから2pxへ減らす。
- 既存の6条件render経路で同じパネル寸法の画像を比較する。Help追加差分はNo impact（既存の選択/移動/調整、項目/文言/数値同期は不変）。全verifyと描画確認後に結果・表示人数を記録する。
- `ui-entities-density-20260915-1`は6条件renderを独立verify/pass seal/finalize済み。全6枚を目視し、1920×1080/1.25では完全に見えるSoulが2→5、1280×720/1.25では1→2に増えた。同じパネル寸法、文字14/12px、各使い魔2体のfixtureで比較。名前そのものとAI状態は起動ごとに異なる。入力・高DPIの証拠にはしない。
- source `25b82e58d7315c04ad04f5958dbb104c13fbd8c5b994ca213eb77aa12720d021`、harness `f9e3f5e51cac9b449818ef1d57b7e0c7b15a4b3dd3858fedc1ebf4621cbe76fe`。最新画像は`target/native-acceptance/ui-usability-feedback-20260915T001117Z-d8c57be9`、hold=`ui-density-current-images`、owner=ui-usability、consumer=ui-usability-implementation。次の用途は表示密度へのフィードバック、解除条件はレビュー終了または新画像への置換。
- check、rust-analyzer診断0、全verify（session 97213、All quality gates passed）を確認。Help/Clippy 0/docs/storageを含む。最新画像15,093,760 bytesと通常cacheを保持。比較終了した旧job `target/native-acceptance/ui-usability-feedback-20260914T192649Z-e146b668`はhold解除・process参照なしを確認して削除（14,970,880→0 bytes）。
- df空きは整理前677,363,007,488→整理後677,361,164,288 bytes。他書込み/Btrfs共有extentのためdu差とは一致しない。最終storage checkはpass、未分類0。

### Entities一覧の画像フィードバック対応（2026-09-15）

- 提供画像の現地ボタンの不揃い、使い魔調整ボタンの意図しない折返し、スクロール案内と最終行の重なりを修正する。
- Soulは固定列の2段グリッド（名前/作業、数値）と右端の現地ボタン、使い魔は見出しと調整操作を区画化する。既存の選択・移動・調整・値同期経路を維持する。
- 実装後にcheck/全verify/Help影響レビューを行い、既存native helperの6条件でEntities画像と操作を確認する。最新比較画像と通常cacheはフィードバック終了まで維持する。
- 実装: Soulを最小56pxの2段グリッドへ変更し、氏名/作業と数値を分離。現地ボタンを48px列へ固定し、配下の字下げをpaddingへ変更。使い魔は32pxの増減操作を見出し下段へ固定し、共通UI書体を使う。スクロール案内は本文外の通常フローへ移した。
- Help影響の追加差分はNo impact。既存SoulListItem/FamiliarListItem→受理済みクリック、FamiliarMaxSoulAdjustButton→上限調整、MenuAction::FocusEntity→カメラ移動、直接children順の値同期経路を維持する。キー・文言・対象・成立条件・数値の意味は不変で、配置と書体だけを変更。UI改善全体の既存Update required判断は維持する。
- `ui-entities-design-20260915-1`はportal Startの300秒タイムアウトでinvalid、入力0件。同条件の接続再試行はしない。失敗画像で見出しの空白を発見し、入力なしのEntities専用render経路を追加した。profiling fixtureでDev本文を隠し、その後はread-only観測とPID所有window撮影だけを行う。操作テストの代用にはしない。
- render初回6条件で空白を再現。見出しTextの幅を親列の100%へ明示後、`ui-entities-render-20260915-2`の6条件で文字矩形と画像を確認、独立verify/pass seal済み。調査用のsize/position記録は撤去し、見出しText/rectのpass条件を恒久検証へ残した。配下2体と停止状態を固定した最終撮影を続ける。
- 最終`ui-entities-render-20260915-3`は6条件のEntities描画を独立verify/pass seal/finalize。全6枚を目視し、使い魔名、Soulの2段表示/現地操作列、本文外のscrollヒントを確認。source `23c9442f05d53bb0fa1fd794c1ac75e782f29cddbb34f2eb672623acac164262`、harness `f9e3f5e51cac9b449818ef1d57b7e0c7b15a4b3dd3858fedc1ebf4621cbe76fe`。OS入力なしの描画結果であり、操作・高DPI・長名の受入へ拡張しない。
- 最新コードのcheck、全verify（session 80912、All quality gates passed）、Clippy 0、Help exact/coverage、Python描画拒否条件7件、native helper self-test、Skill quick_validate、rust-analyzer spawn/render診断0件を確認。
- 最新画像は`target/native-acceptance/ui-usability-feedback-20260914T192649Z-e146b668`、14,970,880 bytes。hold=`ui-entities-current-images`、owner=ui-usability、consumer=ui-usability-implementation、next_action=新一覧画像のユーザーレビュー、release_when=フィードバック終了または画像置換。通常primary cacheを保持する。
- 比較終了した旧job `target/native-acceptance/`直下の`ui-usability-feedback-20260914T185246Z-b9b70b1b`、`ui-usability-feedback-20260914T190743Z-1dd53ee2`、`ui-usability-feedback-20260914T191328Z-275d8ca9`、`ui-usability-feedback-20260914T171926Z-d6d53166`はhold解除・process参照なしを確認して削除。合計240,984,064→0 bytes。製品コード・asset・通常cache・他ownerの保持物は削除対象外。
- 整理前後のdf空きは677,140,238,336→677,139,546,112 bytes（他の書込み/Btrfs共有extentによりdu減少と一致しない）。最終storage checkはpass、未分類0、20 batch。

- 解決したい課題: 一覧の後方へ到達できない、表示状態が競合する、操作対象・確認段階・失敗結果が分かりにくい。
- 到達したい状態: 管理対象が増えても目的の対象を探して操作でき、停止中にも状況を整理し、操作結果を確認できる。
- 成功指標: タスク200件・通知64件・Familiar 8体×各8Soulの全対象へ到達できる。filter直接選択は2操作以内、全解除は1操作。誤った対象への確定や背景入力の漏れがない。
- 見た目の改善は1280×720 / 1920×1080、UI scale 0.85 / 1 / 1.25と高DPIで検証する。読みやすさ・操作時間・性能の改善率は未測定であり、計画時点では保証しない。

本書は提案U01〜U32の実行順・依存・完了判定の正本とする。静的なコード根拠と詳しい現状説明は関連提案を参照する。
2026-09-14の実装依頼により着手。計画の作成完了、コード変更、UI改善の受入完了を区別する。

## 2. スコープ

### 対象（In Scope）

- 第一段階はP1の8項目（M1・M2）。一覧への到達性、表示競合、確認表示、Esc、設定画面、Tooltip、Zoneの不整合を解消する。
- 第二段階は管理・詳細・設定・通知・入力（M3〜M5）。第三段階は停止中の計画操作と発見性（M6・M7）。
- U01〜U32をすべて追跡する。§4で初回の採用方式を固定し、後続候補O01〜O05を区別する。計画の採用方針は実装済みを意味しない。
- 各変更に必要なHelp、仕様書、回帰検証、actual-window受入を同じ変更単位で整える。

### 非対象（Out of Scope）

- UIライブラリや描画方式の全面置換、3D asset制作、ゲームバランス変更、全ゲーム操作のUndo、コントローラー全面対応。
- 未完成のFamiliarBuildの公開、HVACの換気・導水・排泥・Room認可domainの新規導入。
- 現在のプレイヤー保存データを検証fixtureとして使うこと。既存の他作業のjob・worktree・cacheの整理。
- 保存サムネイル、Stockpile preset、演出量設定などの任意拡張を基本修正の完了条件へ一括追加すること。

## 3. 現状とギャップ

- root adapter、ViewModel、UiIntent、domain ownerが分離され、capture・load reset・適用直前再検証・有界な表示更新が既にある。
- これらの保護を通る正しい経路でも、scroll入力の未接続や複数systemによる表示状態の競合が起きる。まず経路・成立条件を再確認する。
- Task行はpin時だけaction barを生成するため、自動pinだけを外すと優先度変更・取消が消える。選択・詳細・pin・確認対象を分離する必要がある。
- 一覧検索は折りたたみ後の対象へ適用され、TabはEntity index順。検索・scroll・表示順の整備を先に行ってからTabを合わせる。
- 実機の小画面・DPI・長い日本語での描画は未確認。既存native recipeのpassだけでは新しい操作シナリオを確認したことにならない。

### 既存計画との境界

| 関連作業 | 本計画の責務 | 境界 |
| --- | --- | --- |
| [換気・導水計画](hvac-plumbing-plan-2026-07-13.md) | 現行Roomの囲い成立/不成立の診断と共通の詳細表示 | 新しい換気・排泥・認可理由はHVAC側。Roomのpure判定と表示DTOの拡張点を合わせ、別のRoom真実を作らない |
| [Door本番アート計画](3d-rtt/production-door-art-plan-2026-09-05.md)、[仮設壁計画](3d-rtt/provisional-wall-formwork-plan-2026-09-05.md) | Door状態・lock操作の発見性、選択とInfoPanel | mesh、投影、asset view、他ownerのrelease受入・cleanupを変更しない |
| 入力・通知・Task Dashboard・saveの完了済み計画 | 現在の仕様書と実装を拡張する | archiveを再開しない。古い実機証拠や凍結subjectを今回の候補へ流用しない |

### 自己レビューで修正した計画上の不足

| 所見 | 実装上の根拠 | 計画への反映 |
| --- | --- | --- |
| D01〜D06を着手後の判断に残しすぎていた | Taskはtoolbar/group/actionで可変高、dirty時に本文を再生成する | 初回は20件単位のページング＋ページ内scrollを採用。選択/確認/再生成の規則を固定 |
| 「stable key」だけではload後の失効先が不明だった | `hw_ui/src/lib.rs`に既存world置換reset、`hw_core::WorldEpoch`に既存epochがある | Entity＋既存epochを使い、新UI状態を既存resetへ登録。新しいworld世代の仕組みを重複実装しない |
| Zoneを一律の生成件数で受け入れようとしていた | `bevy_app/src/systems/command/zone_placement/placement.rs`のStockpileはセル生成、Yardは既存境界の拡張 | StockpilePlan / YardExpansionPlanを分ける。最新条件が変わった場合とcursor喪失時のreleaseも定義 |
| pauseを入力capture中心に扱い、commandの副作用を後回しにした | `bevy_app/src/plugins/game.rs`はSpatial/Logic/Actorを停止。AreaはDestination等も更新、配属はvitals変更、Door配置は既存壁を置換し得る | 初回allow-listと除外理由、入力適用/index更新/進行の境界を固定。汎用保留queueは導入しない |
| 既存nativeのpassを新しい操作へ拡張する手順が未定だった | 通知は3件＋history直接toggle、saveはUiIntent直接発行。通知driverはTooltip直前にunpauseする | UI専用の実入力recipeをM0-Bとして追加。driverの回避処理をU07で撤去。状態注入・実入力・描画の証拠を分ける |
| P1の独立提示と全体M8の終了条件が混在していた | 第一段階だけで受け入れる対象・必要な観測が一覧化されていなかった | R01〜R08とC01〜C08を対応付ける。P1受入と全32項目の完了を別に判定 |
| save/loadのrevision保証を広く書きすぎていた | `bevy_app/src/systems/save/catalog_ui.rs`の確認stateはslot/session。保存はrequest時のrevisionを書込直前に照合するが、Load requestにrevision条件はない | R05/C05は既存の保存保証範囲を明記。表示時snapshotやLoadへのrevision照合追加はO03へ分離 |
| 画像capture関数をそのまま他画面へ再利用できると扱った | `take_native_screenshot`内のvalidatorはSave Catalog専用markerを要求する | M0-BでPID所有clientのcapture部分を共通化し、画面別validatorを接続。既存Save検証を維持 |

上表の短縮コードpathは `crates/` 配下。これは既存ゲームの新しい実機再現記録ではなく、計画の自己レビュー結果である。

## 4. 実装方針（高レベル）

### 責務・不変条件

- `hw_ui`: widget、表示状態、focus、scroll、theme、ViewModel表示、UiIntent producer。
- `bevy_app`: ゲーム側の条件とViewModel、入力resolver、capture、save/settings adapter、domainへ渡すcommit順序。
- `hw_jobs` / `hw_world` / `hw_logistics` / `hw_energy`等: 現在のdomain ownerを維持する。UIが資源・予約・建設phaseを直接変更しない。
- 行はstable keyを持ち、ページ切替・再生成・filter・sort・load・対象消滅時に選択/確認/dragを再検証する。行の表示indexを操作対象の識別に使わない。
- `UiDomainCommitSet`、save session/revision guard、Taskのpositive allow-list、未公開capability、Help起因pauseの所有権を維持する。
- Bevy 0.19の既存Help/Operation widgetを参照し、外部APIはdocsrs-mcpまたはローカルregistryで確認する。`Overflow::scroll_y`だけをscroll入力の実装と見なさない。
- UI待ち時間には実時間を用いる。Speech/Dreamなど世界内の時間契約は、Tooltipとまとめて変更しない。
- 新しい共通基盤を先に全面導入せず、同じ規則が必要な具体的な画面から抽出する。

### 仕様判断の確定案

以下をこの計画の初回実装方式とする。実装で成立しない根拠が見つかった場合は、根拠と代替方式を本表へ記録して変更する。
「後で検討する」だけを完了条件にせず、初回対象と保留した拡張を明示する。

| ID / 対象 | 初回実装方式 | 理由・後続候補 |
| --- | --- | --- |
| D01 / U01 | **20件/ページ＋ページ内ScrollArea**。固定toolbar/footerに先頭・前・次・末尾と「21〜40 / 200件」 | 可変高の行・group・actionを保ち、有界性と全件到達を先に成立させる。仮想scrollはO01 |
| D02 / U15 | 行の単クリックは選択/詳細、明示「現地へ」でcamera、pinは独立ボタン。配属dragはcameraを動かさない | 自動pinとaction barを分離する。ダブルクリックや即focus設定は初回へ追加しない |
| D03 / U10・U28 | 修飾なし1/2/3/4を停止/通常/高速/最高速へ固定。Familiarは既存C/M/Hを残して数字aliasを撤去。modalはTab/Shift+Tabと新規Enter/Space入力 | Ctrl/Alt＋数字のArea presetは維持。Escは閉じる/取消へ統一。Idle/PatrolはFamiliar文脈のIへ移し、同じtargetを使う文脈menu項目も追加 |
| D04 / U09 | 時間停止とsystem menuを分離。閲覧/camera、担当Area、新規Chop/Mine、既存Stockpile単体policy、完成Door lockを許可 | 保存可能な既存domainデータへ直接適用。AI/搬送/建設進行を停止し、再開時に同じcommandを再送しない。建築/Haul等はO05 |
| D05 / U25 | 最小1280×720、scale 0.85〜1.25。本文14px・補助12px・主要操作領域32×32px相当をscale=1の初期設計値とする | M1から長い日本語fixtureを使う。用語の対照辞書を整え、同じ画面内の同じ概念の表記を統一。全面翻訳/文字だけ拡大/演出設定はO04 |
| D06 / U19・U22・U24・U27 | Room診断等は既存domainを表示。通知時間は4/8/12秒（既定4秒）、履歴は実時間の経過を表示。saveは既存slot区分＋絶対日時。初回ガイドは任意開始 | Stockpile preset・通知既読方式・save追加metadataはO02/O03。初回ガイドは保存schemaを増やさず、閉じる/loadで終了しHelpから再開可能 |

数値は実装の初期設計値であり、可読性や規格適合の実測結果ではない。viewport/長文受入で不足した場合は拡張する。

| 後続候補 | 今回の基本範囲から分離するもの | 再開条件 |
| --- | --- | --- |
| O01 | Task仮想scroll、即focus互換設定 | ページングの実利用で移動負担が確認され、可変高/activationの方式と計測条件を追加計画へ記録したとき |
| O02 | Stockpile方針コピー/preset、通知の見た行だけ既読 | 誤適用防止の差分確認、履歴更新中の既読規則を定義したとき |
| O03 | save名・人口・ゲーム内日付・サムネイル、確認表示時snapshotとLoadのrevision照合 | metadataの有界読取、旧形式互換、書込失敗とサイズ上限を定義したとき。revision拡張は表示対象と実際に読み書きするbytesの一致・変更時の再確認手順を定義したとき |
| O04 | 全面日本語化、文字のみ拡大、演出量設定 | 用語辞書/viewport検証と利用者評価から対象範囲を決めたとき |
| O05 | 停止中のBlueprint/Floor/Wall/Zone/Haul/Dream/配属/取消、範囲policy | 副作用・index/mirror・繰返し適用・取消/loadを各owner単位で受け入れる計画を追加したとき |

O01〜O05は未実装の後続候補として本書に保持し、基本範囲のM8を閉じるためだけに「完了」へ変えない。

### 左パネル・ページ・操作対象の状態契約

U02は `LeftPanelMode` と `EntityListMinimizeState` を正本にし、本文・検索行・resize handleの表示writerを一つにする。
toggleはstateと高さを更新し、本文の `display` を直接書かない。表示systemはtoggle/tab変更後に実行し、mode/minimizedどちらの変更でも再計算する。

| mode / minimized | Entities本文 | Tasks本文 | Soul検索行 | resize |
| --- | --- | --- | --- | --- |
| Entities / false | 表示 | 非表示 | 表示 | 有効 |
| Tasks / false | 非表示 | 表示 | 非表示 | 有効 |
| Entities / true | 非表示 | 非表示 | 非表示 | 無効 |
| Tasks / true | 非表示 | 非表示 | 非表示 | 無効 |

タブと展開ボタンは最小化中も残す。検索文字列・元の折りたたみ・各本文のscrollは通常のタブ往復では保持し、検索欄が消えた時点でその文字入力focusを解除する。
capture中の操作は破棄し、解除後に遅れて適用しない。高さは既存expanded_heightを保持し、resizeとscale変更でviewportへ再clampする。

Taskページの表示stateは `hw_ui::panels::task_list` に置き、filter/sort後の論理配列へ適用する。
`start=page_index*20`、`end=min(start+20,total)`。page数は0件でもUI上1、`page_index`は最終pageへclampする。
ページ内group headerはそのページ先頭で再掲し、件数はfilter後全体の当該group数。toolbar/footerをscroll本文の外へ移す。
ページ変更は本文scrollを先頭へ、filter/sort変更はpage=0へ戻す。通常の進捗更新ではpageを維持し、件数減少時だけclampする。
20件の行に加え、group headerは最大20個、action barは選択行分だけとする。hidden/minimizedでは描画再構築を止め、再表示時にdirtyを処理する。

M2は既存pin操作を保ち、M3で `active_task` と `InfoPanelPinState` を分離する。以下はM3の完成時の契約である。

| 操作/変化 | active_task・選択 | pin | 未確定の取消/activation |
| --- | --- | --- | --- |
| Task行を選択 | 当該Entity。pinなしならその詳細を表示 | 自動変更しない | 前の対象の確認を解除 |
| 「現地へ」 | 対象の現在位置を再取得してcamera移動 | 変更しない | 二重適用しない |
| pin/「選択を表示」 | 選択は維持 | 明示pin／解除 | 影響する古い確認を解除 |
| page/filter/sort変更 | 現在pageから外れたactive_taskは解除 | 生存中のpinは維持 | 解除し、別の行へ引き継がない |
| 対象の消滅・work/capability変更 | 対象を再検証し必要なら解除 | 消滅時解除 | 既存 `PendingTaskCancellation` のtarget/work/kindとlive capabilityを再検証 |
| modal/capture開始 | 閲覧対象は維持 | 維持 | press/drag/確認を破棄 |
| world置換/load/rollback | 全解除、page/filterを初期化 | 全解除 | 全解除、復帰focusも破棄 |

行の識別は `TaskEntry.entity` の世代付きEntity＋既存 `WorldEpoch`。snapshot全体の更新だけで未変更対象の閲覧を解除しない。
新しいpage/active_task/activation stateは `crates/hw_ui/src/lib.rs` のworld置換resetへ登録する。
M5のrelease確定は、押下した論理action・対象・epoch・画面sessionがrelease時も同じである場合だけ受理する。
widgetが再生成された場合はそのpressを取消し、新しいNodeへの押し直しを必要とする。M1/M2で新しい共通activation基盤を先行実装する必要はない。

### P1の入力・表示・Zone契約

- **U03:** 見出しとCloseを固定し、履歴本文へScrollAreaと対応Scrollbarを接続する。開いた直後は最新側。読んでいる間の追記は先頭へ強制移動せず、残存する先頭可視通知を基準に位置を保つ。64件の追い出しで基準が消えたら有効範囲へclampする。既読方式は初回変更しない。
- **U04:** slot一覧と確認本文を別表示にし、確認時はslotボタンを無効化/非表示にする。Confirmは常時予約してある固定footer内へ置き、直前に押したslot行と同じ位置へ出さない。対象slot・上書き/現在world置換・Confirm/Backを明示。M1では新しいpressなしに選択入力をConfirmへ引き継がないことを保証し、M5のkeyboard導入時は初期focusをBackにする。
  Confirmはowning session・slot・live capabilityを検証し、request発行後は結果まで再発行を抑止。保存ではrequest発行時のcatalog revisionを既存の書込直前照合へ渡す。確認画面表示時からのrevision固定やLoad requestへのrevision条件追加はO03とし、R05の既存保証と混同しない。空slotの通常保存は現在の直接保存を維持する。Backは元のcatalog ownerへ戻り、Recovery catalogの制限を変えない。
- **U05:** `InputContextSnapshot`へ表示中ContextMenuの有無を加え、`CloseContextMenu`を上位modal処理の後、active-mode取消/通常menu/Idle-Patrolより前に解決する。全面capture overlayへ昇格させない。consumerは `context_menu_system` のpointer抑止による早期returnより前に閉じる処理を行う。同frameの古いopen要求も破棄する。
- **U06:** 既存Operation dialogの中央配置・最大高さを転用し、Settingsは固定header＋可縮本文ScrollArea＋固定Closeへ分ける。外枠はviewport内、本文はmin-height=0相当で縮める。scaleは既存UIの実効寸法から適用し、pixel値を二重拡大しない。設定変更中もCloseとscale復元操作へ到達できることを受け入れる。
- **U07:** rootのTooltip入力だけを `Time<Real>` に変更し、内部delay/fadeへ同じreal deltaを渡す。通知native driverの一時unpauseによる回避を撤去する。M3の配属drag待ちもreal timeへ移すが、Speech/Dreamの寿命は対象外。

U08はroot command ownerに共通のpure plan生成を置き、previewとreleaseが同じ判定を呼ぶ。型名は実装時に合わせられるが、結果の区分は固定する。

| plan | 入力と出力 | commit規則 |
| --- | --- | --- |
| StockpilePlan | 正規化したgrid範囲、セルごとのYard ownerと占有/歩行条件 → 採用セル `(grid, yard)`、除外理由・件数、全体拒否 | Yard外を含む範囲は現仕様どおり全体拒否。複数Yardを跨いでも全セルが所属すれば可。既存Stockpile/建物/非歩行セルは除外し、採用0は失敗 |
| YardExpansionPlan | 開始点のowner、旧bounds、drag範囲、Site/他Yard → 新bounds、追加面積、拒否理由 | 既存の最小寸法・Site/他Yard重なりを検証して同じownerを更新。生成Entity数ではなくboundsと追加タイル数を照合 |

拒否理由は範囲外、owner不在、Site重複、他Yard重複、占有、非歩行、変更なしを区別する。セル列挙と理由の表示順をgrid順に固定する。
「preview件数=commit件数」は同一のworld条件で確認する。releaseでは最新状態で再planし、直前の有効previewと採用対象/owner/boundsが変わっていれば無変更で終了し、再指定を案内する。
**releaseを受けた時点でdrag開始点を解除**し、cursorのworld座標が取得できない場合もcommitせず待機へ戻す。UI capture中は既存gesture rollbackが同じ状態へ戻す。

### 停止中の操作と再開の契約

M6の初回allow-listは次表で固定する。成功した入力は停止中にdomain ownerまで完了させる。
MessageWriterへ入れたまま保持する方式や汎用command queueは追加しない。再開時は保存済みの指定を既存AIが処理する。

| 停止中の操作 | 初回 | 即時変更を許すもの／条件 |
| --- | --- | --- |
| 選択・詳細・camera・Help/Settings/save | 許可 | 閲覧/表示状態。overlayの既存captureと保存順序を維持 |
| 担当Areaの指定・移動・resize・履歴 | 許可 | TaskArea、FamiliarのDestination/ActiveCommand、範囲内の未管理指定のManagedBy/Priority。実位置・履歴の二重登録は禁止 |
| 未指定Tree/RockへのChop/Mine | 許可 | Designation/PlayerIssuedDesignation/ManagedBy/TaskSlots/Priority。既存指定や進行中対象へTaskSlotsを上書きしない |
| 既存Stockpile単体policy | 許可 | live ownerでpolicyだけ変更。inventory/inflight deliveryは変更しない |
| 完成Doorのlock | 許可 | 既存request ownerでDoor/WorldMap通行性とlightingを同期。移動中/未完成/消滅対象は既存理由で拒否 |
| Haul・全建築配置/移動・Zone配置/解除・Dream・配属・建設取消・範囲policy | 初回除外 | capacity/request、置換、資材/Dream、vitals、返金/index等の追加検証が必要。O05でowner単位に拡張 |

許可しないworld変更はbutton/shortcut/domain admissionのすべてで拒否し、「停止中は利用不可」と表示する。
上表にないworld変更も初回allow-listへ自動追加しない。既存の設定編集と復旧専用操作の可否は別の既存契約を維持する。
許可したArea/Chop/Mineの入力・適用だけを `GameSystemSet::Logic` から分離し、変更対象のDesignation/表示indexを停止中にも同期する。
全Spatial/Logic/Actorを再開する修正は禁止。停止開始直前の未反映変更もflushしてから、次の選択・範囲queryが更新結果を読めるようにする。
必要なindex同期は既存ownerの変更対象だけを扱い、停止中に全indexを毎frame再構築しない。

Spaceは時間だけをtoggleし、未処理のEscまたは画面のMenu操作でsystem menuを開く。
system menuの表示は `Time<Virtual>::is_paused()` から独立したstateで管理する。開く時に自分が停止した場合だけ速度復元情報を持ち、閉じる時にそれを使う。
ユーザーが既に停止していた場合は閉じても停止を維持する。Help/Settingsへのhandoff中は復元せず、load/RecoveryFailedでは旧復元情報を破棄する。
停止中の保存には確定済みのArea/指定/policy/Doorを含め、未確定dragはcapture時にrollbackする。autosaveのactive-play時計は進めない。

### 後半項目の初回仕様

| 対象 | 具体化した初回仕様 |
| --- | --- |
| U11/U13 | Soul名の部分一致を維持し、検索前に全Soulを対象化。検索中だけ一致groupを展開、解除で旧開閉を復元。Tab候補は論理ViewModel順で、非表示page/scroll外の対象にも移動する |
| U12/U15/U17 | 本文だけscroll、drag端scrollはviewport端でのみ実行しcapture/離脱で停止。別対象の詳細は先頭へ、同じ対象の更新はscroll保持。pin中は対象名と「選択を表示」を常時表示 |
| U14/U16 | AssignedTaskをexhaustiveに分類。filterは単一選択menuと一括reset。blocker操作の初回対象は対象が確定するFamiliar policyと資材/Stockpile詳細に限定し、不明な対象を推定して開かない |
| U18/U19 | site取消は共通確認modelから同じowner requestへ。Roomはgrid起点の検査で、成立Entityなしでも理由/該当位置を表示。診断は選択/関連world revision変更時だけ更新し、毎frame全mapを検査しない |
| U20/U21/U22 | 値/単位を常時表示。保存失敗は適用済み/未保存を区別し、再試行は現在の設定版を保存して古い設定を復活させない。通知actionは履歴だけに置き、対象消滅/epoch変更で無効化 |
| U23 | stable entry IDで検索/文脈移動。同じHelpを閉じ開きしたときtopic/本文位置を復元し、存在しないentryはfirst topicへ。検索入力中の編集キーとIMEを優先、loadでは一時状態をreset |
| U24/U25 | saveはslot区分・既存日時を使って識別し、scroll末尾まで比較可能。追加schemaは導入しない。長文はwrap、狭幅submenuはviewport内へ反転/移動し、固定座標だけで配置しない |
| U26/U29/U32 | mode表示はownerが返す担当・採用/除外・段階・理由から作る。Areaの空Undo/Redo/presetは理由付きdisabled、実行前に担当と寸法を表示。UIが件数や費用を再推定しない |
| U27 | Help内「操作ガイドを開始」から任意開始。Familiar選択→担当範囲確定→本人の採取指定→実作業開始/完了を追う。対象消滅時は該当段階へ戻し、skip/閉じる/loadで終了。進捗の永続化は行わない |
| U28/U30/U31 | modal focusを循環し、閉じたら生存する呼出元へ復帰。中ボタンpanは配置待機時だけ許可し、範囲drag中は拒否。重なり候補は表示中の「候補 n/m」操作から一覧を開き、選択済みの候補が同地点に残る場合は右クリックもその対象を使う |

右クリックの明示候補優先は通常のentity action対象に限定し、Floor/空地のFamiliar移動を奪わない。候補には既存選択validatorが許す対象だけを載せる。

### 32項目の対応表

各行を一つの修正・レビュー単位とし、同じM内でも独立に確認できる形で進める。実行状況は§9へ記録する。

| 提案 | 実装単位 | 工程 | 主な先行条件 |
| --- | --- | --- | --- |
| U01 | Task全行への到達、表示範囲・総件数 | M2 | M1のU02、D01 |
| U02 | 左パネルのタブ・最小化・検索状態統合 | M1 | M0-A |
| U03 | 通知履歴ScrollArea/Scrollbar接続 | M1 | M0-A |
| U04 | save/load専用確認表示 | M1 | M0-A、既存session/revision guard |
| U05 | ContextMenuのEsc優先処理 | M2 | M0-Aの入力再現条件 |
| U06 | Settingsの最大高さ・本文scroll・固定Close | M1 | M0-Aのviewport条件 |
| U07 | Tooltip delay/fadeの実時間化 | M1 | M0-Aの時間経路確認 |
| U08 | Zoneの共通plan・理由・release cleanup | M2 | M0-Aのpreview/commit対象確認 |
| U09 | 停止中の計画操作 | M6 | M2、M5、D04 |
| U10 | 有効shortcutと表示の一致 | M5 | D03 |
| U11 | 全Soul検索と折りたたみ復元 | M3 | U02 |
| U12 | Entities全体scroll・配属端scroll | M3 | U02 |
| U13 | 可視順のTab/Shift+Tab・行へのscroll | M3 | U11・U12 |
| U14 | 作業アイコン/ラベルの網羅分類 | M3 | 現行AssignedTaskの全variant確認 |
| U15 | 選択・詳細・pin・camera・dragの分離 | M3 | U01、D02 |
| U16 | Task直接filter・全解除・関連情報への操作 | M3 | U01・U15 |
| U17 | InfoPanel固定header・本文scroll | M3 | U15 |
| U18 | Soul Spa取消確認の入口統一 | M3 | U04・U15、既存owner cleanup |
| U19 | Door/Room/電力の詳細と次の操作 | M4 | U17、HVAC境界、D06 |
| U20 | 設定値・単位・適用時期表示 | M4 | U06 |
| U21 | 設定保存失敗と安全な再試行 | M4 | U20、既存通知adapter |
| U22 | 通知時間・時刻・履歴からの対応 | M4 | U03・U21、D06 |
| U23 | Help検索・entry移動・読書位置復元 | M7 | M5のfocus、先行工程で更新したHelp |
| U24 | save一覧scroll・slot区分/絶対日時の表示 | M7 | U04、D06、既存形式の読取確認 |
| U25 | 狭幅layout・文字/操作領域・用語 | M7 | U06・U17・M5、D05 |
| U26 | 配置/範囲の次操作・数量案内 | M7 | U08・U29・U32、M5/M6の操作仕様 |
| U27 | 最初の仕事ループの任意案内 | M7 | U23・U26、D06 |
| U28 | modal focus・keyboard操作・release確定 | M5 | M1のdialog、M3の選択/確認、D03 |
| U29 | Orders担当・適用件数・skip理由 | M4 | U08の共通表示方針、既存designation owner |
| U30 | モードを維持する専用mouse pan | M5 | U05、既存gesture/capture rollback |
| U31 | 重なり候補一覧と右クリック対象規則 | M5 | U05・U15 |
| U32 | Area履歴/presetの対象・寸法・空状態 | M4 | 既存履歴owner、U15の対象表示 |

## 5. マイルストーン

実行順は **M0-A → M1 → M2 → M3 → M4 → M5 → M6 → M7 → M8** を基本とする。M0-BはM1/M2と同じ候補上で主担当が順次整備し、第一段階の実機受入より前に完了する。
最初のリリース候補はM1＋M2のP1全8項目。M3以降を待たずに受入・提示できる。
同じ状態を変更する単位は逐次編集する。並列agentは読み取り専用調査・レビューに限定する。

### 第一段階の実装単位

M0-Aで基準を固定した後、下表の順に主担当が編集する。M0-Bの実入力driverはM1と同じ候補上で整え、R01〜R08の提示前に必要なCケースを通す。
各行の機能・必要なHelp・focused testを一つの差分にする。既存nativeの大きい性能matrixを8回繰り返す必要はない。

| 順 | 対象 | 具体的な差分 | 対応受入 |
| --- | --- | --- | --- |
| R01 | U02 | minimizeから本文displayの書込を除去。既存visibility systemをmode＋minimizedへ拡張し、検索行/resizeのmarkerとorderingを追加 | C01 |
| R02 | U07 | Tooltipのroot時間入力をRealへ。通知native driverのunpause回避を除去。手動real advanceによるdelay/fade回帰を追加 | C02 |
| R03 | U06 | Operation式のbounded shellをSettingsへ適用し、header/body/footerを分離 | C03 |
| R04 | U03 | 履歴bodyへScrollArea/Scrollbarを接続。更新中の可視通知基準とclampを実装 | C04 |
| R05 | U04 | catalog presenterを選択/確認表示に分離し、固定footerのConfirm/Back、session/capability・request発行中の重複抑止を接続。保存request時→書込直前の既存revision照合を維持 | C05 |
| R06 | U01 | 20件ページstate・範囲計算・前後/端ボタン・本文scrollを追加。旧window高ベースのresident上限を置換し、不要resource/helperを残さない | C06 |
| R07 | U05 | ContextMenu存在のsnapshot、CloseContextMenu binding、早期return前のclose consumer、同frame要求破棄を追加 | C07 |
| R08 | U08 | Stockpile/Yardの共通plan、最新条件の再検証、cursor喪失を含むrelease cleanup、typed理由表示を実装 | C08 |

第一段階の完了はR01〜R08、C01〜C08、当該Help review、`verify`、storage checkのpassで判定する。
O01〜O05やM3以降の未着手はこの受入を妨げないが、全体計画のM8完了とは報告しない。

### M0: 着手基準と受入シナリオの固定

- M0-A（着手基準）: HEAD/diff/ownerとC01〜C08のfixtureを固定する。§4の状態表と方式を採用し、変える必要がある場合だけ根拠を追記する。
- M0-B（実入力受入）: §7の新UI recipe、X11入力bridge、受動observer、独立verifierを実装する。既存のnative helperからlauncher・window所有・監視を再利用し、capture部分はSave専用validatorから分離して共通化する。
- 対象ファイル: 本計画、関連提案、該当単位の実装と§7で明記した新設予定driver。M0-Bもコード変更なので通常のHelp review・品質gateを適用する。
- 完了条件:
  - [ ] U01〜U08に再現手順・期待状態・実機観測要否を割り当て、静的確認と実機確認を混同しない。
  - [ ] viewport/scale、200 Task、64通知、8×8配属対象、保存/設定fixtureの条件を固定する。
  - [ ] §4のfocus/activation/失効契約を各工程の回帰ケースへ割り当てる。テスト実装は該当Mで行い、M0の完了にM3/M5の機能実装を要求しない。
  - [ ] M0-Bで実入力→UI→domain結果→client captureの一連を通し、入力不能・別window・古いcheckpointをpassとして受理しない。
- 検証: `python3 scripts/dev.py validation check`。source/featureが同じ直前の品質gateは基準として参照し、差分なしの全ビルドをbaseline目的だけで繰り返さない。

### M1: 表示状態・確認・scrollの小さい修正（U02・U03・U04・U06・U07）

- 変更内容: active tab/minimizedから本文を一意に導出、通知scroll入力、save/load確認文、Settings viewport制約、Tooltip実時間。
- 主な変更ファイル:
  - `crates/hw_ui/src/list/minimize.rs`、`crates/hw_ui/src/setup/entity_list.rs`、`crates/hw_ui/src/panels/task_list/interaction.rs`
  - `crates/hw_ui/src/notifications/ui.rs`、`crates/hw_ui/src/setup/settings_panel.rs`
  - `crates/bevy_app/src/systems/save/catalog_present.rs`、`crates/bevy_app/src/systems/save/catalog_ui.rs`
  - `crates/bevy_app/src/interface/ui/interaction/tooltip/mod.rs`、`crates/hw_ui/src/interaction/tooltip/system.rs`
  - `crates/bevy_app/src/interface/ui/notifications/native_acceptance.rs`（U07の一時unpause回避を撤去）
  - `docs/entity_list_ui.md`、`docs/notifications.md`、`docs/save_load.md`、`docs/settings.md`
- 完了条件:
  - [ ] Entities/Tasks×最小化/展開×検索あり/なしで、本文は最大1つ、検索対象が表示と一致する。
  - [ ] 通知64件の末尾へ到達し、背後のcameraが動かない。
  - [ ] save/load確認で対象・結果・確定/戻るが分かり、連打と古いsessionを拒否する。保存request後のrevision変更は既存の書込直前照合で拒否する。
  - [ ] 小画面・最大scaleでSettingsの全項目とCloseに到達する。
  - [ ] Pause/1x/2x/4xで同じ実時間後にTooltipが出る。
- 検証: §7共通gate、表示状態/保存確認の回帰テスト、対象画面のactual-window受入。

### M2: 到達性と入力・配置の整合（U01・U05・U08）

- 変更内容: Task表示範囲とstableな操作対象、最上位ContextMenuのEsc消費、Zone preview/commitの共通判定とrelease後reset。
- 主な変更ファイル:
  - `crates/hw_ui/src/panels/task_list/`、`crates/bevy_app/src/interface/ui/panels/task_list/`
  - `crates/bevy_app/src/input_actions/`、`crates/bevy_app/src/interface/ui/panels/context_menu.rs`
  - `crates/bevy_app/src/systems/command/zone_placement/`、`crates/bevy_app/src/systems/command/area_selection/indicator.rs`
  - `docs/task_list_ui.md`、`docs/world-selection.md`、`docs/state.md`、`docs/building.md`
- 完了条件:
  - [ ] 取消可能な手動指定200件の先頭/中央/末尾を表示・選択・取消できる。READ_ONLY対象の制限を維持する。
  - [ ] filter/sort/page/対象消滅で操作先がずれず、Task行は最大20件、groupは最大20個。toolbar/footerは本文scrollで消えない。
  - [ ] 通常モードのFamiliar右クリック→Escはメニューだけを閉じ、命令/目的地を変えない。
  - [ ] 同じworld条件でStockpileの採用セルと生成結果、Yardの新bounds/追加面積が一致する。条件変更・cursor喪失を含む失敗release後は `ZonePlacement(_, None)` に戻る。
  - [ ] M1＋M2を第一段階として受入し、未着手の後続項目と区別して結果を記録する。
- 検証: §7共通gate、stable key/入力優先順/Zone状態遷移の回帰テスト、全行到達とpointer操作のactual-window、Task更新量の計測。

### M3: 検索・配属・詳細を一貫させる（U11〜U18）

- 変更内容: 全Soul検索、Entities全体scroll、可視順Tab、作業分類、選択/詳細/pin/camera分離、直接filter、固定header、取消確認統一。
- 主な変更ファイル:
  - `crates/bevy_app/src/interface/ui/list/`、`crates/hw_ui/src/list/`、`crates/hw_ui/src/setup/entity_list.rs`
  - `crates/hw_ui/src/panels/task_list/`、`crates/bevy_app/src/interface/ui/panels/task_list/`
  - `crates/hw_ui/src/panels/info_panel/`、`crates/bevy_app/src/interface/ui/presentation/`、`crates/bevy_app/src/systems/ui_domain_commit.rs`
  - `docs/entity_list_ui.md`、`docs/task_list_ui.md`、`docs/info_panel_ui.md`、`docs/soul_energy.md`
- 実行順: U11/U12 → U13、U14、U15 → U16/U17/U18。Taskの操作対象はpinと独立に保持し、action barの生成条件も合わせて変える。
  Tab候補は検索/折りたたみ後の論理的な表示対象全体から作る。現在画面内にあるNodeだけを候補にして画面外への移動を失わない。
- 完了条件:
  - [ ] 折りたたんだSoulを検索でき、解除で元の開閉へ戻る。8×8の最下部へ配属できる。
  - [ ] Tab/Shift+Tabが表示順で双方向循環し、移動先行が見える。modal内のfocusとはM5で再検証する。
  - [ ] GeneratePower/Deconstructを区別し、将来のvariant追加で誤分類へ黙って落ちない。
  - [ ] 配属dragだけでcameraが動かず、Taskをpinしなくても優先度/取消を操作できる。pin対象と選択対象を識別できる。
  - [ ] 任意Typeの選択は2操作以内、全解除は1操作。0件から復帰できる。
  - [ ] 関連情報への操作を提供するblockerを定義し、正しいFamiliar設定/資材詳細へ移動できる。対象消滅・診断更新で不適切になった操作は拒否し、未対応理由は操作可能に見せない。
  - [ ] 詳細末尾でも対象名・pin解除が見え、対象変更時のscroll規則が一定。
  - [ ] Soul Spa取消は全入口で確認し、対象変更/load/modal後に古い確認が適用されない。
- 検証: §7共通gate、ViewModel順序/対象保持/owner確認の回帰テスト、検索→配属→詳細→取消のactual-window通し操作。

### M4: 対象と操作結果を画面に返す（U19〜U22・U29・U32）

- 変更内容: Door/Room/電力の判断情報、設定値・失敗/再試行、通知の読返し、Orders担当/件数、Area履歴/preset表示。
  不成立RoomはEntityが存在しないため、grid上の検査起点とtypedな理由を表示modelへ渡す。成立Roomの詳細を増やすだけで完了としない。
- 主な変更ファイル:
  - `crates/bevy_app/src/interface/ui/presentation/`、`crates/hw_ui/src/panels/info_panel/`、`crates/hw_world/src/room_detection/`
  - `crates/hw_core/src/settings.rs`、`crates/bevy_app/src/systems/settings/`、`crates/bevy_app/src/interface/ui/interaction/handlers/settings.rs`、`crates/hw_ui/src/setup/settings_panel.rs`
  - `crates/hw_ui/src/notifications/`、`crates/bevy_app/src/interface/ui/notifications.rs`
  - `crates/bevy_app/src/interface/ui/interaction/mode.rs`、`crates/bevy_app/src/systems/command/area_selection/`、`crates/hw_ui/src/area_edit/state.rs`
  - `docs/info_panel_ui.md`、`docs/room_detection.md`、`docs/settings.md`、`docs/notifications.md`、`docs/state.md`
- 完了条件:
  - [ ] Doorのlock状態と操作、電力停止理由・優先度、現行Roomの不成立理由が読める。domain判定をUIへ複製せず、Room再生成後の古いEntityを保持しない。
  - [ ] 設定値・単位・適用時期が一致し、保存失敗はセッションへの適用と次回用保存を区別して通知・再試行できる。
  - [ ] 通知を後から読んで対応でき、toast入力透過・上限・dedupe・load後resetを維持する。
  - [ ] Chop/Mine/Haul/Areaの自動担当選択と件数表示が実処理に一致する。Deconstructを同じ自動選択経路と誤認しない。
  - [ ] Area Undo/Redoの担当・操作、preset寸法、空状態が分かる。履歴64件と新規編集時のredo消去を維持する。
  - [ ] D06の基本仕様を満たし、O02/O03の未実装を残件と区別して記録する。
- 検証: §7共通gate、書込不能の隔離fixture、対象消滅/load、Room理由と現行domain結果の一致、複数Familiarの履歴と部分成立のactual-window。

### M5: キーボードとpointer操作を統合する（U10・U28・U30・U31）

- 変更内容: canonical shortcut、modal内focus・復帰、release確定、専用pan、重なり候補選択と右クリック対象規則。
- 主な変更ファイル:
  - `crates/bevy_app/src/input_actions/`、`crates/bevy_app/src/interface/ui/interaction/`、`crates/hw_ui/src/interaction/`
  - `crates/bevy_app/src/interface/selection/`、`crates/bevy_app/src/plugins/input.rs`、`crates/bevy_app/src/systems/settings/apply.rs`
  - `crates/hw_ui/src/setup/time_control.rs`、`docs/world-selection.md`、`docs/state.md`、`docs/settings.md`
- 完了条件:
  - [ ] 選択なし/Familiar/命令モード/各overlay/文字入力で、表示shortcutと解決結果が一致する。
  - [ ] keyboardだけで設定・保存・戻るができ、背景の一覧Tab・命令へ漏れない。IME中のEnter/Escを維持する。
  - [ ] 外へdrag/releaseした確定操作は発火しない。pressや確認表示で行を再生成しても、古いpointer対象へ確定しない。
  - [ ] BuildingPlace/Move/Zone/Areaで専用panが使え、panだけで配置/選択/範囲確定しない。初回は範囲drag中のpanを拡張しない案を優先する。
  - [ ] 重なり候補を時間制限なしで選べ、Floor/空地でのFamiliar移動と非表示対象の除外を維持する。
- 検証: §7共通gate、resolver/foreground/gestureの回帰テスト、mouse/keyboard/IMEのactual-window。U04・U05・U13・U18を再確認する。

### M6: 停止中の計画操作（U09）

- 変更内容: §4の初回allow-listを実装し、simulation pauseとsystem menuを分離する。未完gesture・Help pause・save/load順序を統合する。
  UIのcaptureだけでなくdomain側のpause拒否、autosaveのactive-play計時、RecoveryFailedでの操作制限も確認する。
- 主な変更ファイル:
  - `crates/hw_ui/src/interaction/pause_menu.rs`、`crates/bevy_app/src/input_actions/`、`crates/bevy_app/src/interface/ui/help_controller.rs`
  - `crates/bevy_app/src/systems/ui_domain_commit.rs`、必要なcommand owner、`docs/state.md`、`docs/architecture.md`、`docs/help-screen.md`
  - `crates/bevy_app/src/plugins/game.rs`、`crates/bevy_app/src/plugins/logic.rs`、`crates/bevy_app/src/plugins/spatial.rs`、`crates/bevy_app/src/systems/save/autosave.rs`
- 完了条件:
  - [ ] 閲覧/camera/採用した計画操作が停止中にでき、Soul移動・建設進捗・資材消費は進まない。
  - [ ] 停止中の入力はownerまで完了し、更新した計画状態を再開時に再送しない。指定の重複・Messageの期限切れ・load後の旧対象を残さない。
  - [ ] Helpを閉じてもユーザー自身の停止を解除しない。system menuと各modalのcaptureを維持する。
  - [ ] 計画停止中はautosaveのactive-play時間を加算しない。RecoveryFailed中の復旧専用操作制限を緩めない。
  - [ ] O05の操作と未分類world変更は理由付きで拒否し、入口だけ許可された実行不能な操作を公開しない。
  - [ ] 停止直前/停止中に更新された対象を、次の選択・一覧・範囲queryが最新indexから読める。進行するsystem群を再開していない。
- 検証: §7共通gate、変更を許可する計画状態と凍結する進行状態を分けたsnapshot、commandの適用回数、Help/menu/save/loadとの遷移、actual-windowで停止中の計画→再開。

### M7: 発見性・読みやすさ・導入案内（U23〜U27）

- 変更内容: Help検索/文脈entry/読書位置、save一覧の到達性と既存日時による識別、狭幅layout/用語、mode案内、任意開始の初回ガイド。
- 主な変更ファイル:
  - `crates/bevy_app/src/interface/ui/help_controller.rs`、`crates/bevy_app/src/interface/ui/help_content/`、`crates/hw_ui/src/help.rs`、`crates/hw_ui/src/interaction/help.rs`
  - `crates/bevy_app/src/systems/save/`、`crates/hw_ui/src/setup/dialogs.rs`、`crates/hw_ui/src/setup/bottom_bar.rs`、`crates/hw_ui/src/setup/submenus.rs`、`crates/hw_ui/src/theme.rs`
  - `crates/bevy_app/src/interface/ui/interaction/mode.rs`、`docs/help-screen.md`、`docs/fonts.md`、`docs/save_load.md`、`docs/settings.md`
- 完了条件:
  - [ ] Helpを開き直すと読書位置へ戻り、検索/「詳しく」からstable entryへ進む。既存sealed schemaとcoverage検証を維持する。
  - [ ] 最大世代・長文のsave一覧をslot区分と絶対日時で比較でき、候補表示だけではworldを変更しない。既存の有界読取と形式互換を維持する。
  - [ ] 小画面・拡大・長い日本語で全操作へ到達し、重要情報を文字/形でも識別できる。
  - [ ] mode案内の数量・費用・段階が実planに一致する。Area Undoを全ゲーム操作へ広げた説明をしない。
  - [ ] 初回ガイドはHelpから任意に開始/終了でき、実状態で達成判定する。既存プレイを強制中断しない。
  - [ ] D05/D06の基本仕様を受け入れ、O03/O04を未実装の後続候補として記録する。評価未実施の操作時間や可読性を改善済みと報告しない。
- 検証: §7共通gate、Helpの検索/復元/coverage、save互換、viewport matrixのactual-window。初回案内の効果は達成時間・誤操作・詰まり箇所を記録して評価する。

### M8: 統合受入・仕様同期・終了

- 変更内容: 採用した各項目を横断して受入し、仕様とHelpを正本へ集約する。実装の未完了と任意拡張の保留を分けて残す。
- 完了条件:
  - [ ] §7の統合matrixがpassし、U01〜U32の対応表に実装/採否・検証結果を記録する。
  - [ ] `check`・Clippy警告0・workspace test・`verify`、rust-analyzer診断、Help review、storage checkが確定している。
  - [ ] 診断用code/material/設定を撤去し、未検証範囲と残件を正確に記載する。
  - [ ] 文書へ採用結果を同期し、計画終了時はarchiveまたは削除して両索引を再生成する。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| page/filter/行再生成でEntity・確認対象が入れ替わる | 別対象へ取消/優先度変更 | Entity＋epoch、action時の再検証、選択/確認の失効条件を共通化 |
| 自動pin撤去で操作欄も消える | Task管理機能が後退 | U15でaction barの表示条件も変更し、非pin時の操作を受入に含める |
| release確定時に元widgetが消える | 確定不能・誤確定 | stableなactivation対象とcancel規則、行再生成/対象消滅ケースの検証 |
| UI focusと世界操作が競合する | 背景命令・誤配置 | resolver/captureを単一入口にし、受理frameから抑止 |
| pause分離が進行・save順序を壊す | 消費/二重適用/古い予約 | command副作用表、適用回数、load/recovery resetを先に定義 |
| 大量行や検索で毎frame再生成する | CPU/割当負荷の増加 | dirty/revision、有界行、hidden/visible/filter別の計測 |
| 任意機能で第一段階が膨らむ | 到達性の修正が遅れる | M1/M2を独立受入し、任意拡張は採否付きの別単位へ分離 |

## 7. 検証計画

### 共通gateとHelp更新

Rust変更の各まとまった単位で、対象の状態遷移・拒否経路を検証する。単なる色/余白変更や実装の写しとなるテストは増やさない。

```bash
python3 scripts/dev.py check
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
python3 scripts/dev.py verify
python3 scripts/dev.py docs --check
python3 scripts/check_help_impact.py
git diff --check
```

`verify`が実行したworkspace test/Clippyを同じ変更なしに重ねて走らせない。focused testは変更した契約を対象にする。
Rust変更はrust-analyzer診断も確認する。API取得不成立を0件と記録しない。

**Help更新はM7まで待たない。** 各実装単位で `hell-workers-review-help-impact` Skillにより入力→成立条件→結果→表示を確認する。
操作・文言・shortcut・設定・失敗理由が変わる単位はUpdate requiredを基本とし、root manifest/provider/coverageとexact snapshotを同じ差分で更新する。
shortcutはcanonical bindingから生成する。snapshotは専用generatorで生成して全差分を読み、通常testをwriterへ変えない。
純内部変更のNo impactは、その変更範囲で理由を改めて記録する。

### actual-window受入と性能

実行時は `hell-workers-run-native-acceptance` Skillを読み、primary coordinatorへ登録したno-prompt launcherを使う。
修正中は同じworkspace/feature/targetのfeedbackを利用し、候補が固まった時点で必要な正式受入へ進む。

#### 第一段階のC01〜C08

テスト用seedは `20260914`、save/settingsはjob内の隔離rootに固定する。変更前の不具合を再現するlayerと変更後のpassを記録し、既存のplayer dataをfixtureにしない。
「回帰」は純粋関数・production schedule・observerの検証、「実入力」はM0-BのOS入力とclient captureを意味する。

| ID | 回帰で固定する条件 | 実入力の手順と期待結果 |
| --- | --- | --- |
| C01 / U02 | §4の4状態、検索有無、capture中pressの破棄。minimizeとvisibilityをproduction順序で複数frame実行 | Tasks→最小化→同タブ展開、最小化中のタブ変更、検索後のタブ往復。本文の描画/クリック領域は最大1つ、TasksにSoul検索は出ず、検索解除後の元状態へ戻れる |
| C02 / U07 | TimePluginの手動real advanceでPause/1x/2x/4x。delay未満は非表示、delay/fade後は表示。実時間step1個以内の差 | 停止の伝播後に未hoverの説明付きbuttonへpointerを入れる。unpauseを挟まず表示され、別buttonへの移動も同じ待ち時間。入力/表示時刻とframe時間を記録し、速度間の差は描画2frame分以内 |
| C03 / U06 | Settingsのheader/footerがscroll body外、Scrollbarの対象がbodyであること | 1280×720 / 1920×1080 × scale 0.85/1/1.25の6条件で末尾とCloseへ到達。実際のSettings操作でscaleを1.25へ変更後、元へ戻す。長い日本語fixtureで文字/操作がclipされない |
| C04 / U03 | 異なるkeyの重要通知64件、Line/Pixelの標準ScrollArea observer、追記/追い出しのanchor/clamp | 通知buttonを押す→wheelで最古へ→thumbで最新へ。先頭/中央/最古の固定IDを画像と表示modelで一致確認。camera transform/projection・world選択は不変。Pixel observerのpassを実trackpadの証明にはしない |
| C05 / U04 | 空slot直接保存、確認のowner/session、保存request後→書込直前のrevision変更拒否、requestの多重発行抑止 | occupied slotを同じ座標でdouble-clickしてもtransactionは0。確認画面を撮影し、明示Confirmで1回だけ実行。Back→再open後の旧session Confirmは回帰側で拒否。Load/Recoveryの各ownerを確認 |
| C06 / U01 | 0/1/20/21/200件のpage範囲・clamp、filter/sort reset、対象失効、world reset | 未配属で自動進行しない取消可能な手動指定200件を使い、1/5/10ページの先頭・末尾を選択/取消。最後のpageで件数減少後も空pageへ取り残されない。行数≤20、pin対象と操作先、READ_ONLY拒否も確認 |
| C07 / U05 | resolverのmodal→ContextMenu→mode/menu順、close consumerの早期return、古いopen要求破棄 | 通常Familiarの命令/目的地を記録し、右クリック→Escでmenuだけ消える。Helpを重ねた場合は最初のEscでHelpだけ閉じる。連打・同frame inputを回帰側でも確認 |
| C08 / U08 | Stockpile/Yard各plan、条件変更、cursor喪失、capture、逆方向/0面積の正規化、release reset | Stockpile: Yard外、複数Yardに跨る有効範囲、16セル中4不可、全不可。Yard: 有効拡張、Site/他Yard重なり、変更なし。採用セル集合または新boundsを照合し、失敗後に枠がcursorへ追従しない |

C03以外の基本操作は1920×1080/scale=1で実行し、最も厳しい1280×720/scale=1.25でもC01/C04/C05/C06を繰り返す。
高DPIは実機のscale factorと物理/論理寸法を明記して追加する。UI scale変更やscale-factor overrideだけの検証は実OSの高DPI確認と区別する。
環境上未実施のDPI/trackpad条件は結果に残す。画像や機能のpassを使って、その未実施条件も通過したとは報告しない。
P1の基本受入は上記6viewport条件のX11操作を対象とし、高DPIの正式な表示受入はM7で行う。実trackpad/Wayland対応の証拠は初回recipeの完了条件へ含めない。

#### 既存検証の再利用先

| 対象 | 既存の入口 | 追加・変更する点 |
| --- | --- | --- |
| U02/U01 | `hw_ui/src/panels/task_list/interaction.rs` の `captured_row_press_is_drained_without_delayed_focus`、`captured_toolbar_press_is_not_applied_after_capture_ends` | 既存capture契約を保ち、page/minimize/searchのproduction scheduleと新stateのworld resetを追加 |
| U03 | `hw_ui/src/notifications/ui.rs` の `toast_descendants_are_pick_through_and_unchanged_frames_keep_rows` | 64件とLine/Pixel observer、scrollbar、追記/追い出し時の位置保持を追加 |
| U04 | `bevy_app/src/systems/save/catalog_ui.rs` の `load_confirmation_returns_to_its_catalog_owner`、`catalog_owner_must_match_recovery_mode_before_issuing_load`、`bevy_app/src/systems/save/saving.rs` の `exact_recheck_rejects_changed_revision` | presenterの専用確認・無効slot・明示Confirm・多重発行を追加。既存owner/revision判定を置換しない |
| U06 | `hw_ui/src/setup/dialogs.rs` の `operation_dialog_uses_bounded_scroll_and_every_work_type_row`、`operation_dialog_centers_a_percentage_bounded_shell_without_fixed_offset` | Settingsへ同じ構造契約を適用し、実入力でscaleを戻すケースを追加 |
| U07 | `bevy_app/src/interface/ui/notifications/native_acceptance.rs` のTooltip表示段階 | 既存のunpause回避を撤去し、TimePluginによる新規hover回帰とC02を追加 |
| U05/U08 | `bevy_app/src/input_actions/cancel.rs` の `capture_rollback_clears_only_drag_payload_variants`、`escape_cancels_zone_placement_to_normal` | ContextMenu closeの優先順、cursorなしrelease、typed planの同条件一致を追加 |

ここでも短縮コードpathは `crates/` 配下。表のtest名は既存入口であり、追加ケースが既に実装済みという意味ではない。
focused testは `python3 scripts/dev.py cargo -- test -p hw_ui <filter>` または `python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 <filter>` で実行する。
新しいtestを追加した単位は、そのtestが選択されていることを出力で確認し、0件実行をpassとして数えない。

#### M0-Bで新設するUI実入力recipe

以下の4ファイルを実装依頼後に新設した。現在のrecipeはnavigation/layoutのfeedback用であり、
表に示すC01〜C08全体の受入はまだ完了していない。保存/ContextMenu/Zoneの実入力ケースを今後追加する。

| 新設予定 | 責務 |
| --- | --- |
| `.codex/skills/hell-workers-run-native-acceptance/scripts/ui_usability_acceptance.py` | C01〜C08のplan/run/status/verify、既存launcher・resource guard・PID所有client captureの再利用、独立検証 |
| `scripts/native_ui_input.py` | X11/XTestでpointer移動・button・wheel・keyboardを送る小さいbridge。対象PID/window/focusと送信の対応を記録 |
| `crates/bevy_app/src/interface/ui/native_acceptance.rs` | profiling専用fixtureの準備と、操作結果/対象矩形/epoch/frameを読む受動observer。通常プレイでは無効 |
| `scripts/tests/test_native_ui_input.py` | 別PID/focus喪失/古いnonce/timeoutで送信を止めること、押下状態の終了処理をmockで検証 |

bridgeはPython標準ctypesからシステムのX11/XTestを使う方針。2026-09-14時点で `libX11.so.6` と `libXtst.so.6` の存在は確認したが、稼働displayのXTEST拡張・focus取得・実入力は未検証である。
M0-Bのpreflightでdisplay接続、拡張、既存の `xprop` / `import`、開始したprocess配下のclientを確認する。初回recipeはX11のみとし、Waylandや実trackpadに対応済みとは扱わない。
入力が使えない場合はその理由でnative結果を不成立とし、UiIntent/Interactionの直接代入へ切り替えてpassさせない。

手順は `fixture ready → 対象矩形/epochを観測 → OS入力 → production結果を観測 → client画像取得 → checkpoint ACK`。
fixture投入後に検証driverが `Interaction`、`ScrollPosition`、`MenuState`、Tooltip alpha、Confirm済み状態を書き換えることは禁止する。
受動observerはfixtureの期待値から独立して実値を出力し、verifierが固定fixture/操作列の期待値と比較する。PASS bannerだけで判定しない。
画像の対象欠け・遮蔽はclient画像を確認し、矩形のviewport内判定だけで読みやすさを合格にしない。

各操作でrun nonce、step ID、window ID、owner PID、source/harness hash、world epoch、入力時刻、前後frame、対象key/矩形、観測値、画像pathを対応付ける。
送信済みstepを再送しない。ACK欠落は再試行で操作を重複させずtimeoutとして不成立にする。focus/ownerが変わった場合は新規入力を止め、押下中の入力を終了処理して失敗を記録する。
既存 `native_acceptance.py` の `x11_client_windows_for_process_tree` を再利用する。`take_native_screenshot`は内部の `finalize_native_screenshot` がSave専用markerを要求するため、そのまま他画面へ呼び出さない。
PID所有clientの取得・画像capture部分を共通化し、UI recipeではstep/画面ごとのvalidatorを接続する。既存Save marker検証を弱めず、root desktopや別windowの画像へfallbackしない。
新helper/bridgeは `NATIVE_HARNESS_FILES` と `scripts/perf_tool/execution.py` の対応するfingerprint一覧へ加え、Skill同期・既存self-testも確認する。
sourceと入力driverを固定した後、primary validation coordinatorからplanを登録し、返されたkitty launcherだけを実行する。独立verify後にseal/finalize/checkを通す。

#### 全体の統合matrix

| シナリオ | 対応 | 成功の証拠 |
| --- | --- | --- |
| 2タブ×最小化/展開×検索 | M1/M3 | 表示本文と検索対象、scroll位置、操作先が一致 |
| Task200・通知64・8×8配属 | M1〜M3 | 先頭/中央/末尾への入力と対象確認。Task取消/最下部配属も実操作 |
| save/load/取消/設定失敗 | M1/M3/M4/M5 | fixtureで確認表示、外release、連打、対象失効、失敗通知・再試行 |
| Zone/Orders/Area | M2/M4/M5/M7 | 全不可/一部可/境界、担当・適用件数、失敗release、履歴対象が一致 |
| 時間/foreground | M1/M2/M5/M6 | Pause/1x/2x/4x、Help/menu/Settings/save、Esc、IME、背景抑止 |
| 表示matrix | M1/M3/M5/M7 | 1280×720 / 1920×1080 × scale 0.85/1/1.25。高DPIは実際のscale factor・論理/物理寸法を記録 |
| キーボード/重なり/pan | M5 | focus可視化・復帰・Tab循環、候補選択、配置へ漏れないpan |
| 停止中の計画→再開→load | M6 | 許可した計画状態だけ変更し、Soul移動・建設進捗・消費は不変。入力の再送なし、複数回指定/index反映、load後の旧対象消去 |
| Help→試す→Help/初回ガイド | M7 | 読書位置とentry復元、skip、実状態による達成 |

既存Task Dashboard recipeはhidden/visible/active-filterのCPU/割当評価に利用する。pixel layout・pointer操作の証拠には使わず、専用actual-windowシナリオで確認する。
通知recipeの既存3件fixtureは64件の末尾到達を証明しない。save recipeも新確認UIの入力経路は追加観測が必要。
不足する実入力driver/verifierはM0-Bで実装し、後半の操作ケースは該当機能と同時に拡張する。headlessのpassだけで実機受入を閉じない。
性能が変わる一覧/検索は同一subject条件で更新時間・常駐行/node数・割当を比較する。Capture→Memoryを逐次実行し、独立artifact検証で実adapter/backendと計測有効性を確認する。
短いsmokeから改善率を算出せず、正式な性能比較が必要な場合は既存の30秒warm-up/60秒measure規則を使う。
P1ではTask行20件の上限、hidden/minimizedの描画再構築0、未変更frameの再構築0を数値gateにする。
現在の `render_visible_rows` はfilter後の全件数を数えるため、これを常駐行数と混同せずresident行/node数を別に観測する。
CPU/Memoryの既存recipe gateは維持する。新旧の性能差を評価する場合は同条件の比較と既存の回帰許容値を計測前に固定し、結果を見てから緩めない。

### 検証データ管理（各バッチの開始前・報告前に更新）

正本は [validation-storage-workflow.md](../development-infra/validation-storage-workflow.md)。開始・再開時に読む。

- primaryの `python3 scripts/dev.py validation` で保持/plan/execute、seal/finalize/checkを行う。台帳はGit common directoryに置く。
- 旧subjectのsource/asset/binaryが比較に必要なら、着手前にその依存を登録して凍結する。凍結checkoutへ新helperや文書をコピーしない。
- 成功・失敗・中断とも結果を確定し、用途のないjob/raw/binary copyを整理する。全jobのarchive/capsule保存は不要。
- feedback中は同じcandidateとCargo targetを維持する。無応答やreview日付を理由に破棄しない。通常primary Cargo cacheは別の保守寿命を持つ。
- 最終closeで本計画のconsumerを解除し、残る利用者がなければ専用環境を撤去する。共有資源の保持は具体的な別consumerを記録する。

| 記録項目 | 現在の実装・検証時点 |
| --- | --- |
| batch ID / 判断対象 / owner / consumer | `ui-feedback-20260914-4` / P1入力経路診断 / `ui-usability` / `ui-usability-implementation` |
| worktree/clone/branch / subject/asset view / job roots | 同じprimary。HEAD `a4051f9b`＋今回の未commit source。診断job: `target/native-acceptance/ui-usability-feedback-20260913T193322Z-4fcf8876` |
| 開始時bytes / 結果の正本 | 本計画専用の検証出力0 bytes。結果は本計画§9、静的調査は関連提案 |
| 削除path / 前後bytes / filesystem空き差 | feedback 1〜3の専用jobのみ撤去（69,632 / 98,304 / 3,588,096 bytes → 各0）。filesystem空き差は未測定 |
| 残存path / bytes / owner / consumer / next action / release_when | `ui-usability-primary` holdでprimaryを登録。開始時実測255,425,888,256 bytes（既存通常Cargo cache等を含む、新規生成量ではない）。同じ環境で修正/受入を継続、UI改善の対応終了時にconsumer解除。primary自体は削除しない。他作業cacheは維持 |
| review状態 / 最新提示・修正日時 | 実装中、2026-09-14。feedback 1〜4はinvalid、受入checkpoint 0。OS入力経路を確認中 |
| 整理状態 | batch終了時に結果をsealし、現在の修正/画像確認に不要なjobを整理してfinalize/checkする |

## 8. ロールバック方針

- M全体ではなく対応表の修正単位で戻せる差分にする。入力/表示/Helpは同じ単位で整合を戻す。
- save/settings schemaを拡張する場合は旧形式の読込を維持し、新形式を書いた後の旧binary起動条件を先に確認する。検証fixture以外の保存を上書きしない。
- revert前に `git log --oneline -5` と対象の `git diff HEAD -- <file>` を読み、並行作業の差分が含まれていないことを確認する。未確認の差分を破壊的に戻さない。
- 仮説検証の変更は確認/否定する条件を明記し、変化がなければ同じ調整を繰り返さない。診断用code/material/設定は切り分け後に撤去する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 計画の自己レビュー・具体化: 完了。実装着手済み、受入完了の項目はまだない。
- 着手基準: HEAD `a4051f9b`、開始時production差分なし。既存の計画・提案・索引差分を保持し、primaryで主担当が編集。
- 作業中: U01〜U08の初期コードとHelp本文/coverage/snapshotを変更。M0-BのXTest bridge・受動observer・feedback recipeを新設。P1の追加回帰/実機検証、M3以降は残作業。
- 検証経過: アプリ通常構成653 passed / 2 ignored、hw_ui 69 passed。その後のresize追加回帰1 passed、profiling all-targets checkもpass。全体の最終verify・actual-window受入を意味しない。
- 現在のHelp判断は **Update required**。タブ/ページ/通知/保存確認/Settings/Tooltip/ContextMenu/Zoneの到達可能な操作変更を既存entryへ反映し、新しい操作surfaceをcoverageへ追加。snapshot全差分を確認済み。
- feedback 1: ビルドと200 Task fixture準備は成功。最初のpointer移動後にfocusが外れ、button送信前に停止。checkpointは0件で不成立。OS倍率2で論理1920×1080が画面制約により物理2880×1662となった。基本matrixは明示scale-factor override=1と物理寸法指定へ修正し、OS base倍率も記録する。これは高DPI受入の代用ではない。
- feedback 1はinvalidとしてseal/finalize済み。専用job（`ui-usability-feedback-20260913T191358Z-1846ec72`）の生成物のみ撤去し、allocated 69,632 bytes→0。通常Cargo cacheと他作業の保持物は維持。入力bridgeをWM activation要求へ修正し、同じcandidateでfeedback 2を準備中。
- feedback 2: WM activation後の3入力（move/press/release）はfocusを維持して送信できたが、Tasksの表示を確認できず不成立。起動後のscale override変更でBevyのresize処理が960×540へ再計算していたため、確認用寸法をWindow生成前へ移した。カーソルとbutton状態の受動記録・失敗時client画像を追加しfeedback 3で入力経路を確認する。
- feedback 2もinvalidとしてseal/finalize済み。専用job（`ui-usability-feedback-20260913T192328Z-d6f4c1c5`）を撤去しallocated 98,304 bytes→0。既存Save marker検証を維持したままclient PNG capture部分を共通化し、helper self-testはpass。
- 次工程: 通知anchor消滅時のclampを確認し、全verifyの結果を記録する。OS入力経路の回答を確認後にM0-Bの実入力受入を再開する。C01〜C08の未実装recipeを補い、U15〜U18/M4以降へ進む。
- 32項目を一括実装せず、M1/M2のP1全8項目で第一段階を受入する。
- 計画整備時のNo impactは文書編集に限る過去の判断。現在のUI実装は上記Update requiredで扱う。

### 次のAIが最初にやること

1. primaryでREADME/開発規則/本計画/関連提案/変更対象の仕様書、Git status/diffを確認する。
2. M0-Aでsource基準とfixtureを確認する。D01〜D06と状態契約を読み、M0-Bの新設予定ファイルを既存実装と誤認しない。
3. 対応表の先行条件を満たす単位を主担当が編集し、Bevy 0.19一次情報、Help review、§7の検証を同じ単位で行う。

### ブロッカー/注意点

- 計画作成のブロッカーはない。D01〜D06の初回方式を固定済み。O01〜O05は理由付きの後続候補であり、P1着手の一括承認待ちではない。
- M0-Bの実入力driverは作成済みだが、X11/XTEST preflightと実入力受入は未実施。ライブラリの存在だけを実入力受入済みと解釈しない。
- productionの修正中。実画面・GPU・native allocatorの新しい証拠はまだない。
- code編集をsubagentへ委譲しない。他作業のDoor/Wall/HVAC成果物・cacheを変更しない。

### 参照必須ファイル

- [関連提案](../proposals/ui-usability-audit-proposal-2026-09-13.md)、[不変条件](../invariants.md)、[crate境界](../crate-boundaries.md)、[Help](../help-screen.md)
- [状態](../state.md)、[Tasks](../task_list_ui.md)、[Entities](../entity_list_ui.md)、[詳細](../info_panel_ui.md)、[選択](../world-selection.md)
- `crates/bevy_app/src/input_actions/`、`crates/bevy_app/src/systems/ui_domain_commit.rs`、各Mに列挙した変更対象

### 最終確認ログ

- 2026-09-14、計画整備前の同一productionコード: `python3 scripts/dev.py verify` pass。workspace check、Clippy警告0、profiling/通常構成testを含む。これはUI改善後の受入ではない。
- 同コードのrust-analyzer個別診断: UI plugin入口 `core.rs` のerrors/warnings/hintsは0。workspace診断APIは `Unexpected response format` で取得不成立。
- 計画整備時は文書のみの追記のため、確認済みコードの全ビルドを繰り返さず、文書索引・Help影響・差分・storageを検査した。
- 2026-09-14、計画書整備後: `docs --write` / `docs --check`、`check_help_impact.py`、`git diff --check`、`validation check`はpass。
- 初版検証では対応表のU-IDは32/32で重複なし、依存graphに循環なし、45件のsource pathの存在を確認した。
- 自己レビュー後: `docs --write` / `docs --check`、`check_help_impact.py`、`git diff --check`、`validation check`はpass。U-IDは32/32で重複・依存循環なし、R/U/Cの8組が一致。既存source/script参照56件と既存test名10件を照合し、新設予定4ファイルが未作成であることを確認した。test名の照合はtest実行ではない。
- **実装前の計画整備時のHelp判断: No impact。** 計画・提案・索引のみを編集し、実入力→UiIntent→root/domainの処理と `build_help_panel_content`→静的Help treeの内容は不変だった。runtimeは `docs/*.md` を読み込まない。現在のUI実装にはこの判断を流用しない。

### Definition of Done（UI実装の完了条件）

- [ ] U01〜U32の初回仕様とマイルストーンが完了し、O01〜O05を未実装の後続候補として記録
- [ ] 各単位のHelp review、仕様書・索引の同期が完了
- [ ] `check`、Clippy警告0、workspace test、`verify`が成功し、Rust診断を確認
- [ ] 対象actual-windowシナリオと必要な性能/Memory受入が完了
- [ ] 各batchの結果確定・不要出力整理・storage checkを報告前に実施
- [ ] 最終close時に本計画のconsumerは0。共有残存は別consumer・owner・bytes・release_whenを引継ぎ、削除path/実測容量差と最終結果を正本へ集約

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-14 | Codex | 提案32項目をM0〜M8へ割当。P1第一段階、設計判断、既存計画との境界、受入・Help・storage・引継ぎを整備 |
| 2026-09-14 | Codex | 自己レビューで8件の計画上の不足を修正。方式D01〜D06、状態/失効/Zone/pause契約、R01〜R08とC01〜C08、M0-B実入力recipe、後続候補O01〜O05を具体化。保存revisionと既存captureの保証範囲も明記 |

### 追加の実装・検証記録

- feedback 3: Window生成前の倍率指定で実clientとrendererを1920×1080に統一。入力要求後もBevy cursorが変わらずinvalid。診断を4へ引き継ぎ、jobを撤去（3,588,096 bytes→0）。
- feedback 4: XTest直後のXQueryPointerもclient座標401,568のまま。15秒の到達待ちが失敗し、button送信前に停止。UIのhit testより前の未到達を確認した。OS側の許可画面の有無はユーザーへの確認待ちで、許可拒否とは断定しない。
- feedback 4はinvalidとしてseal/finalize済み。上表のjobをhold `ui-input-diagnostic-4`（owner `ui-usability`、consumer `ui-input-path-diagnosis`、3,686,400 bytes）で保持。次は入力経路の確認、診断終了/打切り時に所見を本書へ残して撤去する。storage check pass。
- 追加回帰: Zone placement 6 passed（cursor喪失/capture中releaseでdrag終了を含む）、Tooltipの新規hoverを停止/1倍/2倍/4倍で比較した実systemテスト1 passed。bridge 5 passed。直前のworkspace Clippyは0 warnings、その後の追加変更は再検証する。
- OS入力確認と独立に進められるM3/U11へ着手。検索時に折りたたみ前の全Soulを対象にし、一致groupを一時展開、解除時に元のfoldへ戻す。M1/M2受入の完了扱いはしない。

- M3追加: U11の折りたたみ検索回帰1 passed。U12の単一ScrollAreaと検索欄固定の構造テスト1 passed。配属待ちをTime<Real>へ変更し、drag中の上下端scrollを接続。U13のVM表示順巡回・逆方向wrap・移動先scrollを実装、一覧非表示時は巡回しない。実画面の8×8配属とTab scrollは未受入。`dev.py check` pass（その後に非表示guardとHelpを追加）。

- U14: AssignedTaskの全variantを明示matchし、発電・解体・移設・精製・骨回収を分離。行に作業名を併記して値更新も同期する。Help既存entryへU11〜U14と端scrollを追記、snapshot再生成後の全差分をレビュー済み。
- Settingsのheader/Close固定・全controlのscroll所属を検証する回帰を追加。初回はfixtureのAssetServer不足で失敗したが、AssetPlugin/ScenePluginを加えて1 passed。製品実装の回避策は追加していない。
- workspace Clippyは追加変更でtest module位置の警告を検出し、テストをファイル末尾へ移動して再実行pass（0 warnings）。rust-analyzer workspace診断APIは形式エラーだったため変更ファイルを個別診断し、全通常構成ファイルでerror/warning 0を確認。profiling専用経路は全verifyでコンパイル/テストする。
- `python3 scripts/dev.py verify` 全ゲートpass。通常/profiling workspaceテスト、追加profiling featureのcompile、Clippy 0、Python/tooling/Help/docs/storage/diffを含む。hw_uiは73 passed。この後、通知anchor消滅時のraw scroll clampについて追加回帰を行うため、最終差分は再検証する。

- 通知anchor回帰で、対象通知消滅時にScrollPositionが250のまま残り上限150へ戻らないことを再現。anchorが消えた場合もclampを適用し、同じテスト1 passed。既存Helpの位置保持契約内の修正。
- M3/U15着手: 行選択/Tabから自動camera移動を除き、明示「現地へ」を追加。Taskのactive_taskをpinから分離し、pinなしで操作欄を表示。現地へは既存generic intent経路で現在Transformを再取得し、選択/pinを変更しない。新UiIntentをcoverageへ登録、Help/snapshotは次のバッチで更新する。全verify後の追加変更なので再検証が必要。

- M3/U15: Task選択/操作対象をpinから分離、明示「現地へ」/「詳細を固定」を追加。Task一覧6回帰pass、最新座標/選択pin維持/抑止/対象消滅のfocus回帰1 passed。
- M3/U17: InfoPanelを固定headerと本文ScrollArea/Scrollbarへ分離。「固定: 対象名」と「選択を表示」を本文外へ残す。別targetでscroll reset、同じtarget更新で保持する回帰1 passed。
- M3/U16: filter直接選択と全解除を実装。blockerの関連情報操作は未実装。U18の取消確認統一も残作業。

- M5/U10: Familiarの数字aliasを撤去し時間操作を統一。Idle/PatrolをIと対象付き文脈menuへ移し共通処理化。Escを閉じる/取消へ限定。入力文脈と対象保持の回帰検証中。

- U10: 入力resolver 38 passed、明示target/capture回帰1 passed。新queryのActiveCommand競合を既存ParamSetへ統合して修正。M4/U20は現在値・単位・適用時期表示を追加し検証中。

- U20: 適用済み値/正規化された単位の回帰1 passed。U15〜U17/U10/U20のHelpを再生成しsnapshot全差分をレビュー。Update required（選択・カメラ・pinの分離、直接filter、固定見出し、canonical shortcutと設定値の表示を変更）。manifestの既存owner/topicを維持。全verifyを新しい差分で実行中。

- U18: 情報パネル/Task搬入取消の入口を共通確認へ統一中。既存Taskのinline確認と同じ非modal方式で、独立したConfirm/Backと現地確認を表示する。target/epoch/ticketに選択/pin/active_taskのsnapshotを組み合わせ、変更時は確認を破棄する。world置換でstate/表示/button payloadを同時reset。ownerの資材返却処理は維持。
- 全verify再実行はprofiling root testsで旧shortcut alias期待1件が失敗（716 passed、2 ignored）。期待を新canonical bindingへ修正済み。U18追加後に改めて全gateを通す。

- U18: Soul Spa関連31 passed、world置換で確認/表示/buttonを失効させるleaf回帰1 passed。搬入タスクの参照先も確認開始/確定前に照合し、途中で配送先が変わった場合は取消を送らない。native受入は未実施。
- U21: 書込失敗の重要通知と未保存表示、履歴だけの再保存buttonを実装中。試行ticketで古い操作を拒否し、retry時には現在の設定値を保存する。reducer後のaction失効も接続。隔離された書込不能fixtureで検証中。

- U21: 隔離書込不能→現在値retry→古いticket拒否→capture拒否の回帰1 passed、toastにbuttonを置かない構造回帰1 passed。U18/U21までのClippyは0 warnings。U22の4/8/12秒設定（旧RONは4秒）と履歴経過時間のTextだけの更新を追加し検証中。

- U22: 通知suite 11 passed。Help snapshot再生成/全差分review済み。全verifyはprofiling root通過後、hw_uiの旧root-scroll期待1件で停止したため本文ScrollAreaを確認する期待へ修正。hw_ui全80 tests passed。通知時間の適用順をintent処理後/reducer前へ固定し、全verifyを再開。新しいRustファイルを含むRAはerrors/warnings 0（初期化中のproc-macro未ロード診断は再取得で解消）。

- 全verifyはprofiling全テストと各feature checkを通過後、Clippyのitems_after_test_module 1件で停止。設定関数をtest moduleより前へ移動して修正。U19として扉状態・操作案内と床起点Room診断を実装し検証中。domainとUIで判定を複製せず、同一flood-fillから失敗理由・座標を返す。Room関連15 tests passed。

- U19: Room診断15 testsと境界撤去/再設置のUI adapter回帰1 passed。無関係なTransform削除で再検査しないよう建物だけへ絞った。既存電力優先度・給電停止理由は維持。
- U29: 運搬の部分成立（対象2、適用1、受入先なし1）と使い魔不在拒否→自動担当選択の回帰各1 passed。mode担当表示とowner集計結果の重要通知を接続。Areaの配属件数も既存ownerから表示。
- U32: 画面の履歴担当/寸法・preset状態、理由付き空状態と既存shortcut consumerへのUI経路を実装し検証中。表示revision/epoch/現在のcontextを照合し、削除済み担当へのUndo/Redoは拒否する。

- U32: 既存shortcutを含む4 tests passed、U32までのClippy 0 warnings。追加Room/AreaのRAはerrors/warnings 0。
- U30: 中ボタン専用panを追加。primary gesture実行中には開始せず、pan中のprimary入力は全ボタンreleaseまで抑止。capture・focus消失・world epoch変更で再押下を要求する。純粋gesture回帰2 passed。Helpを再生成し全snapshot差分をレビュー済み。
- U16残部: 担当/運搬anchorへの情報リンクとFamiliarの停止作業種別表示を追加し検証中。集計blockerから対象を推測せず、live relationとinspect可能な型を表示時・実行時に確認する。参照変更・消滅時は開かず一覧を再更新する。

- U16: 関連づけ変更後の旧button拒否1 test passed。Helpを再生成し全差分を再レビュー済み。新しい全verifyを実行中（profiling全workspace testとmemory/tracy/renderdoc check通過、Clippy以降確認中）。

### M5/U28の設計確認・実装記録

- 検証再開時、直前の全verify実行sessionが取得できなかったため、全体passは未確定として扱う。残存Cargo processがないことを確認して編集を再開。
- M7/U24は先に独立実装: slot区分・UTC絶対日時・相対日時を一覧と確認へ共通表示し、一覧Scrollbarを追加。暦のうるう年・世紀・年境界・範囲外の回帰1 test passed。UI実画面受入は未成立。
- M7/U23: Helpの項目別読書位置、再表示時復元、見出し・本文のAND検索、stable entryへの移動、情報パネルの「詳しく」を実装・検証中。hw_ui Help 11 tests passed（追加のentry/capture回帰は次のgate対象）。固定検索文言をrootのsealed chromeへ追加し、exact snapshot生成器にも追加した。実画面での位置・focusは未受入。
- M7/U25/U26: themeの本文14/補助12pxとButtonの32px最小領域、submenuのviewport補正、主要用語対照、モード案内の折返しと次操作、床/壁の実採用planによる数量・材料費を追加。check/全verifyで統合検証中。高DPI・小画面実機matrix、操作性の効果評価は未受入。
- 上記U23〜U26の追加を含む `python3 scripts/dev.py verify` が終了code 0、`All quality gates passed`（session 25574）。profiling/通常workspace tests、追加feature check、Clippy 0、Help exact/coverage、docs/storageが通過。Help controller / hw_ui Help / floor preview / Task actions / middle panのrust-analyzer診断もerrors・warnings 0。以後の入力統合変更には再検証が必要。実機受入は未成立のまま。

- 現行capture prepassはBevy UI Focus後・Picking Hover前に走る。UI widgetsのActivate observerへ一括置換するだけでは同frame captureの順序を守れないため、Buttonのarm/releaseをこのprepassより前に収集し、既存consumerとcapture prepassが共通のactivationを読む。
- Interactionはhover/pressed描画とSoul長押しdragに残す。操作の確定だけをreleaseへ移す。MenuButtonのpayload、Task action/control、Soul/Familiar行、section、tab、max-soul target等とworld epochを照合し、押下後の対象変更・非表示・focus/capture変更・外releaseを拒否する。
- modal内のTab順は表示中・有効なButton/Slider/Checkbox/EditableTextをUI階層順で作る。InputFocusを設定し、ScrollIntoViewとfocus表示を行う。openerまたは直前focusを保存し、閉じた時に生存/表示状態を確認して復帰する。
- Enter/Spaceはmodal内で新規押下だけを採用し、同frameのゲーム用shortcutへ二重配送しない。既存slider/checkbox/text inputのBevy 0.19 keyboard処理は維持する。開いた直後の保持キーからConfirmを発生させない。
- Soul rowの長押し配属dragはactivationを失効させ、drop後に行選択を送らない。area resizeやslider dragはButton activationへ置換しない。
- 関連一次情報をローカルBevy 0.19で確認済み: UiStack.uinodesはback-to-front順、ScrollIntoViewはentity event、InteractionDisabledはbevy_ui、EditableTextはbevy_text。既存bevy::ui::Buttonとui_widgets::Buttonは別型。
- 追加確認: 標準TabNavigationはTabIndexだけで候補を集め、Nodeの非表示・InteractionDisabledを自動除外しない。このまま全Buttonへ付ける方式は採らず、前景root内の実際に表示・有効なcontrolsをUI階層順に収集する。
- keyboardもcanonical bindingから解決する方針。新規Enter/Spaceの対象・payload・epochを短いqueueに固定し、次のPreUpdateのpointer activationと共通collectorで再照合する方式なら、既存capture prepassより前に受理できる。開いた画面の新しいConfirmへ保持キーを転送しない。queueの1frame遅延をテストに明示する。Tab/ShiftTabはmodal内だけ循環し、text fieldではTab移動だけ許可して文字編集のEnter/Spaceは既存widgetが所有する。

- U28を実装中: 共通のpress/release確定へMenu・Task・一覧・通知・DevPanelを移行し、対象/payload/epoch/foreground変更を拒否する6回帰がpassed。canonical modal Tab/Shift+Tab/Enter/Space、可視有効control循環、戻る優先・呼出元復帰・focus枠を追加。modal関連8 tests passed。ownerの従来Pressed fixtureはtest専用のaccepted-input注入へ移し、gesture protocolの証拠と区別する。最新変更後の全gate/実画面受入は未実施。
- OS入力診断の追加観測: ユーザー回答「リモート操作の許可ダイアログは表示されていない」。permission denial/応答待ちを原因として扱わない。XTest送信成功とpointer不変の観測は維持し、同じ送信の繰返しは行わない。アプリ側の回帰を進め、実画面受入0件の状態は変えない。

- U28: root lib 679 passed/2 ignored、hw_ui 90 passed。Clippyのtest helper配置1件を修正し、再実行0 warnings。新しいmodal本文・canonical shortcutとcoverage 3 surfaceのexact snapshot差分を確認した。旧deconstruction native driverはOS入力を証明しないowner/renderer fixtureとしてaccepted-button入力へ移し、Soul Spa取消の2回押しを共通確認ボタンへ修正中（まだnative未実行）。
- U31: 連打500 ms期限を撤廃し、元world地点を保持する「候補 n/m」一覧と右クリックの選択候補優先を実装。元地点のlive resolver/revision/epochで失効させる。selection回帰20 tests passed。driver/doc変更後のgateは次回対象。

- 追加のprofiling付き全target Clippyで既存Dream metricsテストの初期化スタイル1件を検出したため、同値のstruct初期化へ修正。productionのDream挙動は変更しない。通常Clippyの既合格とは分け、profiling Clippyは再検証対象。

- U28/U31統合時点の全verify（session 86575）は終了0・All quality gates passed。通常/profiling tests、Clippy、storageを通過。以後のU09/U27差分には再検証が必要。
- U09: TimeとSystemMenuStateを分離し、Menu/未処理Escapeで開閉、menu起因pauseだけを元速度で解除。明示allowlistをButton受理・表示・root UI/domain・keyboardへ適用。Familiar/Area計画をInput後へ分離し、選択用差分索引だけPreUpdateへ移した。新規採取は既存owner/slotsを保持。DoorはPostUpdate単一consumerからsave前に確定し、Light Fieldは次Updateに反映する。
- U27: Help内のroot sealed開始文言から任意起動。実世界の使い魔・範囲・本人designation・所属SoulのCollecting/Doneを追い、成功despawnと単なる対象消滅を区別する2回帰を追加。既存範囲/指定は現在の有効な確定状態として利用する。終了/loadでruntime stateを破棄する。
- U09/U27途中のroot testは683 passed/3 failed/2 ignored（688件）。2失敗は既存Paused rejection outcomeを入口guardが消したため修正、残る1件はHelp exact未再生成。新規guide/採取保護/menu所有権・input71件は通過。修正後のsnapshot生成・全gateを実行中であり、完了扱いしない。

- 最終レビュー: SettingsのSlider/Checkbox行も32px以上へ調整。候補選択はcamera不在でも失効させる。pauseによる減光とmode表示はTime Resourceの毎frame変更ではなくpause状態遷移で更新し、通常時hover色を上書きしない。capturedの時間intent/Domain操作を破棄する回帰を追加。Clippyの複雑なQueryは型aliasへ分離し、nested条件を整理した。再実行Clippy 0、全verifyはsession 31047で進行中。
- Help impact再レビューはUpdate required。Time/メニューの入力→共通capture→root time owner、paused allowlist→domain owner、Help開始→WorkGuideのreadonly実状態→案内表示を確認し、既存stable entryへ本文とcoverageを追加。新しいguide開始chrome、メニューのcanonical Esc、timeのSpace/1〜4、saveのMenu表記を生成snapshotで確認した。U25の最小操作領域もfonts仕様へ同期。
- 実入力診断: Xwayland 24.1.13、RemoteDesktop portal v2（AvailableDeviceTypes=7）、Mutter RemoteDesktop v1（SupportedDeviceTypes=7）の公開をread-onlyで確認。APIの存在は入力許可/配送の成功を証明しない。ユーザーの「ダイアログ非表示」とXQueryPointer不変を保持し、原因は未確定。診断holdのnext_actionを回答待ちから接続成立条件の切り分けへ更新した。
- 次のnative確認はM0-AのTask Dashboard acceptance-smokeに限定する。primary cacheを再利用し、既存coordinator/kitty launcherでCapture→Memoryを逐次実行、hidden/visible/active-filter各3回（warm-up1秒/measure2秒）。owner=ui-usability、consumer=ui-usability-implementation。これは現在の全UI配線でのnative起動・計測経路の確認であり、OS pointer/keyboard・文字可読性・C01〜C08の実画面受入や正式30/60秒性能比較へ読み替えない。結果の独立verify/seal後は用途のないjobを削除し、primary cacheと入力診断holdを維持する。

### 最終差分の検証結果（2026-09-14）

- 文書空白修正とnative batch整理後、`python3 scripts/dev.py verify`を再実行（session 82818）。終了0、`All quality gates passed`。通常/profiling workspace tests、追加feature check、Clippy 0、Help exact/coverage、docs/storage、Diff hygieneまで通過。以下のsession 31047の途中結果を最終結果としては使用しない。
- `verify` session 31047: Python/契約/Help exact/docs/storage、workspace check、profiling/通常workspace tests、追加feature check、Clippy 0を通過。最後のDiff hygieneだけがarchitecture.mdの空白行2箇所で終了2。該当空白だけ修正し、`python3 scripts/dev.py check`（session 80424）と`git diff HEAD --check`が終了0。verify全体の終了0と取り違えない。
- rust-analyzer: 最終intent_handler.rsとpause_menu.rsはerrors/warnings 0。workspace診断APIはUnexpected response formatのため全workspace 0の証拠には使わず、workspace compileとファイル診断で確認した。
- batch `ui-task-dashboard-20260914`: HEAD `a4051f9b` + dirty source fingerprint `9741d7df1f872d30910a34e6bc5f89d7fe9cc0a584952662345e84a788efe3c1`を固定。2026-09-14 00:52 UTCにvalid。固定audit各状態1回、X11/VulkanのCapture/Memoryはhidden/visible/active-filter各3回、並列game 1。実adapterはIntel(R) Arc(tm) Graphics (MTL)、Mesa 26.1.8。warm-up1秒/measure2秒のacceptance-smokeのみ。
- `verify-artifacts --adapter Intel --min-runs 3`の独立再検証とcoordinator sealがpass。audit/Capture binary SHA-256 `79972293933b57fa9aa415635c4056f9a8b934f741bd141cf9dca74721b58e9c`、Memory `d1a4def7fc252dce2afe538cbacd3d796d9a8dab74a241099c20cde45a5ea627`。正式性能比較、画素可読性、OS入力、停止中操作の実画面受入には充当しない。
- 未完了: XTest送信成功でもpointer不変、許可ダイアログ非表示という観測の原因切り分け。C01〜C08・viewport/scale matrixの実操作受入は0件のまま。再試行は接続成立条件の新しい根拠が得られてから行う。
- 整理: 独立verify/seal後、終了processと生成物のみであることを確認し、`target/native-acceptance/task-dashboard-20260913T235213Z-ebe28c9e`を削除（du 3,137,536→0 bytes）。batch finalizeとstorage checkはpass、未分類0。df空きは678,851,444,736→678,837,829,632 bytesで、同時のcache/filesystem変化を含みdu減少とは一致しない。primary Cargo cacheは継続修正用、`ui-input-diagnostic-4`の3,686,400 bytesは入力経路診断用に保持。他作業のholdは変更していない。

### 続行時の入力経路診断（2026-09-14）

- Xwayland PID 5650は`-enable-ei-portal`付きで、実binaryがlibei/liboeffisへlinkされている。公式24.1.13 sourceの`setup_ei`→`setup_oeffis`→`oeffis_create_session`と、device resumed前はeventをqueueする経路を確認。XTestの成功返却だけでは配送成功にならない。出典: [X.Org release source](https://www.x.org/releases/individual/xserver/xwayland-24.1.13.tar.xz)。
- 同じsession busへ診断用RemoteDesktop sessionを作成し、CreateSessionとSelectDevices(types=3)がresponse=0を返すことを確認して閉じた。続く独立診断ではStartを1回要求し、request handleは返ったが30秒以内にResponseを受け取れなかった。期限でRequest.Closeを呼び、sessionは既に消滅（Session.CloseはUnknownMethod）していた。マウス・キー・EIS入力は送信していない。
- portal-gnomeには`Failed to associate portal window with parent window`（parent空文字）、終了時には`Session not started`が記録された。これだけで開始待ちの原因を断定しない。今回の直接Startでダイアログが表示されたかをユーザーへ確認中。前回の「表示されていない」は前回のXTest試行についての確定事実として保持する。
- product source/harnessは変更していない。新しいnative batch、実機再ビルド、同じXTest入力の再送は行っていない。既合格のコード検証・Task Dashboard smokeは維持し、実操作受入0件は変わらない。次はStart待ちの表示・応答条件を確認し、接続成立の根拠が得られてからowned-client入力を再開する。

- ユーザーが直接Start診断ではダイアログが「表示された」と回答。OS開始許可が得られたこととは区別する。前回の診断sessionは終了済み。
- 次のowned-client smokeへ、明示的な`--input-backend portal`を追加。RemoteDesktop CreateSession→SelectDevices→Startの成功・devices=pointer+keyboard確認後だけNotify入力を送る。永続権限/restore tokenは使わず、同じrecipe内でsessionを共有し終了時に閉じる。開始は最大300秒heartbeat付きで待機し、拒否・期限・失効は停止。OS許可はユーザーが操作する。
- X11 PID/nonce/focus/point境界とBevy observer ACKは従来通り。NotifyPointerMotionは観測root座標との差分を1回送信し、scale等で到達しなければクリックへ進まず失敗。座標補正の再送、XTestへのfallback、アプリ内input注入はしない。keyboardはXwaylandのevdev+8、wheelはreleaseでdiscrete入力。新moduleもfrozen harness hash対象に追加。
- owner=ui-usability、consumer=ui-usability-implementation。既存primary cacheを再利用し、coordinator登録のsmokeで動作確認後、独立verify/seal/finalizeする。失敗時は理由と未確認範囲を記録し、具体的な診断用途がない生成物を整理する。
- Help impact Skill: この追加差分はNo impact。検証CLIのportal transport→OS→既存Bevy入力だけが変わり、通常起動のUiIntent/capture/domain/Help本文は変更しない。UI改善全体のUpdate requiredと既存snapshotレビューは維持し、全dirty差分をNo impactへ読み替えない。
- portal試行1（`ui-portal-smoke-20260914-1`）: Start成功後ui-inputへ進んだが最初のpoint=[290,31]のACKが15秒で失敗。button/keyは未送信。OS開始待ちは越えたが実操作受入は0件。invalidとしてseal済み。
- 座標原因の一次情報: Mutter 50.4の[meta-seat-impl.c](https://github.com/GNOME/mutter/blob/50.4/src/backends/native/meta-seat-impl.c)の`meta_seat_impl_filter_relative_motion`はphysical layoutでvirtual relative deltaにmonitor scaleを乗算する。GetCurrentStateでlayout-mode=1/scale=2、X11 root=2880×1800を確認。単一・原点0・非回転・physical mode寸法一致だけを許可し、deltaを取得scaleで除算するよう修正。serial/scale変更時は失敗。経験的な値調整や同じeventの再送ではない。
- 追加検証: portal/X11 unit 11件、lint、9 native self-tests、Skill validate、agent-rule gateを実施。先行全verifyはsession 8961で終了0。native/perfのharness一覧不一致1件は新moduleを両方へ登録して修正、自己テスト再通過。座標修正後の全gateと新規smokeを実行する。
- 座標修正後: `verify` session 26863が終了0・All quality gates passed。11 unit tests、native self-testとagent rulesも再通過。root 2880×1800とMutter serial=1/scale=2.0をread-onlyで照合した。
- portal試行2（`ui-portal-smoke-20260914-2`）: 開発buildは既存cacheを再利用。Startは300秒間応答なしで失敗、request/sessionとowned gameを終了。入力は0件で、座標修正を実機合格とは扱わない。invalidをsealし、再要求はしない。次回はOSの開始許可応答を得てから、既存のpoint ACKを満たすことを確認する。
- XTest経路の原因調査は打切り、OS許可に到達する明示portal経路へ移行する。旧`ui-input-diagnostic-4`は新しい座標変換の検証入力にならないため、診断consumerを解除して生成物を整理する。過去のXTest pointer不変・ダイアログ非表示の観測は本書へ保持し、原因確定とは記載しない。
- 整理完了: `target/native-acceptance/`下の`ui-usability-feedback-20260914T134426Z-6a07b2cc`（4,030,464 bytes）、`ui-usability-feedback-20260914T135237Z-4ce82220`（6,365,184 bytes）、旧`ui-usability-feedback-20260913T193322Z-4fcf8876`（3,686,400 bytes）を終了process・生成物のみと確認して撤去。合計14,082,048→0 bytes。最後の2件整理前後のdf空きは678,036,881,408→678,045,700,096 bytes。両portal batchはinvalid/seal/finalize済み、storage checkはpass・未分類0。primary Cargo cacheは継続修正用に保持し、他作業のholdは変更していない。Help追加差分No impact判断は維持。
- portal試行3（`ui-portal-smoke-20260914-3`）: ユーザーの再開依頼後に実施。15 OS eventsでpoint ACK・press/releaseを確認し、scale変換の到達性は実証。tasks/last-page/minimizedの観測を通ったがrestoredが失敗、全smokeはinvalidとしてseal。
- 試行3の最初のdev-minimizeはHover[290,31]→Pressed[291,31]→release[385,61]と、motion要求を送っていない押下中にcursorが外れた。共通collectorの取消条件に合致し、DevPanelが残って後続のminimize位置と重なった。製品の復元bugとは断定しない。driverに許可後1秒のcursor安定待ち、対象Hover/Pressedの確認、release位置ずれの失敗、DevPoc欄消滅による最小化成立確認を追加。owned-client外のbutton pressも拒否し、release cleanupは維持する。12 unit testsとlintを通過。実操作の完全受入0件は維持。
- 上記driver修正後の`verify` session 17522は終了0・All quality gates passed。native helper self-testとagent rulesもpass。Help追加差分は検証transport/driverだけで通常player経路が不変のためNo impact（UI実装全体の既存Update requiredは維持）。Rust本体への追加変更はない。
- portal試行4（`ui-portal-smoke-20260914-4`）はStartが300秒無応答でinvalid。新しいdriverでの入力は0件、request/sessionとgameを終了。trial 3のpointer到達実証と区別し、全smoke、C01〜C08、scale matrixを合格にしない。次回はOS許可後にpointer/keyを操作せず自動gestureを観測する。
- 試行3/4のjobはinvalid seal/finalize済み。`target/native-acceptance/ui-usability-feedback-20260914T140023Z-995eaefa`（12,800,000 bytes）と`ui-usability-feedback-20260914T140747Z-5c040a1d`（6,361,088 bytes）を停止process・生成物のみと確認して削除、合計19,161,088→0 bytes。最後のjobのdf空きは678,030,839,808→678,030,839,808 bytesで同時点のfilesystem解放量とは一致しない。storage checkはpass、未分類0、primary feedback cache保持、他ownerのholdは不変。

### 実入力成立後の画像レビュー（2026-09-14）

- portal試行5（`ui-portal-smoke-20260914-5`）は1920×1080・UI倍率1で12 checkpointを通過し、独立verify/sealをpassとして記録した。OS pointer/keyがアプリへ届くこと、Tasksの最終ページ・最小化/復元・Entities切替、Settings閉鎖、通知履歴の最古へのwheel移動を確認。これは限定feedbackであり、C01〜C08の全受入ではない。
- 画像レビューではTasksの固定footerと復元後の181–200/200、SettingsのClose、通知00の本文/Closeを確認。一方、paused-tooltip画像に説明本文が見えない。旧verifierのfade_alpha >= 0.95は満たすが、**Tooltipの描画・可読性は未合格**と訂正する。内部12 checkpoint passを12項目の目視合格へ読み替えない。
- `HoverTooltip`はボタンの子に付け替えられ、旧`ZIndex(50)`は兄弟間だけの順序指定だった。後続のModeTextとの遮蔽を解消する仮説として共通GlobalZIndexへ移す。根拠はBevy 0.19のui_node.rs/stack.rs。次回画像で変化がなければこの遮蔽仮説を打ち切り、配置経路を再調査する。observerにはTooltip本文/矩形/描画順を追加し、画面外・本文なし・ModeTextより背後の重なりをverifierが拒否する。画像の目視は引き続き必須。
- Settings画像では背後のPauseメニュー文字が透けていたため、Settingsのパネル面だけを不透明化。capture、値の反映、保存、文言は変更しない。これら描画修正の実画面再確認は未実施。
- 6条件matrix `ui-portal-matrix-20260914-1` はStartが300秒無応答で入力0件。request/sessionとgameを終了しinvalid seal。matrixは未成立であり、試行5の配送成功を取り消す根拠でもない。
- Help impact再レビュー: 今回の描画修正は既存の「停止中も実時間でTooltip表示」というHelp契約を回復するもの。入力→Tooltip target/Real timer→内容builder、Settingsのcapture→表示/close→保存の意味・文言・成立条件は不変のため、追加差分はNo impact。UI改善全体のUpdate requiredと既存provider/coverage/exact snapshotの承認は維持する。検証observer/driverの追加はprofiling専用で通常起動に入らない。
- 比較用画像は`ui-portal-smoke-images`でexact job `target/native-acceptance/ui-usability-feedback-20260914T141827Z-703f97df`を42,610,688 bytes保持。owner=ui-usability、consumer=ui-usability-implementation、next_action=Tooltip/Settings修正前後の比較、release_when=比較終了または新画像へ置換。primary Cargo cacheは継続修正用。他ownerのholdは不変。
- matrix中断job `target/native-acceptance/ui-usability-feedback-20260914T142451Z-8dd4b806`は、停止process・build log/観測/失敗画像のみと確認して削除（6,406,144→0 bytes）。df空きは677,966,364,672→677,966,364,672 bytes。invalid seal/finalizeとstorage checkはpass、未分類0。旧成功画像は上記の修正比較用途だけで保持する。
- 描画修正後の検証: `dev.py check`はpass。Tooltip evidenceの画面外/本文なし/ModeText遮蔽を拒否する3件を含むPython関連15件、Bevy UiPluginの実Stack systemによる再parent後の描画順回帰1件がpass。native helper self-testもpass。最後の`dev.py verify`（session 4534）は終了0・All quality gates passedで、通常/profiling workspace tests・追加feature check・Clippy 0・Help exact/coverage・docs/storageを通過。production 3ファイルのrust-analyzerはerrors/warnings 0。profiling専用observerはIDE未リンクhintのためIDE診断の証拠にせず、profiling compile/testで検証した。修正後のnative画像と全matrix/C01〜C08は引き続き未受入。

### 許可待ちを再試行しない追加レビュー

- U27のGuidePanelは高さ上限がなく、Familiar名を含む本文の伸長が終了ボタンを押し出せる構造だった。既存Settingsと同じ標準ScrollArea/Scrollbarを使い、最大高さ60%・本文のみscroll・閉じる/スキップは固定へ変更した。本文14pxもthemeから取得する。
- 同じ案内ではScrollPositionを保持し、手順/停止状態による本文更新だけで先頭へ戻す。回帰`guide_preserves_reading_position_until_instruction_changes`で保持とresetを確認する。ゲームの進行、ガイドの開始/終了/対象選択条件は変更しない。
- Help impactはUpdate required。Help開始→WorkGuideの案内表示→本文scroll/固定終了操作というplayer-visible経路を確認し、既存`getting-started-work-loop`へ説明を追加。manifest/StartWorkGuide・EndWorkGuideのcoverageは同じstable entryへ既に接続済みなので追加surfaceは作らない。root生成器でexact snapshotを再生成し、新しい案内1段落の反映を確認した。仕様は`docs/help-screen.md`へ同期。
- 本続行では新しいportal session/native jobを作っていない。既存の修正前比較画像42,610,688 bytesとprimary修正cacheは現在の用途で保持し、他ownerのholdは変更していない。
- ガイド追加差分の`dev.py check`、読書位置の回帰1件、rust-analyzer errors/warnings 0を確認。最終`dev.py verify`（session 1032）は終了0・All quality gates passed。通常/profiling tests、Help exact/coverage、Clippy 0、docs/storageを通過。実画面の残件は冒頭表のとおりであり、全作業の完了とは扱わない。

### ユーザー依頼による修正後の実画面確認

- 「確認してください」の依頼で、既存matrixへHelp→StartWorkGuide→表示→終了の2 checkpointを追加し、coordinator batch `ui-portal-matrix-20260914-2`を1回実行。source `854877f5560fe1a97559ff19e91e26fa8d8377037f4ae53cd4d8ad5a2ec0266b`、harness `0d999ed0dcf33ed745dc842c20b9ba88f7f1b4206d62bea7d9274fb08b93e929`、HEAD `a4051f9b`を固定した。
- 1920×1080・倍率1の14 checkpoint通過。client画像でTooltip本文「システムメニュー」、Settingsの不透明な背景とClose、ガイド1/5の本文と終了2ボタンを確認。長い名前を含む後続ガイド・長文scrollは今回のfixtureでは確認していない。入力はportal Notify経由、OS base倍率2/override1なので高DPI受入ではない。
- 1280×720・倍率1.25はTasks/last-page/minimized/restored/entitiesの5 checkpointまで通過。paused-tooltipは文字alphaの条件で停止し、matrix全体はinvalid seal/finalize。撮影画像には説明があり、停止時の最新observerは文字alpha=0.99949・本文矩形あり・Tooltip stack1535 > ModeText stack614だった。失敗判定に用いた過去frameの値は旧driverが保存していないため、最新値を失敗frameへ遡及しない。
- 待機は本体fade_alphaだけを見ていたが、productionのfade.rsでは文字色が本体fadeへ別途補間される。driverを既存の本文/矩形/描画順/alpha条件全体が成立するまで待つよう修正し、失敗判定frameを`failed-checkpoint.json`へ保存する。判定閾値を緩めず、root=1・text=0.9で未ready、text=0.99でreadyとなる回帰を追加（関連Python16件pass）。この待機修正後のnative再試行は行っていない。
- 追加差分のHelp impactはNo impact。検証driverの案内表示/終了操作とread-only待機・観測保存だけを変更し、通常ゲームの入力/表示/Help契約は不変。直前のガイド本文scrollに対するUpdate requiredは維持する。
- 修正前の比較は終了したため`ui-portal-smoke-images`のconsumerを解除し、旧job `target/native-acceptance/ui-usability-feedback-20260914T141827Z-703f97df`を停止process確認後に削除（42,610,688→0 bytes、df空き677,854,142,464→同値）。新job `target/native-acceptance/ui-usability-feedback-20260914T162305Z-acc66b64`は`ui-matrix-current-images`で58,998,784 bytes保持。owner=ui-usability、consumer=ui-usability-implementation、next_action=現在画像のレビューと残viewport確認、release_when=レビュー/残確認終了または画像置換。primary cacheと他ownerのholdは維持し、storage checkはpass・未分類0。
- 待機修正後の`dev.py verify`（session 55413）は終了0・All quality gates passed。通常/profiling tests・Help exact/coverage・Clippy 0・docs/storageを通過。製品Rustはこの確認で変更していない。

### 6条件matrixと画像レビューで判明した重なり（2026-09-15）

- `ui-portal-matrix-20260914-3`は1280×720 / 1920×1080 × UI倍率0.85/1/1.25、14項目ずつ計84 checkpointを通過。元環境で独立verifyとpass sealを行った。これは操作/観測の結果であり、全画像の無欠陥を保証しない。
- 画像レビューで1280×720・倍率1.25のModeTextがTasksの固定ページfooterを覆うことを確認した。Settings各倍率の末尾/Close、通知00/Close、ガイド1/5の本文/終了は確認できたが、この重なりを残したまま見た目の受入完了とはしない。
- `entity_list_resize_system`が下部バーまでしか高さを制約していなかったため、ModeTextの実測上端も下端制約へ追加。ComputedNodeのinverse_scale_factorでUI座標へ変換し、capture中・cursor不在でも補正する。既存の倍率変更回帰へModeTextの実寸を追加してpass。driverにもページ4ボタンとModeTextの幾何的な重なり拒否を追加し、関連Python17件がpass。rust-analyzer errors/warnings 0、`dev.py check`もpass。
- Help impact追加差分はNo impact。既存のresize→Node高さ→共通本文visibility/固定footerの表示経路を確認し、操作・キー・文言・結果は不変のまま案内による遮蔽を防ぐ。追加UI契約は`docs/entity_list_ui.md`へ記録した。UI改善全体の既存Update requiredは維持する。
- 新しいsubjectの`ui-layout-matrix-20260915-1`で同じ6条件を再確認する。owner=ui-usability、consumer=ui-usability-implementation。旧84枚は`ui-matrix-layout-overlap`で修正比較中だけ保持し、新結果への置換時に旧jobを整理する。

### 遮蔽修正後の実画面確認結果（2026-09-15）

- `dev.py verify`は一度MemAvailable 7.87 GiBで開始を拒否した。11 GiB超への回復を確認後、安全条件を変更せず再実行し、session 56060は終了0・All quality gates passed。通常/profiling tests、Clippy 0、Help exact/coverage、docs/storageを通過。
- batch `ui-layout-matrix-20260915-1`は6条件×14 checkpoint=84件がpass。独立verifyはsessions=6/status=valid、coordinatorのpass seal/finalizeを完了。source `c9b74b80cff01570bee3c2710d9dfd5a28ae317c0d5032468aba97c68b21cb9e`、harness `fc08b2560e642a2deae80f1e41fef394258e60fa9c1f5115a8b7b3d9250ac2dd`、HEAD `a4051f9b`。
- 全6条件のlast-page画像でfooterとModeTextが重ならないことを目視確認。1280×720/1.25では停止中Tooltip本文、Settings末尾/Close、通知00/閉じる、ガイド1/5本文/終了2ボタンも確認した。保存・ContextMenu・Zone確定、後続ガイド、高DPIを今回のpassに含めない。
- 最新job `target/native-acceptance/ui-usability-feedback-20260914T171926Z-d6d53166`はhold `ui-layout-current-images`で204,029,952 bytes保持。owner=ui-usability、consumer=ui-usability-implementation、next_action=修正footerと6条件画像へのフィードバック確認、release_when=比較終了または新画像への置換。primary cacheも同じフィードバック用途で維持する。
- 比較を終えたhold `ui-matrix-current-images` / `ui-matrix-layout-overlap`のconsumerを解除。稼働processのcwd/exe/fdに参照がないことを確認し、旧job `target/native-acceptance/ui-usability-feedback-20260914T162305Z-acc66b64`（58,998,784 bytes）と`target/native-acceptance/ui-usability-feedback-20260914T164912Z-8f20beea`（203,763,712 bytes）を削除。計262,762,496→0 bytes、df空き677,349,502,976→677,350,064,128 bytes。コード・asset原本・primary cache・他ownerの保持物は対象外。
