# 全体実装レビュー追補: 横断リファクタリングロードマップ

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `system-wide-correctness-refactoring-plan-2026-07-12` |
| 文書種別 | 横断ロードマップ |
| ステータス | `In Progress` |
| 作成日 | `2026-07-12` |
| 最終更新日 | `2026-07-15` |
| 作成者 | `Codex` |
| 関連提案 | `N/A`（2026-07-12 の全体実装レビューに基づく） |
| 関連Issue/PR | `N/A` |

## 1. 目的

2026-07-12 の全体実装レビューで確認した問題を、依存関係とロールバック境界が異なる4つの実装計画へ分割する。

本書は実装手順の正本ではない。各変更の具体的なAPI、対象ファイル、テスト、完了条件はリンク先の子計画を正本とする。

## 2. 子計画

| 分類 | 計画 | 対象 | 開始条件 |
| --- | --- | --- | --- |
| 正しさ | [archive/runtime-correctness-contracts-plan-2026-07-12.md](archive/runtime-correctness-contracts-plan-2026-07-12.md) | 最小test harness、通知transport、タスク終了/Relationship、RemovedComponents、障害物同期 | Completed |
| 永続化 | [archive/save-load-hardening-plan-2026-07-12.md](archive/save-load-hardening-plan-2026-07-12.md) | 外部save header、schema、preflight/rollback、frame境界、load reset、legacy shim | Completed |
| 構造・品質 | [archive/structural-maintainability-followups-plan-2026-07-12.md](archive/structural-maintainability-followups-plan-2026-07-12.md) | production App composition、SpatialIndex共通化、Clippy allow、toolchain/local quality gate、format baseline | Completed（GitHub CI 実行確認は対象外） |
| 性能 | [system-wide-runtime-performance-plan-2026-07-12.md](archive/system-wide-runtime-performance-plan-2026-07-12.md) | 計測基盤、変更検知、A*予算、低頻度化、描画hot path | archive済みの履歴。再開時は新しい現行計画を作成する |

## 3. 分割理由

### 3.1 正しさ修復を先に完了可能にする

- 通知未配送、タスク誤完了、removal取りこぼしは現在の挙動不良であり、Save互換や全体formatを待たず修正する。
- `cargo fmt --all --check` の既存失敗や任意のCI導入を、runtime修復のDefinition of Doneに含めない。
- 各子計画は独立してarchiveできる。

### 3.2 Save/Loadを独立トランザクションとして扱う

- Save format versionはDynamicWorldの外側で判定する必要がある。
- schema、filesystem I/O、live World適用、load resetは一つの整合境界を構成する。
- `ReservedForTask` のruntime削除は旧save互換と不可分なためSave/Load計画で扱う。

### 3.3 構造整理で挙動修正を隠さない

- `bevy_app` production composition、SpatialGrid generic化、全体rustfmtは広い機械差分を生む。
- 正しさテストが揃った後に実施し、挙動変更と同じコミットへ混在させない。

## 4. 全体依存関係

```text
runtime M0: 最小library/test harness + all-target Clippy baseline
  ├─> runtime M1: RemovedComponents drain primitive
  ├─> runtime M2: notification transport
  └─> runtime M3: task lifecycle / Relationship

runtime M1 + M3 ─> runtime M4: source-aware obstacle sync

runtime M0 ─> save-load M1〜M3
runtime M1 + M2 + M4 ─> save-load M4 load reset/source rehydrate
runtime M3 ─> save-load M5 ReservedForTask migration

runtime M0 ─> performance M0: baseline計測
runtime M3 ─> performance task/reservation最適化
runtime M4 ─> performance obstacle/path最適化

runtime 完了 + save-load 完了
  ├─> structural M1 / M3 / M4
  └─> structural M2: tag / alias / update policyを保つ機械的共通化
       （2026-07-15 の明示指示により性能計画と独立して実施）

performance M0〜M7 + performance M8の実施/skip決定
  └─> 性能最適化の採否・後続の性能計画
```

