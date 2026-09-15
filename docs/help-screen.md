# プレイヤーヘルプ画面

プレイヤー向け Help は、現在到達可能な操作と完了可能なワークフローをゲーム内で確認するための
モーダル画面です。ボトムバーまたは Pause メニューの `Help`、あるいは `F1` で開きます。

## 操作と時間

| 操作 | 結果 |
|:--|:--|
| `F1` / Help button | 通常、開いたメニュー、配置・範囲指定中、Pause から Help を開く |
| `F1` / `Escape` / Close button | Help を閉じる |
| `ArrowUp` / `ArrowDown` | 前後の topic を選ぶ |
| `PageUp` / `PageDown` | 本文を一画面単位で移動する |
| `Home` / `End` | 本文の先頭 / 末尾へ移動する |
| `Tab` / `Shift+Tab` | 前景ダイアログ内の可視・有効な操作へ循環移動（本文scroll外も対象） |
| `Enter` / `Space` | フォーカス中のボタンを決定。入力欄では編集を優先 |
| mouse wheel / scrollbar | navigation または本文をスクロールする |

項目を切り替えると本文の読書位置を項目IDごとに保持し、戻ると復元する。同じ項目の再選択では先頭へ戻さない。
閉じて開き直すと最後の項目を開き、非表示中のlayout補正で本文offsetが失われても保持した値を復元する。
保持は実行時のみで、world loadによるHelpPanelState reset時に破棄する。
検索欄は見出し・本文を小文字化して空白区切りAND部分一致で検索する。結果はstable entry IDに結び、選択後のlayout確定時にScrollIntoViewで該当見出しを表示する。検索語変更でnavigation scrollは先頭へ戻り、空欄で通常の目次を戻す。Enterは編集を終了し、Escは検索を消して編集を終了する（同frameのHelp閉鎖やゲームshortcutへ伝播しない）。
情報パネルの「詳しく（ヘルプ）」は既存OpenHelp capture経路を使用し、受理されたopenerが同ボタンの場合だけ表示中のinspectionからentryを選ぶ。Soul、Stockpile、Soul Spa、Power以外は情報パネルの説明へ進む。検索入力と結果は静的な検証済catalogだけを参照し、ゲーム状態を変更しない。

Save/Load catalog、Settings、Operation dialog、Load confirmation が前景の場合は Help を開きません。通常時に Help を開くと
`Time<Virtual>` を一時停止し、閉じたときだけ直前の相対速度で再開します。すでに Pause 中だった場合は
Help を閉じても Pause を維持します。Help の開閉は `MenuState`、`PlayMode`、`TaskMode`のvariant、
Architect categoryを維持します。ただし受理frameに未確定のpointer dragがある場合は、その開始位置だけを
`None`へrollbackして同じmodeの待機状態へ戻します。

## 所有権

Help は widget schema とゲーム固有 catalog を分離します。

| 所有者 | 内容 |
|:--|:--|
| `hw_ui::help` | opaqueなsection/topic/entry ID、sealed本文/固定copy/shortcut chrome DTO、型付き`HelpChromeSlot`、`HelpPanelState`、marker |
| `hw_ui::setup::help_panel` | hidden UI tree、navigation、共有本文ScrollArea、標準Scrollbar |
| `hw_ui::interaction::help` | topic reducer、keyboard scroll、表示・選択色 |
| `bevy_app::interface::ui::help_content` | feature manifest、owner別provider、固定chrome copy、surface coverage、catalog validator |
| `bevy_app::interface::ui::help_controller` | accepted capture、Help起因pauseの所有権、load reset |
| `bevy_app::input_actions` | canonical binding、context arbitration、key label formatter |

`HelpPanelContent` は plugin build 時に一度だけ検証・構築し、Startup で静的UI treeへ渡します。runtime は
`docs/*.md` を読み込まず、topic切替でもノードを再生成しません。

## capture と前景順序

