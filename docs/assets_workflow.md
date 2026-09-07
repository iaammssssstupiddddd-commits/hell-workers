# アセット共有・同期ワークフロー

`Hell Workers` の画像・モデル・音声などのバイナリアセットを、`git` 以外で複数 PC 間共有するための運用仕様。

## 1. 方針

- 共有手段は `Syncthing` を推奨する。
- **原本** と **ゲームが直接読む実行用アセット** を分離する。
- 原本同期フォルダは **リポジトリ外** に置く。
- リポジトリ配下の `assets/` には、ゲームが直接読む最終ファイルだけを置く。

## 2. 推奨ディレクトリ構成

例:

```text
~/Sync/hell-workers-assets/
├── source/
│   ├── blender/
│   ├── generated/
│   └── references/
├── staging/
│   ├── imports/
│   ├── blend/
│   ├── exports/
│   ├── renders/
│   ├── reports/
│   └── snapshots/
├── exports/
│   ├── textures/
│   ├── models/
│   └── audio/
├── manifests/
└── licenses/
```

- `source/`
  - Aseprite, PSD, Krita, Blender, 参照画像などの原本を置く。
  - ここを `Syncthing` で複数 PC 間同期する。
- `exports/`
  - ゲーム投入前の最終出力物を置く。
  - `png`, `glb`, `ogg`, `wav` など、Bevy が直接読む形式に揃える。
- `staging/`
  - AI生成物・変換コピー・検証前exportを隔離する。ここは正本ではない。
  - 自動化は `source/` と canonical `exports/` へ直接書かない。
- `manifests/` / `licenses/`
  - 生成元、hash、Blender/export条件、利用許諾、review結果を記録する。

新規authoring環境では`source/`と`exports/`を空から開始してよい。repo内`assets/`は
実行用referenceであり、GLBやtextureを制作原本へ逆変換してcanonical扱いしない。
最初の原本はstagingで新規作成し、検査・license／provenance・目視承認後にだけ昇格する。

## 3. リポジトリとの責務分離

- リポジトリ外の `~/Sync/hell-workers-assets/source/`
  - 編集中の原本
  - 共同作業用の参照素材
- リポジトリ外の `~/Sync/hell-workers-assets/exports/`
  - `assets/` に反映するための中間成果物
- リポジトリ内の `assets/`
  - ゲーム実行時に読む最終アセット
  - `fonts/` と `shaders/` は既存ルールを維持する

この分離により、原本共有とゲーム実行用アセットの責務が混ざらない。`Syncthing` の競合や一時ファイルがリポジトリ運用へ直接流れ込むのも防げる。

## 4. 日常運用

1. AI生成物や外部変換物を `staging/imports/` に置く。
2. Blenderの作業コピーを `staging/blend/` に保存する。
3. scene検査、`staging/exports/` へのGLB書き出し、Khronos検査を実行する。
4. `staging/reports/`、render、license、provenanceを確認する。
5. 人が明示承認した成果物だけを `source/` / canonical `exports/` へ昇格する。
6. `scripts/sync_external_assets.py --dry-run` で差分を確認してから `assets/` へ反映する。
7. ゲーム内またはvisual testで確認する。

Blender AI編集、品質gate、MCPの安全境界は
[`blender-setup.md`](blender-setup.md) を参照する。

production Wall M1候補は1つの`.blend`にあるexact named collectionを個別に検査・exportする。各GLBは
Khronos validatorの後に`tools/blender_ai_workflow/bin/validate-wall-glb`で実bytesを再検査し、scene reportだけを
合格根拠にしない。M1中の`.blend`、GLB、texture、reportはすべて`staging/`に留め、asset-set manifestと
promotion receiptの実装・検証・承認が終わる前に`source/`、canonical `exports/`、repo `assets/`へ移さない。

マゼンタ背景付き画像から透過 PNG を作る場合は、既存の `scripts/convert_to_png.py` を使ってから `exports/textures/` に置く。

### 画像・モデル生成時の共通規約

- アート方向は `docs/world_lore.md` §6.2 と `docs/art-style-criteria.md` を正とする。古い個別 prompt のスタイル文をコピーして正本化しない。
- 透過 cutout 用の生成画像は、背景を **solid magenta `#FF00FF`** にする。gray / off-white 背景は `convert_to_png.py` の前提外。
- 出力ファイル名と配置先は、生成前に現行 `assets.rs` / asset catalog と `assets/` の実体を確認する。旧 2D Soul スプライト名を新規生成の前提にしない。
- P02以降のSoulは既存sprite画像を共有pool化したalpha-mask 3D billboard、Familiarは2D前景表示を使う。旧Soul GLB / face atlas / shadow proxyはruntimeと`visual_test`のどちらも読み込まない。建築物はplaceholderから最終assetへ段階移行する。媒体ごとの現況は `docs/plans/3d-rtt/asset-milestones-2026-03-17.md` を参照する。
- 再利用する具体的な生成 prompt は `source/references/` 側で原本と一緒に管理し、repo 内には安定したスタイル・背景・命名・受入規約だけを残す。

