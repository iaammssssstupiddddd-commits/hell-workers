# UI実装の横断レビューと操作性向上案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `ui-usability-audit-proposal-2026-09-13` |
| ステータス | Approved |
| 作成日 | 2026-09-13 |
| 最終更新日 | 2026-09-17 |
| 作成者 | Codex |
| 調査対象 | primary repository、HEAD `a4051f9b`、開始時の作業差分なし |
| 完了記録 | UI改善計画はクローズ済み。[現行仕様・検証結果](../ui-world-first.md)と未検証事項へ集約 |
| 関連Issue/PR | N/A |

## 1. 背景と問題

本書の調査本文は2026-09-13〜14時点の記録。U01〜U32と後続の再設計は実装され、2026-09-17に現行UIをユーザーが承認した。以下の旧配置・不具合・提案時点の未確認事項を現仕様と扱わず、現在の動作と検証結果は [地図中心のUI](../ui-world-first.md) と各機能仕様を参照する。

UIの入口、表示状態、入力制御、ゲーム側の適用先を横断して確認した。
改善の中心は、一覧の全対象に到達できること、操作対象と結果が明確なこと、停止中にも考えて操作できることである。
見た目の変更より先に、スクロールの未接続、タブと最小化の競合、確認操作の分かりにくさを解消することを推奨する。

本書は**実装の静的調査に基づく提案**である。現在のゲームを起動した実機操作、画面の撮影、DPI別の描画、操作時間の測定はしていない。
「コードで確認」は入力から状態・表示への経路を確認したという意味であり、実機再現済みを意味しない。
レイアウトの切れ方、文字の読みやすさ、視点移動の快適性は実機・利用者評価の対象として明示する。
過去のnative受入記録を今回の観測結果として流用しない。

## 2. 目的と非目的

- 初心者が操作を発見でき、管理対象が増えても一覧から目的の対象へ到達できる。
- 誤操作から戻りやすく、確認・取消・選択・ピン留めの意味が画面間で一致する。
- 既存の入力capture、ownerによる再検証、ロード時reset、差分更新と有界な表示量を維持する。
- 今回は提案までとし、ゲームコード・操作仕様・Help本文・設定値・プレイヤーの保存データは変更しない。
- 描画方式の刷新、UIライブラリの置換、コントローラー全面対応、ゲームバランス変更は本提案の前提にしない。

## 3. 調査範囲と既存の良い実装

| 領域 | 確認した操作・表示 | 評価と関連項目 |
| --- | --- | --- |
| UI shell | root/mount、下部バー、パネル、overlay、theme、font | 入力透過rootとcapture rootを分離済み。U06・U25 |
| Architect / Orders / Zones / Dream | メニュー、カテゴリ、モード表示、各intentの適用 | 完了可能な建築経路と未公開FamiliarBuildを区別済み。U08・U10・U26・U29 |
| 時間・Pause | Space、速度ボタン、停止時メニュー、復帰 | captureは厳密。計画操作との分離が課題。U07・U09・U10 |
| ワールド選択・カメラ | screen-space snap、release確定、重なり巡回、右クリック、pan guard | drag slopとUI上の誤選択防止を維持。U05・U09・U15・U30・U31 |
| 建築・移動・範囲編集 | ghost、preview/commit再検証、部分採用、Tank companion、Soul Spa、範囲undo | typedな拒否理由とowner cleanupあり。Zone側へ展開。U08・U26・U32 |
| Entities | 名前検索、開閉、人員上限、Tab、配属drag/drop、resize、minimize | 差分同期と検索/rename入力分離あり。U02・U11〜U15 |
| Tasks | blocker、filter/sort、priority、cancel、pin、camera focus | positive allow-listと適用直前再検証あり。U01・U16・U18 |
| InfoPanel | Soul/vitals/rename、Familiar、Blueprint、Stockpile、Power、Soul Spa | 固定対象のEntityに対する編集を保証。U15・U17〜U19 |
| Stockpile / 電力 | 9資材の方針、目標量、priority/export、範囲patch、給電理由、稼働枠 | 既存の範囲適用・typed理由を維持。U19 |
| Settings | UI scale、camera、default speed、debug、autosave、永続化 | Bevy標準slider/checkboxを使用。U06・U20・U21 |
| Save / Load | catalog、slot capability、上書き/読込確認、復旧、結果通知 | session/target revision確認と復旧専用経路あり。U04・U24 |
| 通知 | toast、dedupe、重要履歴、未読、world replace reset | 3 toast/64履歴、実時間expiry、toast入力透過あり。U03・U22 |
| Help | F1/入口、topic/本文scroll、pause所有権、manifest/provider/coverage | 完全一致snapshot・canonical shortcut・到達可能性分類あり。U23・U27 |
| Tooltip / 文字入力 | hover delay/fade、配置理由、Popover、focus、IME、clipboard | 画面内配置とIME中Enter/Escape抑止あり。U07・U28 |
| ワールド内フィードバック | 採取/運搬のphase表示、Speech、Dream獲得、Room境界 | Speechの優先度/cooldownとDream実獲得量との対応あり。Room成立理由・演出量はU19・U25。タスクリンクはデバッグ専用 |
| 開発専用UI | DevPanel、FPS、IBuild、3D/Light切替、F12 | 通常プレイヤー向け改善と区別。設定整理U20に含む。GPU性能は未評価 |

主な仕様参照: [選択](../world-selection.md)、[Entities](../entity_list_ui.md)、[Tasks](../task_list_ui.md)、
[詳細](../info_panel_ui.md)、[設定](../settings.md)、[通知](../notifications.md)、[Help](../help-screen.md)、[保存](../save_load.md)。