## 5. 全体で固定する設計判断

1. `commands.trigger()` はObserverだけを起動し、Messageへは配送しない。Messageは `MessageWriter::write()` または `Commands::write_message()` で明示発行する。
2. RelationshipTargetは手動insert/removeしない。最後のsource削除でtarget component自体が消えることをconsumerが扱う。
3. `RemovedComponents<T>` はpredicate付きの場合も全件消費する。`.next()`と`.any()`の短絡を禁止する。
4. `obstacle_version`はwalkability topologyだけを表す。Door追加/削除と`Locked`境界では更新し、`Open`↔`Closed`のcost変更では更新しない。cost世代が必要なら別Resource/fieldに分ける。
5. Save format判定はDynamicWorld deserializeより先に行う。
6. load後はMessageだけでなく、旧simulation Entityを保持するResource、Local、UI/visual state、removed-component bufferを無効化する。
7. `hw_spatial` は `hw_logistics` に依存しない。generic indexのtagは依存循環を作らない所有位置に置く。

## 6. 期待する影響

- runtime/save-load計画は正しさと互換性を優先し、hot pathの高速化を主目的にしない。
- source-aware obstacle同期とSpatialIndex共通化は、不要な全件走査・再構築を減らす副次効果を持つ。
- 定量的なCPU/GPU改善は性能計画の再現可能なbaselineで判定し、対象カウンタが減らない変更は採用しない。
- Save/Loadのstagingとrollback snapshotによる一時メモリ・load時間増加は、preflight前のlive World不変とapply後のplayableなdegraded recoveryのため許容する。

## 7. 全体検証方針

各子計画の全マイルストーンで以下を必須とする。

- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 変更対象crateのテスト
- `cargo test --workspace`（子計画完了時）
- rust-analyzer workspace diagnostics 0件
- `git diff --check`

既存の全体rustfmt失敗は構造・品質計画で別コミットとして処理する。変更ファイル自体は各マイルストーンでrustfmt済みとする。

## 8. ロールバック方針

- 子計画単位ではなく、各マイルストーンを独立コミットにする。
- event型とProducer/Consumer、save headerとreader、Relationship source/consumerは同じコミットに含める。
- format-only commitは機能コミットから分離する。
- archive移動後に追跡が必要な場合は `git add -f docs/plans/archive/<file>` を使用する。

## 9. AI引継ぎメモ

### 現在地

- 進捗: 非性能の runtime / Save-Load / 構造子計画は archive 済み。性能子計画だけが継続し、本依頼の対象外。

### 次のAIが最初にやること

1. 性能作業を再開する場合は [system-wide-runtime-performance-plan-2026-07-12.md](archive/system-wide-runtime-performance-plan-2026-07-12.md) を履歴として確認し、必要なら新しい現行計画を作る。

### Definition of Done

- [x] 非性能の3子計画が完了・archive済み（性能計画は別トラック）
- [x] 確認済みの挙動不良に自動回帰テストがある
- [x] 恒久ドキュメントが現行コードと一致
- [x] `cargo check --workspace` 成功
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 成功
- [x] `cargo test --workspace` 成功
- [x] docs index更新済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-12` | `Codex` | 初版作成 |
| `2026-07-12` | `Codex` | 自己レビューを反映し、単一の9マイルストーン計画を3子計画のロードマップへ再編 |
| `2026-07-12` | `Codex` | 性能計画を横断ロードマップへ統合し、分割後の依存関係を明記 |
| `2026-07-12` | `Codex` | 性能M0の開始条件をruntime M0完了後へ統一 |
| `2026-07-15` | `Codex` | 非性能の残件を更新。runtime / Save-Load 計画を archive し、構造計画 M2 を性能最適化と独立した tag / alias 共通化として完了。M4 の GitHub CI 確認と性能子計画は継続。 |
| `2026-07-15` | `Codex` | 指示により structural M4 の GitHub CI 実行確認を対象外とし、ローカル品質ゲートで構造計画を完了・archiveした。 |
