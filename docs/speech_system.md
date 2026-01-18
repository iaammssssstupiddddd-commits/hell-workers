# セリフシステム仕様書 (Speech System)

このドキュメントでは、Soul および Familiar が表示する吹き出し（セリフ）の出現箇所、条件、およびビジュアル仕様について解説します。

---

## 1. 出現タイミングと条件

セリフは主に Bevy の **Observer** イベント、または **AI 状態の変化** によってトリガーされます。

### Soul (魂) のセリフ
Soul は主に「感情」を絵文字で表現します。表示時は各感情に対応した色の **グロー（光彩）背景** が伴います。

| トリガー | 絵文字 | 感情 (BubbleEmotion) | 優先度 | 内容・条件 |
| :--- | :--- | :--- | :--- | :--- |
| `OnTaskAssigned` | 💪 | Motivated | Low | タスクが割り当てられた瞬間 |
| `OnTaskCompleted` | 😊 | Happy | Low | タスクを正常に完了した瞬間 |
| `OnExhausted` | 😴 | Exhausted | High | 疲労限界に達し、休息へ向かう時 |
| `OnStressBreakdown` | 😰 | Stressed | Critical | ストレス崩壊を起こした時 |
| `OnSoulRecruited` | 😨 | Fearful | Normal | 勧誘された時 (0.3s遅延) | [NEW]
| `OnReleased` | 😅 | Relieved | Normal | 使役から解放された時 | [NEW]
| `OnGatheringJoined`| 😌 | Relaxed | Normal | 集会所に到着した時 | [NEW]
| `OnTaskAbandoned` | 😓 | Frustrated | Normal | タスクがキャンセルされた時 | [NEW]
| `Periodic: Idle` | 💤.. | Bored | Low | 長時間アイドル時 (10s+) | [NEW]
| `Periodic: High` | 😰/😴..| (各種) | High | 状態異常時の定期リマインド | [NEW]

### Familiar (使い魔) のセリフ
Familiar は命令や状態を「ラテン語」で表現します。表示は **9-slice 吹き出し** と **タイプライター効果** を伴います。

| トリガー | ラテン語 | 感情 | 優先度 | 内容・条件 |
| :--- | :--- | :--- | :--- | :--- |
| `OnTaskAssigned` | (複数) | Motivated | Low | 部下にタスクを命じた時 |
| `OnSoulRecruited` | Veni | Neutral | Normal | 新しい Soul を分隊に勧誘した時 |
| `AI: Idle` | Requiesce | Neutral | Normal | 命令が解除され、待機状態に入った時 |

#### 命令別のラテン語 (OnTaskAssigned)
| 作業種別 (WorkType) | セリフ | 意味 (参考) |
| :--- | :--- | :--- |
| `Chop` (伐採) | **Caede** | 切れ / 伐採せよ |
| `Mine` (採掘) | **Fodere** | 掘れ / 採掘せよ |
| `Haul` (搬送) | **Portare** | 運べ |
| `Build` (建築) | **Laborare** | 働け / 構築せよ | -- |
| `Release` (解放) | **Abi** | 去れ / 去りなさい | 疲労・崩壊時 |

---

## 2. ビジュアル・アニメーション仕様

### 共通仕様
- **Pop-In/Out**: スケールバウンスによる出現と、縮小フェードによる消失。
- **スタッキング**: 同一人物が連続して発言した場合、古い吹き出しが上に押し上げられ、重ならないように配置されます。

### Soul 固有
- **感情カラーグロー**: 吹き出しの背後に、感情に対応した色の円形グラデーション（`glow_circle.png`）が表示されます。
    - Motivated: 黄緑系
    - Happy: ピンク系
    - Exhausted: グレー系
    - Stressed: 赤系
    - Fearful: 紫系
    - Relieved: 水色系
    - Relaxed: ミント系
    - Frustrated: 濁ったグレー
    - Unmotivated: 黄色系
    - Bored: 薄い青系

### Familiar 固有
- **9-slice 吹き出し**: テキストの長さに応じて、吹き出し背景（`bubble_9slice.png`）が動的に伸縮します。
- **タイプライター効果**: 1文字ずつ高速（0.03秒間隔）でテキストが表示されます。

---

## 3. 優先度とスロットリング (New)