## 4. 優先順位

P1は操作の到達性・状態不整合・誤確定を先に解消する項目、P2は日常操作の負担を減らす項目、P3は導入支援・情報拡充である。
工数は未見積り。小/中/大は相対的な変更範囲であり、日数や効果の実測ではない。

| 順 | 項目 | 優先 | 範囲 | 根拠の種類 |
| --- | --- | --- | --- | --- |
| U01 | タスク一覧の全行へ到達可能にする | P1 | 中 | コードで確認 |
| U02 | 左パネルのタブ・最小化・検索表示を統合する | P1 | 小 | コードで確認 |
| U03 | 通知履歴のスクロール入力を接続する | P1 | 小 | コード・Bevy 0.19一次情報で確認 |
| U04 | セーブ/ロードの確認を専用表示にする | P1 | 小 | コードで確認 |
| U05 | Escで文脈メニューを閉じ、命令変更を防ぐ | P1 | 中 | コードで確認 |
| U06 | 設定画面を画面内に収める | P1 | 小〜中 | 制約欠如はコード確認、切れ方は実機確認 |
| U07 | Tooltipの時間をゲーム速度から独立させる | P1 | 小 | コード・Bevy 0.19一次情報で確認 |
| U08 | Zoneのpreview・失敗理由・再試行をそろえる | P1 | 中 | コードで確認 |
| U09 | シミュレーション停止とPauseメニューを分離する | P2・効果大 | 大 | 現仕様を変える提案 |
| U10 | 数字キーの意味と画面上の案内を一致させる | P2 | 中 | コードで確認 |
| U11 | 名前検索を折りたたみから独立させる | P2 | 中 | コードで確認 |
| U12 | Entities本文全体をスクロールする | P2 | 中 | 経路欠如はコード確認、配置は実機確認 |
| U13 | Tab/Shift+Tabを表示順・双方向循環にする | P2 | 小 | コードで確認 |
| U14 | Soulの作業表示を網羅的に分類する | P2 | 小 | コードで確認 |
| U15 | 選択・カメラ移動・ピン留めを明示的に分ける | P2 | 中 | 動作はコード確認、好みは利用者評価 |
| U16 | タスクの直接フィルタ選択と解決操作を提供する | P2 | 中 | コードで確認 |
| U17 | 詳細パネルの対象名と固定解除を常時表示する | P2 | 中 | 構造はコード確認、描画は実機確認 |
| U18 | Soul Spa建設取消の確認を全入口で統一する | P2 | 中 | コードで確認 |
| U19 | 建物・部屋・電力の詳細を次の操作へつなぐ | P2 | 中 | 既存情報からの改善提案 |
| U20 | 設定値・単位・適用時期を表示する | P2 | 小 | コードで確認 |
| U21 | 設定保存の失敗を画面に返す | P2 | 小 | コードで確認 |
| U22 | 通知を読み返し・対応しやすくする | P2 | 中 | 既存仕様からの改善提案 |
| U23 | Helpに検索・文脈移動・読書位置復元を加える | P2 | 中 | コードで確認 |
| U24 | 保存スロットの識別情報と一覧の到達性を改善する | P3 | 中 | コードで確認、長文配置は実機確認 |
| U25 | 小画面・文字拡大・用語統一に対応する | P2 | 中〜大 | 定義はコード確認、視認性は実機確認 |
| U26 | 配置・範囲編集の操作案内を強める | P3 | 中 | 既存経路からの改善提案 |
| U27 | 最初の仕事ループを画面内で案内する | P3 | 中 | 静的Helpを基にした新規提案 |
| U28 | キーボードfocusと取消可能なボタン確定を整える | P2 | 大 | コードで確認 |
| U29 | Ordersの担当者と適用結果を示す | P2 | 中 | コードで確認 |
| U30 | 配置モードを維持したマウスdrag panを提供する | P2 | 中 | 現在のgesture契約を拡張する提案 |
| U31 | 重なった対象を時間制限なしで選べるようにする | P2 | 中 | コードで確認 |
| U32 | AreaのUndo/Redo・presetを見える操作にする | P2 | 中 | コードで確認 |

## 5. 詳細な所見と提案

### U01: タスク一覧の後方行に到達できない

`hw_ui/src/panels/task_list/render.rs:86` は `visible[..resident_rows]` のみ生成する。
上限は `bevy_app/src/interface/ui/panels/task_list/update.rs:32` で算出され、スクロール位置やページを持たない。
`hw_ui/src/setup/entity_list.rs:290` の本文も `clip_y`。同条件のタスクが多数あると、filter/sortだけでは全個別行を選べない。
**提案:** toolbarを固定し、表示範囲を動かせる仮想スクロールまたはページングを導入。単に生成上限を増やさない。
**受入:** 同条件の取消可能な手動指定200件の先頭・中央・末尾を表示・選択・取消可能。総件数と表示範囲が分かり、行数に比例して全widgetを常駐させない。

### U02: Tasksで最小化して戻すとEntities本文も表示状態になる

`hw_ui/src/list/minimize.rs:82` は展開時に `EntityListBody` を無条件にFlexへ戻す。
`hw_ui/src/panels/task_list/interaction.rs:106` の補正はタブ変更時だけで、同じTasksタブでは競合が残る。
検索行も両本文の外にある（`hw_ui/src/setup/entity_list.rs:102`）ため、TasksでもSoul向けSearchが残る。
**提案:** active tabとminimizedから本文・検索・resize可否を一括決定する。TasksのSearchは対応検索を実装するまで隠す。
**受入:** 両タブで最小化/展開/最小化中のタブ変更を組み合わせても本文は最大1つ。検索欄の対象と表示一覧が一致する。

