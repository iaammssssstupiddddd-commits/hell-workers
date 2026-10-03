# TAK-14製品成果の通常Orca統合計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | tak14-product-integration-plan-2026-10-04 |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-10-04 |
| 作成者 | main agent |
| 関連Issue | TAK-14（履歴参照のみ、旧workflowは再開しない） |

## 1. 目的

別branchに保存された製品成果を現行の通常Orcaルールへ選択統合する。取り込みと製品の全体受入は分け、未検証のアート・数値・release・native性能を完了扱いしない。

## 2. スコープ

- 対象: cratesの建設物変更、building/perfツール、Blender候補生成と関連tests、必要な恒久仕様とHelp。
- 非対象: 廃止済み統括/Driver/recovery、外部公開、push/merge、原本撤去、無関係なnpm成果。

## 3. 現状とギャップ

- primary基点: `80df44c471a964907fb0fe8b4d0c634d11f33f13`。
- 採用元: cleanなTAK-14最終成果 `0272fa0da12eaf2364705c369f634371f8027111`。各マイルストーンはこのheadの祖先であり、重複取り込みしない。
- primaryの未コミット通常Orca移行・storage・文書成果を保持する。building-asset-sets仕様は既に採用元と同一。
- 旧技術passは新subjectの受入ではない。全対象の独立アート・数値承認、release、native/性能、文書closeは引き続き未達として扱う。

## 4. 実装方針

- branch: `codex/tak14-product-integration`。現primaryから作成し、dirty変更を保全。
- 選択pathだけを機械的に取り込み、現行dev/storage/Cargo/agentルールを上書きしない。
- production runnerの旧認証済みhost前提は実行可能性を別途監査する。旧helperを再起動しない。
- Bevy 0.19の現行gateでAPI整合を確認する。
- 公開/PR作成は今回の許可範囲外。

## 5. マイルストーン

### M1: 来歴と取り込み

- [x] branch/worktreeをread-only棚卸しし重複を除外。
- [x] 既存dirty変更を保全して専用branch作成。
- [x] 製品pathのみ108ファイル取り込み。現行dev/storage/Cargo/agentルールを保全。

### M2: 統合検証

- [x] Help実レビュー: Bridge配置条件・拒否結果を既存provider/coverageの境界で確認し仕様へ同期。現コードの生成器でsnapshot再生成とexact一致test PASS。
- [x] `python3 scripts/dev.py check`、全tooling、全Rust quality group、deps監査PASS。基点 `80df44c471a964907fb0fe8b4d0c634d11f33f13` の変更別CI相当検証で全4群成功、source不変を確認。
- [ ] 必要なnative確認を正式skillで実施、または不足するhost能力を具体化。
- [ ] exact-sourceの独立read-only review。

開発中チェック: 2026-10-04 `python3 scripts/dev.py check` PASS（format、Clippy suppression、agent契約、locked workspace compile）。
HelpはBridge実入力/preview/commitの成立・拒否経路からUpdate requiredと判断し、既存entryとsnapshotを取り込んだ。
全toolingはPython351件（新規docs scope testを含む）＋Blender189件とperf self-test/Ruff成功。
Rustはprofilingのworkspace test、profiling-memory/tracy/renderdoc compile、Clippy警告0、通常workspace test成功。
depsは成功（既存重複/yanked warningあり、advisories/bans/licenses/sourcesはok）。
rust-analyzer MCPはunexpected responseでdiagnostics未取得。診断0件と誤記せず、コンパイラとClippyの成功を記録する。

### M3: 保存と文書

- [x] docs索引とstorage確認。第三者node_modulesのREADMEを検査対象外にする限定補正と回帰testを追加し、docs check PASS。
- [x] 利用者指定の `tak-14` 制作原本・候補と最新Tank/MudMixer比較入力だけを外部stagingへ引継ぎ、全847ファイルの内容SHA-256一致を確認。保全先・用途・非承認境界は `docs/assets_workflow.md` に記録。他作業場は取り込まない。
- [x] 所有を区別したcommitと簡潔な未達事項の記録。

2026-10-04の保存単位: `9d7cc29f`（製品runtime・Bridge/Help）、`17d05acb`（制作・受入tooling）、
`1bda3877`（第三者文書の検査scope）、`12bbe72f`（storage登録preflightと拒否系7test）、
および本記録を含む通常Orca文書整理。push/mergeは行わない。
今回の最終コード検証は同じ元baseから全4群pass、Python358件・Blender189件・Rust各構成・Clippy警告0。
source fingerprintは `32277a08dafe65d5998e6c6920a0410769a98d3448f2eca6376ee5d57e6050e7`。
その後の文書のみの歴史資料表示追加はcontractsで再確認した。
独立read-onlyレビューはstorage/preflight、Bridge preview/commit、Help、authority境界の限定監査で
重大欠陥未検出。製品全範囲・native性能・アート・数値・releaseの最終受入は依然未完である。