画面の混雑を防ぎ、重要な情報を際立たせるために優先度システムを導入しています。

### 優先度別の差別化
| 優先度 (BubblePriority) | 表示時間 | Soul 絵文字サイズ | 用途例 |
| :--- | :--- | :--- | :--- |
| **Low** | 0.8秒 | 18px (小) | タスク開始・完了 |
| **Normal** | 1.5秒 | 24px (中) | 勧誘、休息開始 |
| **High** | 2.5秒 | 28px (大) | 疲労限界 |
| **Critical** | 3.5秒 | 32px (特大) | ストレス崩壊 |

### クールダウン (スロットリング)
同一エンティティからの連続した発言を制限します。
- **高優先度 (High以上)**: クールダウンを無視して即座に表示されます。
- **低優先度 (Low/Normal)**: 前回の発言から一定時間（0.5〜1.5秒）経過するまで、新しい吹き出しの生成がスキップされます。

---

## 3. 関連コンポーネント・定数

- **コンポーネント**: `SpeechBubble`, `BubbleAnimation`, `TypewriterEffect`, `BubbleEmotion`
- **定数**: [src/constants.rs](file:///f:/DevData/projects/hell-workers/src/constants.rs) 内の `BUBBLE_` プレフィックスが付いた各定数（速度、色、サイズ等）
- **システム構成**:
    - `spawn_soul_bubble` / `spawn_familiar_bubble`: 生成ロジック
    - `animate_speech_bubbles`: アニメーション制御
    - `update_typewriter`: テキスト表示制御
    - `update_bubble_stacking`: 位置調整制御
    - `periodic_emotion_system`: [NEW] 定期的な感情判定ロジック
    - `reaction_delay_system`: [NEW] 勧誘時の遅延リアクション制御

---

## 5. パフォーマンス最適化 (Performance Optimization)

大規模環境（Soul 300体、使い魔 30体以上）での動作を支える以下の最適化が導入されています。

### 感情判定の分散実行 (Distributed Processing)
毎フレーム全 Soul を走査するのではなく、エンティティの ID に基づいて処理を **10フレーム（`PERIODIC_EMOTION_FRAME_DIVISOR`）** に分散させています。
- **仕組み**: `Entity.to_bits() % divisor` を利用し、1フレームあたり全 Soul の 1/10 程度のみを処理します。
- **効果**: 300体存在する場合でも、1フレームあたりの判定負荷は最大30体分に抑制されます。

### スタッキング再計算のイベント制御 (Event-driven Stacking)
吹き出しの垂直方向の位置調整（スタッキング）は、毎フレーム計算されるのではなく、以下のタイミングでのみトリガーされます。
- **トリガー**: 新しい吹き出しの追加（`Added<SpeechBubble>`）または既存の吹き出しの削除。
- **実装**: `ParamSet` と `RemovedComponents` を用い、変更を検知したフレームでのみ O(N log N) のソートおよび再配置処理を実行します。
- **効果**: 定常状態（吹き出しの増減がないフレーム）での CPU 負荷をほぼゼロに抑えます。

---

## 4. ランダムフレーズシステム

各使い魔はスポーン時にランダムな「口癖傾向」（`FamiliarVoice`）を持つ。

### 仕組み
- **お気に入り確率**: 60〜90%（使い魔ごとにランダム設定）
- **フレーズ選択**: お気に入り確率でお気に入りフレーズを使用、残りは完全ランダム

### フレーズ候補一覧 (各5つ)
| コマンド | フレーズ候補 |
|:--|:--|
| Veni | "Veni!", "Ad me!", "Huc!", "Sequere!", "Adesto!" |
| Laborare | "Laborare!", "Opus!", "Facite!", "Agite!", "Elabora!" |
| Fodere | "Fodere!", "Effodite!", "Excava!", "Pelle!", "Fodite!" |
| Caede | "Caede!", "Seca!", "Tunde!", "Percute!", "Incide!" |
| Portare | "Portare!", "Fer!", "Cape!", "Tolle!", "Affer!" |
| Requiesce | "Requiesce!", "Quiesce!", "Siste!", "Mane!", "Pausa!" |
| Abi | "Abi!", "Discede!", "I!", "Vade!", "Recede!" |