### U03: 通知履歴のスクロールが入力につながっていない

`hw_ui/src/notifications/ui.rs:111` の履歴パネルは `overflow: scroll_y` を指定するが、
`ScrollArea`、Scrollbar、独自ScrollPosition更新consumerがない。64件を保持する一方、隠れた履歴へ移動する経路がない。
Bevy 0.19の標準ホイール処理は `With<ScrollArea>` を条件とすることをdocsrs-mcpとローカルソースで確認した。
**提案:** 既存Help/Operationと同じ標準ScrollArea＋Scrollbarを使用。見出しを固定する。
**受入:** 64件の最古までwheel/trackpad/scrollbarで到達し、背後のカメラが動かない。履歴更新後も読んでいた位置を不意に失わない。

### U04: 確認状態に移っても押す行の表示が同じ

`bevy_app/src/systems/save/catalog_present.rs:26` は選択したslotのactionをConfirmへ変更するが、
タイトルはSave Game/Load Gameのまま（:86）、行ラベルもslot・状態・更新時刻のまま（:112）。追加されるのはBack（:164）。
`bevy_app/src/systems/save/catalog_ui.rs:129` と同:202には確認状態とsession/revision guardが存在する。確認段階の伝達を改善する。
**提案:** 対象名、上書き/現在world置換の意味、「上書き保存」「読み込む」「戻る」を明示した確認領域にする。
**受入:** 同じ行を再クリックする知識なしで完了でき、連打で選択から実行まで通過しない。古いsession/変更済みslotの拒否を維持する。

### U05: 文脈メニューをEscで閉じる経路がなく、Familiar命令へ流れ得る

`bevy_app/src/interface/ui/panels/context_menu.rs:94` の閉じる操作はworld左押下であり、Esc処理がない。
`bevy_app/src/input_actions/context.rs:101` の前景判定にContextMenuは含まれず、
`bevy_app/src/input_actions/bindings.rs:469` には通常プレイでFamiliar shortcutが有効な場合のEsc→Idle/Patrol切替がある。
**提案:** 文脈メニューをキャンセル解決順に含め、最初のEscはメニューだけを閉じる。Idle/Patrolは専用操作へ移す案を併せて評価する。
**受入:** 通常モードでFamiliar右クリック→Escを行っても命令・目的地が変わらず、メニューが閉じる。モーダル/配置取消の既存優先順位を壊さない。

### U06: 設定画面が下方向へ伸び、画面内に収める仕組みがない

`hw_ui/src/setup/settings_panel.rs:78` は幅380px、top45%、横方向の負marginだけで配置される。
最大高さ、縦中央揃え、スクロールを持たず、autosave項目とCloseまで縦に積む（:187、:214）。
通常の1280×720（`bevy_app/src/main.rs:132`）と拡大時は優先して実機確認すべきである。
**提案:** Operation Dialogと同じ中央配置、viewportに対する最大高さ、本文scroll、固定Closeを採用する。
**受入:** 1280×720とUI scale 0.85/1/1.25で全項目・Closeに到達できる。拡大した本人が設定を戻せる。

### U07: Tooltipの待ち時間・フェードがPause/倍速に従う

`bevy_app/src/interface/ui/interaction/tooltip/mod.rs:54` の `Res<Time>` が
`hw_ui/src/interaction/tooltip/system.rs:271` のdelayと:283のfadeへ渡る。
Bevy 0.19ではUpdate中の汎用TimeはVirtual。停止後に新しくhoverすると表示待ちが進まず、倍速では実時間が短縮される。
**提案:** 純粋なUI表示待ち・フェードにTime<Real>を使う。Dreamのゲーム内演出は意味を調べて別判断し、一括置換しない。
**受入:** Pause/1x/2x/4xで同じ実時間後にtooltipが出る。Pauseメニュー内でも新規hover説明が読める。

### U08: Zone配置は理由・部分採用・失敗後の待機復帰が不足

`bevy_app/src/systems/command/zone_placement/placement.rs:57` はYard包含などの失敗で、
drag開始点のresetより前にreturnする。Stockpileは非歩行/建物セルを個別に飛ばす（:104）が、
`bevy_app/src/systems/command/area_selection/indicator.rs:74` のbool判定にはこの詳細がない。プレビューは有効色でも生成0件になり得る。
**提案:** Building/Floorと同じtypedなplanをpreview/commitに用い、採用数・除外数・最初の理由を示す。失敗後は次のdragを開始可能な待機へ戻す。
**受入:** Yard外、複数Yard、全セル不可、一部可を検証し、preview件数と実生成数が一致。失敗release後は `ZonePlacement(_, None)` へ戻り、ボタンを離した枠がcursorへ追従しない。

### U09: 一時停止中に計画できる操作モードを設ける

`hw_ui/src/interaction/pause_menu.rs:6` はTimeの停止と全面Pauseメニューを直結する。
captureによりカメラと背景の世界選択・配置UIが止まる。PauseからSettingsへ進む操作などは可能である。計画重視のゲームでは時間停止と計画操作の両立を検討したい。
**提案:** 時間停止とシステムメニュー表示を別状態にする。停止中に許可する選択、詳細、範囲、設計予約、方針変更を明示的に定義する。
**受入:** 停止中に予約でき、Soul移動・建設進捗・消費は進まない。再開後に予約が実行される。Helpは自分が止めた場合だけ復帰する契約を維持する。

### U10: 数字キーの表示と現在の意味を一致させる

