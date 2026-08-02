# 新PC開発環境移行・Blender新規構築計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `development-workstation-blender-migration-plan-2026-07-29` |
| ステータス | `In Progress` |
| 作成日 | `2026-07-29` |
| 最終更新日 | `2026-08-01` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 0. ここから始める（ユーザー用の操作票）

> **2026-08-01 方針変更（現在の実行基準）**
>
> ユーザー判断により、別の旧Blender PCから資産、`.blend`、設定、addon、presetを
> 引き継がない。旧Blender PCはsource PC、G0/G1、旧access失効、secure eraseの対象外とし、
> このPCを唯一の新規Blender／asset authoring hostとして開始する。旧環境とのversion一致、
> asset snapshot照合、Syncthing pairing／writer切替は受け入れ条件ではない。
> `~/Sync/hell-workers-assets/source/` と `exports/` が空なのは正常な初期状態であり、
> repo内`assets/`は実行用referenceとして保持するがBlender正本へ逆変換しない。
> 旧開発PCのGit/WIP M0／M1証跡と外付けbackupは引き続き有効で、廃棄しない。
>
> 以下の旧source PC向けM0／M1手順は、完了済みの開発環境移行の監査記録として残す。
> 現在の作業はM3の新規workspace基準化から再開し、§10の引継ぎメモを正とする。

この章だけ読めば移行を開始できる。以降のM0〜M6はAIが実行時に参照する技術手順であり、
ユーザーがコマンドを手入力して順番を管理する前提ではない。

この計画の「AI」は、**その時点でAIを起動しているPCと、そこへmountされた媒体だけ**を確認できる。
「source PC」は、WIP／repository、Blender原本、runtime asset、save／settingsのいずれかを
保持する移行元端末を指す。これらが別端末に分かれている場合、1台だけを調べて移行完了にしない。
原則として次の順でAIセッションを引き継ぐ。

1. 現在のWIP／repository／runtime assetを持つ旧開発PCでM0／M1を実行する。
2. Blender原本やsave等を別の旧PCが持つ場合、その全source PCでもM0／M1を繰り返す。
3. 端末別のM1成果物を同じ暗号化外付け媒体へ保存し、全source PCのG1合格を確認する。
4. その後にだけ新PCでAIを起動し、同じ計画とM1成果物を渡してM2以降を実行する。

### 0.1 最初の1セッションでユーザーが行うこと

開始場所は、**現在のrepository、未コミットWIP、runtime assetを持つ旧開発PC**の
repository rootとする。Blender原本が別PCなら、この最初の端末のM0／M1後に同じ手順を
Blender PCでも繰り返す。最初はM0の読み取り専用監査だけを行う。

1. 次の操作をまだ行わない。
   - 旧PCの初期化、売却、譲渡。
   - Blender／addonの更新、原本 `.blend` の再保存。
   - Syncthingの再設定、folder override、device／folder削除、同期再開。
   - repo内 `assets/` の整理、上書き、`--delete-missing`。
   - 現在の未コミット作業の一括commit／discard。
2. 可能なら暗号化した外付け媒体を旧PCへ接続してunlockする。
   passphrase、token、private keyはAIへ渡さず、ユーザーが対話画面へ入力する。
   媒体が未用意でもM0の読み取り専用調査とチャット報告までは開始できる。
   ただしG0成果物は確定せず、`<MIGRATION_ROOT>` が用意できるまでM1は開始しない。
3. 旧PCのrepository rootでAIを起動し、次の入力票を埋めて渡す。
   不明な項目は `不明` でよく、AIが読み取り専用で調査する。

| 入力項目 | ユーザーが伝える内容 |
| --- | --- |
| この端末 | 旧PC／新PC／別の作業PC |
| この端末が持つデータ | WIP／repository、Blender原本、runtime asset、save／settingsの該当項目 |
| 他のsource PC | Blenderやsave等を別端末が持つ場合は端末名と保持データ。不明でもよい |
| 新PC | OS、CPU architecture、GPU、RAM、disk容量 |
| 旧PCの予定 | 保管／再利用／売却／譲渡／廃棄／未定 |
| backup媒体 | mount path、空き容量。未用意ならその旨 |
| 検索を許可するroot | repo、Blender制作folder、asset folderなど。AIはこの範囲外を検索しない |
| `<ASSET_ROOT>` 候補 | 例: `~/Sync/hell-workers-assets`。不明でもよい |
| 任意機能 | Web/WASM、GCP/TRELLIS、perf履歴を移すか |
| 現在のWIP方針 | commit候補／backupのみ／未定 |

4. AIのM0報告を受け取ったら、次の4点だけをユーザーが判断する。
   - canonicalな `.blend` 原本をどれにするか。見つからない場合にfallback再構築を認めるか。
   - 現在のWIPをscoped commit／pushするか、backupだけにするか。
   - M1のbackup先と必要容量を承認するか。
   - 旧PCの最終処遇をどうするか。
5. 上記を承認した後にだけ、AIへM1の実行を依頼する。

### 0.2 ユーザー作業とAI作業の境界

ユーザーが担当するのは、物理操作、秘密情報、主観的な目視判断、正本の意味判断、
外部サービスの対話認証、不可逆または同期役割を変える操作の承認である。
それ以外のterminal作業、ファイル調査、backup作成、checksum、復元試験、
環境構築、検証、記録、文書更新はAIが担当する。

| 段階／作業PC | ユーザーが行うこと | AIへ引き継ぐこと | 次へ進む条件 |
| --- | --- | --- | --- |
| M0／各source PC | 入力票、検索許可root、正本・WIP・旧PC処遇の判断 | 端末ごとのGit、Blender、Syncthing、asset、saveの読み取り専用棚卸し、容量見積、台帳、blocker一覧 | 全source PCが列挙され、ユーザーが原本／fallback、WIP、backup方針を承認 |
| M1／各source PC | 外付け媒体の接続・unlock、編集停止、commit／pushを行う場合の明示承認 | 端末別のGit全状態とGit外データのsnapshot、SHA-256 manifest、実restore test | 全source PCのG1証跡が揃い、ユーザーが全source PCのfreezeを宣言 |
| M2／新PC | OS初期設定、再起動、管理者承認、GitHub等の対話認証 | package／Rust／MCP導入、fresh clone、WIPの隔離復元、`doctor`／`check` | AIがG2の自動検証結果を提示 |
| M3／新PC | 新規asset workspace方針と、このPCを唯一のwriterにする判断 | 空の`source/`／`exports/`、staging、manifest、license領域を初期化し、smoke一周と基準記録 | G3の新規workspace基準が合格 |
| M4／新PC | 最初のcanonical assetと見た目の承認 | 現PCで新規`.blend`を制作し、staging export、validator／構造／MCP検査、文書化 | validatorだけでなくユーザーの目視も合格 |
| M5／新PC | visual test、F5/F9、最初のcanonical promoteの明示承認 | 全自動gate、manifest、backup、promote後の再検証 | ユーザーが新規assetを正本として承認 |
| M6／新PC | 14日または2開発サイクルの利用と問題報告 | 最終restore test、安定化監査、恒久docs、plan完了処理 | 新PCだけで開発・Blender一周と独立backup復元が成功 |

次の操作は、AIが技術的に実行できる場合でもユーザーの明示承認なしに行わない。

- Gitのcommit／push。
- Syncthingのpairing、folder共有開始、folder role変更。
- staging `.blend`／GLBのcanonical `source/`／`exports/` へのpromote。
- 旧PCのaccount／device access失効。
- 旧PCのsecure erase／初期化。

### 0.3 セッション間で引き継ぐ固定成果物

ユーザーが暗号化外付け媒体上のrootを `<MIGRATION_ROOT>` として指定し、
AIが次の構成を作成・更新する。ユーザーが個別ファイルを整理する必要はない。

