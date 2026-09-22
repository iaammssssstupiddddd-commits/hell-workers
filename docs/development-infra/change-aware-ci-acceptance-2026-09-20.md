# 変更内容に応じたCI・ルール更新の完了記録

2026-09-20完了。計画ID: `change-aware-ci-plan-2026-09-19`。
運用仕様は[開発ガイド](../DEVELOPMENT.md)を参照する。

## 完了範囲と日次監査の扱い

依頼の目的は「変更内容に応じてCIを自動実行すること」と、それに合わせた開発ルール・branch運用の更新である。
PR更新・master pushで変更を分類し、必要群だけを実行する仕組み、必須jobの失敗を通さない集約、
同じ対象の成功CIを完了判定へ使うルールを導入・検証した。

日次依存監査は今回の依頼で新設した要件ではない。導入前のcommit `52913b61`の`ci.yml`にも
`17 3 * * *`のscheduleがあり、既存機能を`dependency-audit.yml`へ分離して維持した。
コード変更のない日にも依存ライブラリの新しい脆弱性情報を確認する補助機能であり、変更時CIの成立には不要である。

当初の計画では、日次監査の自然起動まで必須受入に加え、起動待ちを理由にcloseを保留していた。
2026-09-20のユーザーの「定期実行は指示していない」という指摘を受け、この過剰な必須条件を撤回した。
変更時CIとルール更新は受入済みとして計画をcloseする。自然起動の成功を確認したことにはしない。
2026-09-20 03:42 UTCの確認では自然起動runは0件。Actionsと両workflowは有効、default branch上のcronも確認済み。
今回のcloseでは既存の日次設定を変更していない。

## 採用内容

- 共通driver: `dev.py quality`、`ci plan/result/check`、`quality.py`、`ci_scope.py`、`ci_result.py`。
- 検証群: contracts / tooling / deps / rust。文書のみはcontracts、Rust変更はtoolingとworkspace全体も実行する。
- 対象固定: eventのbase/head/tested SHA、plan digest、local dirtyを含むsource fingerprintで対象を照合する。
- 集約: 選択jobの成功と出力一致、未選択jobのskipを要求し、失敗・取消・欠損を通さない。
- ルール: 独立変更は目的別branch、同目的の修正はbranch再利用、PRが自動CIの入口。
  root rules、task lifecycle、関連Skillと各adapter、開発ガイド、plan templateを同期済み。
- GitHub: [実装PR #20](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/20)と
  [監査イベント修正PR #25](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/25)をmasterへmerge済み。
  branch保護は新設しておらず、任意CIとして導入した。

## GitHub受入証拠

C=contracts、T=tooling、D=deps、R=rust。所要時間は待機・setup・cache復元を含む。
Rust実行はwarm/exact cache hit。cold cacheは未測定で、固定の短縮率は主張しない。

| 対象 | run | 確認結果 | 時間 |
| --- | --- | --- | --- |
| 実装PR #20 | [35459170013](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459170013) | C/T/D/R・集約success | 9分21秒 |
| 文書のみ #21 | [35459220974](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459220974) | Cのみ、T/D/R skip。Rust build・native apt・Cargo cache復元0件 | 32秒 |
| Toolingのみ #22 | [35459250390](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459250390) | C/T・集約success、D/R skip | 1分24秒 |
| Rust testのみ #23 | [35459267058](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459267058) | D skipでもC/T/R・集約success | 10分20秒 |
| 意図した契約失敗 #24 | [35459294822](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459294822) | C failure、選択済みT/R未起動、集約failure | 34秒 |
| 修正PR #25 | [35460335473](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35460335473) | C/T/D/R・集約success | 9分28秒 |
| 最終master push | [35460869075](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35460869075) | C/T/D/R・集約success | 16分34秒 |
| 最終master手動full | [35460868974](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35460868974) | C/T/D/R・集約success | 9分26秒 |
| 最終master独立監査 | [35460870538](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35460870538) | Daily dependency auditのみsuccess。品質checkを生成しない | 23秒 |

実装headは`134fe6c3b0cbcb7fe2c23555ec8d13d1d9010da1`、baseは`52913b61ca524f1cd429d3d73a0468faf3d0999b`。
子PR #21〜#24は実装headをbaseとして、fixtureをmasterへmergeせず確認した。

| PR | tested merge SHA |
| --- | --- |
| #20 | `1697b499ecbe60f98723bf5c3b6860c2123ffdaa` |
| #21 | `2db4bbf79c9a41f4171b34151f33038ac0cf62a0` |
| #22 | `34c4a4e20fcdd1c49f77ff531a2957ab33545005` |
| #23 | `bd83ece0b00c5c22b6e046b90d26d3f1aad52c33` |
| #24 | `0ba88bfa5e0732d86e3e90310c95888555c5b573` |
| #25 | `5b47054ceed2ac6c18846fd18e6eec2c58113007` |

PR #25はhead=`4a339c657cc9c529dfe429922a37b871b825a852`、base=`a0b515a8f44711ca81a3713a5aece4e5f6ef44a2`。
最終masterの3 runの対象SHAは`b68bafd7358580ff8ad77962fa02987a215b5b60`。
pushと手動fullのbaseは`a0b515a8f44711ca81a3713a5aece4e5f6ef44a2`。

取消分離は、PR #24の連続pushで[旧run 35459533448](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459533448)だけcancelled、
[新run 35459541624](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459541624)は意図した契約失敗、他PRのRustは継続したことで確認した。

受入中の[独立監査初回 35459819407](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459819407)では
入力なし手動イベントの`inputs:null`処理が失敗した。PR #25で修正し、null／省略／空inputsとscheduleの回帰test、
手動品質planのbase必須条件を確認した。修正後の独立監査は成功している。

## ローカル検証・Help・整理

- 初回導入時にprimaryで`dev.py verify`、`check`、全4群の`ci check`が成功。
  通常・profilingのworkspace test、Clippy警告0、memory/tracy/renderdoc最小feature、online依存監査を含む。
- 分類・出力欠損・SHA不一致・skip/cancel・dirty変更・SHA取得失敗などをfixtureで検査した。
  Cargo/rustc/rustup/Blenderを失敗するstubにしたPATHでもtooling全群が成功し、実compiler呼出し0件。
- Help実レビューはNo impact。変更経路は開発driver・Actions・ルール内で完結し、
  `build_help_panel_content`→manifest/provider→`hw_ui`、ゲーム入力・状態・runtime assetは不変。native受入は対象外。
- primaryへ`86eec196`で採用し、`caa5188a`でmasterの検証済み履歴をtree差分0件で統合。
  別PR #19・建築アートの内容はCI公開へ混ぜていない。
- retain `change-aware-ci-candidate`のconsumerをreleaseし、`target/change-aware-ci`を撤去済み。
  local専用branch 7本、remote専用branch 6本も整理済み。子PR 4件はmergeせずclose。
  回収量は26,660,864 bytes（約25.4 MiB）。共有cacheと別作業の環境は保持した。
- 本記録への集約後、一時計画を削除し、plans/proposals両indexを再生成した。
  過去の詳細計画はGit履歴の`53c8b6fb`に残る。