## 6. リスクと対策

旧hostへの依存を通常Orcaの権限と誤認しない。旧文書とauthority receiptは根拠資料であり新承認にはしない。並行変更は破棄せず、重なるpathは再照合する。

## 7. 検証・データ管理

正本は `docs/development-infra/validation-storage-workflow.md`。primaryのvalidation coordinatorを使い、review-active cacheを保全する。開始時に既存storage checkはPASS、docs検査は既存untracked node_modulesの外部リンクで失敗している。無関係な原本は削除しない。
上記文書検査の失敗は検査scope補正後に解消。npm成果そのものは変更していない。
最終diff hygieneでも第三者dependencyの末尾空白を検出したため、root node_modulesを生成物としてGit ignoreへ追加。
package.json/lockfileは引き続き可視とし、既存dependencyの内容は保全する。

### 旧作業場の整理（2026-10-04）

利用者の明示指定により、primary以外を個別点検して整理した。Orca正規APIでPTY不在を確認し、
ホストprocessのcwd・実行binary・open fileも照合した。稼働参照がない以下12作業場は
旧保持consumerを正規releaseした後、`orca-ide worktree rm`（force・旧hook起動なし）で削除した。

- `tak-14-black-background-fix`
- `tak-14-building-asset-m1-a2`
- `tak-14-full-acceptance`
- `tak-14-full-acceptance-repair`
- `tak-14-m1-b-display-connection`
- `tak-14-m1-c-introduction-pipeline`
- `tak-14-m2-tank-mixer`
- `tak-14-m3-rest-spa`
- `tak-14-m4-bridge`
- `tak-14-m4-door-audit`
- `tak-14-m5-temporary-images`
- `tak-14-production-tooling-correction`

full-acceptanceにあった未追跡監査文書1件は、利用者の不要指定を確認してOSのゴミ箱へ移動したため復旧可能。
コミット履歴は採用元TAK-14 branchに残り、Orcaが未mergeと判定したbranchも保持する。
削除した専用cache自体は復旧対象ではなく、必要なら再buildする。

続いて利用者がprocess終了を明示承認した。`orca-abcd-expansion`、`orca-hardening-next`、
`orca-parallel-development`、`tak-14`、`tak-14-m6-nine-close`、`tak-14-production-tooling-merged-check`
の6作業場を参照していたClaude/MCP・bashを、PID/出生/cwd一致を再確認してTERM/HUPで終了した。
ホストprocess参照がなくなったことを確認してから、保持consumerを正規releaseし各作業場を削除した。
Orcaのinactive表示だけで未使用と判断せず、primaryとkitty親processは終了対象に含めていない。

旧基盤3作業場の未保存ソースは `git stash push --include-untracked` で復旧可能に退避し、
変更・未追跡fileの内容SHA-256をstash内と照合した。既存stashは維持する。

| 元作業場 | 復旧stash commit | 照合file数 |
| --- | --- | --- |
| orca-abcd-expansion | `ff8597f91a786e4f389acdd22f8ba3fb5899c469` | 117 |
| orca-hardening-next | `e652c04ebc0237aa5d3380d8a1df1b0bb73d3a25` | 181 |
| orca-parallel-development | `e139630be03fbe5bd3e4f101d9a47c41c0f48ac2` | 3 |

最終的に18作業場を削除し、Orca/Gitのhell-workers作業場一覧がprimaryだけであることを確認した。
旧TAK-14の72保持登録/89consumerも終了し、引継ぎ済みの外部staging holdだけを制作資料の保全先として維持する。

primaryのHEAD・既存dirty成果は不変。引継ぎ済みTAK-14資料847ファイルのinventory fingerprintも再一致した。
整理後のprimary storage checkとreconcileはPASS。filesystemのavailableは343,224,688,640から
619,746,779,136 bytesへ増加（全18作業場合計で約258 GiB）。旧workflowや未達受入を再開・成功化していない。

## 8. ロールバック

取り込み単位を専用commitに分ける。primary既存変更、採用元のコミット履歴と外部stagingの制作資料を保全し、reset/checkoutによる破棄はしない。旧worktreeの専用cacheは利用者指定で整理するため、rollbackに必要な原本は作業場へ依存させない。

## 9. AI引継ぎ

旧統括・Run・通知は再開しない。取り込み後は実diffからHelp判断し同subject検証へ進む。統合成功もTAK-14全体受入ではない。完了時に恒久結果を残し、本計画のarchive/removalと索引更新を行う。

独立read-only reviewとnative受入は未実施。正式全種campaignにはreleased identityとhost/session/計測producerが必要で、
移入したproduction runnerの旧context/receiptを通常Orcaへ流用しない。旧branchのpassや台帳は再開しない。
製品差分とprimary移行・storage変更は目的別のローカルcommitへ保存し、外部公開は行っていない。
無関係なnpm manifest/lockfileは未追跡のまま保全し、今回の採用対象へ含めない。
