# ミキサーと休憩所の用途判読性を修正する

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | mixer-rest-readability-2026-11-15 |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-11-15 / 2026-11-15 |
| 作成者 | Codex |
| 関連提案 / Issue | 利用者の直接フィードバック / TAK-14 |

## 1. 目的

利用者が現行Mixerのアートを不採用、RestAreaを判読困難と評価した事実を受入条件とする。
通常縮尺でドラム・口・支持架台・車輪、休憩所の屋根・壁・閉じた入口を読み取れる描画へ修正する。
過去の技術passや独立採用を新フィードバックの解決へ読み替えない。

## 2. スコープ

新しい2種の制作原本・明示style、個別normal/far・回転姿勢の実画面比較を先に行う。
屋根の入口遮蔽が残れば一度で色調整仮説を打ち切り、新geometry/numeric契約へ進む。
論理占有・施工・入所Soul非表示・Dream・回転軸/速度・保存を維持し、他7種とBridgeを変更しない。

## 3. 現状とギャップ

Mixer g4は口縁が明るい同心円で、暗い台車と支持構造が一塊に見える。
Rest g4は紫灰の屋根と壁・入口カーテンが同色で、短い縫目だけが強く見える。
mode4は実際に有効。照明係数ではなく、object-localな図柄と用途を示す輪郭を修正する。

## 4. 実装方針

同目的branch `codex/tak14-non-bridge-release`、比較基点 `c4747ce8165b4e3695549c92197f7e6ac9f3a8d9`。
mainだけが編集・build/test・Git・原本操作を所有し、既存担当はread-only調査/独立reviewのみ。
旧illustrated-v1は不変再現できるよう、新illustrated-v2を明示選択する。
Mixerは口縁/胴/支持/車輪を分離する色面と太い接合線、非対称摩耗を部品UVに固定する。
Restは棟/軒、布の大きな継ぎはぎ、専用の暗い入口と枠・垂れ幕を描き分ける。
新原本・previewに旧art/numeric承認を流用しない。既定bindingは新しい採否が成立するまで維持する。
公開・push/mergeは本修正へ含めない。

## 5. マイルストーン

### M1: 図柄の修正と比較準備

- [x] 原画像/原本/generatorとshader経路を照合し、独立read-only調査を行う。
- [x] 新styleと拒否/旧再現/UV回転/部材判読性の回帰を実装する。
- [x] `37dc337c`のHelp No impact、contracts/tooling CI、storage、独立コードreviewを通した。
- [ ] 以下のgeometry試作にも新しい回帰と独立reviewを通す。

### M2: 実画面の用途判読性

- [ ] 新しい隔離原本を生成し、normal/far・Mixer四姿勢を独立比較する。
- [ ] 屋根が入口を隠す場合は新geometryを採用前に別numeric reviewする。
- [ ] 改訂した画像を利用者へ提示し、新しいart判断へ結合する。

### M3: 採用と終了

- [ ] 新しいart/numeric/candidate/release判断、切戻し・通常起動・必要な状態/性能確認を行う。
- [ ] 正本原本・仕様・採用画面を同期し、用途終了jobを整理してstorage checkを通す。
- [ ] 完了した一時計画を閉じて両索引を再確認する。

## 6. リスクと対策

全体tintや細い傷だけの反復を避ける。通常縮尺で入口と支持構造が読めるかを先に判定する。
旧原本・正式世代・receiptは上書きしない。試作・技術passから正式採用を推測しない。

## 7. 検証計画

該当Python toolingの実入力/拒否回帰、Help実経路の判断、`dev.py ci check --base <上記full-SHA> --mode auto`、
`git diff --check`、必要なnativeを登録済みdirect kitty→独立verify/seal/finalize/checkで行う。
Rust変更時はcheck/Clippy0/診断を追加する。外観が成立する前に全性能matrixを実行しない。

### 検証データ管理

正本は `docs/development-infra/validation-storage-workflow.md`。
consumer: `Mixer/Rest readability 2026-11-15`、owner: Codex。
primaryと既存Cargo cacheをこのフィードバック単位で維持し、新規targetを作らない。
新staging/jobはexact path・bytes・次用途・release_whenをprimary validationへ登録する。
採用/不採用の判断後に原本を正本へ保全し、用途終了jobを整理する。無応答はcloseにしない。

## 8. ロールバック方針

新正式世代まで旧g4 bindingを維持し、採用時はguard付きcanonical/runtime rollbackを確認する。
旧source/authorityを破壊して試作を表示する方法は使わない。

