# 動的集会システム (Dynamic Gathering System)

Soulの待機行動に基づいて自然発生的に集会所が生成され、人が集まるにつれて拡大し、距離に応じて統合され、過疎化すると消滅する動的なシステム。

## 概要

| 項目 | 値 |
|:---|:---|
| 最大参加人数 | 8人 |
| 維持最低人数 | 2人 |
| 発生待機時間 | 10秒 (近傍Soul数で短縮) |
| 消滅猶予時間 | 10秒 |

## 中心オブジェクト確率

オブジェクトは発生時にランダム決定。人数によって確率が変動:

| 人数 | Nothing | CardTable | Campfire | Barrel |
|:---:|:---:|:---:|:---:|:---:|
| 1〜4人 | **50%** | 30% | 10% | 10% |
| 5〜6人 | 20% | **50%** | 20% | 10% |
| 7〜8人 | 5% | 25% | **40%** | **30%** |

## 統合ルール

- 人数が少ないほど統合距離が長い (不安定な集会は遠くの集会に吸収されやすい)
- 小規模 → 大規模に吸収
- 同規模の場合は先に発生した方が残る

## 関連ファイル

- [gathering.rs](file:///f:/DevData/projects/hell-workers/src/systems/soul_ai/gathering.rs)
- [提案書](file:///f:/DevData/projects/hell-workers/docs/proposals/dynamic_gathering.md)