時間tooltipは1/2/3/4を固定表示する（`hw_ui/src/setup/time_control.rs:80`）。一方、
通常プレイ中でFamiliar shortcutが有効な場合の1/2/3は、高優先度のChop/Mine/Haul（`bevy_app/src/input_actions/bindings.rs:373`〜:448）になる。
**提案:** 速度キーを全選択状態で一定にする案を優先比較する。互換性を維持するならresolver contextから有効shortcutと上書き理由を表示する。
**受入:** 選択なし/Familiar/命令モード/overlayで、画面のキー案内と実際の解決結果が一致する。

### U11: 検索結果を折りたたみ状態から独立させる

`bevy_app/src/interface/ui/list/view_model.rs:153`、:208で閉じたグループのSoulを収集せず、その後:217で名前検索する。
検索は大文字小文字を区別する部分一致（:14）で、Familiar名検索ではない。
**提案:** 全Soulを検索してから表示グループを構成し、検索中のみ一致グループを展開。件数、0件、クリア、対象範囲を表示する。
**受入:** 折りたたんだ既知のSoulも同じ結果に出る。解除時に元の開閉へ戻る。Familiar検索を追加する場合は対象を明示する。

### U12: Entities本文全体のスクロールを提供する

`hw_ui/src/setup/entity_list.rs:156` のFamiliar一覧は通常Columnで、標準ScrollAreaは未所属一覧だけ（:225）。
各Familiar sectionは縮まない（`hw_ui/src/list/spawn.rs:134`）ため、多数展開時の下方グループへ進む経路がない。
**提案:** 本文全体を標準スクロールにし、必要ならグループ見出しを固定。配属drag中の端scrollも対応する。
**受入:** Familiar 8体×各8Soulと未所属一覧の全対象へ到達でき、最下部グループへ配属できる。実際の切れ方はnativeで確認する。

### U13: Tab巡回を視覚順に合わせる

`bevy_app/src/interface/ui/list/interaction/navigation.rs:42` はEntity index順で巡回する。
逆方向はsaturating_sub（:54）、正方向はmodulo（:58）で先頭/末尾の規則が異なる。
**提案:** 可視ViewModelの順を使い、双方向で循環する。移動先行を可視範囲へ出す。
**受入:** 検索/折りたたみ後も表示対象・順序と一致し、先頭⇄末尾を両方向に移動できる。

### U14: 発電・解体を水運搬の表示へ落とさない

`bevy_app/src/interface/ui/list/view_model.rs:80` のtask_visualはGeneratePower/Deconstructを明示せず、
`:104` のfallback `TaskVisual::Water` へ送る。表示側は運搬アイコン＋水色（`hw_ui/src/list/spawn.rs:324`）。
**提案:** 全AssignedTaskの意味を網羅し、Tasksと共通の作業ラベル・アイコン分類を利用する。
**受入:** 発電・解体が区別でき、将来variant追加時にも誤った既存分類へ無言で落ちない。

### U15: 一覧を見るだけで視点が動く・別対象に固定される負担を減らす

Entity行pressは即選択＋カメラ移動（`bevy_app/src/interface/ui/list/interaction.rs:108`）。
配属dragは同pressから長押し成立後に始まる（`bevy_app/src/interface/ui/list/drag_drop.rs:70`。待機もVirtual Timeを使用）。Task行はfocus＋自動pin（`hw_ui/src/panels/task_list/interaction.rs:153`）、
詳細はpin優先（`bevy_app/src/interface/ui/presentation/mod.rs:179`）である。
**提案:** 単クリックは詳細、ダブルクリック/専用ボタンは現地へ移動、pinは明示操作にする案を評価する。pinと選択が違う場合は「選択中の対象へ戻る」を表示。
**受入:** 配属dragだけで視点が移動しない。固定対象と世界選択対象を区別できる。従来の即focusを好む利用者には設定で残す案も比較する。

### U16: タスクのfilterを順送りから直接選択へ

`hw_ui/src/panels/task_list/render.rs:133` の6種toolbarと `hw_ui/src/panels/task_list/types.rs:324` は候補を順送りする。
Typeを多数通過する操作と、複数filterが残った0件状態からの復帰に負担がある。
**提案:** 作業種別を選択メニュー、状態を直接選べるボタンにし、適用中filterと一括解除を表示。
既存blockerから関連Familiar設定/資材情報を開ける操作を段階追加する。診断が古い場合は表示し、実行前に再検証する。
**受入:** 任意Typeへ2操作以内、全解除は1操作。0件の条件と戻り方が見える。詳細から設定を開いても対象を取り違えない。

### U17: 詳細の見出しとpin操作を固定する

`hw_ui/src/panels/info_panel/layout.rs:307` はroot全体をスクロールし、対象名（:332）とUnpin（:415）もその中にある。
**提案:** 見出し・pin状態・解除を固定し、本文だけ標準ScrollAreaへ移す。対象変更時のscroll復元/先頭復帰ルールを定義する。
**受入:** Stockpile詳細の末尾でも対象と固定状態が分かる。短い別対象へ切り替えて内容が確認できる。

### U18: Soul Spa取消を入口によらず同じ確認にする

Task側はConfirm cancel siteの二段階（`hw_ui/src/panels/task_list/render.rs:429`、`bevy_app/src/interface/ui/panels/task_list/actions.rs:188`）だが、
InfoPanelのCancel Soul Spa Construction（`hw_ui/src/panels/info_panel/layout.rs:177`）は直接intentからowner requestへ進む
（`bevy_app/src/systems/ui_domain_commit.rs:111`）。
**提案:** 施設全体が対象であることと搬入済み資材の扱いを示した行内確認を共通化する。進行後も戻せる保証がないundoは付けない。
**受入:** どの入口でも単発の誤pressでは取消されず、対象変更・modal・load後に古い確認を使えない。

