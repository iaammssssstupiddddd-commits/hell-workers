# マルチツール AI ルール体系の構築

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `multi-tool-ai-rules-plan-2026-03-14` |
| ステータス | `In Progress` |
| 作成日 | `2026-03-14` |
| 最終更新日 | `2026-03-14` |
| 作成者 | Claude Code |
| 関連提案 | `docs/proposals/agent-md-for-branches.md` |
| 関連Issue/PR | N/A |

---

## 1. 目的

- **解決したい課題**: AI エージェントが root の `CLAUDE.md` のみを参照するため、サブシステム固有のルール・依存制約・禁止事項が伝わらず、誤変更が発生しやすい。また複数ツール（Claude Code / Codex）で同一ルールを共有できていない。
- **到達したい状態**: 各システムディレクトリに `_rules.md`（正規コンテンツ）を置き、`CLAUDE.md` と `AGENTS.md` がそのシンボリックリンクとして存在する。どのツールからアクセスしても同じルールが読まれる。
- **成功指標**:
  - `ls -la` で各対象ディレクトリに `CLAUDE.md -> _rules.md`、`AGENTS.md -> _rules.md` が存在する
  - `_rules.md` にそのシステムの責務・禁止事項・依存制約・検証方法が記載されている
  - `docs/invariants.md` と `docs/events.md` が存在し、最低限の内容がある

---

## 2. スコープ

### 対象（In Scope）

- `docs/invariants.md` の新規作成（ゲーム不変条件の明文化）
- `docs/events.md` の新規作成（イベントカタログ）
- 以下の各ディレクトリへの `_rules.md` 作成 + シンボリックリンク配置

**優先度 High（ECS 疎結合が大きいシステム）**
| ディレクトリ | 理由 |
|:---|:---|
| `crates/hw_familiar_ai/` | ビジネスロジック中核・誤変更リスク高 |
| `crates/hw_jobs/` | AssignedTask 体系・ECS 依存が複雑 |
| `crates/hw_logistics/` | サイレント失敗トラップが既知 |
| `crates/bevy_app/src/systems/familiar_ai/` | ECS 接続層・Bevy API 誤用リスク |
| `crates/bevy_app/src/systems/soul_ai/` | 同上 |

**優先度 Medium**
| ディレクトリ | 理由 |
|:---|:---|
| `crates/hw_world/` | 空間・部屋検出ロジック |
| `crates/hw_visual/` | Bevy UI/シェーダー・バージョン依存 |
| `crates/bevy_app/src/systems/room/` | 部屋検出の ECS 契約 |
| `crates/bevy_app/src/systems/jobs/` | ジョブ ECS 接続層 |

**優先度 Low（将来）**
| ディレクトリ | 理由 |
|:---|:---|
| `crates/hw_ui/` | UI は比較的独立 |
| `crates/hw_spatial/` | 空間グリッドは安定 |
| `crates/hw_core/` | 共通型・変更頻度低 |

### 非対象（Out of Scope）

- `target/` などビルド成果物
- `docs/plans/` 内の既存計画書（AI作業文書）
- root `CLAUDE.md` の内容変更（既に充実）
- テストスイート導入（別計画）

---

## 3. 現状とギャップ

- **現状**: AI ルールは root `CLAUDE.md` 1ファイルに集約。サブシステムに固有の制約・禁止事項・ECS 契約が文書化されていない。
- **問題**:
  - Familiar AI の「Familiar は直接作業しない」などの不変条件が推論不可
  - Logistics の「TransportRequest なしで Haul系WorkType はサイレントにフィルタされる」が非明示
  - ECS Observer/Relationship の書き込み責務（どのシステムが書くか）が散在
  - Claude Code と Codex で異なるファイル名（`CLAUDE.md` vs `AGENTS.md`）のため二重管理が必要だった
- **本計画で埋めるギャップ**: シンボリックリンク方式により単一正規ファイル + 多ツール対応を実現する

---

## 4. 実装方針（高レベル）

- **方針**: `_rules.md`（正規コンテンツ）を各ディレクトリに置き、`CLAUDE.md` と `AGENTS.md` はそのシンボリックリンクとする
- **シンボリックリンクの規則**:
  - 相対パスで作成: `ln -s _rules.md CLAUDE.md`
  - `git` はデフォルトでシンボリックリンクを追跡するので `.gitignore` 変更不要
- **`_rules.md` の構成（各ファイル共通フォーマット）**:
  ```markdown
  # [システム名] — AI Rules

  ## 責務（このディレクトリがやること）
  ## 禁止事項（AIがやってはいけないこと）
  ## 依存制約（依存してよいもの・してはいけないもの）
  ## 既知のサイレント失敗トラップ
  ## docs 更新対象（変更時に必ず更新するドキュメント）
  ## 検証方法
  ```
- **Bevy 0.18 APIでの注意点**: コードは変更しないため該当なし

---

## 5. マイルストーン

## M1: グローバル不変条件とイベントカタログ

- **変更内容**: `docs/invariants.md` と `docs/events.md` を新規作成
- **変更ファイル**:
  - `docs/invariants.md`（新規）
  - `docs/events.md`（新規）
