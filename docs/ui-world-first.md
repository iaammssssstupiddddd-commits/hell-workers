# 地図中心のUI

最終更新: 2026-09-20

通常時は地図を広く表示し、管理・詳細・補助表示は必要なときに開く。カメラ移動、世界の選択、詳細の固定は独立した操作である。

## 画面と遷移

| 場所 | 表示と操作 |
| --- | --- |
| 左上 | 時計、一時停止と速度、設定で切替可能なFPS |
| 右上 | 世界全体の使い魔・魂の人数、要対応、Dream、保存・読込・設定のメニュー |
| 左下 | 建てる／作業を指示／範囲を設定／管理／表示／Dream／ヘルプ。主操作の位置を固定 |
| 右下 | 通知履歴。未読数と要対応数は別 |
| 右の共通枠 | 管理・対象詳細・表示設定を排他的に表示 |

`hw_ui::shell::UiShellState` はWorld / Management / Inspector / Displayと戻り先を保持する。
初期状態はWorld。使い魔・魂の管理幅は400px、仕事タブは480px、詳細と表示設定は296px。管理の初期高さは380pxで、戻る・タブ・最小化・閉じるを1行にまとめる。個人の表示項目と文字サイズを保ち、魂は1行で比較する。詳細は最大46vhで本文のみスクロールする。
管理から人物を選ぶと詳細へ移り、戻ると検索・絞り込み・スクロールを保持した一覧へ戻る。
仕事の行選択は管理内に留まり、行内の優先度・取消操作を使う。明示的な詳細リンクからだけ詳細へ進む。
「現地へ」だけがカメラを移動する。開閉でカメラ位置・倍率を変更しない。

固定対象Aを保持して別対象Bを選んだ場合、Aのカードと「現在選択: B → 詳細へ」を表示する。
Bの一時詳細から戻るとAへ戻る。「閉じる」は枠を隠し、固定と選択を解除しない。
参照の自動消滅で閉じた枠を再表示せず、管理・表示のページも維持する。world置換ではページ・選択・固定・戻り先・補助表示を初期化する。

建築・指示・範囲ツールの開始時は共通枠を閉じる。管理／表示を開く場合は既存の共通cleanupで未確定ツールを終了する。
Escは入力欄・最前面overlay・context menu・未確定操作・操作メニュー・共通枠・システムメニューの順に、その時点で所有する1段だけ処理する。
同フレームのpointer releaseで再選択・再表示しない。隠れた検索・renameのfocusとdragを破棄する。
共通枠の遷移ではTask確認と建設取消確認を失効させる。停止中の閲覧は可能だが、従来のdomain書込制限を維持する。

## 要対応と人数

人数はworldのFamiliar/Soul総数。追加・削除時だけ更新し、管理の検索と折畳みで変化しない。
使い魔ごとの所属人数・上限に加え、全所属から仕事あり／休息中を集計する。
仕事ありはAssignedTask != None、休息中は仕事なしでIdleBehavior::Resting/ Sleeping。集会内睡眠や休息所への移動を休息中とは数えない。
一覧の値同期は既存100ms cadenceを使用する。詳細も入力変更時と100msの実時間更新を使い、pauseで表示を止めない。

`AttentionSummary` はTask Dashboardの新鮮なBlockedだけを集計し、PendingEvaluationを別表示する。
Floor/Wallのtyped parent、配送kindとtyped target・anchor・live ownerが一致する工事だけを同じ対象としてまとめる。
Soul SpaはConstructingの場合に限る。一般のrelated_anchorや担当者から施工元を推測しない。
Task snapshotまたはowner関係が変わった場合に再計算し、loadで破棄する。HUD件数と仕事明細行数は一致しない場合がある。
要対応から開くと4フィルタを解除してBlockedに絞り、先頭ページへ戻る。並び順は保持する。

## 建築と理解