例:

```bash
python scripts/convert_to_png.py \
  "~/Sync/hell-workers-assets/source/ui/icon_idle.png" \
  "~/Sync/hell-workers-assets/exports/textures/ui/icon_idle.png"

python scripts/sync_external_assets.py \
  --source ~/Sync/hell-workers-assets/exports \
  --dry-run
```

## 5. `scripts/sync_external_assets.py` の責務

このスクリプトは、外部同期済み `exports/` からリポジトリ内 `assets/` へ、許可されたサブディレクトリだけをコピーする。

- 既定の同期対象: `textures`, `models`, `audio`
- 既定では削除を行わない
- `--delete-missing` 指定時のみ、コピー元に存在しない同期対象ファイルを `assets/` から削除する
- `fonts/` と `shaders/` には触れない

Wall production asset-set v2は、staging全体を対象にする上記legacy modeではなくmanifest allowlist modeを使う。
`--manifest`と`--selection core`は必ず組で指定し、art-approved final manifestの全artifact / report / license /
source hashとtool commitを検証した後、normalを含まないexact 8 fileだけを明示したasset rootへコピーする。
manifest外のfileをcopy / deleteせず、symlinkやroot外pathも拒否する。

```bash
ASSET_ROOT="${HELL_WORKERS_ASSET_ROOT:-$HOME/Sync/hell-workers-assets}"

python3 scripts/sync_external_assets.py \
  --source "$ASSET_ROOT/staging/exports" \
  --dest "$VALIDATION_WORKTREE/assets" \
  --manifest "$ASSET_ROOT/staging/reports/wall-production-v1.asset-set-final.json" \
  --selection core \
  --dry-run
```

pending candidateと`optional:normal`は同期対象にしない。manifest modeでは`--delete-missing`を併用できない。
isolated candidate projectionはfinal payloadをreceiptなしで隔離検証するためだけに使い、primary / canonicalへ
直接同期しない。release projectionはgeneration-scoped promotion receiptを必須とする。

仮設Wallのauthoring v3は、既存generation 4の完成Wall 8 fileをhash不変で再利用し、
`mesh:formwork:<family>` 6 fileと`texture:formwork_albedo` 1 fileを加えたexact 15 fileを持つ。
未承認bytesは`project_wall_formwork_preview.py`でruntime wallset v2へ投影し、profiling buildかつ
`HW_WALL_ART_PREVIEW=1`、generation、manifest hashの三点一致時だけ読み込める。projectionは
`authority=art_preview` / `review_status=art_preview` / receiptなしで、正式candidate・release・promotionの
代用にはならない。`provision_wall_formwork_preview.py`は外部`staging/validation/`配下の新規viewだけへ
allowlistを固定し、異なる既存bytesの上書きを拒否する。

ユーザーがゲーム所有windowのArtPreviewを承認したら、`record_wall_formwork_approval.py`で技術候補manifest、
`wall-formwork-art-preview-v1`のvalid job、PNGの実bytes、subject/source/harness/asset-view fingerprint、承認UTC、
ユーザー文言を`wall-formwork-v1.art-approval.json`へ結ぶ。failed job、別candidate、標準zoom以外、または
`evidence_kind=art_preview`でない入力は拒否する。このartifactはアート判断の記録であり、単独では
`isolated_candidate`やrelease authorityを与えない。

Door M2を含むclean runtime subjectが確定した後、`seal_wall_formwork_final.py`が承認済みcandidateを新generationの
authoring v3 final manifestへ封印する。`project_wall_formwork_candidate.py`はそのfinalだけをruntime schema v2の
`authority=isolated_candidate`へ投影する。正式candidateは`art_review.status=art_approved`、exact 15 core、
`normal_decision=rejected`、receiptなしを要求し、ArtPreview projectionを正式受入へ流用しない。
`provision_wall_formwork_candidate.py`はこのfinalだけを外部`staging/validation/`配下の新規viewへコピーし、
異なる既存bytesやlocatorの上書きを拒否する。
provision後の正式画像は`wall_art_acceptance.py`の`--candidate --matrix --formwork`を標準zoomと
`--zoom farthest`で各1 job実行する。profileはそれぞれ`wall-formwork-v1` / `wall-formwork-v1-farthest`で、
`art_preview`を受理せず、provisional 96 ownerがexact 15 core由来のformwork mesh 6種とOpaque materialへ
全件収束してfallback 0であることを検証する。完成Wall用matrixのpassはこの検査の代用にしない。

