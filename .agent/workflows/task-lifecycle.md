---
description: タスクの開始時と終了時に行うべきドキュメント管理のワークフロー。
---

# /task-lifecycle ワークフロー

タスクの整合性を保つため、開始時と終了時に以下の手順を実行してください。

Use the primary repository's `python3 scripts/dev.py validation` coordinator for validation planning/execution and pass its storage check before reporting.

## タスク開始時
1. `docs/` フォルダ内のファイルを確認し、現状の仕様と最新の実装状況を把握する。
2. 検証を行う場合は `docs/development-infra/validation-storage-workflow.md` を読み、
   primary計画の現在の用途・終了条件・旧checkoutを確認してから検証バッチを登録する。
3. 見た目・操作の修正中は `python3 scripts/dev.py feedback` を使う。
   壁・ドア合同storyboardはnative Skillの `plan --feedback` を選ぶ。
   同じdev cacheを継続利用し、必要な正式受入・性能計測へ進む時だけprofilingを使う。

## タスク終了時（完了報告前）
1. `docs/` フォルダ内のドキュメントを必要に応じて更新、または新規作成する。
2. ドキュメント化の対象は、実装・ゲーム仕様・開発運用とその検証記録にする。
3. You MUST use the repository `hell-workers-review-help-impact` Skill after implementing, changing, or removing functionality, code, or runtime data and before reporting completion, committing, or publishing.
4. Complete the Skill's `Update required` / `No impact` decision from the actual player-visible path; a passing Help impact gate alone does not count as the review.
5. If the current product does not expose that Skill natively, read and follow `.cursor/skills/hell-workers-review-help-impact/SKILL.md` directly before completion.
6. 検証バッチの結果を元環境で確定し、用途のないjob・binary copy・専用検証環境を整理する。
   全jobのarchiveは不要。採用成果物と短い最終結果は既存の正本へ集約する。
   残す場合はexact path、所有者・consumer・bytes・次の作業・終了条件を記録する。
   フィードバック待ち・修正中は同じcandidate worktreeとtargetを継続利用し、差分ビルドを保つ。
   最後のconsumer終了後に専用環境を撤去する。固定日数・容量の既定値は設けない。
   runner / 品質gateの成功だけを整理完了とせず、実際の残存と回収容量を確認する。
