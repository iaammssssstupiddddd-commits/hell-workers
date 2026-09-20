# Orcaによる並列実装と専任レビューの運用素案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `orca-parallel-development-proposal-2026-09-20` |
| ステータス | Review — Orca導入済み、運用基盤は計画化 |
| 作成日 / 最終更新日 | 2026-09-20 |
| 作成者 | Codex |
| 対象 | `stablyai/orca`、Hell Workersのローカル開発運用 |
| 調査基点 | `6abeeed92cf14e2e5aafc8240e1c64989b609a84`。primaryには別作業のdirty変更がある |
| 関連計画 | [Orca導入・仕様確認と並列開発基盤の実装計画](../plans/orca-parallel-development-plan-2026-09-20.md) |
| 関連Issue/PR | N/A |

本書は提案であり、現行の編集用サブエージェント禁止を解除しない。
2026-09-20にOrca 1.4.205のuser-local導入・primary登録・terminal smokeを完了した。
role設定、エージェント起動、資源制御の実装は未実施。以下の数値は試行用の初期設定案であり、実測結果ではない。
導入実績、固定版の仕様と本素案への補足、後続工程の正本は関連計画とする。

## 1. 背景と問題

独立した実装を並列化し、レビュー担当を1エージェントに固定して、作業待ちと判断のばらつきを減らしたい。
現在の禁止規則は、共有checkoutの競合、範囲外変更、進捗を追えない編集委譲への対策である。
別worktreeはファイルの衝突を減らすが、共有APIの不整合、Git管理情報、CPU/RAM/GPU、統合後の回帰は別に扱う必要がある。

| 現行の根拠 | 導入時に埋める差 |
| --- | --- |
| [AGENTS.md](../../AGENTS.md)のBackground Agent Policy | 編集はmain agentのみ。専用worktreeでの限定委譲を認める規則への変更が必要 |
| [開発ガイド](../DEVELOPMENT.md)と`scripts/build_lane.py` | 2つのlaneはworkspace内のsession所有。worktreeが増えるとホスト全体の上限にはならない |
| `scripts/cargo_runtime.py::workspace_cargo_target`、`scripts/build_coordination.py::activity_lock_path` | targetとactivity lockは各workspace基準。別worktreeの通常buildとnative実行を相互排他できない |
| [rust-analyzer共有運用](../development-infra/rust-analyzer-mcp.md) | 別worktreeは別backend。エージェント数に加え、解析対象数でも常駐メモリが増える |
| [検証データ管理](../development-infra/validation-storage-workflow.md) | primaryのcoordinatorが保持・終了を管理。Orca側の削除操作と自動では連動しない |

Orcaについて2026-09-20に確認した一次資料:

- [公式repository](https://github.com/stablyai/orca): 複数CLI agent、端末、差分レビューを扱う開発環境。
- [Worktrees](https://www.onorca.dev/docs/model/worktrees): worktreeの作成・既存treeの表示・削除、ignored directoryの共有機能。
- [Settings](https://www.onorca.dev/docs/settings): project用Quick Commands、agent startup hook、worktree作成時のcommand設定。

これらは製品機能の確認であり、本プロジェクトのロック・権限・検証手順との互換性の実証ではない。
導入した1.4.205のtag sourceと同梱CLIから、setupの並行起動既定、既存treeでのsetup省略、
agentのYOLO引数既定、nativeのTask/Dispatch/gateを追加確認した。具体的な制御境界は関連計画§3〜5を参照する。
Orcaのversionは試行開始時に照合し、起動hookの完了順・子process・終了時の挙動をその版で確認する。

## 2. 目的（Goals）

- 独立した2件の実装を、担当範囲と進捗が見える状態で進める。
- 専任レビュー1名が個別差分と統合結果を確認し、編集者の自己承認を防ぐ。
- 全参加worktreeを横断して重い処理を制御し、既存のnative受入・storage・品質gateを維持する。
- 通常の単独開発へ戻せる小さな試行で、総所要時間と手戻りの改善を判定する。

## 3. 非目的（Non-Goals）

- 同じ課題を多数の実装agentへ競争させる運用、workerによる再委譲、無制限の常駐。
- Bevyの実機検証やHelp impact reviewを、OrcaのUI操作・agentレビューだけで代替すること。
- raw Cargo実行、凍結subjectへの新driverのコピー、自動merge・push・PR公開の包括許可。
- 複数OSユーザーや別hostまで一括制御する常駐サービスの初期導入。

## 4. 提案内容（概要）

**統括1・実装2・レビュー専任1を上限に、実装は並列、統合は直列とする。**
初期の重い実行枠はホスト全体で1件とし、実装agent数とbuild並列数を分離する。
Orcaは作業・端末・差分の表示と起動に使い、実行許可はrepository側のdriverと台帳で決める。

| 役割 | 担当 | 書き込み範囲 / 禁止事項 |
| --- | --- | --- |
| 統括 / integrator | タスク分割、共有仕様、担当登録、順次統合、検証起動、完了判断 | integration候補とprimaryの正本文書。稼働中workerのtreeへ書き込まない。自分の変更も専任レビューに出す |
| 実装A | タスクAの実装、局所検証、指摘修正 | A専用branch/worktreeの許可pathのみ。共有設定変更・他tree操作・自己承認・再委譲は禁止 |
| 実装B | タスクBの実装、局所検証、指摘修正 | B専用branch/worktreeの許可pathのみ。同上 |
| レビュー専任 | 個別差分・設計契約・統合差分・検証証拠の審査 | sourceは読み取り専用。指摘は統括へ返す。コード修正・commit・承認対象の書換え・worker兼務は禁止 |

レビュー担当は同時に1名。セッション再開時は同じ役割を引き継ぎ、base/head、未解決指摘、承認範囲を読み直す。
固定するのは責務と承認履歴であり、無期限の会話contextや特定モデルへの依存ではない。
離脱中は新しい承認を止める。交代が必要なら旧担当を停止し、統括が記録を渡して1名体制を維持する。

## 5. 詳細設計

### 5.1 タスクの分割と所有権

各タスクに以下の依頼票を持たせ、統括がprimaryの実装計画へ集約する。
workerの報告はmessage/差分として渡し、workerはprimary文書を直接更新しない。

| 必須項目 | 内容 |
| --- | --- |
| 識別 | task ID、owner/session、branch、exact worktree path、base full SHA |
| 変更範囲 | 許可path、禁止path、共有型・イベント・system順序への影響、依存タスク |
| 成功条件 | 再現条件、期待挙動、不変条件、必要test、Help/nativeの必要性 |
| 提出物 | head full SHA、差分概要、検証command/result、未解決事項、文書更新案 |
| 寿命 | worktree/targetのconsumer、next_action、release_when、実測bytes |

「別crateだから独立」とは判定しない。例えばSoul task実行とFamiliar割当は共有payloadや予約規約に依存する。
共通API、enum variant、Messages/Events、save schema、system setの順序を変更する場合は、
統括が先に契約を確定し、必要な基盤変更をレビュー・統合してから同じbaseでworkerを開始する。
実装中に契約変更が必要になったworkerは該当作業を止め、統括が範囲と依存を再分割する。

`Cargo.toml` / `Cargo.lock`、toolchain、共有型、root plugin wiring、CI/driver、Help manifest/snapshot、
正本文書・索引・agent規則は同一batchでwriterを1名にする。必要なら統括がその変更自体を行い、専任レビューへ出す。
単なるmerge conflictの不在は独立性の証拠にしない。

初回対象は、共有契約を変えない異なるleaf moduleの局所修正2件とする。
crate境界変更、renderer全面変更、save互換変更、agent制御基盤自身の変更は初回の並列対象にしない。

### 5.2 Worktreeと権限

- 実装A/Bは別branch・別worktree・別`target/`を持つ。同じ候補は修正・レビュー期間中そのまま再利用する。
- primaryは文書正本とcoordinatorの入口。並行作業でdirtyな場合は統括がbranchを切り替えず、
  code統合用の専用候補を用いる。文書をprimaryで更新する際は所有者と編集範囲を調整する。
- worktreeはprimaryの`target/`配下を避け、永続disk上の明示した場所へ置く。
  採用時はstorage規則に「登録された編集隔離用途」の作成条件を追加し、比較用途のみという現行条件との不整合を解消する。
- reviewerは提出中のworker treeを読み取る。提出者はその審査中に書き換えず、専用の重いreview build cacheを増やさない。
  統合レビュー時はintegration候補を同様に固定する。
- worktreeはセキュリティ境界ではない。workerは他tree・primary・共有Git設定へwriteしない権限設定を使い、
  reviewerはsourceへのwrite権限を持たない。Git commitに必要な共有metadata権限は起動adapter設計時に限定する。
  手元CLIでこの境界を実現できない場合、指示文だけで制約成立とせず、sandbox/実行brokerの整備まで自律並列編集を有効化しない。
- 許可外path、symlink経由の越境、renameによる範囲外変更を検出する。提出時は追跡済みだけでなく未追跡変更も確認する。
  workerのcommitは割当branchの更新だけを許可する。その他の共有refs、config、hooks、branch削除、
  worktree管理は統括に限定する。Git metadataを広くwrite可能にする代わりに、必要なら限定commandをbrokerへ委ねる。
- Orcaのshared pathsへ`target/`、runtime save/settings、検証jobを登録しない。
  role用hookはsourceを改変せず、worktree/base/権限/driver versionの確認に使う。設定名・具体的CLIは実装時に確定する。

### 5.3 ホスト全体の資源制御

初期対象は同じLinux host・OSユーザーの参加sessionすべて（Orca外のterminalを含む）。
repoやGit common directoryごとのロックだけでは独立cloneを横断できないため、
host/user共通のcanonical lock namespaceを設け、識別をrepo pathから独立させる。
namespaceはowner-onlyとし、process間で同じ実体になることを検査する。別OSユーザーの負荷は外部負荷として扱う。
具体pathとAPIは実装計画で固定する。現行コードにこの機能はない。

| 資源 | 試行時の設定案 | 契約 |
| --- | --- | --- |
| 実装worker | 最大2 | 再委譲禁止。レビュー待ちを含め1人1候補。待ち候補が2件なら新規実装の投入を止める |
| 重い実行枠 | 全参加worktree合計1 | compile/check/test/clippy、link、重いtooling test、feedback実行を対象。統合検証も同じ枠を消費 |
| Cargo内部並列 | 初期`jobs=1` | test実行thread/processは別に制限。Cargo jobsだけでテスト負荷まで制御したことにしない |
| rust-analyzer | activeなbackendは初期最大2 | 別tree間のbackend共有はしない。自動check/buildも実行枠に入れるか無効化。reviewerはまず差分・source・既存診断を読む |
| native / GPU / 性能測定 | ホストで1件、排他 | buildからCapture→Memory終了まで他の重い処理・feedback windowを開始しない。性能採取時は関連background解析も静止 |
| RAM / disk | 現行guardを維持 | 既存RAM下限8 GiBは開始時条件。予約可能量や全工程のpeak保証ではない。実測peakと外部負荷から余裕を評価 |
| build cache | 候補ごとに固定 | agent数に応じてlaneを2本ずつ確保しない。1 writerのworkerは通常targetを再利用。primary既存laneは維持 |

最小構成では重い実行枠1件でnativeとの排他も満たす。2件へ拡張する場合は、
通常処理用の共有許可とnative用の排他許可をhost側に設け、workspace内の既存leaseも維持する。
raw Cargoや別`CARGO_TARGET_DIR`で枠を回避する起動は許可しない。

取得順は全経路で「host許可 → 実行slot → workspace activity lease → 子process開始」に統一する。
session用laneは事前に固定するが、待機中にhost/実行/activity枠を握り続けない。
MemAvailableと空きdiskはslot取得後・子process開始直前に再確認する。
親helperから子driverを呼ぶ場合は検証可能なleaseを引き継ぎ、二重取得によるdeadlockを防ぐ。

既存driverのbusy停止を維持し、投入順は統括のqueueで管理する。無限retryや細かいpollを行わない。
通常はFIFO、再レビューに必要な修正検証・統合検証を優先できるが、待機taskの順序と理由を記録する。
native予約後は新しい通常処理を入れず、既存処理の終了を待ってstarvationを防ぐ。

leaseは実際の子process終了まで有効にする。terminal/Orcaが閉じたことだけでは枠を解放しない。
取消・親の異常終了・子が残る場合のprocess group管理と回復を試験する。
稼働不明時にロックfileを削除して空き扱いにせず、識別可能なownerと子processを確認する。

既存の凍結native subjectへ新driverを上書きしない。primaryのcoordinatorが外側でhost許可を取得し、
旧helperを起動する互換経路を用意する。旧driverを使う未登録sessionが稼働中なら試行を開始しない。

### 5.4 レビューと統合

状態は次の順で進める。書換え・base変更・指摘修正後は新しいheadとして再提出する。

`Assigned → Implementing → Ready for review → Changes requested / Reviewed → Integrating → Integrated review → Validating → Accepted`

1. 統括が共通baseと依頼票を固定し、実装A/Bを開始する。
2. workerは局所検証を行い、未コミットsourceを残さずhead SHA・検証結果・文書案を提出する。
   gate対象外のignored runtime dataが検証に関わる場合も、asset/設定の識別情報を添える。
3. reviewerは1件ずつ、base→head、範囲逸脱、契約、不変条件、testの妥当性、Help影響を確認する。
   必須修正と任意提案を区別し、必須修正未解決または重要な未検証事項があれば承認しない。
4. 指摘修正は元workerが同じtreeで行う。reviewerはコードを直さず、新headの変更と指摘解消を確認する。
5. 統括はレビュー済みheadをintegration候補へ1件ずつ取り込む。
   baseが進んだ場合は契約の差を確認する。衝突解消や追加修正を無審査の統合作業として扱わない。
6. 同じreviewerが実際の統合head、worker間の相互作用、統括が変更した文書・wiringも確認する。
   個別headの承認は統合headの承認へ自動転用しない。
7. 統括が統合headで必要群のCIまたはlocal gate、必要なnative受入を実行する。
   reviewerはbase/head/tested SHA・scope・結果を照合し、追加修正なら再レビュー・再検証へ戻す。
8. 統括がHelp判断・storage整理を含む完了条件を確認する。push/PR/merge等の外部公開はタスクの許可範囲に従う。

承認記録にはtask ID、reviewer、base/head full SHA、対象差分、指摘と解消状況、検証対象SHA、未検証範囲を残す。
Orcaの会話履歴だけに依存せず、primaryの現行計画または許可されたPRへ簡潔に集約する。
reviewerが停止・交代した場合やcontextが失われた場合も、この記録から未審査範囲を復元する。

### 5.5 保持・終了

worktree/targetはowner・consumer・bytes・next_action・release_whenをprimaryで管理する。
レビュー待ちは保持理由になり、日数・無応答だけでは撤去しない。終了した検証jobは候補cacheと別に整理する。
Orcaの削除機能から台帳確認を迂回しない。初期試行の撤去は統括が既存storage手順で行い、Orcaには表示を追従させる。
専用treeを削除する前に未統合commit、dirty/ignoredの独自成果、稼働process、他consumerを確認する。
branchの存続だけで未コミット成果が保全されたとはみなさない。

### 5.6 現行ルールの改定案

採用時にBackground Agent Policyを次の趣旨へ置き換える。**この引用は未発効の案である。**

> 編集委譲は、統括が登録したタスクと専用branch/worktree、許可path、単一writer、
> host資源制御、専任レビューを満たす場合に限り許可する。
> workerはprimary・他worker tree・共有Git設定を編集せず、再委譲しない。
> reviewerは1名、source読み取り専用とし、自己実装の承認を行わない。
> 統合は統括だけが直列実行し、レビューと検証を実際の統合headへ結び付ける。
> これらの前提を満たせない場合は、main agentの単独編集へ戻す。

AGENTSだけでなく、CLAUDE/GEMINI/Cursor等の対応規則、検査、Skillの関連契約も同じ変更batchで整合させる。
提案段階では実際の規則・Skill・起動設定を変更しない。

### 5.7 変更対象（想定）とデータ

| 対象 | 想定変更 |
| --- | --- |
| `scripts/dev.py`、`build_coordination.py`、`build_lane.py`、`cargo_runtime.py` | host許可・slot・継承・busy理由。既存workspace内leaseとの組合せ |
| `scripts/validation_storage.py`とnative/perf起動adapter | 登録済み編集treeの保持、凍結subjectの外側からのhost制御 |
| rust-analyzer MCP adapter / IDE設定 | backend上限の運用、暗黙buildを含む負荷管理 |
| agent規則・workflow・対応検査 / Skill同期 | 限定委譲、役割、正本文書owner、終了条件の整合 |
| Orca project設定 / role起動adapter | 登録taskの起動・権限・cwd確認。具体schemaは固定版で確認後に決定 |
| `docs/DEVELOPMENT.md`、storage/解析運用仕様、root README | 採用した通常運用と退避手順を反映 |

新規データはtask依頼票・review記録・host lease metadata。game component、save schema、runtime APIの変更は不要。
初期は依頼票・review記録を計画内の表で扱い、新しい汎用タスク管理サービスを作らない。

## 6. 代替案と比較

| 案 | 判断 | 理由 |
| --- | --- | --- |
| 既存terminalで単独編集 | fallback / 比較基準 | 資源と統合が単純。並列化の改善幅を測る基準 |
| Orcaで単独編集＋review | 導入前段として採用 | UI・権限・起動互換を確認できる |
| 実装2＋review1＋統括 | 本提案 | 独立taskの並行進行と一貫したreviewを両立しやすい |
| 実装を3名以上へ拡張 | 初期は保留 | review・統合待ちと解析cache増加を先に測る |
| worktree間でtargetをsymlink共有 | 不採用 | 候補ごとのcache寿命・検証所有権を曖昧にし、切替再buildも招く |

## 7. 影響範囲

- ゲーム挙動・UI・セーブ互換: 本素案による変更なし。
- 開発性能: 総所要時間の短縮を期待するが、build競合・review待ちで逆効果になる可能性がある。未測定。
- Help影響: 本batchは提案と索引のみで、Help provider・操作・設定・通知・runtime dataへ変更を流さないためNo impact。
  将来の実装batchではrepository Help impact Skillを使って改めて判定する。別作業のdirty変更へこの判断を流用しない。
- 文書: 本提案、提案索引、docs入口。実装計画と現行運用仕様の改定は採用後。

## 8. リスクと対策

| リスク | 対策 / 停止条件 |
| --- | --- |
| 共有仕様の意味的な競合 | 契約を先行統合し、base差分と統合結果をreview。依存が強いtaskは直列化 |
| reviewerの見落とし・待ち行列 | 小さな差分、自動検証、1名のqueue上限。長期化時は実装投入を減らす |
| workerの範囲外変更 | 権限制限＋差分/未追跡検査。検出した候補の統合を止め、成果を保全して調査 |
| OOM・disk不足・GPU計測干渉 | 初期1実行、解析数制限、peak計測、native排他。発生時は新規投入を停止 |
| 多段helperのdeadlock・孤児process | 取得順とlease継承の契約試験。生存確認なしで枠を開放しない |
| agent/Orca更新で起動仕様が変わる | 試行versionを固定し、更新時に起動・取消・権限の互換試験 |
| review cacheの誤削除 | primary台帳を正本とし、review holdを無応答で終了しない |

## 9. 検証計画

### 制御基盤の受入

実装前にunit/integration試験へ落とす。単なるドキュメント遵守の宣言では合格にしない。

1. 別worktree/独立clone/Orca外terminalの同時要求でも、host枠を超える子processが起動しない。
2. native排他中の通常build・feedbackと、その逆方向がbusyで止まる。解析の暗黙buildも迂回しない。
3. 低RAM・空きdisk不足・frozen対象・無効lease・権限不明の場合、子process開始前に止まる。
4. 正常終了・取消・親異常終了・子残存から回復し、二重起動・永続的な枠占有・deadlockを起こさない。
5. workerの越境write/割当外の共有Git操作、reviewerのsource writeを拒否する。自身のbranchへのcommitは成立し、許可path逸脱・未追跡変更も提出時に検出する。
6. review後のhead/base/source変更が承認と検証の再照合を要求する。統合時の修正が未審査で通らない。
7. 未統合成果やreview consumerがあるtreeをcleanup対象にしない。終了後は既存storage checkを通す。

driver・規則を変更する実装batchは変更分類に従う全群検証を実施する。
Rust変更を含む候補は`python3 scripts/dev.py check`、rust-analyzer診断、workspace Clippy警告0を維持し、
完了時に意図したfull SHAをbaseとする`python3 scripts/dev.py ci check --base <full-SHA> --mode auto`、
または同じ対象・必要群の成功CIを確認する。分類不明なら`verify`へ戻す。
native機能の受入には専用Skillとprimary validation coordinatorを使い、Capture→Memoryの順を維持する。

### 試行の評価

同程度の独立taskを複数組選び、単独実装と実装2件並列を比較する。
task規模・検証scope・cacheのcold/warmを記録し、単一の成功例から速度向上を断定しない。
計測は着手から統合受入までの時間、build待ち、review待ち、再修正回数、統合回帰、peak RAM、
target増分bytes、agent使用量を含める。短縮率の足切りはbaseline取得後・並列試行前に固定する。

採用条件は、範囲外変更・未審査統合・資源制御逸脱が0件、必須gateがpass、
同程度taskで総所要時間が改善し、手戻りと費用の増加が合意した範囲内であること。
重い実行枠の2件化は別判断とし、1件設定でのpeakと余裕を確認してから通常buildに限って試す。

## 10. ロールアウト/ロールバック

| 段階 | 内容 | 次へ進む条件 |
| --- | --- | --- |
| P0: 素案 | 役割・所有権・資源・review・採用条件を合意 | 本書の判断事項を解消し、実装計画を作成 |
| P1: 基盤 | host制御・権限・記録・対応ルールを主担当が直列実装 | 制御基盤の受入をpassするまでworker編集は禁止 |
| P2: 起動確認 | Orca固定版で実装1＋review1 | cwd、権限、hook、lease、停止/再開、既存gateの互換確認 |
| P3: 並列試行 | 実装2＋review1＋統括、重い実行1 | 複数task組の指標と全gateを評価 |
| P4: 定常化 | 実証された設定を運用仕様へ昇格 | 実装数・build枠は測定に基づいて個別判断 |

不具合時は新規worker投入を止め、稼働processと未統合成果を確認して停止・保全する。
単独編集へ戻してもhost/native保護は維持し、review待ちtreeは用途が終わるまで残す。
Orcaの設定やbranchを一括削除して戻さない。規則の復元は差分を確認した明示変更として行う。

## 11. 未解決事項（Open Questions）

- [ ] 初期実装taskの組合せ、baseline測定、許容する時間・費用・手戻りの判定値。
- [ ] 使用するagent CLIでのwrite境界とGit metadata権限、reviewerのread-only設定の実現方法。
- [ ] host lock namespace、旧helperへのlease継承、解析backendの入場制御の具体API。
- [ ] primaryが別作業中の場合の文書更新枠とintegration候補の維持先。
- [ ] Orca固定版のhook契約・異常終了・削除操作の運用確認。

役割数、単一reviewer、初期build枠1、文書primary集約、target非共有は本素案の推奨として固定する。
未解決事項はP1の設計・試験対象であり、現時点で「並列実装に問題なし」と認定しない。

## 12. AI引継ぎメモ（最重要）

### 現在地

- Orca 1.4.205導入・仕様調査済み。後続基盤は関連計画M2〜M5へ具体化した。ルール改定・並列worker起動なし。
- 作成時primaryは`codex/building-art-migration`、別作業の未コミット変更あり。
  本書と索引だけを追加するため、共有branchを切り替えず、別作業をcommit/退避/復元しない。
- 今回の素案には新規worktree・target・native job・binary copyはない。
- 素案の文書リンク・索引検査、storage checkはpass。`ci check --base 6abeeed92cf14e2e5aafc8240e1c64989b609a84 --mode auto`
  は別作業のdirty Rust/toolingも含めcontracts/tooling/rustを選択し、既存production変更のHelp判断未更新でcontracts内に停止した。
  全体gateの成功は未確認。今回の文書No impactで別作業の判断を代替せず、tooling/Rustの成功も主張しない。

### 次のAIが最初にやること

1. 関連計画と現行規則を確認する。提案を読んだことだけでworker編集を開始しない。
2. 後続実装の依頼範囲と並行sessionを確認して専用実装branch/作業場を用意する。
3. 関連計画M2から単独編集で実装・検証する。文書正本とstorage coordinatorはprimaryを使う。
4. 試行では版・対象・測定条件を記録し、同じ候補とcacheを修正期間中維持する。

### 参照必須ファイル

- `AGENTS.md`、`docs/DEVELOPMENT.md`、`docs/plans/plan-template.md`
- `docs/development-infra/validation-storage-workflow.md`、`docs/development-infra/rust-analyzer-mcp.md`
- `scripts/dev.py`、`scripts/build_coordination.py`、`scripts/build_lane.py`、`scripts/cargo_runtime.py`
- `scripts/check_agent_rules.py`、`scripts/sync_agent_skills.py`、`scripts/validation_storage.py`

### 完了条件（Definition of Done）

- [x] 素案の役割・範囲・資源・統合・review契約を記述した。
- [x] リスク・検証・段階導入・単独運用への復帰を記述した。
- [x] 実装計画の作成先と、未発効の規則変更案を明示した。
- [ ] P1〜P4の実装・試行・採用判断を完了した（今回の依頼範囲外）。

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-20 | Codex | 現行規則と資源制御を踏まえた初版。実装2名・専任review1名・統括、host制御と段階導入を提案 |
| 2026-09-20 | Codex | Orca 1.4.205を導入。固定版仕様確認と後続工程を実装計画へ移し、導入状態を同期 |