- **完了条件**:
  - [ ] `invariants.md`: 以下を含む
    - Soul は AssignedTask が None なら Idle 状態
    - Familiar は直接作業しない（指揮のみ）
    - タスクは二重割当しない（WorkingOn は 1 Soul に 1 つ）
    - reservation と inventory の整合性
    - UI は simulation state を直接変更しない
    - TransportRequest のない Haul 系 WorkType はサイレントにフィルタされる
  - [ ] `events.md`: 主要イベントの Producer/Consumer/Timing 表を含む
- **検証**: ファイルの存在確認（`cargo check` 不要）

---

## M2: hw_familiar_ai + hw_jobs（最高優先）

- **変更内容**: 最も複雑な 2 crate に `_rules.md` を作成し、シンボリックリンクを配置
- **変更ファイル**:
  - `crates/hw_familiar_ai/_rules.md`（新規）
  - `crates/hw_familiar_ai/CLAUDE.md`（シンボリックリンク）
  - `crates/hw_familiar_ai/AGENTS.md`（シンボリックリンク）
  - `crates/hw_jobs/_rules.md`（新規）
  - `crates/hw_jobs/CLAUDE.md`（シンボリックリンク）
  - `crates/hw_jobs/AGENTS.md`（シンボリックリンク）
- **`hw_familiar_ai/_rules.md` 必須記載事項**:
  - 責務: Familiar の状態遷移・タスク探索・リクルート・Squad管理のビジネスロジック
  - 禁止: Bevy ECS への直接アクセス（Commands, Query）を持ち込まない
  - 禁止: Soul の WorkingOn を直接操作しない
  - 依存: `hw_jobs`・`hw_world` のみに依存可（`bevy_app` への逆依存禁止）
  - 参照ドキュメント: `docs/familiar_ai.md`
- **`hw_jobs/_rules.md` 必須記載事項**:
  - 責務: AssignedTask の定義・ライフサイクル・状態遷移ロジック
  - 禁止: `unassign_task` 内で WorkingOn・CommandedBy を操作しない（各 Observer の責務）
  - 禁止: `#[allow(dead_code)]` の使用
  - サイレント失敗トラップ: Haul 系 WorkType に TransportRequest がないと無音でスキップされる
  - 参照ドキュメント: `docs/tasks.md`
- **完了条件**:
  - [ ] 各 `_rules.md` が上記必須事項を含む
  - [ ] `ls -la crates/hw_familiar_ai/ | grep CLAUDE` がシンボリックリンクを示す
  - [ ] `ls -la crates/hw_jobs/ | grep CLAUDE` が同上
- **検証**: `cargo check` 不要（ドキュメントのみ）

---

## M3: hw_logistics（サイレント失敗トラップの明文化）

- **変更内容**: Logistics の既知トラップを `_rules.md` に記録
- **変更ファイル**:
  - `crates/hw_logistics/_rules.md`（新規）
  - `crates/hw_logistics/CLAUDE.md`（シンボリックリンク）
  - `crates/hw_logistics/AGENTS.md`（シンボリックリンク）
- **`hw_logistics/_rules.md` 必須記載事項**:
  - 責務: リソース予約・在庫管理・Auto-Haul 要求の生成
  - サイレント失敗トラップ: `SharedResourceCache` の予約解放は `unassign_task` の責務
  - サイレント失敗トラップ: TransportRequest がない場合、運搬ジョブが検索対象にならない
  - 参照ドキュメント: `docs/logistics.md`
- **完了条件**:
  - [ ] `_rules.md` が既知トラップを 2 件以上含む
  - [ ] シンボリックリンク 2 件配置

---

## M4: bevy_app システム層（ECS 接続層）

- **変更内容**: ECS 接続層のルールを作成
- **変更ファイル**:
  - `crates/bevy_app/src/systems/familiar_ai/_rules.md`（新規）
  - `crates/bevy_app/src/systems/familiar_ai/CLAUDE.md`（シンボリックリンク）
  - `crates/bevy_app/src/systems/familiar_ai/AGENTS.md`（シンボリックリンク）
  - `crates/bevy_app/src/systems/soul_ai/_rules.md`（新規）
  - `crates/bevy_app/src/systems/soul_ai/CLAUDE.md`（シンボリックリンク）
  - `crates/bevy_app/src/systems/soul_ai/AGENTS.md`（シンボリックリンク）
  - `crates/bevy_app/src/systems/jobs/_rules.md`（新規）
  - `crates/bevy_app/src/systems/jobs/CLAUDE.md`（シンボリックリンク）
  - `crates/bevy_app/src/systems/jobs/AGENTS.md`（シンボリックリンク）
- **各 `_rules.md` 共通必須記載事項**（ECS 接続層向け）:
  - 責務: leaf crate のロジックを Bevy ECS へ接続するアダプタのみ
  - 禁止: このディレクトリにビジネスロジックを書かない（leaf crate に書く）
  - 禁止: Bevy 0.18 以前の API 使用（必ず `docs.rs/bevy/0.18.0` または既存コードで確認）
  - ECS システムセット実行順: `Input → Spatial → Logic → Actor → Visual → Interface`