Help は `MenuState` ではなく独立した full-viewport capture overlay です。入力と描画は同じ順序を使います。

```text
LoadConfirm > Save/Load catalog > Help > Settings > Pause > OperationDialog
```

各capture rootは`GlobalZIndex(20_050 .. 20_010)`を使います。Help button/F1は
`PendingWorldInputCapture`でHelp rootが実際に優先度勝者となった場合だけ受理されます。受理frameから
focus clear、world selection抑止、camera guardが有効になり、通常/active modeから開いた場合だけ未確定gestureを
一度rollbackします。Pauseからのhandoffはcapture継続なのでrollback latchを再発火しません。
button経路は保存済みのforeground値だけに依存せず、そのframeで表示中のcapture rootから実効foregroundを
再計算します。このため、Helpを閉じた直後に前frameのHelp ownershipが残っていても再表示やPause操作を誤って拒否しません。

説明表示のHoverTooltipはcapture rootとは別に`TOOLTIP_LAYER = GlobalZIndex(20_060)`を使う。
ボタンの子に付け替えてもモード案内の背後へ隠れないための描画順であり、上記の入力所有権を変更しない。

## catalog の正本

`crates/bevy_app/src/interface/ui/help_content/manifest.rs` の `player_help_features!` が
player-facing feature、owner、provider、section/topic順の唯一のinventoryです。providerは次のowner単位に分かれます。

- `getting_started`
- `camera_selection`
- `familiars`
- `orders_building_zones`
- `soul_energy`
- `save_settings_notifications`

`coverage.rs` は `InputAction`、`UiIntent`、Help navigation、`MenuState`、`PlayMode`、
building/resource/work/task/time/zone/stockpile/transport surfaceとTask Dashboardのcontrol/filter/sortを
exhaustive matchで分類します。各top-level variantは1つ以上のstable surface IDを持ち、macroは複数variantを
1行のor-patternへまとめられない形に制限します。runtime catalogへ表示できるのは
`Player + Published`だけです。内部実装・debug・dependency既定入力は理由付き`Excluded`、
未完成flowは表示先、理由、解消owner付き`Blocked`にします。validatorはcatalog entryとPublished entryを
双方向で完全一致させ、Blocked entryの混入、存在しないPublished entry、launcher欠落、
`HelpPanelChrome`に存在しない汎用chrome扱い、stable surface IDの重複を拒否します。

`FamiliarBuild`は通常完了consumerが未整備の間、canonical bindingを持たず、
`Blocked(MissingCompletionConsumer)`として掲載しません。外部から`SelectBuildTarget` intentが届いても
既存mode/menuを変更せず拒否します。これを実装するときは、
実際のtarget選択・assignment・完了consumer、到達可能behavior test、coverageの`Published`分類、provider entry、
approval snapshotを同じ変更で追加します。現在は部分実装だけで公開へ切り替えられるcapability switchを持ちません。
Architectから始める建築workflowは別の完了可能flowとして掲載します。

`TaskMode::DesignateDeconstruct`はTrack C1 M3〜M4でplayer入力、hover preview、結果通知、Operation dialog、
Task Dashboardと全BuildingType / Operational Soul Spaのowner cleanupまで接続済みです。`WorkType::Deconstruct`、TaskMode、UiIntentは
`building-deconstruction` entryへ`Published`として紐づきます。HelpはOrdersからの単一建物指定、
Familiarごとの許可、優先度変更・確認付きキャンセル、安全な資材退避を待つblocked state、
structure / Room / Powerの撤去後再計算を説明します。Constructing Soul Spaの専用cancelは
`soul-energy-recovery` entryで実搬入Bone返却とpause中の操作条件を説明します。

project-owned shortcut文字列は`DEFAULT_BINDINGS`から生成します。provider本文へ同じキー名を重複記載せず、
新しいpublic keyをformatterが扱えない場合はcatalog構築を失敗させます。カメラのdependency既定入力や
rename widget固有の編集キーは、型付き`InputAction`を持たない明示的な例外です。