```text
<MIGRATION_ROOT>/
├── ledger.md
├── runbook/
│   └── development-workstation-blender-migration-plan-2026-07-29.md
├── manifests/
│   └── <SOURCE_HOST>/
│       ├── git-state/
│       ├── assets.sha256
│       ├── blender-source.sha256
│       └── user-data.sha256
├── snapshots/
│   └── <SOURCE_HOST>/
│       ├── git/
│       ├── assets/
│       ├── blender/
│       └── user-data/
├── reports/
│   └── <SOURCE_HOST>/
│       ├── blender-inventory.md
│       ├── blockers.md
│       └── restore-test.md
└── acceptance/
    ├── new-pc-toolchain.md
    ├── blender-bevy-roundtrip.md
    └── cutover.md
```

secret、passphrase、token、private key、credential DBはこのrootへ保存しない。
`<SOURCE_HOST>` は台帳で一意にした端末名とし、別端末の成果物を同名で上書きしない。
AIは本計画の作業時点copyを `runbook/` へ保存してSHA-256を `ledger.md` に記録する。
repositoryがないBlender専用PCでは、このcopyを読み、同じM0／M1契約で作業する。
媒体が未用意のM0ではこの構成をlocal diskへ仮作成せず、チャット報告だけを返して
G0を `NO` とする。媒体準備後にM0を再開し、確定成果物をここへ保存する。
旧PCから新PCへ移る際は、ユーザーが外付け媒体を物理的に移動してunlockし、
新PC側のAIへ `runbook/` の計画copyと `<MIGRATION_ROOT>/ledger.md` を最初に読ませる。

### 0.4 旧PCのAIへ渡す開始メッセージ

次を埋めて、そのまま旧PC側のAIへ渡す。
repositoryがないBlender専用PCでは、1行目のplan pathを
`<MIGRATION_ROOT>/runbook/development-workstation-blender-migration-plan-2026-07-29.md`
へ読み替える。

```text
docs/plans/development-workstation-blender-migration-plan-2026-07-29.md の
「0. ここから始める」とM0に従い、M0だけを開始してください。

この端末:
この端末が持つデータ:
他のsource PCと保持データ:
新PCのOS / CPU / GPU / RAM / disk:
旧PCの予定:
backup媒体のmount path / 空き容量:
検索を許可するroot:
ASSET_ROOT候補:
Web/WASM、GCP/TRELLIS、perf履歴の要否:
現在のWIP方針:

source側は読み取り専用にし、書き込みは指定したMIGRATION_ROOT内の報告だけに限定してください。
commit、push、同期、削除、upgrade、Blender保存は行わないでください。
完了時は「AIが完了した作業」「証跡path」「blocker」「ユーザーが次に行う作業」
「G0合否」を報告し、M1開始前に停止してください。
```

M1完了後、新PC側のAIへは次のように引き継ぐ。

```text
この端末は新PCです。
同計画のM2だけを実行してください。
全source PCのM1成果物は <MIGRATION_ROOT> にあります。
最初に ledger.md と、各SOURCE_HOSTのmanifests／reports/restore-test.mdを確認し、
全source PCのG1合格を確認してください。未監査端末があればM2を開始しないでください。
Syncthingのpairing、canonical assetへの書き込み、WIPの通常worktreeへの重畳はまだ行わず、
対話認証または管理者承認が必要な時だけユーザーへ依頼してください。
```

### 0.5 AIの完了報告形式

AIは各マイルストーンの終了時に、技術ログを並べるだけでなく次の形式で返す。
AIが実行できるterminal commandをユーザーへ手作業として転嫁しない。

1. 現在のマイルストーンとゲート。
2. AIが完了した作業。
3. 証跡path、checksum、検証結果。
4. 未解決blockerと、変更してはいけない対象。
5. ユーザーが次に行う作業。最大3項目に絞る。
6. 次のマイルストーンへ安全に進めるか: `YES`／`NO`。

## 1. 目的

- 解決したい課題: PC買い替えに伴う開発状態を保全して新PCへ移し、Blender／asset制作は旧環境を移送せずこのPCで再現可能な新規環境として開始する。
- 到達したい状態:
  - 新PCだけでコード開発、ゲーム実行、Soul GLBのBlender編集・書き出し、外部アセット同期を完結できる。
  - 旧開発PCのM1 backupは開発WIPのロールバック元として保持し、旧Blender PCには依存しない。
  - Git、現PCで新規作成する外部アセット原本、実行用アセット、ローカルユーザーデータの正本と復元経路が明確である。
  - Blenderの正確なバージョン、配布経路、アドオン、書き出し条件が記録され、別PCでも再現できる。
- 成功指標:
  - byte-for-byteで移すsnapshotについて、旧PC側と新PC側のファイル一覧・サイズ・SHA-256照合が完了する。
    新規構築するtool／認証／設定はversionと機能受入で判定する。
  - 新PCで `doctor`、rust-analyzer診断、`check`、`verify` が成功する。
  - `cargo run --locked -p visual_test` と `cargo run --locked` で、
    GLB、runtime用フォント、テクスチャ、GPU描画を確認でき、audio backend初期化errorがない。
    現状はruntime音声asset／参照がないため、音声再生自体は `N/A` とする。
  - 現PCで新規作成したcanonical `.blend` からGLBを書き出し、
    staging、`exports/`、`assets/`、Bevy読込まで一周できる。
  - Syncthingを移行条件にせず、現PCが唯一のBlender／アセット編集端末になる。
  - 最初のcanonical promote後、独立バックアップから `.blend` とmanifestの復元テストが成功する。

## 2. スコープ

### 対象（In Scope）

- 新PCのOS、CPUアーキテクチャ、GPU、保存容量の確定。
- Gitのbranch、commit、stash、未コミット差分、未追跡ファイルの保全。
- Rust、Bevy向けOS依存パッケージ、Python、Git、`rg`、IDE、MCPツール、
  Linux x86_64選択時の `mold` の再構築。
- GitHub認証、必要時のGCP/TRELLIS接続、SSH、ローカルAIツールの再認証。
- OS固有の `<ASSET_ROOT>/source/`、`staging/`、`exports/`、`manifests/`、`licenses/` の新規初期化。
- Git管理外の `assets/`、`saves/`、`settings/` と、必要な性能artifactの移行。
- Blender本体、プロジェクト用設定、アドオン、asset library、外部参照、glTF書き出し条件、Blender MCPの新規構築。
- 新PCでのコード品質ゲート、実ゲーム、visual test、save/load、Blender往復の受入。
- 実測後の移行先OS用setup文書、`docs/assets_workflow.md`、Blenderセットアップ文書の恒久化。
- 最低14日または通常開発2サイクルのうち長い方の安定化期間と、旧PCの安全な廃止。

### 非対象（Out of Scope）

- 移行と同時のBevy、Rust、Blenderの機能アップグレード。
- 旧Blender PCの棚卸し、asset／設定／addon／preset移送、access失効、初期化。
- 移行目的のSyncthing pairing、旧folder ID再利用、writer role切替。複数端末共有が必要になった場合は別計画で導入する。
- 建築GLB量産やHeadless Blender品質ゲートそのものの実装。
  - これは既存の `docs/plans/3d-rtt/asset-milestones-2026-03-17.md` にある
    `MS-Asset-Pipeline` の責務とする。
- TRELLIS/GCPアーキテクチャの再設計。
- `target/`、`dist/`、`.trunk/`、Cargo registry、ログ、一般キャッシュのコピー。
- 新旧PCの性能値を同一baselineとして比較すること。
- 移行完了前の旧PC初期化、secure erase、売却、譲渡。

## 3. 現状とギャップ

### 3.1 2026-07-29監査時点

- Git:
  - `master` の `HEAD` は `origin/master` と一致する。
  - 作業ツリーには `modified/other 79`、`deleted 3`、`untracked 3`、合計85パスの未コミット状態がある。
  - cloneや通常のpullだけでは、この監査時点の85パスは新PCへ移らない。
    実行時は件数を固定せず、M0で再取得したworktree manifestを正とする。
- 開発環境:
  - リポジトリはRust `1.96.1`、`rustfmt`、`clippy`を `rust-toolchain.toml` で固定する。
  - Bevyは0.19、Rust editionは2024である。
  - Linux x86_64では `.cargo/config.toml` が `mold` を指定する。
  - 現在調査できたホストはFedora 44 x86_64で、`python3 scripts/dev.py doctor` は成功した。
  - 新PCのOS／CPU／GPUは本計画M0で確定する。Fedora 44は参照環境であり、移行先OSを強制しない。
