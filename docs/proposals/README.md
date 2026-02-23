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

| Document | Notes |
| --- | --- |
| `08_visual_update_prompts.md` | ビジュアル更新プロンプト集 |
| `room_detection.md` | 壁・扉・床から閉領域（Room）を検出する提案 |
| `soul_spawn_despawn_optimization.md` | Soul Spawn/Despawn 最適化提案 |
| `speech_optimization.md` | スピーチシステム最適化提案 |

補足: 過去提案は `docs/proposals/archive/` を参照。
