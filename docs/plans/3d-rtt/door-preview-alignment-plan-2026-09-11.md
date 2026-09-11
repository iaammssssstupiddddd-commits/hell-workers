# ドア設計図の表示位置修正と正式反映への引継ぎ

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `door-preview-alignment-plan-2026-09-11` |
| ステータス | `In Progress` |
| 作成日 | `2026-09-11` |
| 最終更新日 | `2026-09-11` |
| 作成者 | `Codex` |
| 関連計画 | [Door](production-door-art-plan-2026-09-05.md)、[型枠](provisional-wall-formwork-plan-2026-09-05.md) |
| 関連提案 / Issue / PR | `N/A` |

## 1. 目的

ユーザー提供画像の左上の支持壁・Door設計図の位置ずれを解消する。旧J1の機械判定と目視判定では
設計図の実画素を十分に確認できていなかったため、旧合格を修正後の表示承認へ流用しない。
修正後に進めるとのユーザー指示に従い、再検証後はWall generation 10 / Door generation 6の正式反映へ進む。

## 2. スコープ

- 対象: Door設計図・配置previewの表示経路、J1検証、関連仕様・Help影響レビュー。
- 非対象: 扉の形状、左右の開度、建築材料・時間、論理占有・保存形式の変更。

## 3. 現状とギャップ

- 旧job: `wall-door-joint-20260910T175148Z-bb4a4563`、subject `5de169bc`。
- Blueprintはgrid `(14,50)`、最初の支持壁は `(13,50)` / `(15,50)`。
- 旧observerはImage handle / Anchor / InheritedVisibilityと画面内座標だけを検査し、描画対象可視性・
  sprite alpha・実画素と支持壁の整合を検査していない。単なるROIの輝度分散は地形や隣接壁でも通る。
- 既存preview PNGは斜め方向の撮影画像。位置だけでなく実際にどの経路で描画されるかを先に確定する。

## 4. 実装方針

1. 実機observerへ透明度・ViewVisibility・sprite geometryの検査を追加し、表示の成立条件を確定する。
2. 原因となる表示経路だけを修正。論理rootのgridや占有を表示都合で動かさず、診断専用変更は撤去する。
3. 完成Door / 支持壁 / previewの画素位置を照合する回帰を追加し、両軸・load後を再採取する。
4. 修正後J1が通ってから既存M4を進める。正式反映の条件付き指示を証跡と結び、旧画像を承認扱いしない。

Bevy 0.19のローカルregistryと既存実装をAPIの一次情報とする。描画APIは推測で変更しない。

## 5. マイルストーン

- [ ] M1: 観測条件を追加し、原因を確定。
- [ ] M2: 表示位置修正と回帰test。承認済み形状・開度は保持。
- [ ] M3: clean subjectの実機J1、独立verify、対象画像の目視確認。
- [ ] M4: 既存両計画へ結果・正式反映指示を引継ぐ。

## 6. リスクと対策

- 隣接壁や地形をpreviewの証拠にしない。消去差分または対象固有の可視画素を検査する。
- 旧jobは履歴として保持。修正後の合格へ再ラベルしない。
- helperの初期選択や自動応答ではなく、今回のユーザーの「修正してください。その後、進めてください」を条件付き指示として記録する。

## 7. 検証計画

- focused regression、rust-analyzer、`python3 scripts/dev.py check`。
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`、`python3 scripts/dev.py verify`。
- native skillのdirect kitty、固定auditと6 checkpointを逐次実行し、独立verify・実画像確認。
- Help skillで実経路から分類、docs索引と`git diff --check`。

## 8. ロールバック方針

位置修正は表示責務に限定する。正式登録前にcanonical / runtime locatorを変更しない。
旧assetと証跡を保持し、既存M4の前像保存・pointer最後の切替手順へ従う。

## 9. AI引継ぎメモ

- 現在地: 原因調査中。primary開始時HEAD `0889dd71`、dirtyなし。
- 最初にやること: 描画対象・alpha・projectionの成立を確定し、値調整の繰り返しを避ける。
- 参照: `door_preview.rs`、`joint_actual_window.rs`、`hw_visual::blueprint`、`docs/help-screen.md`。
- DoD: 修正・全gate・新実機証跡・既存M4引継ぎ後にarchiveし、索引更新。
- worktree整理は両track close時。成功・失敗jobを今は削除しない。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-09-11` | `Codex` | ユーザー指摘から位置ずれの調査・修正・再受入を計画化 |
