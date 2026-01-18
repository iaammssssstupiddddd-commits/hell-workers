# セリフシステム仕様書 (Speech System)

このドキュメントでは、Soul および Familiar が表示する吹き出し（セリフ）の出現箇所、条件、およびビジュアル仕様について解説します。

---

## 1. 出現タイミングと条件

セリフは主に Bevy の **Observer** イベント、または **AI 状態の変化** によってトリガーされます。

### Soul (魂) のセリフ
Soul は主に「感情」を絵文字で表現します。表示時は各感情に対応した色の **グロー（光彩）背景** が伴います。

| トリガー | 絵文字 | 感情 (BubbleEmotion) | 内容・条件 |
| :--- | :--- | :--- | :--- |
| `OnTaskAssigned` | 💪 | Motivated | タスクが割り当てられた瞬間 |
| `OnTaskCompleted` | 😊 | Happy | タスクを正常に完了した瞬間 |
| `OnExhausted` | 😴 | Exhausted | 疲労限界に達し、休息へ向かう時 |
| `OnStressBreakdown` | 😰 | Stressed | ストレス崩壊を起こした時 |

### Familiar (使い魔) のセリフ
Familiar は命令や状態を「ラテン語」で表現します。表示は **9-slice 吹き出し** と **タイプライター効果** を伴います。

| トリガー | ラテン語 | 感情 | 内容・条件 |
| :--- | :--- | :--- | :--- |
| `OnTaskAssigned` | (複数) | Motivated | 部下にタスクを命じた時（以下参照） |
| `OnSoulRecruited` | Veni | Neutral | 新しい Soul を分隊に勧誘した時 |
| `AI: Idle` | Requiesce | Neutral | 命令が解除され、待機状態に入った時 |

#### 命令別のラテン語 (OnTaskAssigned)
| 作業種別 (WorkType) | セリフ | 意味 (参考) |
| :--- | :--- | :--- |
| `Chop` (伐採) | **Caede** | 切れ / 伐採せよ |
| `Mine` (採掘) | **Fodere** | 掘れ / 採掘せよ |
| `Haul` (搬送) | **Portare** | 運べ |
| `Build` (建築) | **Laborare** | 働け / 構築せよ |

---

## 2. ビジュアル・アニメーション仕様

### 共通仕様
- **Pop-In/Out**: スケールバウンスによる出現と、縮小フェードによる消失。
- **スタッキング**: 同一人物が連続して発言した場合、古い吹き出しが上に押し上げられ、重ならないように配置されます。

### Soul 固有
- **感情カラーグロー**: 吹き出しの背後に、感情に対応した色の円形グラデーション（`glow_circle.png`）が表示されます。
    - Motivated: 黄色系
    - Happy: ピンク/オレンジ系
    - Exhausted: 青/紫系
    - Stressed: 赤系

### Familiar 固有
- **9-slice 吹き出し**: テキストの長さに応じて、吹き出し背景（`bubble_9slice.png`）が動的に伸縮します。
- **タイプライター効果**: 1文字ずつ高速（0.03秒間隔）でテキストが表示されます。

---

## 3. 関連コンポーネント・定数

- **コンポーネント**: `SpeechBubble`, `BubbleAnimation`, `TypewriterEffect`, `BubbleEmotion`
- **定数**: [src/constants.rs](file:///f:/DevData/projects/hell-workers/src/constants.rs) 内の `BUBBLE_` プレフィックスが付いた各定数（速度、色、サイズ等）
- **システム構成**:
    - `spawn_soul_bubble` / `spawn_familiar_bubble`: 生成ロジック
    - `animate_speech_bubbles`: アニメーション制御
    - `update_typewriter`: テキスト表示制御
    - `update_bubble_stacking`: 位置調整制御