- **完了条件**:
  - [ ] 3 ディレクトリに `_rules.md` + シンボリックリンク 2 件ずつ

---

## M5: 残りの Medium 優先ディレクトリ

- **変更内容**: hw_world, hw_visual, room 検出
- **変更ファイル**:
  - `crates/hw_world/_rules.md` + シンボリックリンク 2 件
  - `crates/hw_visual/_rules.md` + シンボリックリンク 2 件
  - `crates/bevy_app/src/systems/room/_rules.md` + シンボリックリンク 2 件
- **`hw_visual/_rules.md` 必須記載事項**:
  - 責務: レンダリング・シェーダー・スピーチバブル等の視覚表現
  - 禁止: シミュレーション状態を直接変更しない（UI/Visual は読み取り専用）
  - Bevy 0.18 注意: Window/UI API は変更が多い、必ず既存コードか docs.rs を参照
- **完了条件**:
  - [ ] 3 ディレクトリに `_rules.md` + シンボリックリンク配置

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `_rules.md` と `docs/*.md` の内容が矛盾 | AI が矛盾した指示を受ける | `_rules.md` は「禁止事項・依存制約」のみ。詳細仕様は `docs/` を参照するリンクにとどめる |
| `CLAUDE.md`（シンボリックリンク）が root `CLAUDE.md` と混同される | ルールの重複/矛盾 | サブの `_rules.md` はそのディレクトリ固有の内容のみ記載。root の内容を繰り返さない |
| git でシンボリックリンクが壊れる | リンク切れ | `git config core.symlinks true`（デフォルト）を前提。Windows 環境では別途対応が必要 |
| `_rules.md` が更新されず陳腐化 | 古いルールで誤変更 | 各 `_rules.md` の「docs 更新対象」セクションに自分自身を含める |
| ディレクトリ数が多く `_rules.md` の品質がばらつく | 一部ディレクトリのルールが薄い | M2（高優先）を先行し、品質基準を確立してから M4・M5 を進める |

---

## 7. 検証計画

- **必須**:
  - `ls -la <対象ディレクトリ>` でシンボリックリンクの存在確認
  - `cat <対象ディレクトリ>/CLAUDE.md` で内容が `_rules.md` と同一であることを確認
  - `cargo check` は不要（コード変更なし）
- **手動確認シナリオ**:
  - 新しい AI セッションを開いて `crates/hw_familiar_ai/src/lib.rs` について質問し、Familiar AI のルール（禁止事項）が自動的に適用されているか確認

---

## 8. ロールバック方針

- **どの単位で戻せるか**: マイルストーン単位。シンボリックリンクと `_rules.md` を削除するだけで元に戻る
- **戻す時の手順**:
  ```bash
  # 対象ディレクトリ内の全シンボリックリンクと _rules.md を削除
  rm crates/hw_familiar_ai/CLAUDE.md crates/hw_familiar_ai/AGENTS.md crates/hw_familiar_ai/_rules.md
  ```
- コード変更がないため、ビルドへの影響はゼロ

---

## 9. AI 引継ぎメモ（最重要）

### 現在地

- 進捗: `20%`
- 完了済みマイルストーン: M1（docs/invariants.md・docs/events.md 作成完了）
- 未着手/進行中: M2（hw_familiar_ai / hw_jobs）

### 次の AI が最初にやること

1. `docs/invariants.md` を新規作成（M1）
2. `docs/events.md` を新規作成（M1）
3. `crates/hw_familiar_ai/_rules.md` を作成し、シンボリックリンクを配置（M2）

### シンボリックリンク作成コマンド例

```bash
# _rules.md を作成後、そのディレクトリで実行
cd crates/hw_familiar_ai
ln -s _rules.md CLAUDE.md
ln -s _rules.md AGENTS.md
```

### ブロッカー/注意点

- `_rules.md` の内容は薄すぎてもいけない（禁止事項・依存制約が空では意味なし）が、`docs/*.md` の内容を丸ごとコピーしてもいけない（重複・矛盾リスク）
- M4（bevy_app システム層）は ECS 接続層であり、ビジネスロジックは M2/M3 の leaf crate にあることを念頭に置いて `_rules.md` を記載する

### 参照必須ファイル

- `docs/familiar_ai.md` — Familiar AI 仕様
- `docs/tasks.md` — タスク ECS 接続マップと unassign_task の契約
- `docs/logistics.md` — Logistics サイレント失敗トラップ
- `docs/architecture.md` — システムセット実行順

### 最終確認ログ

- 最終 `cargo check`: N/A（コード変更なし）
- 未解決エラー: なし

### Definition of Done

- [x] M1: `docs/invariants.md` と `docs/events.md` が存在し最低限の内容がある
- [ ] M2: `hw_familiar_ai/` と `hw_jobs/` にシンボリックリンク体系が完成
- [ ] M3: `hw_logistics/` にシンボリックリンク体系が完成
- [ ] M4: `bevy_app/src/systems/` の 3 ディレクトリが完成
- [ ] M5: `hw_world/`・`hw_visual/`・`systems/room/` が完成

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-14` | Claude Code | 初版作成 |