## 9. AI引継ぎメモ

現状: 原本と実画面を照合済み、実装開始。ユーザー不採用の解消が今回の目的。
最初に現diffと本計画を照合し、旧Orca統括や終了済みnativeを再送しない。
未完了を技術passへ隠さず、実際の新画像と次の判断を残す。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-11-15 | Codex | 利用者フィードバックに基づき2種の判読性修正を開始 |

### 配色比較の結果と次の形状試作

`37dc337c`のillustrated-v2原本は2種ともsource/GLB/UV/atlas/toolchainの登録済み独立verifyとfinalizeに成功した。
しかし独立外観reviewは両方ともchanges requested。Mixerの支持部と口の色分離、Rest入口の暗色化は改善したが、
Mixerの大きな板台座と円盤状の見え、Rest屋根の本/厚紙状の外形が残った。技術passをart採用へ変更しない。
通常画面へ公開せず、同系統の色調整はここで打ち切る。

Mixerは明示v4へ細い二本の実レールと支持脚を作り、共有yawだけを新しい数値候補へ分離する。
local Y軸・pivot・55度tilt・4秒/回転・Refining条件は維持する。連続全周sweepを新bodyへ再検査する。
Restは明示v3（Rest限定、Spa不変）へ連続したたわみ屋根と閉じた波打つカーテンを作る。
旧v2 fixture/原本/採否は保持し、新numeric/art採用は別に取り直す。

旧最初の生成wrapperは登録後・未起動reviewでassert guardを指摘され、child/出力なしを確認してabandoned/finalizeした。
通常if/raiseへ修正した別wrapperをPYTHONOPTIMIZE0/1/2で拒否確認し、fresh v2出力を生成した。旧登録は再送していない。

### Rest v3のUV照合補正

`aa053470`のRest v3はGLTF検査0errorsで出力されたが、indexed UVの照合でexit1となりinvalid/finalizeした。
576頂点中1点のUは元double `0.5202349856035131`、実f32は`0.520235002040863`で、旧round5のkeyが分かれた。
expectedへ実出力と同じf32格納→V反転→f32格納を適用すると576件全一致した。
許容幅やface privacy/windingを変えず、expectedの型変換順序を補正する。旧失敗をpassへ変更せずfresh sourceで再生成する。

### 手描き原画を基準に再設計する

形状改訂の原画像も独立外観reviewはchanges requested。Mixerの中央の抜けは改善したが胴が円盤状、
Restの入口は改善したが屋根は硬い板状に見える。これらを採用せず、実機の技術passで解決扱いにしない。
新しい手描き基準画をbuilt-in image_genで作成し、原本bytesを保存した。

- [Mixerの手描き基準画](../design/building-art/mixer-hand-ink-direction-20261115.png)
- [Restの手描き基準画](../design/building-art/rest-hand-ink-direction-20261115.png)

いずれもゲーム実画面・numeric/art/release承認ではない。旧原本・既定bindingを保持する。
次は静的部分へこの線と色面を結合し、Mixer可動部は回転に追従するUVを保つ制作経路を組む。
Tankで採用した画像とgeometryの分担を参照し、動く部品へカメラ固定絵を貼って動きを失わせない。

### 明示handpaint-v1の制作経路

部品を含む2方向画像からの投影は裏面/開口の未被覆や他部品混入を起こすため、その方式は採らない。
built-in image_genで「物体や陰影を含まない」不透明な錆鉄・灰鉄・布の素材面を作り、固定SHA/寸法/元promptと共に
`tools/blender_ai_workflow/fixtures/handpaint-materials-20261115-v1/`へ保存した。これ自体に採用権限はない。
ドラムは物体localの円周/長さへ連続に割当て、一つの明示溶接線でwrapを閉じる。静的部材のinkは物理部材の縁へ置き、
UV島やtriangulationへ枠を描かない。屋根/幕は布の面に結合し、元画像のalphaで穴を開けない。
全入力はtoolchain hashへ入り、source.blendへ画像とsnapshotをpackし、authoring bundleにも元PNG/manifestをcopyする。

Mixer v5は長い側胴とyaw -65度（軸/支点/tilt/4秒回転は維持）、Rest v4は四隅を上げ中央を下げた連続布屋根、
傾いた木柱と閉じた幕を明示numeric提案へ分離した。Restは実304tri、提案cap320、旧と同じ外接bounds。
旧版/他kindへの暗黙適用は拒否する。5件の素材/拒否/実atlas/回転/形状回帰はpass、独立reviewと実画面は未実施。

