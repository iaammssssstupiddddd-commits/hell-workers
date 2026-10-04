# Orca GitHub / Linear 隔離連携試験

2026-09-22実施。ユーザーが許可した試験専用の作成・push・branch間mergeを完了した。
これはGitHub→Linear標準連携の受入であり、Orcaの実装→review→差戻しcontrollerの受入ではない。

## 対象と境界

- Linear: [TAK-7](https://linear.app/takumi-sato/issue/TAK-7/orca-github-linear-isolated-acceptance-2026-09-22)。TAK-5/TAK-6とは独立した試験課題。
- GitHub: [PR #26](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/26)。Draft作成後、ready化し、試験targetだけへmergeした。
- source: `orca-test/tak-7-github-linear-20260922`。
- target: `orca-test/github-linear-base-20260922`。`master`はtargetに使用しない。
- 差分: `scripts/tests/fixtures/orca_github_linear/README.md` の12行追加だけ。
- 既存primary/candidateのHEAD・indexを動かさず、未commit変更を保全した。別indexとGit plumbingでfixture commitを作り、専用refだけをpushした。primary docsへの受入記録追記は別の未commit差分。新規worktreeやagentは起動していない。
- Linearのstatus、PR attachment、assigneeは手動更新していない。GitHub/Linearの設定・権限も変更していない。

## 観測結果

時刻はUTC。Linearのfull issue readでattachment、activity、stateを読み戻し、partial=falseを確認した。

| GitHub操作 | Linearで確認した結果 | 証拠 |
| --- | --- | --- |
| Draft PR作成 | PRを自動添付。stateはBacklogのまま | attachment `2141066d-166b-48d1-a9eb-70f33eb2ff44`、13:55:33.920。source.type=`github`、pullRequestId=`4602931802` |
| Draft解除 | Backlog → In Progress | 13:55:54.779、activity `1d1a7ed5-93b9-4ba5-bec6-487d7d10c784`、actor=`GitHub` integration |
| 専用targetへmerge | In Progress → Done | GitHub merged_at=13:57:17、Linear=13:57:19.872、activity `9c60f04b-d9d5-4385-879c-8fc877b9e373`、actor=`GitHub` integration |

PRはbranch名・titleにTAK-7を含め、本文に`Fixes TAK-7`を記載した。
この試験はmergeで試験課題を完了させる意図があるためclosing keywordを使った。
GitHub連携によるassigneeの自動設定も観測した。

今回のtargetではready化は **In ReviewではなくIn Progress** になった。統括は名称を決め打ちしない。
TAK teamの全automation設定や別targetのruleを取得したわけではないので、この結果を全branchへ一般化しない。
PR attachmentが実際に生成されたため、標準PR連携は動作している。repository hooksが空でも未接続とは判断できない。
`gh api user/installations`は使用中のtoken種別では403であり、installation一覧の確認には使えなかった。

仕様上の区別は[Linear GitHub公式ドキュメント](https://linear.app/docs/github)を参照する。
ローカルbranch作成、内部agent verdict、native GitHub reviewを同じeventとして扱わない。

## Git / CIの対象照合

| 項目 | 値 |
| --- | --- |
| base | `b68bafd7358580ff8ad77962fa02987a215b5b60` |
| PR head | `cb402021ff02526def4c5c561c89db052ff31e29` |
| CI tested merge SHA | `38f8ac18584b44bcbe9a0a66a56dc788516a1be5` |
| 実merge commit | `36df3db6c76e595bcc458eea44061038600d7e8f` |
| merge後tree | `d5b4ed4e03ed5fae0cec43518dc01d8a82debc5d` |
| 採用CI | [run 35736783946](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35736783946)、attempt 1、pull_request、auto |

CIのaggregate logにあるQUALITY_PLANのbase/head/testedを、merge直前のPR read-backと照合した。
選択群は`contracts=true`だけ。Classify changes、Repository contracts、Quality gatesがsuccess。
Tooling、Dependency、Rustは非選択のためskipped。ゲームbuild/test/nativeは実施していない。
Draft作成時のrun `35736732311`はready化によるconcurrency cancellationのため、成功証拠には使用しない。

merge操作には`--match-head-commit cb402021ff02526def4c5c561c89db052ff31e29`を使用した。
実mergeのparentsは上記baseとheadで、treeはheadと一致した。実merge SHA自体のCI成功とは主張しない。
専用targetはprotected=false、適用rules一覧は空、native review一覧も空だった。
独立したGitHub reviewerの承認やchanges requestedは未検証で、内部reviewで代用した扱いにもしていない。

`master`は試験前後とも`b68bafd7358580ff8ad77962fa02987a215b5b60`で不変。
Orca基盤candidateの未commit実装は、このPRへ含めず、publish/mergeもしていない。

## Help・資源・残件

Help impact review: **No impact**。試験差分は不活性Markdownだけで、rootの
`build_help_panel_content` → static provider → `hw_ui::setup::help_panel`へ入らず、
player input、game state、runtime assets、Help本文・coverageを変更しない。

primary storage checkは既存の使用中登録3件のpath欠損でfailした。対象は
`wall-door-joint-83ad3f85`、`refactor-r01-r14-92a7d87235e6`、`tak-6-implementation`。
所有者の用途判断が未確認のため、台帳解除や再生成で隠していない。外部CI成功でこの失敗を相殺しない。

試験branchとmerged PRは、owner=`orca-git-review-loop`、consumer=`M5 GitHub adapter read-back受入`として保持する。
次の用途は、adapterによる既存PRのclosed/merged判定とbase/head/merge照合。release_whenはその受入終了時。
fixtureの唯一の成果はGitに保存済みで、専用worktree/Cargo cacheは作っていない。
baseからの追加Git objectは8個、非圧縮サイズ計7,806 bytes（commit/tree/blobのlogical sizeであり、disk占有量ではない）。
一時indexとfixture本文 `/tmp/orca-git-linear-acceptance.pH29aZ` は撤去し、duのapparent sizeは193,603 bytesから0へ減った。
本文は上記headのGit blobから復元可能。既存candidate/cacheは保持する。

文書の`docs --write` / `docs --check`、Help gate、`git diff --check`はpass。
primary dirty docsの変更別検証はbase=`e947155c91f3bf8c87c85f0d882296fe7e6ac581`からcontractsだけを選択し、
前記storage欠損3件で停止した。この文書変更は未commit・未pushで、fixture CIの検証対象には含まれない。

未完: 統括event driver、実装→差戻し→再reviewの自動進行、A/B直列統合・combined review、
GitHub publish/CI adapter、異常系、受入済み基盤の配備。TAK-7のDoneはこの試験の完了だけを意味する。

次工程は[Git基点レビュー反復計画](../plans/orca-git-review-loop-plan-2026-09-22.md)に従う。
