---
name: hell-workers-review-help-impact
description: Review completed hell-workers implementation changes for player-facing Help impact. Use after implementing features, fixes, UI, input, gameplay, settings, notifications, or runtime data to update Help when required or validate a precise no-impact decision.
---

# Hell Workers Review Help Impact

実装済み差分からプレイヤーが知る必要のある変更を判定し、Help更新または根拠付きno-impact判断まで完遂する。
gateの成否だけで要否を決めず、実際の入力、成立条件、結果、表示経路を確認する。

## 1. 対象差分を確定する

1. `docs/help-screen.md`を最後まで読む。
2. `git status --short`、`git diff --name-status`、対象pathの`git diff`を確認する。
3. 直前の実装依頼と作業開始時の状態から、今回の変更と既存・並行変更を分ける。
4. producerからplayer-visible consumerまで実経路を追い、到達可能性、前提条件、成功・失敗結果を確認する。
5. mixed worktreeで全production差分を説明できない場合、包括的なno-impact理由を作らず追加調査する。

既存のHelp変更が同じworktreeにある、または`check_help_impact.py`が通るという事実だけを、今回の実装が
Helpへ反映済みである証拠にしない。

## 2. Help影響を分類する

次のいずれかに分類し、根拠となる型、system、UI、data、testを記録する。

| 判断 | 条件 |
|:--|:--|
| `Update required` | playerが使える機能、操作、workflow、前提、結果、失敗理由、label、tooltip、shortcut、設定、通知が追加・変更・削除された |
| `Update required` | `InputAction`、`UiIntent`、player-facing domain variant、Task Dashboard control/filter/sort、runtime text/data sourceが変わった |
| `Update required` | 機能の到達可能性が`Blocked`、`Published`、`Excluded`の間で変わった |
| `No impact` | 内部refactor、性能改善、test、diagnosticだけで、player-visibleな操作・意味・文言・成立条件が不変であることを実経路で確認した |
| `Undetermined` | 対象差分、到達可能性、または外部依存の挙動を確認できない |

`Undetermined`をno-impactとして処理しない。一次情報または実装を追加確認し、それでも確定できなければ
ブロッカーとして報告する。

## 3. 更新が必要な場合

次を同じ変更バッチで行う。

1. `crates/bevy_app/src/interface/ui/help_content/manifest.rs`の既存feature inventoryを確認する。
2. 適切な`providers/*.rs`で既存stable IDを維持して本文を更新する。新しい独立topicだけに新IDを追加する。
3. project-owned shortcutを本文へ直書きせず、canonical bindingから生成する。
4. `coverage.rs`で新規・変更surfaceをstable surface ID付きの`Published`、理由付き`Excluded`、
   またはtarget/reason/owner付き`Blocked`へ分類する。
5. 到達可能な成功経路に加え、非表示context、foreground capture、text input、blocked stateなど、
   適用してはいけない拒否経路を守るbehavior testを追加・更新する。
6. root生成器からexact snapshotを再生成する。

```bash
cargo test -p bevy_app@0.1.0 regenerate_help_approval_snapshot -- --ignored
```

7. 通常のexact testはwriter化しない。更新専用testを明示実行した後、`coverage_approval.snap`の全差分を読み、
   意図した本文、順序、shortcut、owner、coverage判断だけが
   変わったことを確認する。snapshotを手作業で部分的に通過させない。
8. player-facing仕様書を更新する。Helpの所有権、coverage、更新契約自体を変えた場合は
   `docs/help-screen.md`も更新する。広い実装ドキュメント同期には、対応するUpdate Docs Skillを併用する。

## 4. 更新が不要な場合

1. 「何が内部変更で、どのplayer-visible契約が不変か」を1文で具体的に記述する。
2. その理由がdiff base以後とdirty worktreeを含む全production変更を覆うことを確認する。
3. commit前のローカル検証だけ、理由を一時overrideとして渡す。

```bash
HELL_WORKERS_HELP_IMPACT_REASON='具体的なno-impact理由' \
  python3 scripts/check_help_impact.py
```

全体検証にも同じ理由を明示する。

```bash
HELL_WORKERS_HELP_IMPACT_REASON='具体的なno-impact理由' \
  python3 scripts/dev.py verify
```

この環境変数は判断を永続化せず、CIでは無効である。commitが明示的に依頼された場合だけ、全production変更の
子孫となるcommitへ次のexact trailerを付ける。

```text
Help-Impact: none
Help-Impact-Reason: 具体的なno-impact理由
```

no-impact変更でHelp sourceやsnapshotへ空変更を加えない。commitのamend、push、PR操作は依頼なしに行わない。

## 5. 検証する

Helpを更新した場合は次を実行する。

```bash
cargo test -p bevy_app@0.1.0 \
  exact_snapshot_approves_all_player_visible_help_copy_and_coverage
cargo test -p bevy_app@0.1.0 help_
cargo test -p hw_ui help
python3 -m unittest scripts.tests.test_check_help_impact
python3 scripts/check_help_impact.py
python3 scripts/dev.py verify
```

Rustを変更した場合はrust-analyzer診断も0件にする。最後に`git diff --check`を実行する。

## 6. 報告する

次を簡潔に報告する。

- 確認した実装scope。
- `Update required`または`No impact`の判断とコード上の根拠。
- 更新したmanifest/provider/coverage/snapshot/docs、またはexact no-impact理由。
- 実行した検証と結果。
- 未確認の並行差分やブロッカー。
