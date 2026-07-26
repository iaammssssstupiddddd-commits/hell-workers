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
| mouse wheel / scrollbar | navigation または本文をスクロールする |

Settings、Operation dialog、Load confirmation が前景の場合は Help を開きません。通常時に Help を開くと
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
LoadConfirm > Help > Settings > Pause > OperationDialog
```

各capture rootは`GlobalZIndex(20_050 .. 20_010)`を使います。Help button/F1は
`PendingWorldInputCapture`でHelp rootが実際に優先度勝者となった場合だけ受理されます。受理frameから
focus clear、world selection抑止、camera guardが有効になり、通常/active modeから開いた場合だけ未確定gestureを
一度rollbackします。Pauseからのhandoffはcapture継続なのでrollback latchを再発火しません。
button経路は保存済みのforeground値だけに依存せず、そのframeで表示中のcapture rootから実効foregroundを
再計算します。このため、Helpを閉じた直後に前frameのHelp ownershipが残っていても再表示やPause操作を誤って拒否しません。

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

project-owned shortcut文字列は`DEFAULT_BINDINGS`から生成します。provider本文へ同じキー名を重複記載せず、
新しいpublic keyをformatterが扱えない場合はcatalog構築を失敗させます。カメラのdependency既定入力や
rename widget固有の編集キーは、型付き`InputAction`を持たない明示的な例外です。
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
cargo test -p bevy_app@0.1.0 regenerate_help_approval_snapshot -- --ignored
cargo test -p bevy_app@0.1.0 \
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
cargo test -p hw_ui help
cargo test -p bevy_app@0.1.0 help_
python3 scripts/dev.py verify
```