DoorはWall manifestと分離したauthoring/runtime schema v1を使う。`seal_door_candidate.py`は3 GLB、shared albedo、EW/NS previewのexact 6 coreと、scene/export/Khronos/post-export/texture/OCIO report、Blender原本、geometry fixture、imagegen prompt、license、tool source fingerprintをtechnical candidateへ封印する。`project_door_preview.py`はcandidateだけを`authority=art_preview` / `review_status=art_preview` / receiptなしの`manifests/door-production-v1.doorset`へ投影する。

`provision_door_preview.py`は外部`staging/validation/`配下の新規asset viewへexact 6 fileだけをcopyし、異なる既存byteを上書きしない。profiling buildで`HW_DOOR_ART_PREVIEW=1`、generation、authoring manifest SHA-256の三点が一致した場合だけ有効になる。通常起動、正式candidate、releaseのauthorityにはならない。Door初回releaseのrollback先は旧generationではなくprocedural fallbackである。

ゲーム所有windowのDoor ArtPreviewをユーザーが承認したら、`record_door_approval.py`がtechnical candidate、
clean subject、production 6 / fallback 0、X11/Vulkan、capture・review crop・status/ACKの実bytes hashと
承認UTC・承認文言を`double_leaf_approved` artifactへ結ぶ。別candidate、dirty subject、fallback混入、
または`evidence_kind=art_preview`でない証拠は拒否する。このartifactだけでは通常authorityを変更しない。

`seal_door_final.py`は承認済みbytesをclean runtime subjectへ結び直した新generationのauthoring finalを作る。
`project_door_candidate.py`は`manifest_mode=final`、`art_review.status=double_leaf_approved`、exact 6 core、
`normal_decision=not_used_by_design`だけをreceiptなしの`authority=isolated_candidate`へ投影する。
`provision_door_candidate.py`はそのprojectionを外部`staging/validation/`配下へ固定し、
`HW_DOOR_CANDIDATE=1`とgeneration / manifest hashが一致する隔離検証だけで有効にする。
ArtPreviewの成功はこの正式candidate受入へ読み替えない。

canonicalへ昇格した後のprimary同期は、同じmanifest allowlistに`--receipt`を加えたrelease modeで行う。
manifestが`generations/<GEN>/manifest/`にある場合は`--receipt`を必須とし、receiptのmanifest hash / generationと
active pointerの三点一致を検証してからcopyする。配置先は`project_wallset.py`の`runtime_path`をそのまま使い、
`assets/wall_sets/<GEN>/models/<mesh>.glb`と`assets/wall_sets/<GEN>/textures/...`へ落とす。staging候補の
manifest-relative配置とは別であり、release projectionが名指すpathと必ず一致する。

```bash
python3 scripts/sync_external_assets.py \
  --source "$ASSET_ROOT/generations/<GEN>/exports" \
  --dest "$PWD/assets" \
  --manifest "$ASSET_ROOT/generations/<GEN>/manifest/wall-production-v1.asset-set.json" \
  --receipt "$ASSET_ROOT/generations/<GEN>/authority/promotion-receipt.json" \
  --selection core \
  --dry-run
```

同期後はreceiptを`assets/wall_sets/<GEN>/authority/promotion-receipt.json`へ置き、唯一のmutable runtime authorityである
`assets/manifests/wall-production-v1.wallset`をtemporary file→fsync→atomic rename→親directory fsyncで最後に切り替える。

art承認後はpending manifestを上書きせず、normalを除いたtexture reportを新規作成してから新generationを封印する。
`seal_wall_final.py`はcleanなruntime subject、元candidate manifest hash、lit選定画像hash、ユーザー承認UTCを結び、
`normal_decision=rejected`、`art_review.status=art_approved`、core 8 / optional 0を強制する。

```bash
python3 tools/blender_ai_workflow/scripts/validate_wall_textures.py \
  --texture-root "$ASSET_ROOT/staging/exports/textures/buildings/wall" \
  --report "$ASSET_ROOT/staging/reports/wall-production-v1.textures-final.json" \
  --normal-decision rejected

python3 tools/blender_ai_workflow/scripts/seal_wall_final.py \
  --asset-root "$ASSET_ROOT" --repo "$VALIDATION_WORKTREE" \
  --candidate-manifest "$ASSET_ROOT/staging/reports/wall-production-v1.asset-set.json" \
  --texture-report "$ASSET_ROOT/staging/reports/wall-production-v1.textures-final.json" \
  --approval "$ASSET_ROOT/staging/reports/wall-production-v1-art-review.json" \
  --generation '<NEW_GENERATION>' --created-at-utc '<UTC>' \
  --output "$ASSET_ROOT/staging/reports/wall-production-v1.asset-set-final.json"
```