- 実行用アセット:
  - `assets/` は177ファイル、130,691,241 bytes（約126 MiB）。
  - Git追跡はWGSL 15ファイルだけで、GLB、画像、フォント等162ファイルはignore対象である。
  - 監査時点では、コードから参照するruntime asset pathは現ローカル `assets/` に存在する。
  - `assets/models/characters/soul.glb` はゲームとvisual testの必須入力である。
- 外部アセット:
  - 文書上の正本は `~/Sync/hell-workers-assets/source/` と `exports/` だが、現在調査できたホストでは実データがほぼない。
  - 現行Syncthing設定はLinux上でWindows形式のfolder pathを参照しており、正規Linuxパスと分裂している。
  - `2026-08-01`のユーザー判断により旧`.blend`は回収しない。`source/`／`exports/`は空から開始し、現PCで承認した最初の成果物だけを正本化する。
  - `scripts/sync_external_assets.py` は `textures`、`models`、`audio` だけを同期する。
  - Git管理外の `assets/fonts/` には、現在の同期スクリプトだけでは復元経路がない。
    13ファイル中、runtimeが直接読む4フォントは有効だが、未使用候補にはplaceholderやHTML内容の
    `.ttf` もあり、全件を一括で正本化できない。
  - Web用のroot `favicon.ico` はGit追跡済みだが0 byteであり、同一hash復元を合格にできない。
- Blender:
  - 現在調査できたホストにはBlender本体とユーザー設定がない。
  - リポジトリは `.mcp.json` で `http://localhost:9876/sse` のBlender MCPを期待する。
  - プロジェクトとしてのBlender exact version、アドオン一覧、export presetは未固定である。
  - 現行Soul GLBには8 clip
    (`Carry / Exhausted / Fear / Idle / Walk / WalkLeft / WalkRight / Work`) があり、
    mesh／action名の変化はコンパイル成功のまま表示を壊し得る。
- ローカルユーザーデータ:
  - `saves/world.scn.ron` と `settings/settings.ron` はGit管理外である。
  - `target/` は監査時点で約95 GiBあり、移送しない。
  - `target/perf-runs/` は約12 MiBで、履歴が必要な場合だけ選別保存する。
- 文書と診断のギャップ:
  - `docs/linux-setup.md` はUbuntu/Debian中心で、移行先OSの完全な手順ではない。
  - `doctor` はBlender、Syncthing、GPU/Vulkan、gltf-validator、MCP接続を検査しない。
  - `doctor` のasset確認は総ファイル数だけなので、Git追跡済みWGSLしかないfresh cloneでも
    runtime asset欠落を検出できない。
  - 現ホストのMCP binaryは再現可能なinstall sourceが記録されておらず、
    repo adapterは `rust-analyzer-mcp v0.2.0` 互換を前提とする。

### 3.2 データごとの正本と移行方法

| 区分 | 移行前の正本 | 移行方法 | 合否確認 |
| --- | --- | --- | --- |
| Git追跡済みコード／文書／WGSL | GitHub remote | clean clone | branch、commit SHA、`git status` |
| 未コミット／未追跡作業 | 現在作業中のPC | scoped commit/push、または暗号化bundle／patch／archive | 復元先のdiffとファイル一覧 |
| Blender原本／参照素材 | 現PCの新規`<ASSET_ROOT>/source/` | staging検証後の明示promote + 独立backup | provenance、SHA-256、missing fileなし |
| `exports/` | 現PCで検証済みのstaging出力 | 明示promote | manifest、validator、目視一致 |
| repo内 `assets/` | 現ローカルの既知実行コピー | 安全用snapshot後、原則 `exports/` から再生成 | runtime path、visual test、ゲーム |
| `saves/`／`settings/` | 現在利用中のPC | 個別archive | F9 load、F5 save、設定再起動 |
| Blender設定／アドオン | 現PCの固定baseline | project toolingから新規構築 | version、addon、MCP、export往復 |
| 認証／秘密情報 | 各サービス | 新PCを新端末として再認証 | read／push権限。値は台帳に書かない |
| build cache／ログ | なし | 移行しない | 新PCで再生成 |
| 必要なperf artifact | 旧PC | 選別archive | metadata付きで参照専用 |

## 4. 移行方針（高レベル）

- **保全してから変換する**:
  - バージョン更新、パス修正、ファイル整理、Blender再保存より先に、旧PCの状態を読み取り専用snapshotにする。
- **移行とアップグレードを分離する**:
  - Blenderは旧PCと同じexact versionを最初の基準にする。
  - Blenderやアドオンのupgradeは、移行受入後の別作業とする。
- **Syncthingをbackup扱いしない**:
  - 同期とは独立した暗号化backupを最低1つ持ち、実際に1ファイル復元する。
  - 新PCは新しいSyncthing device identityを使い、旧PCのkey／configを複製しない。
- **最初は一方向・削除なし**:
  - 新PCのasset folderは受信専用で開始する。
  - manifest一致前に `--delete-missing` を使わない。
- **Blender／アセットはsingle writer**:
  - 切替宣言までは旧PC、切替後は新PCだけを編集端末にする。
  - 同じ `.blend` を両PCで同時編集しない。
- **秘密情報は再認証する**:
  - token、Syncthing identity、credential DBをホームディレクトリごとコピーしない。
  - API key等はpassword managerから個別登録し、移行台帳には値を書かない。
- **fresh clone再現を最終基準にする**:
  - 現worktreeの単純コピーが動くことではなく、Git clone + 外部asset復元で動くことを確認する。
- Bevy API変更は行わない。実装が必要になった場合は0.19の一次情報で確認し、
  rust-analyzer診断と `cargo check --workspace` を実施する。

## 5. マイルストーン

## M0: 移行台帳・対象PC・停止条件の確定

- 変更内容:
  - WIP／repository、Blender原本、runtime asset、save／settingsを持つ全source PC、
    新PC、現在のrepoホスト、Blender編集PCの役割を明記する。
    M0／M1はsource PCごとに実施し、端末別成果物を同じ `<MIGRATION_ROOT>` へ統合する。
  - 新PCのOS、CPUアーキテクチャ、GPU、driver、Vulkan backend、RAM、保存容量、
    ホスト名を記録する。
  - OS固有のasset rootを `<ASSET_ROOT>` として台帳に定義する。
    Linuxの推奨値は `~/Sync/hell-workers-assets` である。
  - 新PCの必要空き容量を決める。少なくとも現行約95 GiBの `target/` を再生成でき、
    asset snapshotを2世代置ける容量を確保し、M2開始前に実測する。
  - 旧PCの最終処遇を「継続保管／再利用／売却／譲渡／廃棄」から選び、
    secure eraseが必要かを決める。
  - 旧PCのGit、Syncthing、Blender、IDE、MCP、GCP/TRELLISの状態を秘密値なしで台帳化する。
  - editor／agent環境について、extension、keymap、shell設定、Codex／Claude等のversion、
    personal skill／memory、MCP serverのversionと再インストール元を台帳化する。
    auth、session、log、API keyは移行対象の非秘密設定と分離する。
  - `rust-analyzer` 本体、`rust-analyzer-mcp`、`docsrs-mcp`、使用するglTF validatorについて、
    exact version、source／patch、install command、配布物またはsource revisionのchecksumを決める。
  - Blenderについて次を取得する:
    - `blender --version` の完全な出力。
    - インストール元とinstaller/archiveのSHA-256。
    - enabled addon／extension一覧と各version。
    - `startup.blend`、`userpref.blend`、keymap、asset library。
    - render device、color management、unit、axis、glTF export preset。
    - 外部texture、font、ICC／OCIO、script、Python依存。
  - ユーザーが入力票で許可した旧PCの検索rootだけから、`.blend`、`.blend1`、
    参照画像、外部texture、未分類GLBを検索し、各ファイルを
    `source/`、`exports/`、参照資料、不要候補へ分類する。
    個人folderや他projectを含む許可外rootへ検索を広げない。
  - Git外データを「移す／新規構築／移さない／判断待ち」に分類する。
  - 「通常開発1サイクル」を、編集 → `check` → 必要な手動確認 → `verify`
    → scoped commit／pushまで完了する一連の作業と定義する。