### U19: 建物・部屋・電力の詳細を判断しやすくする

Door lockは文脈メニューにある（`bevy_app/src/interface/ui/panels/context_menu.rs:221`）が、
詳細のBuilding queryはDoor状態を持たない（`bevy_app/src/interface/ui/presentation/mod.rs:51`）。
電力詳細にはPriority prefix/Legacy all-or-noneといった内部方式の文言もある（`hw_ui/src/panels/info_panel/update.rs:114`、:177）。
**提案:** Door詳細へ開閉・施錠とlock操作を追加。電力は「需要/供給」「給電されない理由」「変更できる優先度」を主表示にする。
Stockpileには既存範囲patchを使う方針コピー/用途presetを任意追加し、適用範囲と差分を確認できるようにする。
**受入:** 右クリックの知識なしでDoor状態と操作を見つけられる。電力停止理由と次の行動が読め、複数Stockpileへの意図しない上書きがない。

Roomも関連する改善対象である。不成立は `hw_world/src/room_detection/core.rs:217` でNoneへ集約され、
表示は成立Roomの境界（`hw_world/src/room_systems.rs:295`）で、InfoPanelには成立条件の表示がない。
Room検査を追加するなら、仮設壁・扉なし・床欠け・面積上限などの理由と該当位置を区別して示す。
単に境界を強調するだけでは、不成立の原因を把握できない。

### U20: 設定値と適用時期を表示する

`hw_ui/src/setup/settings_panel.rs:246` のsliderはlabel/track/thumbだけで値表示がない。
autosave間隔は内部0〜3（:192）を5/10/20/30分に対応させる（`bevy_app/src/systems/settings/mod.rs:107`）。
**提案:** UI 100%、カメラ速度、10分ごと、3世代などを常時表示。4択の間隔は直接選択でもよい。
Default Game Speedは次回起動時と明記し、表示・操作・保存・開発用の小見出しで整理する。
**受入:** ドラッグ前後と再起動後に正確な値を読める。現在の速度と既定速度を混同しない。

### U21: 設定保存の失敗を画面に返す

`bevy_app/src/interface/ui/interaction/handlers/settings.rs:89` は保存失敗をwarnへ出すだけで、パネルは既に閉じている（:31）。
**提案:** 「現在のセッションには適用済み、次回用の保存に失敗」と区別した重要通知と再試行を提供する。成功は短い控えめな表示でよい。
**受入:** 書込不能時に失敗を画面で認識できる。raw path/errorを出さず、プレイヤー設定を使わないfixtureで検証する。

### U22: 通知を読める時間と対応導線を設ける

`hw_ui/src/notifications/model.rs:5` は4秒・最大3件、`hw_ui/src/notifications/reducer.rs:151` は古いtoastから押し出す。
履歴row（`hw_ui/src/notifications/ui.rs:291`）はtitle/body中心で、発生時刻や対象への操作はない。履歴を開くと全件既読になる（`hw_ui/src/notifications/reducer.rs:52`）。
**提案:** 通知時間を調整可能にし、重要な失敗は履歴に原因/次の操作/時刻を残す。読了と単なるパネル開閉の扱いを分ける案を評価する。
現地へ移動などの操作は履歴側へ付け、toast自体の入力透過を維持する。
**受入:** 連続発生後も失敗を読み返せる。対象消滅/load後に古いEntityへ操作しない。上限とdedupeは維持する。

### U23: Helpを開き直しても目的の説明へ戻れるようにする

`bevy_app/src/interface/ui/help_controller.rs:91` は毎回first topicとscroll=0へ戻す。
`hw_ui/src/interaction/help.rs:23` はtopic選択/前後/scrollだけで、検索や文脈からのentry指定はない。
**提案:** 直前topic・読書位置の復元、本文検索、配置不能理由や設定の「詳しく」からstable entryへの移動を追加する。
**受入:** 調べたあと閉じて試し、再度開くと同じ説明へ戻る。検索結果から正しいentryへ移れ、Help起因pauseの復帰が変わらない。

### U24: 保存スロットを見分けやすくする

catalog表示はslot/状態/相対更新時刻（`bevy_app/src/systems/save/catalog_present.rs:112`）に限られる。
一覧本体は高さ制約・scrollを持たず、外枠だけmax-height520px（`hw_ui/src/setup/dialogs.rs:614`、:648）。
**提案:** 安全なmetadataに名前・ゲーム内日付・人口等の要約を段階追加し、詳細で絶対保存日時も読めるようにする。
手動/自動/legacyの区別を保ち、世代最大・長文でも全行へscrollできる構造にする。サムネイルは後順位。
**受入:** 似た時刻の複数保存を識別でき、候補比較だけではworldを変更しない。metadata読取の有界性と旧形式互換を維持する。

### U25: 小画面・文字拡大・用語の一貫性

下部バーは固定幅ボタンの横並び（`hw_ui/src/setup/bottom_bar.rs:28`、:127）、submenuは固定位置（`hw_ui/src/setup/submenus.rs:66`）。
themeには9/10/11px文字（`hw_ui/src/theme.rs:325`）、人員±は18×18px（`hw_ui/src/list/spawn.rs:91`）、renameは22×22px（`hw_ui/src/panels/info_panel/layout.rs:388`）がある。
Helpは日本語、主要menu/設定/詳細は英語が混在する。これだけで実画面の可読性・コントラスト違反とは断定しない。
**提案:** 下部バーの狭幅配置と画面内submenu、重要文字/クリック領域の拡大、日本語用語辞書を導入。
全体UI拡大と文字だけの拡大は段階的に分け、色だけでなく文字/形も併用する。
**受入:** 長い日本語・IME・最大拡大・小画面でも全操作へ到達する。背景が明暗どちらでも選択/警告を区別できることを実測する。

