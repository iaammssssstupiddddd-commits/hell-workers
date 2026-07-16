# Proposals Index

`docs/proposals` は、機能追加・改善・リファクタリング案をまとめる提案書ディレクトリです。

## 新規提案書の作り方

1. テンプレートをコピーする。  
   `cp docs/proposals/proposal-template.md docs/proposals/<topic>-proposal-YYYY-MM-DD.md`
2. `メタ情報`、`背景と問題`、`提案内容`、`AI引継ぎメモ` を最低限埋める。
3. 進捗に応じて `ステータス` と `更新履歴` を更新する。

## テンプレート

- [proposal-template.md](proposal-template.md): AIが引継ぎしやすい提案書テンプレート。

## 現在の提案書

| Document | Status | Notes |
| --- | --- | --- |
| [gameplay-management-improvements-proposal-2026-07-17.md](gameplay-management-improvements-proposal-2026-07-17.md) | Draft | 操作・フィードバックを基盤に、運営ポリシー、復旧・永続化、進行要素を段階導入する総合提案 |
| [hvac-plumbing-proposal.md](hvac-plumbing-proposal.md) | Accepted / Promoted | 空調・衛生の建築設備導入と世界観への落とし込み提案 |
| [soul-outline-mask-ring-proposal-2026-04-16.md](soul-outline-mask-ring-proposal-2026-04-16.md) | Accepted / Not Implemented | 既存 soul mask RtT を使い、composite 側で画面空間の外周 ring を生成する提案 |
| [soul_spawn_despawn_optimization.md](soul_spawn_despawn_optimization.md) | Draft / Active | Soul Spawn/Despawn 最適化提案 |

## アーカイブ提案書一覧 (`archive/` / `**/archived/`)

| Document | Status | Notes |
| --- | --- | --- |
| [3d-rtt/archived/3d-rendering-rtt-proposal-2026-03-14.md](3d-rtt/archived/3d-rendering-rtt-proposal-2026-03-14.md) | Archived | 建築物は2Dスプライトの静的組み合わせで描画（壁は16+バリアントのテクスチャ切替）の提案。 |
| [3d-rtt/archived/3d-rendering-rtt-proposal-phase2-2026-03-14.md](3d-rtt/archived/3d-rendering-rtt-proposal-phase2-2026-03-14.md) | Archived | 3d-rendering-rtt-proposal-phase2-2026-03-14 |
| [3d-rtt/archived/billboard-camera-angle-proposal-2026-03-16.md](3d-rtt/archived/billboard-camera-angle-proposal-2026-03-16.md) | Archived | Camera3d 角度確定提案（旧：ビルボード方式採用） |
| [3d-rtt/archived/building-visual-layer-plan-2026-03-12.md](3d-rtt/archived/building-visual-layer-plan-2026-03-12.md) | Archived | 建築物ビジュアル多層レイヤー（2D互換・3D準備）詳細設計書 |
| [3d-rtt/archived/character-3d-rendering-proposal-2026-03-16.md](3d-rtt/archived/character-3d-rendering-proposal-2026-03-16.md) | Archived | キャラクター 3D モデルレンダリング採用提案 |
| [3d-rtt/archived/outline-rendering-proposal-2026-03-16.md](3d-rtt/archived/outline-rendering-proposal-2026-03-16.md) | Archived | アウトライン生成設計方針 |
| [3d-rtt/archived/rtt-resolution-scaling-proposal-2026-03-16.md](3d-rtt/archived/rtt-resolution-scaling-proposal-2026-03-16.md) | Archived | RtT 解像度スケーリング設計提案 |
| [3d-rtt/archived/section-material-proposal-2026-03-16.md](3d-rtt/archived/section-material-proposal-2026-03-16.md) | Archived | SectionMaterial 採用提案 |
| [3d-rtt/archived/spatial-grid-architecture-plan-2026-03-12.md](3d-rtt/archived/spatial-grid-architecture-plan-2026-03-12.md) | Archived | **の計画。 |
| [3d-rtt/archived/wfc-terrain-generation-plan-2026-03-12.md](3d-rtt/archived/wfc-terrain-generation-plan-2026-03-12.md) | Archived | **の計画。 |
| [archive/familiar-task-management-hw-ai-extraction-proposal-2026-03-11.md](archive/familiar-task-management-hw-ai-extraction-proposal-2026-03-11.md) | Archived | `src/systems/familiar_ai/decide/task_management/` には、候補収集、優先度評価、搬送元選定、予約影反映、`AssignedTask` 構築など、使い魔 AI の中核ロジックがまとまっていたの提案。 |
| [archived/08_visual_update_prompts.md](archived/08_visual_update_prompts.md) | Archived | ビジュアルアップデート用アセット生成プロンプト案 |
| [archived/speech_optimization.md](archived/speech_optimization.md) | Archived | スピーチシステムの最適化提案 (Scale: Soul 300, Familiar 30) |