- 変更ファイル:
  - 移行中の秘密値を含まない作業台帳（repo外）。
  - 完了時に `docs/blender-setup.md` へ恒久情報だけを反映する。
- 完了条件:
  - [ ] 全source PC、新PC、Blender編集PCの役割と保持データが確定している。
  - [ ] 新PCのOS／CPUアーキテクチャ／GPUが確定している。
  - [ ] Blender exact version、配布経路、アドオン、export presetが採取できている。
  - [ ] 現行Soul GLBに対応する `.blend` 原本の所在が確認できている。
    見つからない場合は、ユーザーが喪失記録を承認し、現行GLBから新しいcanonical原本を
    再構築するfallbackを選択済みである。
  - [ ] MCP／glTF validatorのexact toolchainと再現可能な導入元が確定している。
  - [ ] `<ASSET_ROOT>`、必要空き容量、旧PCの最終処遇が確定している。
  - [ ] 全移行対象にownerと保全方法が割り当てられている。
- 停止条件:
  - `.blend` 原本またはユーザー承認済み再構築fallback、未コミット作業、
    Git管理外runtime assetの保全先が不明ならM1以降へ進まない。
  - 未監査のsource PCが残っている場合、その端末のM0／M1を終えるまでM2へ進まない。

## M1: 旧PCの凍結・二重バックアップ・復元テスト

- 変更内容:
  - 本計画の「freeze」は、project／Blender原本の保存、Syncthing再開、
    package／Blender upgradeを停止することを指す。
    読み取り専用監査と、検証済みbackup先へのsnapshot作成は許可する。
  - Git作業を安全化する。
    - 第一選択は、進行中タスクのscopeでcommit／pushまで完了すること。
    - 完了できない場合は、Git bundle、binary diff、staged diff、未追跡ファイルarchiveを
      暗号化して保存する。
    - Git bundleだけではstaged／unstaged／untracked状態を保存できない。
      全refs、HEADに対するtracked diff、indexのdiff、stash、untracked manifestを別々に保全する。
    - ignoredな `assets/`、`saves/`、`settings/` はGit backupと別に扱う。
  - 次のpoint-in-time snapshotを作成する:
    - 各source PCが持つ `<ASSET_ROOT>/`。
    - 各source PCで見つかった全Blender原本と外部参照。
    - 各source PCのrepo内 `assets/`。
    - 各source PCにある必要な `saves/`、`settings/`、`target/perf-runs/`。
  - 各snapshotの相対path、file size、SHA-256 manifestを作る。
  - snapshotを旧PC以外の独立した暗号化媒体へ複製する。
  - backupから一時ディレクトリへ `.blend` 1件とGit外データ1件を実際に復元する。
    原本喪失fallbackを選んだ場合、この段階では現行Soul GLBと喪失記録を復元対象にする。
  - 凍結日時、Git commit、asset manifest ID、Blender versionを記録する。
- 変更ファイル:
  - repo外のbackupとmanifest。
  - この段階ではrepoの原本やruntime assetを整理・削除しない。
- 完了条件:
  - [ ] 全source PCについて端末別のM1成果物とG1判定がある。
  - [ ] M0で再取得したworktree manifestの全パスがcommit/push済み、
    backup済み、または明示的に不要判定済み。
  - [ ] 全local branch／tag／ref、stash、staged／unstaged差分、untracked manifestを保全済み。
  - [ ] Git、Blender原本またはfallback入力／喪失記録、external assets、
    repo内assets、save/settingsに独立backupがある。
  - [ ] byte-for-byte snapshotのSHA-256照合が成功している。
  - [ ] backupからの復元テストが成功している。
  - [ ] 旧PCは凍結され、以後の編集が停止している。
- 検証:
  - `git status --short --branch`
  - `git branch -vv`
  - `git show-ref`
  - `git stash list`
  - `git bundle verify <BUNDLE_PATH>`（bundleを使用した場合）
  - `git fsck`
  - SHA-256 manifestの検証

## M2: 新PCの非秘密ベース開発環境

- 変更内容:
  - 全source PCのG1合格を `<MIGRATION_ROOT>/ledger.md` と端末別manifestで確認する。
    未監査端末または未完了G1があれば、新PC構築を始めずM0／M1へ戻る。
  - M0で確定したOSに合わせて、OS update、GPU driver、Vulkan、window backend、
    audio、compiler、`pkg-config`、Git、Python 3.11以上、`rg`を導入する。
  - Linux x86_64では、`.cargo/config.toml` の指定に合わせて `mold` も導入する。
    WindowsまたはARMを選ぶ場合は同設定がそのまま適用されないため、M2開始前に
    platform固有のlinker／build tool手順と受入コマンドを `docs/linux-setup.md`
    または対応するsetup文書へ追加する。
  - Rustはrustupから導入し、workspaceの `rust-toolchain.toml` に
    `1.96.1 / rustfmt / clippy` の選択を任せる。
  - GitHubは `gh auth login` / `gh auth setup-git`、または新PC専用SSH keyで再認証する。
    認証方式ごとにreadとpush権限を非破壊で確認する。
  - repositoryをfresh cloneし、remote、branch、commit SHAを照合する。
  - M1でWIPをarchiveした場合は、clean baselineの検証後に隔離branch／worktreeへ復元する。
    staged／unstaged／untracked／stash／refsをM1 manifestと照合し、
    clean cloneへ無検証で重ねない。
  - `target/`、Cargo registry、旧GPUのshader cacheはコピーせず、新PCで再生成する。
  - 必要な開発補助を再構築する:
    - `bacon`
    - `cargo-expand`
    - `docsrs-mcp`
    - `rust-analyzer-mcp`
    - `rust-analyzer` language server
    - M0で確定したglTF validator
    - `gh`
    - Pillow
    - GPU／Vulkan診断tool
    - WASM作業を行う場合だけ `trunk`
  - Codex／Claude等は新PCへ再インストールし、repo外のpersonal skill／memoryは
    内容を確認した非秘密データだけを暗号化backupから復元する。auth／session DBはコピーせず再認証する。
  - `docsrs-mcp` と `rust-analyzer-mcp` はversionだけでなく、
    再現可能なinstall source／commandを記録してから導入する。一時build directory由来のbinaryを
    そのままコピーして完了扱いにしない。
  - `rust-analyzer --version` とIDE workspace診断に加え、
    rust-analyzer MCPでworkspace symbol、docsrs MCPでBevy 0.19 itemを実際に問い合わせる。
  - `.git/hooks` は旧PCからコピーしない。hookを使う場合はportableな導入方法へ修正してtestし、
    使わない場合は不採用を明記してhook記述を恒久docsと整合させる。
  - activeなsetup文書／script／shell設定の旧username、旧drive letter、旧SDK絶対pathを監査し、
    environment変数または相対pathへ置き換える。archive文書の歴史的記録は対象外とする。
  - global Git設定の `user.name`、`user.email`、署名鍵、`credential.helper`、
    `core.autocrlf`、`core.filemode`、`core.hooksPath` を秘密値なしで監査する。
    平文credential helperは使用しない。
  - GCP/TRELLISを現に使う場合だけ新PCで再認証し、使わない場合は `N/A` を記録する。
- 変更ファイル:
  - 移行先OS用setup文書。
  - portable hookに修正が必要なら別scopeとして実装する。