全12種類を画像・日本語名・用途付きのカードで選ぶ。カテゴリ選択は任意。
各画像の`BuildingCatalogPreview(BuildingType)`をrootのasset adapterが識別する。
Doorは`DoorAssetReadiness`とresolved poolのidentityが一致する場合だけClosed EW previewを使用し、それ以外は従来の`door_closed`へ戻す。
`PostUpdate`のreadiness確定後・UI Prepare前に同期するため、pause中、読込後着、世代切替、カード再生成にも追従する。32pxのUI枠と色を保持し、world用anchorや寸法は適用しない。他の11種の画像・操作・文言は変えない。
Soul Spaは発電設備、休息所は休息場所として区別する。配置中はカタログを閉じ、対象・次の入力・終了方法を示す。
床・壁の費用は既存の採用タイルのみのpreviewを使用する。通常時のMode説明は出さない。
submenuは案内の実測上端を避ける。Area操作欄は右上、cursor previewはUiScaleで座標変換し、HUDと下部案内の領域を避ける。
使い魔の詳細上部から作業範囲の指定・変更を始められる。範囲編集欄は幅300pxの不透明な面に対象・現寸法・次の操作・終了をまとめ、履歴／コピー／サイズ保存3枠は初期閉じの詳細操作へ集約する。各操作の対象・寸法・無効理由はtooltipで確認する。適用と終了は分かれており、ドラッグreleaseで適用し「編集を終了」で通常画面へ戻る。
ツールチップは左右端に応じたStart/End整列候補を持ち、カーソル基準のアンカーもUiScaleで変換する。Popoverが自動的に横clampするとは仮定しない。

任意ガイドは採取→休息所の予定を明示選択→木材搬入→施工→完成を追う。
完成ownerが発行するBuildingCompletedVisualMessageの元Blueprint IDで同一工事を確認する。domain Eventとはpublish helperで別配送し、Visual readerが生存確認より先に受領する。
対象消滅だけでは成功にしない。採取資源そのものの物流履歴、床の養生工程、初見参加者の理解度はこのガイドの検証と分ける。

## 地図の補助表示

| 表示 | 意味 |
| --- | --- |
| 通常 | 設定済み作業範囲の四角、使い魔の指揮範囲、Site/Yard境界を常時表示。選択対象は強調 |
| 担当範囲 | 常設エリアに加えて範囲の凡例を表示 |
| 保管場所 | 保管タイルを青枠で表示 |
| 給電状態 | 消費設備の給電を白丸、未給電を橙の×で表示 |
| 成立した部屋 | 既存Room検出結果の室内境界 |

`WorldViewState` は手動表示とツール中の一時表示を分け、終了時は手動表示へ戻す。
設定済みエリアのVisibilityは任意レイヤーや選択状態で隠さない。指揮範囲の濃淡・パルス・選択強調は既存の使い魔表示systemが維持する。
選択Soulの線はSoulTaskVisualStateの確定した作業先だけを結び、経路探索を再実行しない。
製品の補助線は専用WorldViewGizmosを使い、Debug Gizmos設定で消さない。開発用パネルはdebug有効時だけ表示する。

## 検証結果と残る受入

2026-09-17にユーザーが現行UIを承認。実装コミットは `578cc98f`。再設計とUI操作性改善の計画は、承認済み実装・品質検証・文書同期・専用データ整理をもってクローズした。仕様は本書と各機能文書、設計理由・画面例は[デザイン資料](design/ui-world-first/README.md)へ集約する。未実施の操作・理解度評価は下記に残し、全受入完了とは扱わない。

2026-09-16の実装最終版は `dev.py check` / `dev.py verify` を通過。通常・profilingのworkspace test、追加feature compile、Clippy警告0、Help exact/coverage、Python、docs/storageを含む。ページ遷移・固定優先・消滅・Esc所有・capture・pause・フィルタ・集約・ガイドの対象同一性と、範囲編集の開始/終了/拒否を検証した。変更Rustのfile診断はerror 0 / warning 0。profiling専用observerは通常構成でunlinked-file hintになるため、profiling compile/testと実window buildで確認した。

Help判断は **Update required**。管理/表示の入口→`WorkspaceAction`→共通枠、詳細の明示対象→`SelectAreaTaskFor`→既存範囲編集、`FinishAreaEdit`→共通cleanup、建築完成Message→ガイド達成を確認した。人数・要対応・常設エリア・通知の意味もproviderへ同期し、既存manifestのstable IDを維持してcoverageと生成器によるexact snapshotを更新した。

native描画は1280×720 / 1920×1080 × UI倍率0.85 / 1 / 1.25で検証した。初回再設計42条件、管理小型化・エリア復元後6条件、範囲編集と詳細入口の最終版18条件をそれぞれ独立verifyと画像の目視で確認した。改修段階の異なる結果を、同一最終コードの全場面再検証とは扱わない。