### Wall v2 canonical generation

`init-asset-workspace`は既存generic v1を上書きせず、`generations/`、`authority/`、`quarantine/`とWall v2
templateを冪等に追加する。`verify-asset-workspace`は必要directoryが実directoryであることと、generic v1 / Wall v2
template schemaをread-onlyで検査する。

final manifestの昇格は`promote_asset_set.py plan`でcurrent pointer preimageとroot外snapshot pathを封印してから行う。
`apply`はM6の別承認対象であり、M5 evidence、release approval、固有receipt ID、approval UTC、`--confirm`が必須。
世代payloadとimmutable receiptを完全にfsync・renameしてから、最後に`authority/wall-production-v1.active.json`だけを
atomic replaceする。中断時の`recover`とpreimageへ戻す`rollback`は既定read-onlyで、`--apply`時も世代を削除せず
`quarantine/`へ移す。generation番号とreceipt IDは再利用しない。
payloadにはmanifestが指す全fileを含める。core 8 file、per-mesh export / khronos / post-export / scene report、
set report、art review artifact、texture validation report、source `.blend`、license、manifest本体である。
1つでも欠けると昇格後のgenerationを単体でvalidatorへかけられないため、回帰testで
promoted generationがそれ自身のrootだけで`validate_manifest`を通ることを固定している。
不完全なgenerationを昇格させてしまった場合は、pointerを`rollback`で戻し、当該generationを
`generations/.quarantine-<GEN>-<理由>-<UTC>`へ退避してから、修正したtoolで同じpayload manifestを再applyする。
active pointerが指していないgenerationは削除せず、監査のために隔離のまま残す。

validation worktreeは作業場であって成果物ではない。trackを閉じたら、各jobの`manifest.json`、比較CSV、
承認画像だけを`staging/validation/<capsule>/`のような小さなdirectoryへ残し、worktree本体は
`git worktree remove`で削除して使っていたbranchも消す。worktree 1つはRustの`target/`込みで10 GB規模になり、
過去のartifactは後続subjectの証拠に使えない（harnessはfingerprintが一致するfresh runを要求する）ため、
残しても容量を消費するだけである。

### Wall runtime projection

検証済みasset-set manifestは、そのままruntimeへ読ませず、projection toolで
`assets/manifests/wall-production-v1.wallset`へcanonical JSON projectionする。projectionはasset-set generation、
元manifest SHA-256、authority、exact core inventoryを保持し、key順・空白・末尾改行を含むbyte表現を固定する。
Bevy loaderはruntime schema v1 / v2を区別し、非canonical JSON、unknown field、path / role / byte length / SHA-256違反を拒否する。v1ではcore 8 file、v2ではcore 15 fileのactual bytesを
asset rootから一度だけ読み直して照合する。

candidate authorityは通常起動では常にfallbackで、primary / canonicalへ配置しない。隔離validation worktreeだけが
`HW_WALL_CANDIDATE=1`を設定できる。runtime loaderは`art_approved`かつnormal判定済みのfinal payloadだけを受理し、
v1は6 GLBの`Mesh0/Primitive0`、albedo、emissive、2 shared lit production materialを有限poolへloadする。
v2は完成6 GLBに型枠6 GLBと型枠albedoを加え、仮設はOpaqueな専用material、本設は既存materialを使う。
pending review、optional normal、manifest identity opt-in不一致はfail closedとする。

## 6. 競合回避ルール

- 同じ原本ファイルを複数 PC で同時編集しない。
- 原本のファイル名と書き出し先を安定させる。
- `Syncthing` の conflict file を見つけたら、原本側で必ず統合してから `exports/` を更新する。
- 大きな原本（例: `.blend`）は日次バックアップを別経路にも持つ。

## 7. 向いているケース / 向いていないケース

- 向いている:
  - 個人開発または少人数でのアセット制作
  - 複数 PC 間で同じ原本を扱いたい
  - クラウド専用 SaaS へ依存したくない
- legacy modeが向いていない:
  - 同じバイナリを複数人が同時編集するワークフロー
  - 厳密なレビュー承認付き配布物管理

Wall asset-set v2の厳密な配布管理は、manifest allowlistとgeneration / receipt付きpromotion経路で扱う。
legacy assetで公開先を外部storageへ切り替える場合も、`assets/`への既存反映フローは維持する。
