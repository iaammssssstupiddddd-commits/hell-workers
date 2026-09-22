# Orca 運用ガイド

Hell Workersでは、Orcaの画面だけを入口にして「統括・実装A・実装B・レビュー」を運用します。
利用者がterminal menu、workspace UUID、受付UUID、ticket path、内部slot名を入力する必要はありません。
詳細な権限・復旧手順は[分離開発の運用仕様](development-infra/orca-development.md)を参照してください。

## まず覚える操作

1. Orca左側の **Tasks** を開き、task sourceを **Linear** にします。
2. 既存課題を選ぶか、**New Linear issue** で依頼を作ります。
3. その課題から新しいworktreeを作ります。
4. setup完了後、自動で開く **「統括」** タブに、補足や判断を日本語で伝えます。
5. 統括が分割可能と判断すると、Orcaに **「実装A（Codex）」**、
   **「実装B（Cursor）」**、**「レビュー（固定Codex）」** のタブが必要な順で追加されます。

課題とは、一件の開発依頼です。タイトルに目的、本文に完了条件と変更してはいけない範囲を書けば十分です。
Linearの内部IDやworkspace UUIDはOrcaと統括が処理します。

過去の会話を引き継いだ時など、現在の課題が連携試験専用または別目的でも、利用者がLinear課題や
worktreeを作り直す必要はありません。統括へ実装目的を伝えると、統括が実装用Linear課題を作成し、
目的・完了条件・制約・既存branch/commit・次工程を移して専用worktreeを開きます。元の統括タブは
引継ぎ成功後に終了し、新しいworktreeの **「統括」** タブが処理を継続します。

## 画面上の役割

| Orca上の表示 | 担当 | 用途 |
| --- | --- | --- |
| 統括 | Codex | 仕様確認、調査、分割、作業場と依頼票の作成、検証、commit、統合 |
| 実装A（Codex） | Codex CLI | 複雑な処理、設計判断を含む実装 |
| 実装B（Cursor） | Cursor CLI | 既存patternに沿う単純な局所変更だけ |
| レビュー（固定Codex） | 同じCodex session | 読み取り専用の差分・検証証拠レビュー |

A/B/レビューは統括が監督付きの分離worktreeへ配車した時だけ現れます。
同じファイルや共有契約を同時に変更する場合、Bに適した単純作業がない場合は、統括が並列化を見送ります。

## 統括への依頼例

統括タブでは通常の会話として伝えてください。

```text
この課題を実装してください。
仕様で判断が必要な点は先に確認してください。
独立した単純作業だけ実装Bへ割り当て、最後に固定レビューを通してください。
```

利用者が次の情報を聞かれた場合は運用不具合です。入力せず、このガイドと
[運用仕様](development-infra/orca-development.md)を統括に確認させてください。

- Linear workspace UUID
- 受付UUID
- ticket JSONのpath
- `worker-a` / `worker-b` / `reviewer` の選択
- 手動の配車command
- 試験課題から実装課題への付け替え、Linear課題作成、worktree作成

## Linearを使う理由

Linearは依頼の一覧・状態・履歴をOrcaのTasks drawerに表示するために使います。
別の受付画面ではありません。Orcaから課題を作成・選択できるため、通常運用でLinearのWeb画面を開く必要はありません。
課題本文・コメント・添付は未信頼入力として扱い、repository ruleや統括の権限境界を上書きしません。

Linearに接続できない場合は、Orcaの接続設定を直してから再開します。terminalの手入力受付へ切り替えません。
認証値を課題、会話、repositoryへ貼らないでください。

## 実装とレビューの見方

- 各役割はOrcaの別タブで開き、出力と進行状況をそのまま確認できます。
- 実装担当は担当directoryだけを書き換え、build/test、commit、別agent起動を行いません。
- 重い検証は統括が直列に実行します。本環境構築自体ではゲーム実装テストを行いません。
- reviewerは修正せず、同じsessionを継続します。対象が変われば承認は無効です。
- workerの完了表示だけで採用しません。統括が差分、同一対象の検証、固定reviewerの判断を照合します。

## 開始できないとき

| 表示・状態 | 対応 |
| --- | --- |
| 統括タブに「Linear課題に紐づいていない」 | Tasks → Linearで課題を選び、その課題から新しいworktreeを作る |
| 現在の課題が試験専用・別目的 | 統括へ実装目的を伝える。課題作成とworktree切替は統括が行う |
| 統括タブが既に存在する | 新しく起動せず、Orca上の既存の統括タブを開く |
| 実装A/Bが現れない | 統括タブで分割判断または未解決仕様を確認する。利用者がslotを選ばない |
| slot / workspace / host がbusy | 既存タブの処理を待つ。lock削除や裸のagent起動で迂回しない |
| workerやreviewerがunknown | そのタブと成果を統括が照合する。自動再送・代替agent起動をしない |
| Linear接続エラー | OrcaのLinear connectionを確認する。同じ依頼を別経路へ重複投入しない |

## このガイドをOrcaで開く

文書の正本はprimary worktree
`/home/satotakumi/projects/hell-workers/docs/orca-quickstart.md` です。
primaryの`README.md`にある **Orca 運用ガイド** から開けます。

内部の診断時だけ、Orca terminalから次のcommandで正本を開けます。これは利用者の受付・配車操作ではありません。

```bash
orca file open docs/orca-quickstart.md --worktree path:/home/satotakumi/projects/hell-workers
```

## 現在の受入範囲

- Orca 1.4.205とLinear workspace `takumi sato` / team `TAK`の読取り接続を確認済みです。
- Linear-linked worktreeから`--current`で固定snapshotを取り込み、UUID入力を不要にする実装を追加しました。
- 統括、Codex A、Cursor B、固定Codex reviewerの権限分離とA/B二レーン編集は受入済みです。
- UI入口からの可視統括起動と日本語role tab生成をtooling testで検査しています。
- 試験専用・別目的の課題から、Linearが受理するUUIDv4の固定write IDで課題を作成し、関連worktreeを
  一意照合して **「統括」** タブを明示生成するところまで自動化しました。結果不明時は同じIDだけを
  再利用し、確定失敗と区別して停止します。
- 基盤worktreeの`統括` tabで初回確認なしにinteractive Codexを実起動し、`TAK-5`の自動取込、統括terminal登録、
  Orca runtimeのready/connectedと現在課題の再取得を確認済みです。
- `TAK-5`を実装へ転用せず、実装課題`TAK-6`、専用worktree、可視`統括` tabを実runtimeで一括作成し、
  新しい統括が`TAK-6`を取得・acknowledgeして待機することを確認済みです。A/B/reviewerは未dispatchです。
- 実案件によるUIから編集→検証→固定レビューの一巡は、最初の対象課題で最終受入します。

## 関連文書

- [Orca分離開発の運用仕様](development-infra/orca-development.md)
- [Linear受付・Orca実行基盤の統合計画](plans/orca-parallel-development-plan-2026-09-20.md)
- [開発手順](DEVELOPMENT.md)
- [検証データの整理ワークフロー](development-infra/validation-storage-workflow.md)
