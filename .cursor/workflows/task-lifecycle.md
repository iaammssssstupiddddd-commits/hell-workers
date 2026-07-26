---
description: タスクの開始時と終了時に行うべきドキュメント管理のワークフロー。
---

# /task-lifecycle ワークフロー

タスクの整合性を保つため、開始時と終了時に以下の手順を実行してください。

## タスク開始時
1. `docs/` フォルダ内のファイルを確認し、現状の仕様と最新の実装状況を把握する。

## タスク終了時（完了報告前）
1. `docs/` フォルダ内のドキュメントを必要に応じて更新、または新規作成する。
2. ドキュメント化の対象は、実装やゲームの仕様に関するものに限定する。
3. You MUST use the repository `hell-workers-review-help-impact` Skill after implementing, changing, or removing functionality, code, or runtime data and before reporting completion, committing, or publishing.
4. Complete the Skill's `Update required` / `No impact` decision from the actual player-visible path; a passing Help impact gate alone does not count as the review.
5. If the current product does not expose that Skill natively, read and follow `.cursor/skills/hell-workers-review-help-impact/SKILL.md` directly before completion.