world pointerのplayer契約は`camera-selection` topicの`world-selection`と
`world-object-actions`へ掲載します。前者はscreen-space snap、release確定、drag slop、重なり巡回、
footprint選択、後者はentity context menu、Familiar地面移動、Door lock、移動可能BuildingのMoveを扱います。
詳細なruntime契約は[world-selection.md](world-selection.md)を正本とします。
`coverage_approval.snap`はlauncherのlabel/tooltip、画面title・閉じる・navigation・shortcut接頭辞と
それらのrender結果、launcher/chrome shortcut、section/topic/entryのID・title・全paragraph・shortcut、
feature/owner、およびstable surface ID、Published target、Excluded reason、Blocked target/reason/ownerを
完全一致で承認します。空白だけの変更やtest内の期待値変更では承認できません。固定copyも
`build_help_panel_chrome()`でrootから注入するため、`hw_ui`側に未承認のプレイヤー文言を置きません。

## 機能追加・変更時の更新手順

player-facing featureを追加する場合は、同じ変更バッチで次を行います。

1. `player_help_features!`へstable feature ID、owner、provider、表示順を追加する。
2. owner providerへstable topic/entry IDと短いプレイヤー文を追加する。
3. 新しい`InputAction`、`UiIntent`、domain enum variantをstable surface ID付きで
   `coverage.rs`のPublished/Excluded/Blockedへ分類する。
4. canonical binding・label・workflowが変わる場合は生成shortcutとキー非依存のprovider本文を更新する。
5. root testの正規化出力から`coverage_approval.snap`を再生成し、全差分をレビューする。
6. blocker/reachability、入力競合、成功・失敗結果のbehavior testを更新する。

```bash
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 regenerate_help_approval_snapshot -- --ignored
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 \
  exact_snapshot_approves_all_player_visible_help_copy_and_coverage
```

1つ目は通常のworkspace testでは実行されない更新専用testを明示的に起動し、2つ目の常時read-onlyな
exact testで一致を再検証します。生成後は
`coverage_approval.snap`の本文、順序、shortcut、owner、coverage判断をすべて読み、意図しない変更を承認しません。

機能を削除する場合はmanifest、provider、surface linkを同じバッチで削除します。内部変更でHelpへ影響しない場合も、
判断をバッチに残す必要があります。

実装後のAgentレビューでは`hell-workers-review-help-impact` Skillを使い、gateの成否だけでなく今回の差分が通る
実入力・成立条件・player-visible consumerを確認します。mixed worktreeの既存Help更新を、今回の変更が反映済みで
ある証拠として流用してはいけません。

## Help impact gate

`python3 scripts/dev.py verify`は`scripts/check_help_impact.py`を実行します。diff base以後のproduction変更
（test専用fileを除くRust、Cargo/build、runtime text data）を検査し、root所有`help_content/`配下の
production Rustまたはexact snapshotが直接検証する`hw_ui/src/help.rs`のtyped rendererと、
`coverage_approval.snap`の同時更新、あるいはno-impact判断が全production commitの子孫に
ある場合だけ通過します。このためmerge履歴でも、一方のbranchだけを
確認した古い判断は有効になりません。test、README、fixture、approval snapshotだけの変更はHelp更新として
扱わず、Help sourceだけの変更もexact snapshotがなければ承認しません。Helpを変更しない場合は、
production変更を取り込んだcommitへ次のtrailerを付けます。

```text
Help-Impact: none
Help-Impact-Reason: Internal cache only; no player-visible input, label, or workflow changed
```