| 場面・検証段階 | 6条件の画面全体のUI遮蔽率 |
| --- | ---: |
| 通常（初回再設計） | 3.04〜14.65% |
| 選択 / 固定と現在選択（初回再設計） | 6.02〜27.94% / 6.45〜27.94% |
| 建築カタログ / 表示 / 範囲メニュー（初回再設計） | 8.09〜39.12% / 6.23〜30.07% / 3.86〜18.58% |
| 管理（小型化・エリア復元後） | 8.34〜40.42% |
| 作業範囲の通常 / 詳細展開（最終版） | 5.40〜25.87% / 7.63〜36.57% |
| 詳細入口・固定と現在選択（最終版） | 6.82〜27.94% |

遮蔽率は可視UI矩形の和集合。透明な構造Nodeを除き、rendererのCalculatedClipを反映する。管理パネル面積は小型化前より21.46〜24.60%減り、1280×720 / 1.25でも同じ4人の主要値・文字サイズを保持した。最終の範囲編集欄は幅300px・高さ約129〜132px（logical）、展開時も全10操作とラベルを表示する。X11実window、Intel Arc (MTL) / Vulkanでの描画結果であり、OS入力・高DPI・性能改善の証拠ではない。

残る受入は、再設計後のOS入力、C01〜C08の全シナリオ、高DPI、長い対象名、非空のMode案内と範囲メニューの同時表示、ガイド完走、初見ユーザーの理解度。再設計前の限定84 checkpoint通過も、現行UIの操作受入へ流用しない。

2026-09-17の文書整理・コミット前にも `python3 scripts/dev.py verify` を実行し、全品質gateを通過した。文書索引の再生成とHelp影響レビュー、storage checkも完了。整理のためのproduction変更やnative再撮影は行っていない。

詳細: [一覧](entity_list_ui.md)、[仕事](task_list_ui.md)、[詳細](info_panel_ui.md)、[選択](world-selection.md)、[状態](state.md)、[通知](notifications.md)、[Help](help-screen.md)。

## 未検証事項と再検証条件

以下は計画クローズ時の未検証事項であり、既知の不具合一覧や合格済み一覧ではない。実利用で問題が報告された場合、関連UIを変更する場合、またはリリースの受入範囲に含める場合に、現行subjectで必要なシナリオを検証する。旧計画の全項目を継続中として残さず、未確認を成功へ変更しない。

| ID / 領域 | 未実施の全体受入で守る条件 |
| --- | --- |
| C01 管理の表示状態 | 使い魔・魂/仕事×展開/最小化、検索後の往復。本文とクリック領域は最大1つ、仕事にSoul検索を出さず、検索解除で元の状態へ戻る |
| C02 停止中のtooltip | Pause/1x/2x/4xで新規hoverを実入力。unpauseを挟まず同じ実時間delay/fadeで表示し、差は描画2frame分以内 |
| C03 設定 | 6画面条件で末尾とCloseへ到達。実際の設定操作でUI倍率を1.25へ変更して戻す。長い日本語の文字・操作を切らない |
| C04 通知64件 | wheel/scrollbarで先頭・中央・最古の固定IDに到達。追記/追出しの読書位置、cameraとworld選択の不変を確認 |
| C05 保存・読込 | occupied slotの同座標double-clickではtransaction 0、独立Confirmで1回。旧session拒否とLoad/Recoveryのownerを確認。既存の保存request後revision照合を維持 |
| C06 仕事200件 | 取消可能な手動指定の1/5/10ページ先頭・末尾を選択/取消。最大20行、最終ページの件数減少時clamp、pinと操作先、READ_ONLY拒否を確認 |
| C07 ContextMenu | 命令/目的地を維持し、右クリック→Escはmenuだけを閉じる。Helpを重ねた場合は最初のEscでHelpだけ閉じる。連打/同frame入力も拒否経路を確認 |
| C08 Zone | StockpileのYard外/複数Yard/一部不可/全不可、Yard拡張の重複/変更なし。採用セル・boundsを照合し、cursor喪失や失敗release後に枠が追従しない |
| 共通枠・密度 | 管理→詳細→戻るの検索/scroll保持、固定A/選択B、消滅/load、8使い魔×各8魂の比較・配属、明示的なcamera移動 |
| 範囲編集 | 左クリック→詳細から表示対象を編集し、ドラッグ/全10操作/終了を実入力。pause許可、capture拒否、適用済み範囲保持、対象変更時の折りたたみ |
| 入力・表示 | modal focus循環/復帰、IME、Esc一段処理、重なり候補、配置に漏れないpan。長名、非空Mode案内＋範囲メニュー、高DPI |
| ガイド・理解 | 採取→選んだ休息所の搬入→施工→完成を実操作。取消/消滅/skip/load。床完成・担当・停止理由・範囲・取消の初見評価 |