`hw_core/src/settings.rs:8` にはSpeech/Dreamの演出量設定がない。任意の「演出を控えめにする」を追加するなら、
重要な警告とDream実数値を維持し、揺れ・trail・ambient泡を減らす。現状の不快さ・過剰さは未測定である。
Speechの寿命は世界内演出としてゲーム時間を使っているため、U07とまとめて実時間へ置換しない。

### U26: 配置と範囲編集の案内を操作位置に出す

Building/Floorにはtyped feedback、Floor/Wallには部分採用があり、Area editには移動・resize・copy/paste・undo/redoがある。
一方、詳細な操作はHelpやmode textから学ぶ必要がある（`bevy_app/src/systems/command/area_selection/shortcuts.rs`、`bevy_app/src/interface/ui/interaction/mode.rs:154`、:213、:263）。
**提案:** cursor近傍または固定mode領域に「次の操作」「確定/取消」「範囲寸法」「採用/除外」を短く表示。
資材合計を出す場合は実際のplanから計算し、既存の拒否tooltipを重複表示しない。移動/複数段階配置では現在の段階を示す。
**受入:** Helpを開かず開始・確定・取消を理解でき、previewの数量とcommitが一致する。既存undoの対象範囲を全操作へ誤って広げない。

### U27: 最初の仕事を段階的に案内する

`bevy_app/src/interface/ui/help_content/providers/getting_started.rs:8` は基本ループと見る場所を静的文章で説明する。
**提案:** Familiar選択→担当範囲→採取指定→結果確認をスキップ可能な短い案内として提供する。
達成判定は単なるボタンクリックではなく実際の範囲・指定・進行状態を参照する。既存プレイへの強制表示はしない。
**受入:** 新規プレイヤーが最初の仕事を開始でき、詰まった場合に対象・資源・範囲のどこを見るか分かる。効果は初回達成時間と誤操作数で測る。

### U28: modalのキーボード操作と取消可能な確定

通常MenuButtonはChanged<Interaction>のPressedでintentを生成する（`bevy_app/src/interface/ui/interaction/systems.rs:135`）。
一般menuのTabGroup/TabIndex登録はなく、Tabはentity巡回用。Help専用キーと文字編集は実装済みだが、modal全操作を代替しない。
**提案:** modal内focus順、Tab/Shift+Tab、Enter/Space、slider矢印、focus可視化と復帰先を整える。
保存/取消等の確定は押下後に外へ逃がせるrelease/click方式を検討し、pointerとkeyboardを同じintentへ接続する。
**受入:** マウスなしで設定・保存・戻るを完了でき、背景shortcutは発火しない。押して外へdrag/releaseした確定操作は実行されない。

### U29: Ordersの自動選択した担当者と適用件数を示す

Familiar以外を選択してOrdersのChop/Mine/Haul/Areaを開始すると、担当範囲なしのFamiliarを優先しEntity index順で自動選択する
（`bevy_app/src/interface/ui/interaction/intent_context.rs:108`、:127）。Chop/Mine/Haulのmode表示には担当名がない（`bevy_app/src/interface/ui/interaction/mode.rs:167`）。
Haulの受入先なしはdebugとskipだけで、release後に適用件数を示さない（`bevy_app/src/systems/command/area_selection/apply.rs:154`、`bevy_app/src/systems/command/area_selection/input/release/designation.rs:29`）。
**提案:** 「運搬／担当: ○○／対象12・受入先なし3」の短い確認と結果を出し、担当変更へ進めるようにする。Familiar不在時は開始できない理由を表示。
**受入:** 複数Familiar、不在、対象なし、部分成立で担当・件数が実処理と一致する。物流の予約・capacity判定は既存ownerに委ねる。

### U30: 配置モードでもマウスdragでカメラを動かす

左drag panのownerはNormalモード限定（`bevy_app/src/plugins/input.rs:80`）。PanCameraのmouse panは無効（`bevy_app/src/systems/settings/apply.rs:71`）で、
範囲drag中はカメラ自体を停止する（`bevy_app/src/plugins/input.rs:164`）。待機中のWASD/ホイールは使用できる。改善対象はマウスdragの制約であり、範囲指定中の移動禁止は誤確定を防ぐ現契約である。
**提案:** 中ボタンまたは専用modifierによる独立したpanを設ける。範囲中も許可する場合は開始点をworld座標で維持し、一次gestureとの所有権を明確にする。
**受入:** BuildingPlace/Move/Zone/Areaでモードを終了せずpan可能。panだけで配置・選択・範囲確定しない。modal開始時のrollbackを維持する。

### U31: 重なり巡回と右クリック対象の関係を明示する

巡回条件は500ms以内・4px以内・候補列一致（`bevy_app/src/interface/selection/input.rs:253`）。
右クリックは選択済み対象とは独立した先頭のnon-Floor/non-TaskArea候補を取り（:212、:220）、ContextMenuが選択を上書きする（`bevy_app/src/interface/ui/panels/context_menu.rs:121`）。
**提案:** 「候補1/3」の表示、修飾クリック等での候補一覧、現在選択した重なり候補を操作する明確な規則を追加する。
**受入:** Soul/資材/建物/Floorの重なりを時間制限なしに選べる。完成Floor/空地でのFamiliar移動契約を維持し、明示的な候補選択後の操作規則を追加する。収納中・非表示対象を新たに選択可能にしない。