- 完了条件:
  - [x] clone先のcommit SHAが移行台帳と一致する。
  - [x] local／remote refsと対象branchのdivergenceが移行台帳と一致する。
  - [x] archiveしたWIPを隔離repositoryへ復元し、status／stash／HEADをM1 manifestと照合できている。
  - [x] 合意した空き容量を満たし、GPU／Vulkan adapterを記録できている。
  - [x] `python3 scripts/dev.py doctor` が成功する。
  - [x] active toolchain、rustfmt、Clippy、rust-analyzer、glTF validatorのversionが台帳と一致する。
  - [x] rust-analyzerがworkspaceを読め、error診断がない。
  - [x] rust-analyzer MCPとdocsrs MCPの実queryが成功する。
  - [x] GitHubからreadでき、対象repositoryへのpush権限を非破壊で確認できる。
  - [x] hookがportableに導入・test済み、または不採用としてdocsと整合している。
  - [x] secretがremote URL、`.env`、平文credential helper、repo差分に混入していない。
- 検証:
  - OS固有のdisk free確認
  - `python3 scripts/dev.py doctor`
  - `rustup show active-toolchain`
  - `rustc --version`
  - `cargo --version`
  - `rustfmt --version`
  - `cargo clippy --version`
  - `rust-analyzer --version`
  - M0で確定したglTF validatorの `--version`
  - rust-analyzer MCPのworkspace symbol query
  - docsrs MCPのBevy 0.19 item query
  - gh方式なら `gh auth status`、SSH方式なら `ssh -T git@github.com`
  - `git ls-remote origin HEAD`
  - `git rev-list --left-right --count HEAD...origin/<branch>`
  - GitHub APIのrepository permissionまたは安全な `git push --dry-run` によるpush権限確認
  - `python3 scripts/dev.py check`

## M3: 新規asset workspaceの基準化

- 変更内容:
  - 旧Blender PC、旧Syncthing folder、旧`.blend`、旧`exports/`からデータを受信しない。
  - `~/Sync/hell-workers-assets/`を現PCだけが書く新規canonical rootとする。
    `Sync`というpath名は移行中の同期実施を意味せず、Syncthingは本計画の必須要素にしない。
  - `source/`、`staging/`、`exports/`、`manifests/`、`licenses/`を
    `init-asset-workspace`で冪等初期化する。
  - 初期状態の`source/`と`exports/`は空を正とする。自動化はstagingだけへ書き、
    人の明示承認前にcanonicalへpromoteしない。
  - repo内`assets/`は現在動くruntime referenceとして保持し、新しいBlender原本の正本扱いや
    逆変換元にはしない。`--delete-missing`は最初のcanonical export受入まで禁止する。
  - deterministic smokeで`.blend`作成、scene gate、GLB export、Khronos validatorを一周し、
    hashとreportをbaseline manifestへ記録する。
  - 最初のcanonical assetを作る前に、provenance／license／review templateと独立backup先を固定する。
- 変更ファイル:
  - `docs/assets_workflow.md`
  - `docs/blender-setup.md`
  - repo外`<ASSET_ROOT>/manifests/workspace-baseline-2026-08-01.md`
- 完了条件:
  - [x] 新規workspaceの必須directoryとtemplateが存在する。
  - [x] `source/`／`exports/`が空で、旧PC由来のcanonical dataが混入していない。
  - [x] staging smokeのscene検査がerror 0／warning 0である。
  - [x] staging GLBのKhronos検査がerror 0／warning 0で、hashが再現する。
  - [x] Syncthing pairing／writer切替が受け入れ条件から除外されている。
  - [x] 独立backup先と復元手順が記録され、baseline snapshotの実restore testが成功している。
- 検証:
  - `tools/blender_ai_workflow/bin/init-asset-workspace`
  - `tools/blender_ai_workflow/bin/workflow-smoke`
  - `find <ASSET_ROOT>/source <ASSET_ROOT>/exports -type f`
  - staging reportとGLB SHA-256確認
  - `<MIGRATION_ROOT>/snapshots/fedora-new-asset-workspace-20260801/files.sha256`のrestore検証
- Help影響:
  - workspace／文書だけの変更はplayer-visible契約を変えない。runtime dataをpromoteするM4/M5で
    `hell-workers-review-help-impact` Skillから改めて判断する。

## M4: Blender新規canonical asset制作・受入

- 変更内容:
  - 現PCのBlender `5.1.1` Flatpak exact commitを最初のcanonical authoring baselineとし、
    旧PCとのversion一致を要求しない。major/minor upgradeは最初のasset受入後の別作業にする。
  - addon／extensionはproject toolingから新規構築し、旧設定やPython環境をコピーしない。
  - unit、axis、color management、render device、glTF export条件、外部texture pathを
    新規asset用manifestへ記録する。
  - 最初のcanonical `.blend` はstagingで新規作成する。既存`assets/models/characters/soul.glb`を
    import／逆変換して正本候補にせず、runtime比較用referenceとしてだけ扱う。
  - 外部pathは原則 `source/` 内へpromote可能な相対pathにし、canonicalへは目視承認後にだけ昇格する。
  - repo内の現行Soul GLBを直接上書きせず、stagingへexportする。
  - staging GLBについて次を確認する:
    - M0で固定したvalidatorでerror 0となり、warningを全件レビュー済みである。
    - asset固有のnode／mesh／rig／animation契約がmanifestと一致する。
      Soulを新規制作する場合はscene index `0`、face mesh `Soul_Face_Mesh`、
      body mesh `Soul_Mesh.010`、rig `Soul_Rig`、8 animation clip名をruntime契約として検証する。
    - embedded imageが存在し、external textureも欠落しない。
    - 原点、単位、scale、向き、material、face atlas UVが新規asset manifestの契約と一致する。
    - triangle数は用途ごとのbudgetに収まり、既存GLBとの一致は要求しない。
    - バイナリSHAの一致だけを要求せず、構造manifestと目視を合否基準にする。
  - staging GLBの目視は、隔離clone／一時asset treeの対象runtime pathへ配置してから実行する。
    canonical assetを指したまま既知良好な旧GLBを誤って確認しない。
  - hardened Blender MCP addon／serverを起動し、project-scoped stdio serverから
    localhost bridgeへread-only scene queryできることを確認する。
  - exact version、導入元、commit、addon、export条件を `docs/blender-setup.md` に恒久化する。
- 変更ファイル:
  - `docs/blender-setup.md`（新規）。
  - `docs/assets_workflow.md`
  - repo外の新規Blender原本、reference、manifest。
- 完了条件:
  - [x] 現PCの基準Blender exact version／配布commitが固定されている。
  - [ ] 新規canonical候補をmissing resourceなしで開ける。
  - [ ] staging GLBがvalidator error 0、warningレビュー済みで、構造検証に成功する。
  - [ ] staging GLBそのものを隔離cloneで目視確認できている。
  - [x] Blender MCPのread-only接続が成功する。
  - [ ] exact version、addon、export条件、path契約が文書化されている。
- 検証:
  - `blender --version`
  - M0で固定したglTF validatorと構造manifest
  - 隔離cloneで `cargo run --locked -p visual_test`
  - Blender MCPのread-only scene query

## M5: End-to-End受入と主端末切替

- 変更内容:
  - clean clone + 新規external asset workspaceだけで開発環境を再構築する。
  - コード品質ゲートを順に実行する。
  - visual testで次を目視する:
    - `Q` で8 animation clip。
    - `1`〜`6` で6 face atlas state。
    - `V` で全Elevation View。
    - 複数Soul、影、mask、RtT、face material。
    - Build modeの主要2D asset。
  - ゲーム本体で次を確認する:
    - AssetServerのmissing/error logがない。
    - 日本語UI、Familiar用font、Soul名、絵文字が表示される。
    - Soulの代表的なIdle／Walk／Workが表示される。8 clip全件はvisual testを正とする。
    - Familiar、terrain、建物、UI iconが読め、audio backend初期化errorがない。
      runtime音声assetは現状 `N/A` とする。
    - 独立backupから作った検証用saveコピーをF9で読み、F5で新規保存できる。
      唯一の移行元saveを上書きしない。
    - settings変更が再起動後に維持される。
  - 新規canonical候補をstaging保存し、staging export →
    disposable cloneの `assets/` → visual test／ゲームの一周を先に行う。
  - staging受入成功後、canonical候補とmanifestを独立backupへ保存して実restore testする。
  - restore test後、検証済みGLBを現PCのcanonical `exports/` へ、
    検証済み `.blend` と外部参照をcanonical `source/` へpromoteし、
    source／export両manifestを更新する。その後dry-run → repo `assets/` →
    visual test／ゲームまで再確認する。
  - GitHub、必要時GCP/TRELLIS、IDE、MCPを新PCだけで利用できることを確認する。
  - 新PCのGPU／backendで性能を測る場合は、新しいbaselineとしてmetadata付きで保存する。