### 手描き素材の初回生成からの修正

- `753ee1b2`の技術checkpointはcontracts/tooling/deps/rustの変更別検証と独立読取レビューを通過した。
  Mixer v5／Rest v4の手描きsourceはprimary登録→既定launcher→独立verify→比較用途retain→finalize済み。
  ただし両方とも外観は不採用。Mixerは全5姿勢で円盤状、Restは入口が屋根で隠れ紙片状だった。
- 色調整を続けず、別の明示的未承認proposal Mixer v6／Rest v5を追加する。旧fixture・原画・失敗履歴を保持する。
  Mixerは最大径の側胴区間を18wuへ伸ばし、長36／径24、yaw −110°へ変更。
  現cameraの投影で軸長35.68wu、径24wuとなる。支点・55°傾斜・local Y軸・Virtual/Refiningの4秒周期を維持し、
  連続一周の全static component最小clearanceは正。手描き材質の主線を長手へ向け、周方向の帯を増やさない。
- Restは深い切欠き案を撤回。屋根前縁を高くして入口幕の縦面を見せ、中央の低下は屋根奥に残す。
  旧box壁と別幕はこのproposalだけ閉じた布shellに置換。実屋根三角形を外周矩形でclipしたceilingを
  屋根下面から0.25wu重ね、ground cap／全周wallと接続する。bilinear代用面を使わない。
  前面を閉じたentryとして描き分け、巨大な黄色補修面を避け、支持角から低い中央へ少数の大きなfoldを描く。
  2個の金属collar geometryは木柱の描画上のbindingへ置換し、同bounds・298tri／320cap・248UV cellsに収める。
- focused回帰4件は閉mesh／winding／positive volume、全UV頂点の逆変換、実屋根との重なり、内部から上・横のray遮蔽、
  実投影の入口高さ、Mixer側胴の投影長・sweep、旧clockと支点、誤kind/styleの起動前拒否を確認した。
  actual-window、独立art/numeric、candidate・release・既定切替は未実施／未承認。
- Helpは再度実経路を確認しNo impact。新proposalは明示的authoring選択だけで、通常起動bindingとplayer操作／機能／文言／保存／Help到達性は不変。

### 屋根図柄の再評価

新Mixer v6は全5姿勢の独立原画比較で現場ミキサーとしての方向を承認、実機通常／遠景は未確認。
Rest v5は入口面積と被覆を改善したが、対称X線＋横縫い線が荷物に見えるため原画art不採用。
新Rest v6は閉mesh・bounds・UV・numeric構成を維持し、素材のstitch-free領域と離れた2本の曲線へ変更する。
中央の閉じ目／左右の幕の折れ・暗い側壁・輪郭のある木柱を分離する。旧proposalは変更しない。
未起動のMixer ArtPreview包装v2はchild／root不存在を照合してabandoned→finalize。未知操作の再送ではない。
両種を次の同一clean sourceで新規生成・独立検証し、その後actual-windowへ進む。

### 四角い屋根の仮説の打切り

Rest v6も原画比較で「布の看板」という輪郭が残り、独立art reviewはchanges_requested。
四隅高・中央低の形状はこれ以上調整せず、明示Rest v7で急な布テント断面へ転換する。
旧v2の浅い切妻（隆起10wu）の再利用ではなく、側高26–33／中央ridge57–62wuと前後の実支柱を持たせる。
新しい高さ上限64wuは未承認numeric proposalとして扱い、旧bounds承認を流用しない。論理4区画・閉鎖shell・Soul非表示は維持。
actual meshは224tri／320cap、174UV cells、preview全頂点はx66–190／y12.2–196pxで、256px canvasに余白を持つ。
新revisionだけshape-onlyのplanes出力を許可し、実原画で山型・支柱・正面幕を先に確認する。
planesはprepareのstrokes guardで拒否し、ArtPreview／候補／正式導入にはできない。
Mixer v6は原画方向承認後、独立検証したArtPreview g5 manifest `a05c5fb9bc2424cd83429c2cbb4a479d760be80a508e583edae3cdd885f4a09c`へ固定済み。
Restの制作修正で同じMixerを作り直す必要はなく、固定assetを現在のclean製品sourceに載せて別途実機確認する。

### 単色形状とcatalog投影／Mixer試験経路

- Rest v7の単色形状は独立原画比較で布テントとして次へ進める評価。生成自体はinvalidとして記録した。
  world anchor y144の上端12.2pxは収まるが、catalog anchor y128では−3.8pxとなり、既存の頂点crop guardで停止する。
  元batch・Empty画像・blendを診断用途に保持し、成功へ読み替えない。