基本条件は1920×1080 / UI倍率1、C03は2解像度×3倍率。C01/C04/C05/C06は1280×720 / 1.25でも確認する。高DPIでは実OSのscale factorと物理/論理寸法を記録し、UI倍率やoverrideで代用しない。実trackpad/Waylandも別の未検証条件である。

再検証はnative Skillとprimary validation coordinatorの既存no-prompt launcherを使い、seed `20260914` と隔離save/settingsを使用する。fixture準備後はOS入力→production結果→PID所有client画像を照合し、状態の直接代入を操作成功と数えない。source/harness・nonce・window/focusの不一致やtimeoutでは停止し、入力を重複送信しない。

初見評価を行う場合の暫定目標は、5名中4名以上が停止中か・対象の役割・停止理由・次の確認先を各10秒以内に説明できること。操作数、迷った入口、誤解した語も記録する。これは未測定の設計目標であり、実績や統計的保証ではない。

## 任意の後続候補

以下は未実装の拡張で、今回のクローズ条件には含めない。

| ID | 候補 | 着手条件 |
| --- | --- | --- |
| O01 | Task仮想scroll、即focus互換設定 | ページングの実利用で移動負担を確認し、可変高/activationと計測条件を決める |
| O02 | Stockpile方針コピー/preset、通知の見た行だけ既読 | 誤適用防止の差分確認と履歴更新中の既読規則を定義する |
| O03 | save名・人口・ゲーム内日付・サムネイル、表示時snapshot/Loadのrevision照合 | 有界読取、旧形式互換、失敗/サイズ上限、変更時の再確認を定義する |
| O04 | 全面日本語化、文字のみ拡大、演出量設定 | 用語辞書・viewport検証・利用者評価から対象を決める |
| O05 | 停止中の建築/Zone/Haul/Dream/配属/取消/範囲policy | 各ownerの副作用・index・繰返し適用・取消/loadを受け入れる計画を作る |

## 承認後の検証データ整理

2026-09-17、ユーザー承認により `ui-usability-implementation` の画像8件とprimaryのreview holdを解除した。代表PNG4枚をデザイン資料へ無加工で採録・hash照合した後、停止済み・finalized/pass・生成物のみと確認した次の8jobを削除した。prefixは `target/native-acceptance/`。個別batch metadataはprimary validation台帳に残る。

| 削除したjob | 実測bytes → 0 |
| --- | ---: |
| `ui-usability-feedback-20260916T003956Z-206e3aab` | 17,760,256 |
| `ui-usability-feedback-20260916T004343Z-2381fdf7` | 16,334,848 |
| `ui-usability-feedback-20260916T003629Z-586ac298` | 16,818,176 |
| `ui-usability-feedback-20260916T004657Z-f7b1a1e6` | 17,559,552 |
| `ui-usability-feedback-20260916T140325Z-70c6a6f8` | 16,220,160 |
| `ui-usability-feedback-20260916T160815Z-110eb859` | 16,838,656 |
| `ui-usability-feedback-20260916T161052Z-923ef498` | 17,289,216 |
| `ui-usability-feedback-20260916T161604Z-a314e618` | 16,834,560 |

専用出力は合計135,655,424→0 bytes。filesystemの空きは前後とも673,672,392,704 bytesで、Btrfs共有extent等によりdu差と同一ではない。他ownerの保持物は変更しない。

primary `/home/satotakumi/projects/hell-workers` と通常Cargo cacheは通常開発用に維持する。専用UI容量ではなく、引継ぎ時のprimary全体は279,492,227,072 bytes。hold `primary-normal-development`、owner `primary-development`、consumer `primary-workspace-development` とし、次の用途は今回コミットの品質gateと同じprimaryでの通常開発、解除条件はprimaryの廃止または別consumerへの引継ぎ。終了したnative jobの保持には使わない。未実施のUI受入は再開時に新subjectと利用を登録する。