- 変更ファイル:
  - 受入記録。
  - 実測結果に基づく関連docs。
- 完了条件:
  - [ ] `doctor`、rust-analyzer、`check`、`verify` が成功する。
  - [ ] visual testとゲーム本体の手動受入が成功する。
  - [ ] F5/F9とsettings再起動確認が成功する。
  - [ ] BlenderからBevyまでの一周が成功する。
  - [ ] 独立backupのrestore test後も、promoteしたGLBの一周が成功する。
  - [ ] GitHubのread／pushと必須外部接続が新PCだけで成立する。
  - [x] 新PCを唯一の開発／Blender編集端末として新規開始する判断記録がある。
- 検証:
  - `python3 scripts/dev.py doctor`
  - `python3 scripts/dev.py check`
  - `cargo run --locked -p visual_test`
  - `cargo run --locked`
  - `python3 scripts/dev.py verify`
  - `Cargo.lock` と予期しないtracked差分が増えていないこと
- 注意:
  - 監査時点のworktreeには別作業の大きな変更がある。
    `verify` は、その作業を安全に完了した状態または同じcommitのclean cloneで実行する。

## M6: 恒久化・安定化期間・旧PC廃止

- 変更内容:
  - 実測した移行先OS手順、Blender新規構築手順、独立backup／restore手順を恒久docsへ反映する。
  - 新設した `docs/blender-setup.md` を `docs/README.md` へ登録する。
  - 旧開発PCはGit/WIP rollback sourceとして電源OFFで保持し、
    最低14日または通常開発2サイクルのうち長い方を観察する。
    対象外とした旧Blender PCの保持／access／初期化は本計画のgateに含めない。
  - 観察期間中に、コード変更1件とBlender export 1件を新PCだけで完了する。
  - 独立backupからSoul `.blend`、Git WIP、runtime asset、save/settingsを最終復元テストする。
  - 不足がなければ、旧開発PCのGitHub／GCP等の開発accessを必要に応じて失効する。
  - M0で消去対象と決めた場合だけ、最終復元とaccess失効の完了後、実行直前に
    ユーザーの明示承認を得て旧PCをsecure erase／初期化する。
    消去対象でない場合は、電源OFF保持／転用など確定した最終処遇を記録する。
  - 上記の最終処遇まで完了してから恒久情報を通常docsへ移し、本計画をarchiveまたは削除する。
  - plan移動後に `python3 scripts/dev.py docs --write` と `docs --check` を実行する。
- 変更ファイル:
  - 移行先OS用setup文書
  - `docs/assets_workflow.md`
  - `docs/blender-setup.md`
  - `docs/README.md`
  - `docs/plans/README.md`
- 完了条件:
  - [ ] 新PCだけで通常開発2サイクルとBlender作業1サイクルが完了している。
  - [ ] 独立backupからの最終復元テストが成功している。
  - [ ] 必要な旧開発PCの外部accessが失効している。
  - [ ] M0で決めた旧PCの最終処遇（承認済みsecure eraseまたは保持／転用）が完了している。
  - [ ] active docs／scriptsから旧PC固有の絶対pathが除去され、hook方針も文書と整合している。
  - [ ] `docs/blender-setup.md` が `docs/README.md` に登録されている。
  - [ ] 恒久docsと索引が更新され、本計画がarchiveまたは削除されている。
  - [ ] M0で消去対象とした場合は、最終復元、access失効、実行直前の明示承認を経て
    planのarchive／削除より先にsecure eraseが完了している。
- 検証:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `python3 scripts/dev.py verify`

## 6. ゲートと停止条件

| ゲート | 進める条件 | 不合格時 |
| --- | --- | --- |
| G0 棚卸し | 旧開発PCと新PC、Blender／MCP／validator version、移すデータownerが判明 | 旧開発PCを保持し、検索と台帳化を継続 |
| G1 保全 | 旧開発PCのGit全状態、runtime assets、save/settingsのbackupと復元成功 | M2を開始しない |
| G2 開発基盤 | 保存容量、GPU、toolchain、MCP実query、doctor、rust-analyzer、check、Git read/push成功 | OS依存、toolchain、driver、認証を修正 |
| G3 asset新規基盤 | 空のcanonical領域、staging smoke、manifest／license template、旧資産混入0 | canonical promoteを開始しない |
| G4 Blender | 新規原本missing 0、staging GLB validator／構造／目視／MCP成功 | canonicalへpromoteしない |
| G5 主端末受入 | verify、visual test、game、save/load、export往復、backup restore成功 | staging候補を維持しruntimeへ反映しない |
| G6 安定化・最終処遇 | 安定化期間と最終restore成功、必要な旧開発access／処遇完了 | 旧開発PCを消去せず、planも完了扱いにしない |

## 7. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| M0のworktree manifestがcloneで消える | 実装・文書の喪失 | M1で全Git状態を保全し、M2の隔離branch／worktreeで復元照合 |
| 空の`source/`／`exports/`を移行失敗と誤認する | 旧資産を不要に回収／逆変換する | 空を承認済みbaselineとしてmanifest化し、最初の新規promoteまで維持 |
| `--delete-missing` の早期実行 | repo内runtime asset削除 | 最初のcanonical export受入まで禁止し、常にdry-runを先行 |
| 既存runtime GLBをBlender正本へ逆変換する | 編集情報の欠落した偽正本が固定される | 既存GLBはreference限定とし、新規`.blend`をstagingから作る |
| Git管理外assetsがfresh cloneにない | 起動時のmissing asset | assets snapshot、external exports、manifest、実ゲーム受入 |
| runtime fontが同期対象外／未使用fontが壊れている | UI文字欠落、誤った正本化 | runtime 4件だけをparser／license検証し、未使用候補は別分類 |
| root faviconが0 byte | Web build／ブラウザ表示不備 | Web使用時は有効化または参照除去。未使用なら `N/A` とfollow-up |
| Blender version／addon差 | `.blend`破損、export構造変化 | まずexact version一致、コピーで開く、upgradeは別作業 |
| 絶対path／外部texture | 新PCでmissing file | relative path化、path監査、移行用コピーで確認 |
| mesh／action名の変化 | Soul表示・animationの静かな退行 | 構造manifest、8 clip名検査、visual test |
| 新GPU／driver差 | Bevy／Blender表示差、性能差 | Vulkan／backend記録、目視受入、新baselineとして計測 |
| save schemaとcode revision不一致 | F9失敗、状態消失 | saveに対応commitを記録し、copyでF9/F5受入 |
| secretの平文コピー | credential漏洩 | 新端末として再認証、password manager、repo secret check |
| exporter名だけで利用許諾を推測する | 配布／商用利用リスク | TRELLIS fork／checkpoint、Mixamo、入力画像、fontのprovenanceとlicenseを別台帳化 |
| Syncthingだけをbackupと誤認 | 同期削除が全端末へ伝播 | 独立した暗号化backupと復元テスト |
| 旧新PCの同時編集 | `.blend` conflict、成果物逆戻り | single-writer宣言、cutover後の旧PCはpause／電源OFF |
| 旧PCを早期消去 | ロールバック不能 | G6までsecure erase禁止 |

## 8. 検証計画

- 必須の自動確認:
  - `python3 scripts/dev.py doctor`
  - `rustup show active-toolchain`、rustfmt、Clippy、rust-analyzerのversion
  - rust-analyzer workspace診断
  - rust-analyzer MCP／docsrs MCPの実query
  - `python3 scripts/dev.py check`
  - WIP／refs、offline snapshot、source、exports、空destination runtime treeのmanifest検証
  - asset catalog参照pathとruntime用4フォントの形式検証
  - M0で固定したvalidatorによるstaging GLB検証
  - `python3 scripts/dev.py verify`