- Rest v8はv7と全vertex/faceが同一で、preview canvasだけ144WUへ広げる新未承認proposal。
  worldとcatalogの両anchorで全頂点の4px以上の余白を確認する。旧128WUに対する失敗も回帰で保持し、guardは緩めない。
- Mixer v6用のprofiling専用M2 probeに固定numeric SHAを明示追加し、既存candidate/storyboard/context/UI経路を接続する。
  v4/v5や未知revisionを包括許可しない。Tank v1／既存storyboard Mixer v2の既定は維持する。
  UI側の「SHAリスト末尾ならv2」という推測は撤去し、planへ明示revisionを保存、kind/SHAを共通bindingで再検証して
  process envへ渡す。traceも同bindingを検証する。これらはart/numeric/性能/releaseの承認ではない。
- focused Pythonは78件成功、v8を含む7件の形状／投影回帰も成功。Rustは実fixture SHA照合とTank/未知revision拒否を追加。
  Helpは実consumerを再確認しNo impact。profiler検証入口のみで通常player controls、logic、text、Help reachabilityは不変。

### fractional geometryのFloat32照合

Rest v8はcatalogを含む原画の方向を独立評価で承認（正式artではない）。生成はGLB位置照合でinvalid。
clipped ceilingの4頂点がdouble34.241666667と実Float32 34.24166488647461でround5境界を跨いだ。
実572position/UV集合は位置もFloat32表現へ合わせると差0で、全indexed face／winding／perimeterもread-only診断で一致。
位置もBlender/GLTF Float32 storage（/32→*32は二進指数変更）へ変換してから既存round5を適用する。
許容誤差・face私有性・index／winding判定を拡大しない。実GLB Float32 accessorを読む境界正例と実位置変更の拒否を追加した。
旧batchはinvalid封印・診断retain・finalizeし、次はfresh名のsourceで生成する。

### 実機の外観方向と制作原本の再保存

- `cb8deb1b`の2種ArtPreview g5 normal/far 4batchはactual-windowを独立verify／用途retain／finalize済み。
  独立原画評価でもMixerの横長ドラム・台車、Restの山型屋根・幕・柱は通常／遠景で読み分けられる方向と判断した。
  MixerのDirt上の支持台はまだ十分な比較がないため、修正後fresh contextで補う。正式art/numeric/releaseではない。
- 原本点検で、3つの未使用Image IDがsaved blendから落ちていた実欠陥を確認。
  Zstd解凍後のM2 855041B／Rest741020Bにはatlas PNG1枚とsnapshot Textだけがあり、3つのImage名と元PNGbytesはなかった。
  原素材はtracked fixtureと各authoring sidecarに保全済みで、消失はない。旧technical passは変更しない。
- 3Imageとsnapshot Textへfake userを明示し、safe Blender（no-network／autoexec無効）で保存済みblendを再openして
  exact Image名・重複・寸法・保持flag・packed dataのbytes/SHA・snapshot Textを読む検査を追加する。
  外部PNGやdecode済pixelで代用せず、reload／unpack／saveをしない。呼出し前後のblend／素材／inspectorと
  外側のtoolchain／report／出力inventoryを照合する。新inspectorもsource指紋に含める。
- focused回帰2件はpacked欠落／別bytes／重複／寸法／保持／snapshot改変、外部fileだけの偽陽性と、
  readback中のblend・素材・script変異、欠損／重複receipt・nonzeroを拒否する。
  実保存→再openはfresh source生成で確認する。source SHA／identityが変わるためfresh包装・nativeも再取得する。

### 自然生成seedを束縛するDirt補足

保持修正後の2種g5（Mixer b941fe6a…／Rest f5f13a71…）で4つのfresh normal/far実機を独立verify・retain・finalize済み。
M2 contextも成功したが、既定seedの固定anchorは全てGrassだった。Dirtを確認済みにしない。
`building_art_acceptance.py --world-seed`を追加し、非既定値はMudMixerの明示feedback M2 contextだけに限定する。
厳密uint64、plan→argv `--perf-seed`→child env→実PERFログ／summary.csv seedを結合し、両raw hashも結果へ固定する。
旧planは従来seedを維持。地形の書換え、Rust変更、任意command許可、art／性能authorityは追加しない。
別seedがDirtとなる保証はなく、合法supportと実画像のDirtは別に確認する。focused33件成功、採否は未完了。