commit前のローカル検証では、一時的に`HELL_WORKERS_HELP_IMPACT_REASON`へ空でない理由を指定できます。CIはこの
overrideを受け付けず、non-zeroで解決可能かつ`HEAD`とmerge-baseを持つ`HELL_WORKERS_DIFF_BASE`、Help更新、
またはcommit trailerを要求します。decision後のdirty production変更や並行branchのproduction変更は古い判断を
無効にします。merge commitは一時object storeで再構築したGitの自動merge treeと記録treeを比較し、親から
運ばれた変更を再計上せず、手動resolution/merge中の追加編集だけをmerge固有変更として扱います。rename、
staged/unstaged、未追跡fileも判定対象です。

ローカルで`origin/master`とのmerge-baseを解決できない場合、root commit以外は`HEAD^`へ縮退せず
fail-closedで停止します。`origin/master`をfetchするか、`HELL_WORKERS_DIFF_BASE`を明示してください。

自動tree再構築の対象はGit標準の2-parent mergeです。octopus mergeまたは自動treeを再構築できない履歴は
gateをfail-closedで停止します。標準と異なるmerge strategy/optionで記録treeに差が出た場合、その差は安全側に
merge固有変更として扱われ、新しいHelp判断が必要になることがあります。

runtime label/dataの新しいsource rootまたは拡張子を導入する場合は、
`check_help_impact.py`のpath分類とfixtureを同じ変更で追加してください。

## 検証

```bash
python3 scripts/check_help_impact.py
python3 -m unittest scripts.tests.test_check_help_impact
python3 scripts/dev.py cargo -- test -p hw_ui help
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 help_
python3 scripts/dev.py verify
```

## 共通ボタンとフォーカス（U28）

ボタンは共通collectorが左press時のEntity・操作payload・foreground・WorldEpochを保持し、同じ対象上のreleaseだけを1回受理する。途中で外へ移動、非表示、disabled、payload変更、対象消滅、world置換、window/cursor喪失、Soul drag、world pointer claimがあれば取消。Interactionはhover/pressの描画と長押しdrag検出に残し、業務actionはUiInputStateの受理集合だけを消費する。

modalのTab/Shift+Tab/Enter/Spaceはroot canonical bindingで解決する。UI tree順でButton/EditableText/Slider/Checkboxを列挙し、祖先を含む非表示・disabledを除外する。決定ボタンは次のPreUpdateでfocus・payload・foreground・epochを再照合して共通collectorへ渡し、capture prepassより前に受理する。長押しのrepeatを新しい確認画面へ持ち越さない。Slider/Checkboxの値変更キーは既存Bevy widget ownerを使う。

表示更新後にfocusを前景へ限定し、初期位置は戻る/閉じるを優先する。選択箇所に枠を表示し、ScrollIntoViewで見える位置へ移動する。閉じると生存・表示中の呼出元へ戻し、入れ子を戻る場合も親の位置を復元する。world置換では復帰履歴を捨てる。入力欄が消費したEsc/Enter等は同frameのoverlay操作へ伝播しない。

### システムメニューと任意ガイド

Space/時間速度は時間だけを変更し、Menu/未処理Escapeは独立したSystemMenuStateを開閉する。
停止中の計画allowlistと制限は`time-controls`へ公開し、canonical ToggleSystemMenuを同entryへcoverage登録する。
任意のWorkGuideはHelp下部の開始ボタンから起動する。固定開始文言はsealed HelpPanelCopySpecのguide_start_labelとしてrootが供給し、
catalog validation・sentinel fixture・exact approval snapshotに含める。StartWorkGuide/EndWorkGuideは`getting-started-work-loop`へ公開する。
実際の使い魔/範囲/採取指定/Gather進行をrootのreadonly adapterが確認し、対象消滅時の戻りとload時終了をテストする。
ガイドはviewport高さの60%を上限とし、本文を標準ScrollArea/Scrollbarでスクロールする。
閉じる/スキップはscroll body外に固定し、長い使い魔名や案内本文によって終了操作が押し出されるのを防ぐ。
手順や停止状態によって案内が更新されたときだけ本文を先頭へ戻し、同じ案内を読む間は位置を保つ。