- 必須の手動確認:
  - `cargo run --locked -p visual_test`
  - `cargo run --locked`
  - visual testの8 clip、6表情、全Elevation View、影、mask、RtT。
  - 日本語UI、Familiar font、Soul名、絵文字、主要terrain／building／icon。
  - 検証用saveコピーでF9 load、F5 save、settings再起動。
  - 新規canonical候補のcopy-open、missing file 0、staging export、asset反映、Bevy読込。
  - audio backend初期化errorがないこと。runtime音声再生は現状 `N/A`。
  - Blender MCP read-only query。
- セキュリティ確認:
  - `git remote -v`
  - 選択したGitHub認証方式のread／push権限
  - global Git credential／signing／line-ending／hook設定
  - `python3 scripts/check_repo_hygiene.py`
  - `.env`、token、private key、Syncthing keyがrepo差分にないこと。
- パフォーマンス確認:
  - 必須ではない。
  - 採取する場合は新PCのGPU、backend、driver、window backendを記録し、新しいbaselineにする。
- 監査時点の参考値:
  - `assets/`: 177 files / 130,691,241 bytes。
  - Git追跡asset: WGSL 15 files。
  - `assets/models/characters/soul.glb`:
    `SHA-256 2eef9871c626a45686402cfe2885d8e16f1ef0fc6bcbd104c3a29cc4b66bcec9`。
  - これらは移行実行直前に再採取し、最新manifestを正とする。
- Help影響:
  - 計画実行で機能、code、runtime dataを変更した場合は、完了報告前に
    `hell-workers-review-help-impact` Skillで実経路から
    `Update required` / `No impact` を判断する。docsだけの変更では要求しない。

## 9. ロールバック方針

- M0〜M4:
  - 現PCのstagingだけを書き込み先とし、明示承認まで`source/`／`exports/`／repo `assets/`を上書きしない。
  - 旧Blender PCへ戻る経路は持たず、既知良好なrepo runtime assetを比較用に保持する。
- ロールバック条件:
  - checksum不一致、missing `.blend` dependency、provenance／license不足。
  - staging GLBがBevyで読めない、clip／mesh／見た目が退行する。
  - `check`／`verify`が移行環境固有の理由で失敗する。
  - GitHubまたは必須外部接続を復元できない。
- 手順:
  1. canonical promoteとrepo asset同期を停止する。
  2. 現PCで生じたstaging変更を隔離し、承認済みsource／exportsへ混ぜない。
  3. cutover manifestと独立backupを照合し、必要なら最後の承認済みcanonical snapshotを復元する。
  4. repo runtime assetは既知良好なcopyへ戻し、原因を修正して直前のゲートから再実施する。
- G5後:
  - 旧開発PCは電源OFFのGit/WIP fallbackとして最低14日保持する。
  - asset問題は旧Blender PCへ戻さず、現PCの独立backupと承認済みmanifestから復旧する。
- G6後:
  - secure erase後は旧PCへ戻せない。
  - 独立backupの最終復元成功なしにG6を実行しない。

## 10. AI引継ぎメモ（最重要）

### 現在地

- ゲート進捗:
  - 旧開発PC `dell-latitude-5511-fedora` のM0／M1とdevelopment-only G1は
    `2026-07-29` に完了し、外付け媒体のrestore evidenceまで合格した。
  - `2026-08-01`のユーザー判断で旧Blender PCからの資産引継ぎを廃止し、
    その端末のM0／M1とfull G0／G1依存を受け入れ条件から削除した。
  - 新PCのdevelopment-only M2は完了し、G2は `PASS`。内蔵disk暗号化は
    `2026-08-01` のユーザー判断で受け入れ条件から削除されており、gate判定に使用しない。
  - 新規asset workspaceの初期化、空canonical baseline、staging smoke、外付けsnapshotの
    実restore testが完了し、G3は`PASS`。
- 完了済みマイルストーン:
  - 旧開発PC M0／M1、development-only G1。
  - 新PC development-only M2／G2。
  - M3の新規asset workspace初期化、staging smoke、独立backup／restore test。
- 完了済みの先行準備:
  - `~/Sync/hell-workers-assets`へ`source`／`staging`／`exports`／`manifests`／
    `licenses`を分離した作業領域を作成した。
  - Blender `5.1.1` Flatpak commit
    `a55abdc01ce63065cc5c61bb14e83b89820ffe540bae514370a0b20cade4b24e`を
    現PCの再現可能なbaselineとして固定した。
  - Blender MCP upstream `v0.1.3` /
    `7eed33edf4aca2ab0ca84a6da27321f89f68b504`へproject hardening patchを適用し、
    Python/headless/direct exportを既定拒否、localhost bridgeとstaging pathを強制した。
  - `gltf-validator 2.0.0-dev.3.10`、scene validator、staging GLB exporter、
    addon検証、deterministic workflow smoke、project-scoped Codex MCP設定を追加した。
  - vendor test `110 passed`、workflow契約test `6 passed`、ruff／mypy、
    addon安全設定、Blender→GLB→Khronos、direct stdio MCPのread/save/拒否経路が成功した。
- 完了済みの新PC M2受入:
  - Fedora 44、Intel Core Ultra 7 155H、30 GiB RAM、約913 GiB空き、
    Intel Arc (Meteor Lake)／Mesa 26.1.5／Vulkan 1.4を記録した。
  - `~/.bashrc`から既存`~/.cargo/env`を読み、通常login shellでもrepo固定Rust 1.96.1を
    選択するよう修正した。`doctor`とfresh cloneの`check`はpass。
  - `/tmp/hell-workers-fresh-baseline-20260801`へfresh cloneし、
    `HEAD == origin/master == 9e56c6117f942700101ce15d20ea4718be8943bb`、divergence `0/0`を確認した。
  - M1 repository archiveを`/tmp/hell-workers-m1-wip-isolated-20260801`へ隔離復元し、
    status／stash／HEADをM1 manifestと一致確認した。現行WIP worktreeへ重畳していない。
  - `bacon 3.22.0`、`cargo-expand 1.0.124`、`docsrs-mcp 0.1.0`、
    `rust-analyzer-mcp 0.2.0`をcrates.io install record付きで再構築済み。
  - rust-analyzer MCPはsymbol queryとworkspace error `0`、docsrs MCPは
    `bevy_app@0.19.0::App`実queryに成功。Codex project configにも両serverを登録した。
  - Git remoteのread／pushは成功し、移行ブランチもoriginへ公開済み。
    その後`gh auth status`でCLI token無効を観測したため、GitHub API／PR操作前に再認証が必要。
  - active script／docsの旧home絶対pathをportable path／commandへ修正し、
    GCP/TRELLIS固有の承認済み歴史資料は今回 `N/A` とした。
- 未着手/進行中:
  - M3／G3は完了。新しいproduction assetごとに同じpoint-in-time backup契約を適用する。
  - M4は現PC向け基盤だけ完了し、新規canonical assetの制作・目視受入は未着手。
  - M5／M6は未着手。旧Blender PC待ちはなく、最初の制作assetを決めればM4へ進める。

### 次の担当

ユーザー:

1. 最初にこの環境で新規制作するassetを決める。
2. staging render／visual testを確認し、canonical promote時に明示承認する。

AI:

1. ユーザーが選んだ最初のassetをstagingで制作し、scene／Khronos／目視gateを実行する。
2. point-in-time backup／restore test後、明示承認された成果物だけをcanonicalへpromoteする。
3. 明示承認前はcanonical asset切替、repo runtime asset上書きを行わない。

### AIの実行契約

- AIがユーザーへ依頼してよいのは、物理媒体の接続、passphrase／tokenの対話入力、
  OS／accountの管理者承認、Blender／ゲームの目視、正本判断、明示承認gateだけである。
- AI自身が実行できるterminal command、manifest作成、checksum、copy、検証、文書更新を
  ユーザーの手作業へ転嫁しない。
- commit／push、Syncthing role変更、canonical promote、access失効、secure eraseは、
  対応するgateで対象と影響を示し、その操作に対するユーザーの明示承認を得てから行う。
