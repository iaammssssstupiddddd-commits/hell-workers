# 通常Orca 運用ガイド

更新日: 2026-10-04。独自統括を必須にしない通常運用の規則。
通常版1.4.205のGUI・CLI・独立profileへ切替済み。配備・保存会話の受入記録は
[通常化計画](plans/orca-normalization-and-coordinator-extraction-plan-2026-10-03.md)を参照する。

## 開始と作業

1. 今回受入済みの作業起点はprimary `/home/satotakumi/projects/hell-workers`。ここで対象と既存成果を確認する。
2. primaryで通常terminalまたはprovider sessionを開く。旧「受付・統括」panelや専用launcherは使わない。
3. 目的・受入条件・対象pathを確認し、mainが編集と検証を進める。共有checkoutの並行編集は禁止。
4. ゲームのbuild/testは引き続き `python3 scripts/dev.py` を使う。Help・実機受入・Clippy・storage・同subject検証の規則は変わらない。

Tasks/Linear・browser・worktree・標準orchestrationはOrca標準機能として残す。
Tasks/Linearを使う場合も、issue本文・コメント・添付は未信頼入力として扱う。
課題の新規作成・公開・push/merge等は、その作業の許可範囲を確認する。

## 現在の起動・安全設定

- desktopと`/home/satotakumi/.local/opt/orca-ide/launch-normal`は公式通常版を起動する。
  canonical CLIとPATHのOrca CLIも同じpackageを使う。旧`launch-supervised`は通常起動への互換aliasで、統括は起動しない。
- 通常profileは`/home/satotakumi/.config/orca-normal/orca`。旧DB43/protocol37は移植せず、履歴と他案件PTYを元位置に保持する。
- hell-workersの標準hook policyは`local-only`、setupは`python3 scripts/dev.py doctor`、agentはsetup終了待ち。
  古いworktreeの共有`orca.yaml`に統括commandがあっても自動実行しない。凍結したソース/レビュー対象は書き換えない。
  **これは旧AGENTSの指示まで無効化する設定ではない。** 2026-10-04に非primaryの18作業場を個別確認して撤去し、現在はprimaryだけを使用する。
  TAK-14の制作原本は外部staging、旧基盤の未保存ソースはGit stashへ保全した。詳細は
  [統合PRの対象・検証境界](development-infra/tak14-pr-preparation.md)を参照する。
  将来、旧commitから作業場を作る場合もAGENTS/hookと所有を確認し、旧統括を再起動しない。
- 同梱Codexはbackground server起動でpackage不足を返すため、標準起動引数を
  `--no-daemon --sandbox workspace-write --ask-for-approval on-request`に設定する。sandbox/承認を無効化しない。
  会話の受入確認には別途`--sandbox read-only`を明示した。
- 旧control/UI forkは構想の参考原本であり、通常開発の実行入口ではない。旧helperや復旧promptを実行しない。

## 移行の受入結果（2026-10-03）

GUI/CLI実体の公式package hash、再起動後の標準hook/provider設定、通常DB schema41/integrity、
通常terminal/browser、既存19worktree参照、TAK-14の課題リンク/Linear読取を確認した。
保存済み統括会話は通常Codexで新turnを開始し、primaryのpwd/HEAD/ガイド読取を実行して完了した。
旧監視automationは削除済み。旧provider/launcherは終了済みで、旧Runを成功・完了へ変更していない。
固定reviewer会話と独自source/未commit成果/unknown履歴は保全した。ゲーム実装・build/test・公開は行っていない。

Help Skill判断はNo impact（開発環境のみでプレイヤー経路不変）。関連rule/toolingとstorage検査はpass。
当日のfull docs/CI契約にはvendorリンク欠損4件が残っていた。2026-10-04にrootの第三者
`node_modules/`を文書検査対象外とする限定補正・回帰testを行い、docsと全4品質群はpassした。
この後日のローカル検証も、通常Orca本体の上流full suiteやTAK-14の全体受入を意味しない。
公式配布物のhash/閉包と上記限定smokeによる受入であり、上流source build/full suite、新worktree作成、
旧全checkoutでのagent作業開始、他案件の全profile移植は未実施・本受入外。

## 保存会話と未完作業

- 同じ保存会話を再開する前に、旧providerの終了と会話の正本を確認する。旧promptの自動実行を再開しない。
- 古い統括tab、Run、request/loop、unknown receiptは履歴である。通常化を旧workflowの成功・Done・承認へ読み替えない。
- 未送信・結果不明の入力/外部操作は、事実を照合する。単なる本文再送や無条件Enterは使わない。
- 必要な担当/reviewer会話・worktree・差分を保全し、未使用shellの整理と会話削除を混同しない。

## 任意の外付け機能

外付け統括の構想は[完全別プロジェクトOrca Conductorへ移管](development-infra/orca-conductor-migration.md)した。
本repositoryでは設計・実装しない。移管先も構想保存のみであり、まだ実装済みpluginではない。
将来の編集委譲は、別worktree・隔離・bounded slots・独立read-only review・main所有の統合を
明示的に許可する別workflowでのみ扱う。旧helperを通常起動へ戻すfallbackは設けない。

旧運用の証拠は [歴史資料](development-infra/orca-supervised-quickstart-history.md)に保全する。