### U32: Area履歴とpresetの対象を実行前に見せる

Areaの空history/clipboard/presetは無表示でreturnし、Undoは複数Familiar共通の履歴から対象選択を切り替える
（`bevy_app/src/systems/command/area_selection/shortcuts.rs:54`、:107、:124、:139）。presetは名称なしのサイズ3枠（`hw_ui/src/area_edit/state.rs:117`）。
**提案:** 「○○の範囲移動を戻す」Undo/Redoと保存済み寸法のpresetを表示。空状態はdisabled＋理由にする。既存Area履歴のUI化を先行する。
**受入:** 2体を交互編集してもUndo対象が事前に分かる。空操作は無反応に見えず、履歴上限64件・新規編集でredo消去を維持する。

## 6. 推奨する実装順と代替案

1. **到達性と不整合:** U01〜U08。大規模な見た目変更を必要としない修正から分割し、UI scaleによる到達性も検証する。
2. **管理操作:** U11〜U22・U29〜U32。先に検索・一覧・filter・対象表示をそろえ、比較や配属の負担を減らす。
3. **停止中の計画:** U09を独立した設計判断として扱い、U10・U28の入力設計と整合させる。
4. **発見性:** U23〜U27。対応済みの実経路に案内を結び付け、未実装の操作をHelpへ先行掲載しない。

| 案 | 評価 | 理由 |
| --- | --- | --- |
| 既存Bevy UI・ViewModel・UiIntentを拡張 | 推奨 | capture、domain owner、差分更新、標準widgetを再利用できる |
| 表示行数を増やすだけ | 不採用 | 全対象へ到達する問題を解消せず、常駐負荷が増える |
| Help本文だけ増やす | 補助として採用 | 発見性には役立つが、スクロール欠如・表示競合は残る |
| 一括でUIライブラリ/画面構成を刷新 | 今回は不採用 | 確認済みの小さい欠陥の修正を遅らせ、入力契約の再実装を増やす |
| Pause captureを単に解除 | 不採用 | modal・ロード・未確定gestureの安全性を壊す。時間停止の意味から分離する必要がある |

## 7. 影響範囲・変更の所有者

- widget、focus、scroll、layout、themeは `hw_ui`。ゲーム側の条件・表示model・intent適用は `bevy_app` のroot adapterを維持する。
- Zone preview/commitはcommand/selection owner、Task/Stockpile/Power/Soul Spaの変更は既存domain ownerへ委譲する。
- 基本修正はsave schema変更を必要としない。U24のmetadata、U25/U28の永続設定は互換設計を個別に行う。
- Pause中のcommand適用は `UiDomainCommitSet`、Familiar request、saveのフレーム順序までレビューする。
- 採用項目に応じて `entity_list_ui.md`、`task_list_ui.md`、`info_panel_ui.md`、`state.md`、`settings.md`、`notifications.md`、`save_load.md`、`world-selection.md` を同期する。
- 実装時はHelp manifest/provider/coverageとexact snapshotを再評価する。今回の文書のみのNo impactを将来の実装へ流用しない。

## 8. リスクと対策

| リスク | 対策 |
| --- | --- |
| scroll/row再利用により古いEntityへ操作 | stableな表示keyと適用時再検証、対象消滅・loadのresetを維持 |
| focus/キー変更がゲーム操作へ漏れる | resolverとForegroundUiGateを単一入口にし、同frame抑止を維持 |
| 停止中の予約が即時進行・消費を引き起こす | 許可するcommandと進行systemを区別し、停止前後の状態遷移を検証 |
| 拡大で閉じる操作が消える | 固定header/footerとviewport制約を先に導入 |
| 確認追加で頻繁な操作が煩雑になる | 全world操作へ一律確認を付けず、置換・site全体取消など対象を限定 |
| 表示改善で毎frameの全一覧再生成が増える | dirty/revisionと表示範囲を維持し、大量対象でノード数・更新量を測る |

## 9. 検証計画と今回の結果

### 実装採用後の受入

| シナリオ | 確認事項 |
| --- | --- |
| 左パネル | Entities/Tasks×最小化/展開×検索あり/なし、折りたたみ後検索、両方向Tab |
| 大量対象 | 同条件タスク200件、Familiar 8×Soul 8、通知64件で先頭/中央/末尾へ到達 |
| 時間と入力 | Pause/1x/2x/4x、文脈menu→Esc、Familiar選択時数字キー、hover待ち |
| 配置 | Zone全不可/一部可/Yard境界、失敗直後の再指定、companion途中でHelp/取消 |
| 確認 | overwrite/load/建設site取消、連打、押下後の外release、対象消滅・古いsession |
| 表示とfocus | 1280×720、1920×1080、高DPI、UI scale 0.85/1/1.25、長い日本語、IME、keyboardのみ |
| 保存と復帰 | 設定書込失敗、安全な再試行、load後のpin/selection/通知/focusの旧対象消去 |

コード変更時は `python3 scripts/dev.py check`、警告をerrorにしたworkspace Clippy、`python3 scripts/dev.py verify`、rust-analyzer診断を確認する。
既存の通知native driverは3件fixtureと表示・expiryを確認する（`bevy_app/src/interface/ui/notifications/native_acceptance.rs:463`）。
U03の修正では64件の末尾到達を別途確認する。既存testや品質gateのpassを、一覧の到達性・操作性の証拠として扱わない。
実機確認は `hell-workers-run-native-acceptance` Skillのno-prompt launcherを使い、primaryのvalidation coordinatorへ登録する。
必要なCapture→Memory、独立artifact検証、seal/finalize/storage checkを守る。性能比較の数値は測定するまで提示しない。