- 各セッションは `<MIGRATION_ROOT>/ledger.md` を更新し、次のPC／AIが
  会話履歴なしでも再開できる状態で終了する。
- 旧開発PCのG1は開発WIP保全に対して有効。旧Blender PCは対象外であり、
  その監査やbackupを待たずに現PCの新規asset制作へ進める。

### ブロッカー/注意点

- 現PCにはBlender `5.1.1`とhardened MCP環境を構築済みであり、これを新規baselineとする。
  旧Blender端末との一致や旧canonical Soul `.blend`回収は要求しない。
- external `source/` / `exports/` は承認済みの空baselineである。最初のassetはstagingから作り、
  provenance／license／目視承認なしにcanonicalへ置かない。
- repo内 `assets/` は現時点の唯一の既知runtime一式なので、backup前に削除・再同期しない。
- `--delete-missing` は最初のcanonical export受入まで使用禁止。
- 現worktreeには別作業の大きな未コミット変更がある。移行作業へ混ぜない。
- Blender upgradeとPC移行を同時に行わない。
- 現PC用Blender MCP／glTF validatorの導入元は固定済み。旧端末との照合は行わない。
  rust-analyzer MCP／docsrs MCPは新PC側のversion／source／実queryを確認済み。
- 現Blender FlatpakはOCIO config `2.5`をruntime OCIO `2.4.2`で読めずfallbackする。
  geometry smokeには使えるが、色再現性の受入はこの不一致を解消するまでblockerである。

### 参照必須ファイル

- `README.md`
- `docs/DEVELOPMENT.md`
- `docs/linux-setup.md`
- `docs/assets_workflow.md`
- `docs/visual_test.md`
- `docs/save_load.md`
- `docs/settings.md`
- `docs/performance-profiling.md`
- `docs/plans/3d-rtt/asset-milestones-2026-03-17.md`
- `scripts/dev.py`
- `scripts/sync_external_assets.py`
- `scripts/rust_analyzer_mcp_stdio_adapter.py`
- `rust-toolchain.toml`
- `.cargo/config.toml`
- `.gitignore`
- `.mcp.json`

### 最終確認ログ

- 最終 `python3 scripts/dev.py doctor`: `2026-08-01` / 新PClogin shellでpass
- fresh clone `python3 scripts/dev.py check`: `2026-08-01` / pass、
  `HEAD == origin/master == 9e56c6117f942700101ce15d20ea4718be8943bb`
- 最終 `python3 scripts/dev.py check`: `2026-08-01` / pass
- 最終 `cargo check --workspace`: `2026-08-01` / `verify`内でpass
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: `2026-08-01` / pass
- 最終 `cargo test --workspace`: `2026-08-01` / pass
- 最終 `python3 scripts/dev.py verify`: `2026-08-01` / all quality gates pass
- 新PC MCP:
  - rust-analyzer MCP `10 tools`、`rust_analyzer_symbols`成功、workspace error `0`
  - docsrs MCP `4 tools`、`bevy_app@0.19.0::App`成功
- 新PC GitHub: Git remote read／pushとbranch SHA一致はpass。
  `gh` CLI tokenは後続確認で無効となっており、API／PR操作前の再認証が必要。
- 新PC GPU: Intel Arc (Meteor Lake)、Mesa `26.1.5`、Vulkan `1.4` adapter query pass
- Blender AI workflow focused検証（`2026-08-01`）:
  - vendor: `110 passed`、ruff pass、mypy pass
  - project workflow契約: `6 passed`、ruff pass
  - addon: safe mode、auto-start off、inline Python off、staging root／whitelist一致
  - scene: mesh `4`、triangles `432`、errors `0`、warnings `0`
  - Khronos: errors `0`、warnings `0`
  - GLB SHA-256:
    `a818b962edd7e4addf12c83e32f8571455f09291be1f4c631eaff0922e37e94d`
  - direct stdio MCP: `28 tools`、scene read／staging save成功、
    Python／headless／direct export拒否
  - 新規workspace baseline backup:
    `snapshots/fedora-new-asset-workspace-20260801/`、5 artifact checksum／実restore test pass
- 未解決エラー:
  - 最初の新規canonical assetが未作成。backup／restore手順はbaselineで実証済み。
  - fonts／faviconのlicense／provenanceと恒久的な管理経路が未確定。
  - 現Blender FlatpakのOCIO version不一致により色再現性が未受入。
  - `gh` CLI tokenが無効。Git remote pushは成功済みだが、API／PR操作には再認証が必要。

### Definition of Done

- [ ] Gitの全refs、stash、staged／unstaged、untracked内容が移行または明示廃棄済み
- [ ] archiveしたWIPを新PCの隔離branch／worktreeへ復元し、M1 manifestと照合済み
- [ ] 対象branchのremote divergenceとGitHub read／push権限を非破壊確認済み
- [ ] 新規external asset、repo内assets、save/settingsのbackupと復元テストが成功
- [x] 新PCの保存容量、GPU／Vulkan、active Rust toolchain、rustfmt、Clippyを確認済み
- [x] 新PCで `doctor`、rust-analyzer、`check`、`verify` が成功
- [x] rust-analyzer MCPとdocsrs MCPのversion／sourceが固定され、実queryが成功
- [x] glTF validatorのversion／sourceが固定され、staging GLBがerror 0、warningレビュー済み
- [ ] `cargo run --locked` と `cargo run --locked -p visual_test` がGPU込みで起動し、audio初期化errorがない
- [x] 旧Blender PC／Syncthingに依存しない空のcanonical workspaceを現PCに初期化済み
- [ ] asset catalog参照path、runtime 4フォント、license／provenanceを検証済み
- [x] Blender exact version、配布経路、addon、安全設定、export条件が台帳化
- [ ] 最初の新規canonical原本にmissing resourceがない
- [ ] staging Soul GLBのexport、asset sync、Bevy読込が成功
- [x] Blender MCPのread-only接続が成功
- [ ] GitHub read／pushと必須外部接続を新PCだけで利用可能
- [ ] secretがrepo、remote URL、平文credential fileへ混入していない
- [ ] global Git設定とhookがportable化または不採用としてdocsと整合済み
- [ ] active docs／scriptsから旧PC固有のhome／drive／SDK絶対pathを除去済み
- [ ] 検証用copyでF9/F5とsettings再起動確認が成功
- [x] 新PCが唯一の開発／Blender編集端末
- [ ] 安定化期間と最終restore testが完了
- [ ] 必要な旧開発PCのaccess失効と、必要時のsecure eraseが完了
- [ ] 恒久情報と `docs/blender-setup.md` のroot index登録が完了
- [ ] `python3 scripts/dev.py docs --write` と `python3 scripts/dev.py docs --check` が成功
- [ ] 本計画がarchiveまたは削除済み

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-01` | `Codex` | ユーザー判断により旧Blender PCからの資産／設定引継ぎを廃止。このPCを唯一の新規authoring hostとし、M3を空workspace基準化、M4を新規canonical asset制作へ再定義 |
| `2026-08-01` | `Codex` | ユーザー判断により内蔵disk暗号化をM2／G2の受け入れ条件そのものから削除。既完了の自動受け入れ結果に基づき、development-only G2を`PASS`へ更新 |
| `2026-08-01` | `Codex` | 外部M1台帳から正しい現在地を復元し、新PC M2のfresh clone、WIP隔離復元、toolchain、MCP実query、GitHub、Vulkan、portable pathを受入。当初はLUKS欠落をG2 blockとして記録したが、上記のユーザー判断で解消 |
| `2026-08-01` | `Codex` | 現PCのBlender/MCP/validator先行基盤とfocused検証結果を記録。当初は旧原本回収待ちとしたが、上記の新規構築判断で解消。OCIOは未解決 |
| `2026-07-29` | `Codex` | ユーザー用Start Here、全source PCのM0／M1、M0〜M6担当分担、固定成果物、AI引継ぎprompt／報告契約を追加 |
| `2026-07-29` | `Codex` | 現行repo、開発環境、Syncthing、Blender／asset経路の監査結果を反映して初版作成 |
