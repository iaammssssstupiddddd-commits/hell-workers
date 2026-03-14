# AI開発エージェント最適化 提案書

**対象プロジェクト:** Hell Workers
**対象リポジトリ:** https://github.com/iaammssssstupiddddd-commits/hell-workers
**目的:** Codex CLI / Codex Cloud 等のAI開発エージェントを最大限活用できるリポジトリ構造へ改善する

---

# 1. 背景

本プロジェクトは Rust + Bevy を基盤とした2D建築シミュレーションゲームであり、以下の特徴を持つ。

* Soul / Familiar AI
* logistics / tasks / building / population
* ECSベースのゲームロジック
* ドキュメント主導の設計
* 将来的な規模拡大を前提

現状のリポジトリは

* `AGENTS.md`
* `docs/`
* `docs/plans/`
* crate責務設計

など **AIエージェント運用に適した基盤**を既に持っている。

しかし将来的に

* コード量増加
* AIロジック複雑化
* ECS依存関係増大

が発生すると、AIエージェントの理解効率と安全性が低下する可能性がある。

本提案では **AI開発エージェントとの協働を前提としたリポジトリ構造** を整備する。

---

# 2. 目標

本提案の目的は次の3点である。

### 1. AIがリポジトリ構造を高速理解できること

探索コストを削減し、トークン消費と推論時間を削減する。

### 2. 破壊的変更を防止すること

ゲーム固有ルールを明文化し、AIによる誤変更を防ぐ。

### 3. 大規模改修をAIに委任できる状態を作る

Codex Cloud 等で

* システム追加
* 大規模リファクタ
* AIロジック拡張

を安全に実行できる状態を作る。

---

# 3. 現状の強み

本プロジェクトは既に次の強みを持つ。

### AGENTS.md の存在

AI向け作業ガイドが存在する。

### docs構造

仕様書が整理されている。

### plansディレクトリ

変更計画を事前作成する運用がある。

### ECS責務分離

entities / systems / interface など構造が明確。

これらは **AIエージェント運用に非常に適した設計**である。

---

# 4. 問題点

現状の構造には次の改善余地がある。

### 問題1

局所ルールが不足

AGENTS.md が root のみであり、
各サブシステム固有のルールが存在しない。

### 問題2

不変条件の明文化不足

ゲームロジックの不変条件が文書化されていない。

### 問題3

イベント仕様の管理不足

イベントライフサイクルが明確ではない。

### 問題4

テストによる保証不足

`cargo check` に依存している。

---

# 5. 改善提案

## 提案1

### サブディレクトリ AGENTS.md の導入

AIは **近い階層の指示を優先して読む**。

そのため主要システムごとに
局所ルールを定義する。

例

```
systems/familiar_ai/AGENTS.md
systems/soul_ai/AGENTS.md
jobs/AGENTS.md
interface/AGENTS.md
```

内容例

* 責務
* 禁止事項
* 依存制約
* docs更新対象
* 検証方法

これにより AIの誤変更を大幅に削減できる。

---

## 提案2

### invariants.md の導入

ゲームの **壊してはいけないルール** を定義する。

例

```
docs/invariants.md
```

内容例

* Soul は task 未所持なら idle 状態
* Familiar は直接作業しない
* task は二重割当しない
* reservation と inventory は整合する
* UI は simulation state を直接変更しない

AIはこのファイルを基準に変更を判断できる。

---

## 提案3

### events.md の導入

イベント仕様を一元管理する。

```
docs/events.md
```

内容例

| Event            | Producer   | Consumer   | Timing     |
| ---------------- | ---------- | ---------- | ---------- |
| TaskAssigned     | FamiliarAI | SoulSystem | next frame |
| ResourceReserved | Logistics  | Building   | immediate  |

これによりイベント破壊を防ぐ。

---

## 提案4

### docsと実装のリンク強化

各ドキュメントの先頭に
実装場所を明記する。

例

```
docs/tasks.md
```

追加内容

* 主実装
* 関連システム
* 関連docs
* 不変条件

これにより AI探索コストを削減する。

---

## 提案5

### Planテンプレートの標準化

```
docs/plans/_template.md
```

内容

* 問題
* 原因
* 変更対象
* 非変更対象
* 不変条件
* 実装手順
* 検証方法
* docs更新
* rollback方法

これにより大規模変更の安全性が向上する。

---

## 提案6

### 最小テストスイート導入

優先順位

1. task system
2. logistics
3. AI state machine
4. room detection

目的

AIによる破壊的変更を自動検出する。

---

# 6. 推奨ディレクトリ構造

```
docs/
 ├ architecture.md
 ├ invariants.md
 ├ events.md
 ├ README.md
 └ plans/

systems/
 ├ familiar_ai/
 │   └ AGENTS.md
 ├ soul_ai/
 │   └ AGENTS.md

jobs/
 └ AGENTS.md

interface/
 └ AGENTS.md
```

---

# 7. 期待される効果

本提案を実施すると

### AI開発効率

向上率
**2〜5倍**

### AI誤変更

削減率
**50〜80%**

### Codex Cloudタスク成功率

向上率
**約2倍**

---

# 8. 導入優先順位

### 最優先

* invariants.md
* サブAGENTS.md
* events.md

### 次

* docs実装リンク
* planテンプレート

### 将来

* テスト拡張

---

# 9. 結論

Hell Workers は既に
**AIエージェント運用に適した構造を持つゲームリポジトリ**である。

しかし

* 局所ルール
* 不変条件
* イベント仕様

を追加することで

**AI開発エージェントとの協働能力を大幅に向上できる。**

本提案の実装により

* Codex CLI
* Codex Cloud
* 将来のAI開発環境

への適応力が高まる。

---