静的レビュー完了時の検証状況（後続の計画整備結果は関連計画に記録）:

| 検査 | 結果 |
| --- | --- |
| `python3 scripts/dev.py docs --check` | pass。計画索引は変更なし、提案索引とroot索引を更新 |
| `python3 scripts/check_help_impact.py` | pass、no production changes |
| `git diff --check` | pass |
| ソース参照 | 76か所のファイル存在・行番号の範囲を確認 |
| `python3 scripts/dev.py verify` | pass（exit 0、All quality gates passed）。Python/ポリシー/fmt/workspace check、profiling・通常構成test、追加feature check、Clippy警告0を含む |
| `python3 scripts/dev.py validation check` | 開始時・報告前ともpass。未分類path 0、既存review cacheを維持 |
| rust-analyzer個別診断 | UI plugin入口 `bevy_app/src/interface/ui/plugins/core.rs` はerrors/warnings/hintsすべて0 |

rust-analyzer workspace diagnosticsはverify後の再取得でも `Unexpected response format` を返したため、workspace全体の診断0件の証拠として扱わない。
個別診断の初回取得ではproc-macro未準備と19件のエラーが出たが、RAのcompile-time依存準備がverifyのCargo lockを待つ状態をprocess/lock情報で確認した。
verify終了後、同じファイルの再取得で診断0件を確認した。コードや共有解析環境の設定変更・再起動は行っていない。
新しいnative job、専用worktree、binary copyは作成しておらず、他作業のreview-active cacheは維持している。

## 10. Help影響と参照基準

**今回の判断: No impact。** 変更は本提案書・関連計画と文書索引のみ。実入力→UiIntent→root/domain consumer、
`build_help_panel_content`→静的Help treeの経路とruntime文言を変更しない。
ゲームは `docs/*.md` をruntimeのHelp本文として読み込まないため、未採用の提案がプレイヤーへ露出することもない。
`hell-workers-review-help-impact` Skillに従い、文書差分とこの経路から判断する。gateのpassだけを根拠にしない。

一般的な操作性の観点には、Microsoftのゲーム向け一次資料も参照した。
一貫した操作、予測可能なfocus、複雑な情報への検索手段は [Xbox Accessibility Guideline 112](https://learn.microsoft.com/en-us/xbox/accessibility/xbox-accessibility-guidelines/112) の考え方と整合する。
文字サイズ・拡大時の読みやすさは [XAG 101](https://learn.microsoft.com/en-us/xbox/accessibility/xbox-accessibility-guidelines/101)、入力手段と割当は [XAG 107](https://learn.microsoft.com/en-us/xbox/accessibility/xbox-accessibility-guidelines/107) を比較基準とした。
本書はこれらへの適合認証・全項目監査ではない。
ScrollAreaの仕様は [Bevy 0.19 ScrollArea](https://docs.rs/bevy_ui_widgets/0.19.0/bevy_ui_widgets/struct.ScrollArea.html) とローカルregistry sourceで確認した。

## 11. 提案後に決まったことと残る評価

- 停止中の許可操作は [状態管理](../state.md) のallowlistを採用。配属等への拡張は後続候補O05。
- 単クリックで詳細を開き、明示「現地へ」でcameraを移動。固定は独立する。
- 主な操作入口を日本語化。全面翻訳・文字のみ拡大・演出量は後続候補O04。
- 1280×720 / 1920×1080、UI倍率0.85 / 1 / 1.25で描画確認済み。実OS高DPI、実操作、初見理解は [未検証事項と再検証条件](../ui-world-first.md#未検証事項と再検証条件)で追跡する。

## 12. AI引継ぎメモ

- 現在地: U01〜U32と地図中心UIの実装・デザインレビューは完了。コード編集を他agentへ委譲しない。
- 計画はクローズ済み。[未検証事項と再検証条件](../ui-world-first.md#未検証事項と再検証条件)を残し、必要時に現行subjectで再検証する。旧提案を継続中の作業指示と解釈しない。
- `crates/` から始まらないコード参照はすべて `crates/` 配下の相対path。行番号は調査対象HEADの目印であり、将来変更時はsymbolで再確認する。
- 詳細の受入条件を独立した修正単位に分け、表示確認と状態遷移の検証を両方行う。
- 既存のHelp coverage、blocked capability、modal ancestry、positive allow-list、save revision/session guardは維持対象。

完了条件（今回の提案）:

- [x] UI領域の調査範囲と既存の強みを記載
- [x] 優先順位、実装根拠、操作場面、改善案、受入条件を記載
- [x] 静的確認と実機未確認を区別
- [x] 影響範囲、リスク、実装時の計画・検証・Help更新手順を記載
- [x] 文書索引・Help影響・品質gate・storage結果を確定

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-13 | Codex | UI全領域の静的レビューと操作性向上32項目を作成 |
| 2026-09-14 | Codex | コード根拠・受入条件を再レビューし、verify/Clippy・文書・Help・storageのpassと解析ツールの確認範囲を記録 |
| 2026-09-14 | Codex | U01〜U32を対応付けた実装計画へリンクし、引継ぎ先と検証記録の範囲を同期 |
| 2026-09-17 | Codex | 実装承認を反映し、調査当時の本文と現行仕様・残る受入の参照先を分離 |

2026-09-17: UI改善計画をクローズし、未検証事項・任意拡張は現行仕様へ引き継いだ。全受入のpassを意味しない。
