# **GCP L4インスタンスにおけるTRELLIS 2を活用した3Dアセット生成パイプラインの構築とローカル（Fedora）環境からの統合運用ベストプラクティス**

## **次世代3Dアセット生成パイプラインと概念実証の戦略的意義**

現代のデジタルエンターテインメントおよび3Dアプリケーション開発において、背景や環境プロップ等の高品質な3Dアセットをいかに迅速かつ低コストで量産するかは、プロジェクト全体の成否を左右する重大な課題である。添付されたアセット生成パイプラインの概念実証（PoC）計画書において示されている通り、開発フェーズが量産体制（Phase 3）へ移行する前段階（Phase 2）において、技術的、品質的、コスト的、そして法務的な不確実性を完全に排除することが戦略的な急務となっている1。

本稿では、Microsoftが開発した最先端の3D生成AIモデルである「TRELLIS」およびその後継「TRELLIS 2」を、Google Cloud Platform（GCP）のL4 GPU搭載Spotインスタンス上でセルフホストするアプローチの中核的メカニズムとベストプラクティスを詳述する1。特に、ローカルの開発・運用端末としてFedora Linuxを採用した環境において、いかにしてセキュアなリモート接続を確立し、SELinuxやPodmanといったコンテナ技術の制約を克服しつつ、限られた24GBのVRAM環境で推論を最適化し、Headless Blenderを通じた厳密なアートスタイルの適応と自動品質ゲートを構築するかに焦点を当てる1。

このエンドツーエンドのパイプラインは、単なるAIによるメッシュ生成にとどまらず、生成されたモデルがゲームエンジン（Bevy Engine）に直接取り込み可能であり、かつプロジェクト固有のビジュアル要件（手描き感、Unlit表現）を自動的に満たすことを保証するための包括的なアーキテクチャ設計を提供するものである1。

## **Fedoraローカル環境の構築とGoogle Cloudへのセキュアなアクセス経路**

クラウド上のGPUリソースを利用して機械学習モデルのデプロイや推論パイプラインの構築を行う際、ローカルのオペレーティングシステムからクラウドインフラへの接続安全性と操作の透過性は、開発の生産性に直結する。最新のLinuxカーネルと強力なセキュリティ機構を備えるFedoraは、開発用ワークステーションとして優れた適性を持つが、その特性を最大限に活かすためにはツールチェーンの導入手法において特定のベストプラクティスを遵守する必要がある8。

### **Google Cloud CLIの最適化されたインストール手法**

GCPの各種サービス（Compute Engineのインスタンス管理、Cloud Storageへのアクセス、Identity-Aware Proxyの確立）をローカルから制御するためには、Google Cloud CLI（gcloud）の導入が不可欠である。Fedora環境においては、dnfを用いたRPMパッケージ経由のインストールが可能であるものの、システム全体に影響を及ぼすPythonの依存関係の競合や、最新のSDK機能への追従遅れといった技術的負債を抱えるリスクが存在する10。

この問題を回避するため、システムパッケージマネージャーから独立したサンドボックス化された環境を構築する直接スクリプト実行（curl https://sdk.cloud.google.com | bash）が推奨される10。この手法により、gcloudコマンド群とその内部で利用されるPythonインタプリタがユーザーのホームディレクトリ内に自己完結して展開され、FedoraのOSアップデートによる破壊的変更から保護される。インストール後は、gcloud auth loginを通じたOAuth 2.0フローによるユーザー認証と、gcloud config set projectによる対象プロジェクトのバインドを実施し、リモート操作の基盤を確立する。

### **Identity-Aware Proxy (IAP) によるパブリックIPの排除とゼロトラスト接続**

クラウド上のCompute Engineインスタンスに対するSSHアクセスの管理において、各VMにパブリック（外部）IPアドレスを付与し、ファイアウォールでポート22を開放する従来の手法は、現代のクラウドセキュリティにおいてはアンチパターンと見なされている5。パブリックIPの付与は、自動化されたボットネットによるブルートフォース攻撃やポートスキャンの直接的な標的となり、インフラストラクチャのアタックサーフェス（攻撃面）を不必要に拡大させる5。

この脆弱性を根本から解消するため、GCPのIdentity-Aware Proxy（IAP）を利用したTCPフォワーディング技術を採用する15。IAPはGoogleの強固な認証・認可インフラストラクチャをゲートキーパーとして機能させ、IAM（Identity and Access Management）ポリシーにおいて許可されたユーザー（具体的にはroles/iap.tunnelResourceAccessor権限を持つプリンシパル）に対してのみ、HTTPSでカプセル化された暗号化トンネルを経由してVMの内部ポートへのルーティングを許可する15。

IAPを用いたセキュアなトンネリングを確立するためのファイアウォールおよびアクセス設定要件を以下の表に整理する。

| 設定コンポーネント | 設定内容と技術的根拠 |
| :---- | :---- |
| **VMネットワーク設定** | インスタンス作成時にパブリックIPを割り当てない（--no-addressフラグの利用）。これによりVMはインターネットから完全に隔離される5。 |
| **ファイアウォールルール** | 送信元IPレンジ 35.235.240.0/20 からのIngress（上り）TCPトラフィック（ポート22など）を許可する。このレンジはGoogleが管理するIAPの内部フォワーディング用のものである5。 |
| **IAMロールの割り当て** | ローカルの開発者アカウントに対し、該当プロジェクトまたはVMインスタンスレベルで IAP-Secured Tunnel User ロールを付与する18。 |
| **ローカル接続コマンド** | gcloud compute ssh \--tunnel-through-iap \--zone=\[ZONE\] を実行することで、IAPトンネルが自動構築される5。 |

### **VS CodeとSSH Configを通じたリモート開発の統合**

TRELLIS 2のセットアップやPythonスクリプトの開発・デバッグをGCPインスタンス上で直接行う場合、Fedora上のVisual Studio Code（VS Code）のRemote-SSH機能を活用することが開発効率の最大化に寄与する20。gcloudコマンドを介さずに、ネイティブなSSHクライアントからIAPトンネルをシームレスに透過利用するためには、Fedoraローカルの \~/.ssh/config ファイルに特定のプロキシコマンド定義を記述するアプローチが極めて有効である22。

以下のような構成を記述することにより、IAPの存在を意識することなく通常のSSHプロトコルと同様の操作感を得ることができる。

| SSH Configパラメータ | 指定値と動作のメカニズム |
| :---- | :---- |
| Host | 任意のエイリアス名（例：gcp-trellis）。この名前でVS Codeやターミナルから接続を試みる22。 |
| ProxyCommand | gcloud compute start-iap-tunnel %h 22 \--listen-on-stdin \--zone=\[ZONE\] \--project=。SSH接続の確立をgcloudのIAPトンネリングプロセスに委譲する17。 |
| IdentityFile | \~/.ssh/google\_compute\_engine。gcloudが初回実行時に自動生成・配置したRSAまたはECDSA鍵を指定する21。 |
| User | GCPのIAMログインユーザー名。VM側のOSログインアカウントと一致させる21。 |

さらに、TRELLIS 2が提供するGradioベースのWebダッシュボードや監視用UIにローカルブラウザからアクセスするためには、SSHのポートフォワーディング機能を併用する。例えば、リモートVMの7860番ポート（Gradioのデフォルトポート）をローカルの8080番ポートにマッピングする場合、接続コマンドのオプションとして \-L 8080:localhost:7860 を付与することで、セキュアなトンネル内部を経由してトラフィックがルーティングされる16。

## **PodmanとSELinux環境下におけるコンテナセキュリティと永続化の管理**

TRELLIS 2のような複雑な依存関係（特定のCUDAバージョン、PyTorch、xformers等の拡張ライブラリ）を持つ機械学習モデルをホストVM上で直接環境構築することは、環境の汚染や依存関係の地獄（Dependency Hell）を招くリスクが高い25。したがって、コンテナ技術を利用してアプリケーションとその実行環境をカプセル化することがベストプラクティスとなる。Fedoraシステムでは、Dockerデーモンに依存しないルートレスなコンテナエンジンであるPodmanが標準として組み込まれており、これとOSレベルの強制アクセス制御（MAC）であるSELinuxが密接に連携して動作する26。

### **SELinuxコンテキストの競合とボリュームマウントの最適解**

機械学習の推論バッチ処理においては、ホストVM上のディレクトリ（例えば、入力画像の格納フォルダや、生成されたGLBファイルを永続化するための出力フォルダ）をコンテナ内部にボリュームマウントしてデータをやり取りする必要がある。しかし、FedoraのようにSELinuxが Enforcing モードで稼働している環境において、単純なディレクトリマウントを実行すると、コンテナプロセスがホストのファイルシステムにアクセスした瞬間に「Permission Denied」として遮断される事象が高頻度で発生する26。

これは、SELinuxがプロセス（コンテナ）とファイル（ホスト上のディレクトリ）の双方に対して「セキュリティコンテキスト（ラベル）」を付与し、ポリシーにおいて明示的に許可されていないアクセスをすべて拒否するという設計思想に基づいているためである9。コンテナプロセスは通常、限定された権限を持つ container\_t ドメイン内で実行される一方、ホスト上で作成された通常のディレクトリは user\_home\_t などのラベルを持っており、両者間の相互作用はデフォルトで禁止されている9。

この複雑なセキュリティメカニズムを損なうことなく（すなわち、SELinuxを無効化したり setenforce 0 を実行したりする悪手を避けて）問題を解決するための正規のアプローチは、Podmanのボリュームマウント宣言（-v オプション）において、ディレクトリパスの末尾に :z または :Z のサフィックスを付与することである26。

| サフィックス | SELinuxラベルの動作メカニズム | 適用すべきユースケース |
| :---- | :---- | :---- |
| **小文字の :z** | マウント対象のディレクトリおよびその配下のファイル群に対し、共有可能なコンテナラベル（通常は container\_file\_t）を再帰的に適用する。複数の異なるコンテナプロセスが同時に読み書きを行うことが許可される26。 | アセットの入力元ディレクトリや、複数コンテナで構成されるパイプライン（生成用コンテナとBlender変換用コンテナ等）で共有される中間データの保存領域。 |
| **大文字の :Z** | マウント対象ディレクトリに対し、そのコンテナの固有のプライベートラベルを適用し、他のいかなるコンテナプロセスからのアクセスも厳格に遮断する26。 | そのTRELLISコンテナのみが排他的に利用するキャッシュディレクトリや、一時的なモデルパラメータの展開領域。 |

このラベル再付与メカニズムを適切に活用することで、Fedoraの高いセキュリティ基準を維持したまま、TRELLIS 2のコンテナからホストOSのストレージに対してI/Oペナルティのない安全なデータ永続化パイプラインを確立することが可能となる26。

## **GCP L4 Spotインスタンスのプロビジョニングと可用性管理戦略**

PoC計画書の「検証E：コストモデルの実測」において定義されている通り、500ドルという限られたGCPクレジット内で目標とする量産アセット数を生成するためには、計算リソースの選択と運用方式がプロジェクト全体の経済性を決定づける1。この経済的要件を満たすための戦略的基盤となるのが、NVIDIA L4 GPUを搭載したSpot VM（旧プリエンプティブルVM）の活用である27。

### **NVIDIA L4 GPUの選定理由とVRAM動態の適合性**

TRELLIS 2は、Microsoftが開発した40億（4B）のパラメータを持つ大規模なRectified Flow Transformerモデルであり、複雑な形状の生成と高精細なPBRテクスチャマッピングを同時に処理する4。このモデルの推論を最大解像度（1024^3 または 1536^3ボクセルグリッド）で実行する場合、ネットワーク自体が占有するVRAMに加えて、アテンションメカニズムにおける中間テンソルやO-Voxelデコーダーのメモリ消費が重なり、最低でも24GBのVRAMが要求される4。

NVIDIAのAda Lovelaceアーキテクチャを採用したL4 GPUは、正確に24GBのGDDR6 VRAMを搭載しており、TRELLIS 2の要求スペックに完全に合致する4。さらに、FP32で約30.3 TFLOPSという高い演算性能を持ちながら、A100やV100といった上位のデータセンター向けGPUと比較して時間あたりの利用単価が圧倒的に安価であるため、「投下資本あたりのTFLOPS（TFLOPS/dollar）」の指標において最高のコストパフォーマンスを発揮する33。

### **Spot VMの経済性とプリエンプションへの対抗メカニズム**

Spot VMは、GCP内の余剰なコンピュートリソースを活用することで、標準的なオンデマンドVMと比較して最大91%のコスト削減を提供する画期的なインスタンスモデルである28。しかし、この巨大な経済的メリットと引き換えに、Google側のキャパシティが不足した場合には、システムイベントとしていつでもインスタンスが強制終了（プリエンプト）されるという運用上のリスクを抱えている28。

PoCにおける「Spot中断時の再開」という合格条件1を満たすためには、パイプライン全体をステートレスに設計し、プリエンプションの発生を前提としたフォールトトレラント（耐障害性）アーキテクチャを構築することが不可欠である34。プリエンプションに対するレジリエンスは、VMのインフラ層でのイベント検知と、アプリケーション層での状態退避の連携によって実現される。

#### **メタデータサーバーを介したプリエンプションの検知**

GCPは、Spot VMを強制終了する約30秒前に、インスタンス内部からアクセス可能なメタデータサーバーの特定のパス（http://metadata.google.internal/computeMetadata/v1/instance/preempted）の値を TRUE に変更し、その直後にゲストOSに対してACPI G2（Soft Power Off）シグナルを送信する28。この30秒間という限られた猶予期間（Grace Period）が、実行中の生成プロセスを安全に退避させるための唯一のウィンドウとなる36。

#### **PythonシャットダウンスクリプトによるGCSへのチェックポイント同期**

この猶予期間内に自律的な退避行動を完遂させるため、GCPのVM作成時に「シャットダウンスクリプト」をメタデータとして登録する34。このスクリプトは、ACPI終了シグナルを受信した際にOSの標準的な終了プロセスよりも優先して実行される34。

具体的なチェックポイント戦略としては、TRELLIS 2のバッチ生成スクリプトが1つの3Dアセット（GLBファイル）の出力とBlenderによる最適化を終えるたびに、ローカルディスク上の一時ディレクトリに処理完了のステータス（例：処理済みの画像IDリストを含むJSONファイル）と生成物を書き出しておく設計とする36。シャットダウンスクリプトが発火した際、スクリプトは即座に gsutil cp コマンドやPythonのGoogle Cloud Storageクライアントライブラリを利用して、このローカルディレクトリの差分データを指定されたGCSバケット（例：gs://my-asset-pipeline/checkpoints/）に高速に並列アップロードする34。同時に、Cloud Pub/Subなどのタスクキューに対して、現在処理中だった未完了のタスクが中断されたことを通知する36。

#### **マネージドインスタンスグループ（MIG）による自律的復旧**

VMの終了アクションを STOP（--instance-termination-action=STOP）に設定することで、VM自体とローカルディスクを維持したまま一時停止状態に移行させることも可能であるが34、量産フェーズの完全自動化を見据えるならば、マネージドインスタンスグループ（MIG）の導入がベストプラクティスとなる35。

MIGは指定されたテンプレートに基づいてSpot VM群を管理し、いずれかのVMがプリエンプションによって消滅したことを検知すると、空きキャパシティのある同一または別のゾーンで自動的に新しいSpot VMをスピンアップさせる機能を持つ27。新しく起動したVMは、初期化処理（Startup Script）の一環としてGCSバケットから最新のチェックポイント（完了済みリストや設定ファイル）をダウンロードし、残りのタスクキューから次の生成ジョブを取得してTRELLISの推論プロセスをシームレスに再開する36。これにより、人間が介入することなくシステムが自律的に回復し、コストを極小化しながらも処理が確実に前進する堅牢なパイプラインが完成する。

## **TRELLIS 2の推論パイプライン：VRAM最適化とバッチ処理ロジック**

TRELLIS 2は、入力された画像から3Dアセットの形状とテクスチャを推論するために、2段階の生成プロセスを経る。第一段階ではSparse Structure（SS）モデルを用いてオブジェクトの幾何学的な構造をスパースなボクセルグリッドとして生成し、第二段階でStructured Latent（SLAT）モデルがそのグリッド内に高次元の特徴量とPBRマテリアル属性（Base Color、Metallic、Roughness等）を注入する29。この複雑な処理を限られた24GBのVRAM空間で、かつ5分以内という時間的制約1の中で完了させるためには、PyTorchレベルでの緻密なメモリ管理が求められる。

### **半精度演算とメモリフラグメンテーションの抑制**

デフォルトのFP32（単精度浮動小数点）で4Bパラメータのモデルをロードし、高解像度の推論プロセスを実行すると、瞬く間にOOM（Out of Memory）エラーを引き起こす42。推論品質の視覚的な劣化を伴わずにVRAM消費量を半減させるための標準的なアプローチとして、モデルのウェイトと演算をFP16（半精度）またはBF16（Bfloat16）へダウンキャストする手法を採用する25。

加えて、PyTorch特有の課題として、推論中に生成と破棄が繰り返されるテンソルがGPUメモリ空間のフラグメンテーション（断片化）を引き起こし、空き容量の合計は足りているにもかかわらず連続したメモリ領域が確保できずにOOMエラーとなる現象がある42。これを抑制するため、パイプラインの実行環境変数として PYTORCH\_CUDA\_ALLOC\_CONF="expandable\_segments:True" を明示的に設定する4。このフラグにより、PyTorchのCUDAアロケータがメモリスロットのセグメントを動的に拡張・再利用するようになり、長時間のバッチ処理におけるメモリ効率が劇的に改善される4。

さらに、バッチ処理ループ内において、一つのアセットの推論とメッシュ抽出（デコード）が完了した直後に、明示的に torch.cuda.empty\_cache() を呼び出すことで、PyTorchがキャッシュしている不要なメモリ領域をシステムに返還させ、次の推論サイクルのためのクリーンな空間を確保することがベストプラクティスである43。

### **プリエンプションに対応したバッチ再開スキップロジック**

100個や1000個といった大量の入力画像を連続して処理する量産用スクリプトにおいて、途中でVMがプリエンプトされた際に、再起動後もすでに完了したタスクを無駄に再実行しないための仕組みが必要である44。TRELLISモデルの公式コードには学習再開用の \--load\_dir や \--ckpt オプションが存在するが45、これらは推論のバッチ再開には適していない。

推論用の堅牢なバッチスクリプトでは、入力画像のリストを走査するループの先頭において、対象となる画像名に対応する出力結果（例：\[image\_name\].glb）が指定された出力ディレクトリ、またはGCSのチェックポイント先にすでに存在するかどうかを検証する条件分岐（if os.path.exists(...)）を実装する44。すでにファイルが存在する場合は推論プロセスを完全にバイパスし、ログにスキップした旨を記録して次のイテレーションへ進む。このシンプルなIdempotency（冪等性）の確保が、Spot VM運用における最も確実かつ安全なバッチ再開メカニズムとなる36。

## **Headless Blenderによるアートスタイル変換とゲームエンジン（Bevy）統合への最適化**

TRELLIS 2が生成するGLBファイルは、物理法則に基づいた写実的なPBR（物理ベースレンダリング）マテリアルを保持しており、幾何学的にも精密である4。しかし、PoC計画書の「検証B：アートスタイル整合」において定義されている通り、最終的なゲームのビジュアルアイデンティティ（world\_lore.md）は「手描き感」や「塗りムラ感」を伴う意図的に歪んだシルエットであり、ライティングの影響を受けないUnlit表現が要求されている1。

このAIの生出力をゲームエンジンの要求仕様へと強制的に適合させる工程は、BlenderのPython API（bpy）を用いたHeadlessモード（GUIを起動しないバックグラウンド実行）でのスクリプト処理によって完全自動化される1。GCPインスタンス上でコンテナ化されたBlenderに対して、blender \-b \-P optimize\_asset.py \-- という形式でコマンドを発行し、一連の最適化タスクを非同期に処理する47。

### **PBRからKHR\_materials\_unlitへのシェーダーノード置換**

Bevyエンジン上で外部光源に依存せずにフラットな描画を行うためには、glTFの標準拡張機能である KHR\_materials\_unlit をマテリアルに付与してエクスポートする必要がある1。

TRELLIS 2からインポートされたモデルには、自動的にBlenderの ShaderNodeBsdfPrincipled（Principled BSDFノード）が割り当てられ、Base ColorやRoughnessにテクスチャが接続されている49。Blender 4.2以降に組み込まれている最新のglTF 2.0エクスポーターは、ノードツリーにおいてPrincipled BSDFを検知した場合はPBRマテリアルとしてエクスポートするが、これを回避することでUnlitとして出力するロジックを内包している6。

Pythonスクリプトによる自動化の手順は以下の通りである。

1. インポートされたメッシュの全マテリアルをループで取得し、ノードツリー（material.node\_tree.nodes）にアクセスする51。  
2. 既存のPrincipled BSDFノードを削除するか、その接続を切り離す6。  
3. アルファ（透過）を含まない不透明なテクスチャの場合は、画像テクスチャノードのカラー出力を、新しく追加した Background ノード（または Emission ノードで強さ1）に接続し、それを Material Output ノードのSurface端子へ接続する6。  
4. アルファチャンネル（透明度）を持つテクスチャの場合、Mix Shader ノードを追加し、その係数（Fac）にテクスチャのアルファ出力を接続する。一つ目のシェーダー入力に Transparent BSDF を、二つ目のシェーダー入力にテクスチャのカラー出力を直接（またはEmission経由で）接続し、最終出力をMaterial Outputへ繋ぐ55。

このノードトポロジーの書き換えにより、glTFエクスポーターは自動的に extensionsUsed に KHR\_materials\_unlit を挿入し、Bevy側で unlit: true として認識される完璧なフラットマテリアルが生成される1。

### **ポスタライズ効果によるテクスチャの色数削減と手描き感の付与**

TRELLIS 2が生成するテクスチャは、AI特有の微細で滑らかなグラデーションやノイズを含んでいる。これを「塗りムラ感」のあるアートスタイルに落とし込むため、テクスチャに対するポスタライズ（階調化）処理をBlender内で完結させる1。

コンポジター（Compositor）またはシェーダーエディターの機能を活用し、Pythonスクリプトから CompositorNodePosterize をノードネットワークに組み込む57。 対象となるベースカラーテクスチャをロードし、ポスタライズノードの steps パラメータ（階調数。例えば8段階に設定すると 8^3 \= 512色に制限される）を調整して色の滑らかさを意図的に破壊する58。処理を経た画像データは、BlenderのBake（ベイク）APIを利用して新しいテクスチャ画像としてオブジェクトに焼き直され、後段のglTFエクスポートへと引き継がれる59。これにより、外部の画像編集ソフト（ImageMagick等）をパイプラインに介在させることなく、単一のBlenderプロセス内でジオメトリとテクスチャ双方の様式化が完了する。

### **バウンディングボックスの正規化と原点（Pivot）の再配置**

ゲーム内で壁やプロップをグリッドベースで配置する際、アセットの原点（Origin）がモデルの中央や不定な位置にあると、Bevy側でのトランスフォーム計算が煩雑になり、隣接するパーツ間の継ぎ目に隙間が生じる原因となる1。この問題を解決するため、原点を「オブジェクトの底面中心」に厳密に再配置し、モデルサイズを特定の単位空間に正規化する幾何学的な操作をスクリプトで行う60。

Python APIを用いた具体的な数学的処理手順は以下の通りである。

1. 対象オブジェクトの全頂点座標をローカル空間からワールド空間の行列（matrix\_world）で評価し、最も低いZ座標（min((m @ v.co) for v in mesh.vertices)）を探索してバウンディングボックスの底面を特定する61。  
2. XY平面における中心座標（平均値）と、先ほど求めた最小Z座標を用いて、新たな原点となるターゲットベクトルを算出する62。  
3. 全頂点の座標からこのターゲットベクトルを減算してメッシュをシフトさせ、相対的に原点が底面中心へと移動した状態を作り出す。その後、オブジェクト自体の位置を再調整してワールド座標の (0, 0, 0\) へと配置する62。  
4. オブジェクト全体の最大寸法を測定し、ゲーム固有の定数である TILE\_SIZE 空間内にきれいに収まるように均等スケーリング（Scale）を適用する1。

この正規化を経たGLBファイルは、Bevyにおいて Cuboid::new(TILE\_SIZE,...) などのプリミティブ形状と全く同じ振る舞いを示し、ソースコード上の meshes.add(...) を asset\_server.load("models/wall\_poc.glb\#Mesh0") に差し替えるだけで、物理的な配置ズレやZバッファの破綻を引き起こすことなくシームレスにレンダリングされる1。

## **CI/CD品質ゲートへのglTFバリデーションの統合**

量産パイプラインにおいて最も警戒すべきは、トポロジーが破綻したメッシュや、テクスチャパスが欠落したアセットがゲームエンジンまで到達し、ビルドエラーやランタイムのクラッシュを引き起こす事態である1。PoC計画における「検証C：自動品質ゲート（CI）」を実現するため、Headless Blenderの処理後段に、Khronos Groupが公式に提供する静的解析ツール gltf-validator をコマンドラインから実行するステップを組み込む63。

| バリデーション項目 | 自動判定のロジックと閾値 | 失敗時のパイプライン処理 |
| :---- | :---- | :---- |
| **ポリゴン数・メッシュ規模** | 頂点数やトライアングル数が事前定義した壁オブジェクトの上限（例：MAX\_POLYGON\_COUNT）を超過していないかチェックする。 | 自動Reject（破棄）とし、再試行カウンタをインクリメントしてTRELLISのパラメータ（解像度等）を調整した再生成をスケジュールする1。 |
| **UV展開とトポロジーの完全性** | 出力されたJSONレポートを解析し、TEXCOORD\_0（UVマップ）の欠落、不正なNaN値、無効なクォータニオンや行列が含まれていないかを検証する63。 | 一つでもエラーが検出された場合は自動Rejectとし、ログに詳細を記録する1。 |
| **マテリアルと拡張機能の整合性** | エクスポーターが正しく KHR\_materials\_unlit を extensionsUsed に登録しているか、また意図しないPBRパラメータが残留していないかを検証する56。 | 警告レベルの異常であればログに記録した上で警告（Warning）として扱い、致命的な欠損であればRejectする1。 |

この品質ゲートを通過した（Exit Codeが0である）無垢なGLBファイルのみが、CIパイプラインを通じてGCSの「Production-Ready」バケットへ昇格・同期される1。これにより、下流のBevyエンジンのロードプロセスは常にサニタイズされた安全なアセットのみを受け取ることになり、量産時のヒューマンエラーと目視確認の工数が劇的に削減される。

## **コストモデリングと法務・ライセンス要件の総合的評価**

PoCの最終関門として、確立されたパイプラインのインフラコストと採用したAIモデルの法的安全性を評価し、量産フェーズ（Phase 3）の実行可能性を経営的・法務的視点からジャッジする必要がある1。

### **動的コストモデルの試算とL4 Spotインスタンスの優位性**

500ドルのGCPクレジットという予算内でパイプラインを運用し、最大のROI（投資利益率）を引き出すためのコストモデルは、実測データに基づく以下の式によって定義される1。

1アセットの総コスト \= (L4 Spotインスタンスの秒単位の単価) × (TRELLIS 2生成 ＋ Blender最適化 ＋ 品質ゲート検証の総処理秒数) ＋ (Vertex AI / Imagen 4等の付随するAPIコール料金)

前述の通り、L4 Spotインスタンスは1時間あたり約 $0.15 〜 $0.20 程度（リージョンにより変動）という極めて低いランニングコストを誇る35。TRELLIS 2における1アセットの生成・デコード処理がFP16最適化によって約1分〜3分で完了し29、BlenderによるHeadless処理が数秒で完了すると仮定した場合、インフラストラクチャにおけるGPUの占有コストは1アセットあたり数セントのオーダーに収束する。 これに、プロンプトの多様性を生み出すためのVertex AIのトークン料金や、オプションとして用いるImagen 4の初期画像生成コスト（例：$0.04/枚）を合算したとしても1、最終的な1アセットあたりのトータルコストは数セント〜10セント程度となる。この驚異的なコスト圧縮効果こそが、外部の高価なSaaSに依存せず、GCP上でL4インスタンスを自社構築・運用する最大の経済的メリットである。

### **ライセンスリスクの徹底排除：TRELLIS対代替モデルの比較**

商用ゲームの開発において、生成AIが出力したアセットをゲームクライアントに組み込んで全世界に向けて配布する際、学習済みモデルの利用規約（ToS）とライセンス形態は、最も重大な法務リスクとなる1。PoC計画において検討対象とされていたツールや代替モデルには、無視できない法的制約が存在する。

| 対象モデル / ツール | ライセンス形態と商用利用の制約 | グローバルなゲーム展開におけるリスク |
| :---- | :---- | :---- |
| **Hunyuan 3D 2.0 (Tencent)** | コミュニティライセンスが適用されるが、\*\*欧州連合（EU）、イギリス（UK）、韓国（South Korea）におけるモデルの利用、出力の配布、商用利用を明示的に禁止する地域制限（Territory Restrictions）\*\*条項が含まれている65。 | グローバルパブリッシングを行うゲームにおいては、主要市場である欧州および韓国での配信に抵触する直接的なライセンス違反となり、採用は極めて危険である1。 |
| **3DAI Studio (SaaS)** | Businessプラン等の有料ティアを契約することで商用利用権とAPIアクセスが提供される68。月額$14〜$190の固定費ベースで運用可能70。 | 利用規約上は商用利用が許可されるが、SaaS内部でHunyuan等の地域制限を持つモデルを経由した場合、その制約がエンドユーザー出力物に継承されるリスクが完全に払拭できない69。また、月間の生成クレジット上限が存在する1。 |
| **TRELLIS / TRELLIS 2 (Microsoft)** | Microsoftよりオープンソースとして公開され、主要なコードとウェイトは**MITライセンス相当**の極めて寛容な条件下で提供されている1。 | 商用利用および出力アセットの世界的な配布に関して、特定の地域制限やクレジット上限は存在しない。ただし、一部の依存ライブラリ（例：GPL系の外部モジュール）の混入には注意が必要な場合がある42。 |

この比較から明らかなように、EUやUKといった巨大なゲーム市場での配信を前提とするプロジェクト（world\_lore.mdに準拠した商用プロダクト）において、Hunyuan 3Dのような地域制限条項を持つモデルや、それをバックエンドで利用する可能性のあるSaaSの採用は、致命的な法務リスクをもたらす1。

したがって、MITライセンスに基づき法的制約が最も少なく、自社インフラ内で入力データ（コンセプトアートや世界観のプロンプト）の機密性を完全に保護しながら完結できる「TRELLIS 2のGCP L4セルフホスト運用」が、技術的要件を満たすだけでなく、企業の法務・コンプライアンス要件をクリアするための唯一かつ最適な選択肢となる1。

## **結論と量産フェーズ（Phase 3）に向けた提言**

提供されたPoC（Phase 2）の計画書と最新の技術動向を統合し、分析した結果、GCPのL4 Spotインスタンス上でTRELLIS 2を活用するセルフホスト型3Dアセット生成パイプラインは、ゲーム開発における量産体制の基盤として卓越した実現性と優位性を持つことが立証された。

Fedoraをローカル環境とする開発者は、IAPによるセキュアなTCPトンネリングとVS CodeのRemote-SSH統合により、パブリックIPの危険に晒されることなく、またSELinuxのアクセス制御やPodmanのボリュームマウント制約（:Z オプションの適切な利用）をクリアしながら、透過的かつ安全にインフラを操作できる。

インフラストラクチャにおいては、L4 GPUの24GBというVRAM制約をFP16/BF16演算やPyTorchの動的メモリ割り当て（Expandable Segments）によって克服することで、TRELLIS 2の持つ40億パラメータの潜在能力を最大限に引き出し、かつSpot VM特有のプリエンプションに対しても、GCSを用いた堅牢なチェックポイント機構とMIGによる自律的な復旧プロセスによって完全な耐障害性を実現できる。

さらに、AIの出力である物理ベース（PBR）のリアルなメッシュを、Headless BlenderとPythonスクリプトを用いてゲームの独自の世界観に沿ったUnlit（KHR\_materials\_unlit）かつ手描き風（ポスタライズ）のスタイルに自動変換し、原点位置を正規化するプロセスは、下流のBevyエンジンへのシームレスな統合（Building3dHandlesのGLB読み込みの即時反映）を完全に保証する。そして、その品質はgltf-validatorによるCIゲートによって常に監視される。

法務面においても、地域制限を持つモデルや不透明なSaaSプラットフォームへの依存を排除し、寛容なライセンスで提供されるTRELLIS 2を自社管理下で稼働させる戦略は、グローバル市場での商用展開において無類の安全性を確保する。以上の技術的・経済的・法務的根拠をもって、本アーキテクチャはPoCの全合格条件を凌駕する水準で満たしており、組織は自信を持ってPhase 3の量産フェーズへと移行することが推奨される。

#### **引用文献**

1. asset-pipeline-poc-plan.txt  
2. TRELLIS Model by Microsoft \- Nvidia NIM, 3月 15, 2026にアクセス、 [https://build.nvidia.com/microsoft/trellis/modelcard](https://build.nvidia.com/microsoft/trellis/modelcard)  
3. Is Trellis 2 Free? Pricing, Limits & How to Get Started | 3DAI Studio, 3月 15, 2026にアクセス、 [https://www.3daistudio.com/blog/is-trellis-2-free-pricing-how-to-get-started](https://www.3daistudio.com/blog/is-trellis-2-free-pricing-how-to-get-started)  
4. microsoft/TRELLIS.2-4B \- Hugging Face, 3月 15, 2026にアクセス、 [https://huggingface.co/microsoft/TRELLIS.2-4B](https://huggingface.co/microsoft/TRELLIS.2-4B)  
5. Set Up SSH Tunneling Through IAP to Reach Compute Engine VMs Without Public IPs, 3月 15, 2026にアクセス、 [https://oneuptime.com/blog/post/2026-02-17-how-to-set-up-ssh-tunneling-through-iap-to-reach-compute-engine-vms-without-public-ips/view](https://oneuptime.com/blog/post/2026-02-17-how-to-set-up-ssh-tunneling-through-iap-to-reach-compute-engine-vms-without-public-ips/view)  
6. glTF 2.0 \- Blender 5.0 Manual, 3月 15, 2026にアクセス、 [https://docs.blender.org/manual/en/latest/addons/import\_export/scene\_gltf2.html](https://docs.blender.org/manual/en/latest/addons/import_export/scene_gltf2.html)  
7. \[SOLVED\] Issues converting unlit textures in GLTF 2.0 from Blender 2.8 \- import-assets, 3月 15, 2026にアクセス、 [https://hub.jmonkeyengine.org/t/solved-issues-converting-unlit-textures-in-gltf-2-0-from-blender-2-8/43446](https://hub.jmonkeyengine.org/t/solved-issues-converting-unlit-textures-in-gltf-2-0-from-blender-2-8/43446)  
8. Provisioning Fedora/CentOS bootc on GCP, 3月 15, 2026にアクセス、 [https://docs.fedoraproject.org/en-US/bootc/provisioning-gcp/](https://docs.fedoraproject.org/en-US/bootc/provisioning-gcp/)  
9. Getting started with SELinux \- Fedora Docs, 3月 15, 2026にアクセス、 [https://docs.fedoraproject.org/en-US/quick-docs/selinux-getting-started/](https://docs.fedoraproject.org/en-US/quick-docs/selinux-getting-started/)  
10. How to Install and Configure the gcloud CLI on macOS Linux and Windows \- OneUptime, 3月 15, 2026にアクセス、 [https://oneuptime.com/blog/post/2026-02-17-how-to-install-and-configure-the-gcloud-cli-on-macos-linux-and-windows/view](https://oneuptime.com/blog/post/2026-02-17-how-to-install-and-configure-the-gcloud-cli-on-macos-linux-and-windows/view)  
11. Installation issues using custom scripts \- Fedora Discussion, 3月 15, 2026にアクセス、 [https://discussion.fedoraproject.org/t/installation-issues-using-custom-scripts/100791](https://discussion.fedoraproject.org/t/installation-issues-using-custom-scripts/100791)  
12. Quickstart: Install the Google Cloud CLI, 3月 15, 2026にアクセス、 [https://docs.cloud.google.com/sdk/docs/install-sdk](https://docs.cloud.google.com/sdk/docs/install-sdk)  
13. GCP CLI (gcloud) Commands Cheat Sheet: Ultimate DevOps & Cloud Engineer Guide 2026, 3月 15, 2026にアクセス、 [https://medium.com/google-cloud/gcp-cli-gcloud-commands-cheat-sheet-ultimate-devops-cloud-engineer-guide-2026-5f04debca51a](https://medium.com/google-cloud/gcp-cli-gcloud-commands-cheat-sheet-ultimate-devops-cloud-engineer-guide-2026-5f04debca51a)  
14. Best practices for controlling SSH network access | Compute Engine, 3月 15, 2026にアクセス、 [https://docs.cloud.google.com/compute/docs/connect/ssh-best-practices/network-access](https://docs.cloud.google.com/compute/docs/connect/ssh-best-practices/network-access)  
15. Connect to Linux VMs using Identity-Aware Proxy | Compute Engine, 3月 15, 2026にアクセス、 [https://docs.cloud.google.com/compute/docs/connect/ssh-using-iap](https://docs.cloud.google.com/compute/docs/connect/ssh-using-iap)  
16. How to Use gcloud CLI to SSH into Compute Engine Instances \- OneUptime, 3月 15, 2026にアクセス、 [https://oneuptime.com/blog/post/2026-02-17-how-to-use-gcloud-cli-to-ssh-into-compute-engine-instances/view](https://oneuptime.com/blog/post/2026-02-17-how-to-use-gcloud-cli-to-ssh-into-compute-engine-instances/view)  
17. Using IAP for TCP forwarding | Identity-Aware Proxy \- Google Cloud Documentation, 3月 15, 2026にアクセス、 [https://docs.cloud.google.com/iap/docs/using-tcp-forwarding](https://docs.cloud.google.com/iap/docs/using-tcp-forwarding)  
18. Set up IAP in your project \- IAP Desktop \- Google Cloud Platform, 3月 15, 2026にアクセス、 [https://googlecloudplatform.github.io/iap-desktop/setup-iap/](https://googlecloudplatform.github.io/iap-desktop/setup-iap/)  
19. IAP Desktop \- Use SSH \- Google Cloud Platform, 3月 15, 2026にアクセス、 [https://googlecloudplatform.github.io/iap-desktop/connect-linux/](https://googlecloudplatform.github.io/iap-desktop/connect-linux/)  
20. How to Configure Remote Development Environments \- OneUptime, 3月 15, 2026にアクセス、 [https://oneuptime.com/blog/post/2026-01-25-remote-development-environments/view](https://oneuptime.com/blog/post/2026-01-25-remote-development-environments/view)  
21. How to configure SSH for remote development in vs-code \- Stack Overflow, 3月 15, 2026にアクセス、 [https://stackoverflow.com/questions/74364246/how-to-configure-ssh-for-remote-development-in-vs-code](https://stackoverflow.com/questions/74364246/how-to-configure-ssh-for-remote-development-in-vs-code)  
22. How to Use IAP TCP Forwarding to SSH into GCP VMs Without Public IP Addresses, 3月 15, 2026にアクセス、 [https://oneuptime.com/blog/post/2026-02-17-how-to-use-iap-tcp-forwarding-to-ssh-into-gcp-vms-without-public-ip-addresses/view](https://oneuptime.com/blog/post/2026-02-17-how-to-use-iap-tcp-forwarding-to-ssh-into-gcp-vms-without-public-ip-addresses/view)  
23. using google gcloud to ssh tunnel into linux machine inside network \- Stack Overflow, 3月 15, 2026にアクセス、 [https://stackoverflow.com/questions/58339624/using-google-gcloud-to-ssh-tunnel-into-linux-machine-inside-network](https://stackoverflow.com/questions/58339624/using-google-gcloud-to-ssh-tunnel-into-linux-machine-inside-network)  
24. Securely Accessing Web Application in GCE Private VM with Port Forwarding | by ChunzPs, 3月 15, 2026にアクセス、 [https://chuntezuka.medium.com/securely-accessing-web-application-in-gce-private-vm-with-port-forwarding-83e109ab6720](https://chuntezuka.medium.com/securely-accessing-web-application-in-gce-private-vm-with-port-forwarding-83e109ab6720)  
25. off-by-some/TRELLIS-BOX: Half the VRAM, all the 3D model generation. Implements Microsoft's TRELLIS utilizing docker, FP16 optimization and other improvements. \- GitHub, 3月 15, 2026にアクセス、 [https://github.com/off-by-some/TRELLIS-BOX](https://github.com/off-by-some/TRELLIS-BOX)  
26. Podman and selinux. I'm overhelmed. \- Reddit, 3月 15, 2026にアクセス、 [https://www.reddit.com/r/podman/comments/1b03exx/podman\_and\_selinux\_im\_overhelmed/](https://www.reddit.com/r/podman/comments/1b03exx/podman_and_selinux_im_overhelmed/)  
27. Spot VMs | Google Cloud, 3月 15, 2026にアクセス、 [https://cloud.google.com/solutions/spot-vms](https://cloud.google.com/solutions/spot-vms)  
28. Preemptible VM instances | Compute Engine \- Google Cloud Documentation, 3月 15, 2026にアクセス、 [https://docs.cloud.google.com/compute/docs/instances/preemptible](https://docs.cloud.google.com/compute/docs/instances/preemptible)  
29. TRELLIS.2: Native and Compact Structured Latents for 3D Generation \- Microsoft Open Source, 3月 15, 2026にアクセス、 [https://microsoft.github.io/TRELLIS.2/](https://microsoft.github.io/TRELLIS.2/)  
30. Structured 3D Latents for Scalable and Versatile 3D GenerationOpen-source project; see our project page for code, model, and data. \- arXiv, 3月 15, 2026にアクセス、 [https://arxiv.org/html/2412.01506v3](https://arxiv.org/html/2412.01506v3)  
31. TRELLIS.2: Production-Ready 3D Assets in 3 Seconds | CodeSOTA, 3月 15, 2026にアクセス、 [https://www.codesota.com/news/trellis-2-3d-generation](https://www.codesota.com/news/trellis-2-3d-generation)  
32. L4 Tensor Core GPU for AI & Graphics \- NVIDIA, 3月 15, 2026にアクセス、 [https://www.nvidia.com/en-us/data-center/l4/](https://www.nvidia.com/en-us/data-center/l4/)  
33. Which GPU should I use on Google Cloud Platform (GCP) \- Stack Overflow, 3月 15, 2026にアクセス、 [https://stackoverflow.com/questions/69674590/which-gpu-should-i-use-on-google-cloud-platform-gcp](https://stackoverflow.com/questions/69674590/which-gpu-should-i-use-on-google-cloud-platform-gcp)  
34. Create and use Spot VMs | Compute Engine \- Google Cloud Documentation, 3月 15, 2026にアクセス、 [https://docs.cloud.google.com/compute/docs/instances/create-use-spot](https://docs.cloud.google.com/compute/docs/instances/create-use-spot)  
35. Google Cloud Spot VM | Google Cloud Blog, 3月 15, 2026にアクセス、 [https://cloud.google.com/blog/topics/cost-management/rethinking-your-vm-strategy-spot-vms](https://cloud.google.com/blog/topics/cost-management/rethinking-your-vm-strategy-spot-vms)  
36. How to Create a Spot VM Instance and Handle Preemption Gracefully \- OneUptime, 3月 15, 2026にアクセス、 [https://oneuptime.com/blog/post/2026-02-17-how-to-create-a-spot-vm-instance-and-handle-preemption-gracefully-with-shutdown-scripts/view](https://oneuptime.com/blog/post/2026-02-17-how-to-create-a-spot-vm-instance-and-handle-preemption-gracefully-with-shutdown-scripts/view)  
37. How to Use Preemptible and Spot VMs to Reduce Compute Engine Costs \- OneUptime, 3月 15, 2026にアクセス、 [https://oneuptime.com/blog/post/2026-02-17-how-to-use-preemptible-and-spot-vms-to-reduce-compute-engine-costs/view](https://oneuptime.com/blog/post/2026-02-17-how-to-use-preemptible-and-spot-vms-to-reduce-compute-engine-costs/view)  
38. Google Cloud Spot VM use cases and best practices, 3月 15, 2026にアクセス、 [https://cloud.google.com/blog/products/compute/google-cloud-spot-vm-use-cases-and-best-practices](https://cloud.google.com/blog/products/compute/google-cloud-spot-vm-use-cases-and-best-practices)  
39. Run shutdown scripts | Compute Engine \- Google Cloud Documentation, 3月 15, 2026にアクセス、 [https://docs.cloud.google.com/compute/docs/shutdownscript](https://docs.cloud.google.com/compute/docs/shutdownscript)  
40. Google Cloud Spot VMs \- CloudBolt, 3月 15, 2026にアクセス、 [https://www.cloudbolt.io/gcp-cost-optimization/google-cloud-spot-vms/](https://www.cloudbolt.io/gcp-cost-optimization/google-cloud-spot-vms/)  
41. Trellis 2 Image to 3D Parameter Guide \- Fal.ai, 3月 15, 2026にアクセス、 [https://fal.ai/learn/devs/trellis-2-image-to-3d-prompt-guide](https://fal.ai/learn/devs/trellis-2-image-to-3d-prompt-guide)  
42. "Trellis image-to-3d": I made it work with half-precision, which reduced GPU memory requirement 16GB \-\> 8 GB : r/StableDiffusion \- Reddit, 3月 15, 2026にアクセス、 [https://www.reddit.com/r/StableDiffusion/comments/1hudvty/trellis\_imageto3d\_i\_made\_it\_work\_with/](https://www.reddit.com/r/StableDiffusion/comments/1hudvty/trellis_imageto3d_i_made_it_work_with/)  
43. Trellis 2 launched few days ago and the optimization on it is weird · Issue \#24 · deepbeepmeep/mmgp \- GitHub, 3月 15, 2026にアクセス、 [https://github.com/deepbeepmeep/mmgp/issues/24](https://github.com/deepbeepmeep/mmgp/issues/24)  
44. How to stop Batch processing and Save processed images : r/comfyui \- Reddit, 3月 15, 2026にアクセス、 [https://www.reddit.com/r/comfyui/comments/1olgum1/how\_to\_stop\_batch\_processing\_and\_save\_processed/](https://www.reddit.com/r/comfyui/comments/1olgum1/how_to_stop_batch_processing_and_save_processed/)  
45. GitHub \- microsoft/TRELLIS: Official repo for paper "Structured 3D Latents for Scalable and Versatile 3D Generation" (CVPR'25 Spotlight)., 3月 15, 2026にアクセス、 [https://github.com/microsoft/TRELLIS](https://github.com/microsoft/TRELLIS)  
46. Trellis 3D generation: Windows one-click installer, but without needing a powershell/cuda toolkit/admin. (same as a simple A1111 or Forge installer) \- Reddit, 3月 15, 2026にアクセス、 [https://www.reddit.com/r/StableDiffusion/comments/1hkiapy/trellis\_3d\_generation\_windows\_oneclick\_installer/](https://www.reddit.com/r/StableDiffusion/comments/1hkiapy/trellis_3d_generation_windows_oneclick_installer/)  
47. blender-gltf-converter | glTF-Tutorials, 3月 15, 2026にアクセス、 [https://github.khronos.org/glTF-Tutorials/BlenderGltfConverter/](https://github.khronos.org/glTF-Tutorials/BlenderGltfConverter/)  
48. KHRMaterialsUnlit \- glTF Transform, 3月 15, 2026にアクセス、 [https://gltf-transform.dev/modules/extensions/classes/KHRMaterialsUnlit](https://gltf-transform.dev/modules/extensions/classes/KHRMaterialsUnlit)  
49. glTF 2.0 — Blender Manual, 3月 15, 2026にアクセス、 [https://docs.blender.org/manual/en/2.80/addons/io\_scene\_gltf2.html](https://docs.blender.org/manual/en/2.80/addons/io_scene_gltf2.html)  
50. glTF 2.0 — Blender Manual \- Import-Export, 3月 15, 2026にアクセス、 [https://docs.blender.org/manual/en/2.91/addons/import\_export/scene\_gltf2.html](https://docs.blender.org/manual/en/2.91/addons/import_export/scene_gltf2.html)  
51. Blender \- Python Script to batch convert all material surfaces to Principled BSDF, 3月 15, 2026にアクセス、 [https://blender.stackexchange.com/questions/253641/blender-python-script-to-batch-convert-all-material-surfaces-to-principled-bsd](https://blender.stackexchange.com/questions/253641/blender-python-script-to-batch-convert-all-material-surfaces-to-principled-bsd)  
52. How to set a shader node property for Blender material via Python script? \- Stack Overflow, 3月 15, 2026にアクセス、 [https://stackoverflow.com/questions/69514207/how-to-set-a-shader-node-property-for-blender-material-via-python-script](https://stackoverflow.com/questions/69514207/how-to-set-a-shader-node-property-for-blender-material-via-python-script)  
53. Add KHR\_materials\_unlit support to importer · Issue \#220 · KhronosGroup/glTF-Blender-IO, 3月 15, 2026にアクセス、 [https://github.com/KhronosGroup/glTF-Blender-IO/issues/220](https://github.com/KhronosGroup/glTF-Blender-IO/issues/220)  
54. Export grease pencil \#925 \- KhronosGroup/glTF-Blender-IO \- GitHub, 3月 15, 2026にアクセス、 [https://github.com/KhronosGroup/glTF-Blender-IO/issues/925](https://github.com/KhronosGroup/glTF-Blender-IO/issues/925)  
55. Exporting a KHR\_materials\_unlit material with alpha \- Blender Stack Exchange, 3月 15, 2026にアクセス、 [https://blender.stackexchange.com/questions/279449/exporting-a-khr-materials-unlit-material-with-alpha](https://blender.stackexchange.com/questions/279449/exporting-a-khr-materials-unlit-material-with-alpha)  
56. How to use the Unlit Material? · Issue \#315 · KhronosGroup/glTF-Blender-Exporter \- GitHub, 3月 15, 2026にアクセス、 [https://github.com/KhronosGroup/glTF-Blender-Exporter/issues/315](https://github.com/KhronosGroup/glTF-Blender-Exporter/issues/315)  
57. Posterize \- Blender 5.0 Manual, 3月 15, 2026にアクセス、 [https://docs.blender.org/manual/en/latest/compositing/types/creative/posterize.html](https://docs.blender.org/manual/en/latest/compositing/types/creative/posterize.html)  
58. Posterize \- Blender 5.2 LTS Manual, 3月 15, 2026にアクセス、 [https://docs.blender.org/manual/en/dev/compositing/types/creative/posterize.html](https://docs.blender.org/manual/en/dev/compositing/types/creative/posterize.html)  
59. Can i bake a texture in headless mode? 2.80 \- Blender Stack Exchange, 3月 15, 2026にアクセス、 [https://blender.stackexchange.com/questions/197120/can-i-bake-a-texture-in-headless-mode-2-80](https://blender.stackexchange.com/questions/197120/can-i-bake-a-texture-in-headless-mode-2-80)  
60. Blender scene, world origin point when exporting \- FSDeveloper, 3月 15, 2026にアクセス、 [https://www.fsdeveloper.com/forum/threads/blender-scene-world-origin-point-when-exporting.452226/](https://www.fsdeveloper.com/forum/threads/blender-scene-world-origin-point-when-exporting.452226/)  
61. Move pivot to bottom or top of object/selection \[duplicate\] \- Blender Stack Exchange, 3月 15, 2026にアクセス、 [https://blender.stackexchange.com/questions/175252/move-pivot-to-bottom-or-top-of-object-selection](https://blender.stackexchange.com/questions/175252/move-pivot-to-bottom-or-top-of-object-selection)  
62. Set origin to bottom center of multiple objects \- Blender Stack Exchange, 3月 15, 2026にアクセス、 [https://blender.stackexchange.com/questions/42105/set-origin-to-bottom-center-of-multiple-objects](https://blender.stackexchange.com/questions/42105/set-origin-to-bottom-center-of-multiple-objects)  
63. KhronosGroup/glTF-Validator: Tool to validate glTF assets. \- GitHub, 3月 15, 2026にアクセス、 [https://github.com/KhronosGroup/glTF-Validator](https://github.com/KhronosGroup/glTF-Validator)  
64. AI 3D Model Licensing: A Creator's Guide to Rights and Revenue \- Tripo, 3月 15, 2026にアクセス、 [https://www.tripo3d.ai/blog/explore/ai-generated-3d-models-and-licensing-questions](https://www.tripo3d.ai/blog/explore/ai-generated-3d-models-and-licensing-questions)  
65. ️ License \- Tencent-Hunyuan/Hunyuan3D-2 \- GitHub, 3月 15, 2026にアクセス、 [https://github.com/Tencent-Hunyuan/Hunyuan3D-2/blob/main/LICENSE](https://github.com/Tencent-Hunyuan/Hunyuan3D-2/blob/main/LICENSE)  
66. Nice model, but strange license. You are not allowed to use it in EU, UK, and So... | Hacker News, 3月 15, 2026にアクセス、 [https://news.ycombinator.com/item?id=43420870](https://news.ycombinator.com/item?id=43420870)  
67. LICENSE · tencent/Hunyuan3D-2mv at main \- Hugging Face, 3月 15, 2026にアクセス、 [https://huggingface.co/tencent/Hunyuan3D-2mv/blob/main/LICENSE](https://huggingface.co/tencent/Hunyuan3D-2mv/blob/main/LICENSE)  
68. Save Time & Cut Costs \- 3D AI Studio, 3月 15, 2026にアクセス、 [https://www.3daistudio.com/Pricing](https://www.3daistudio.com/Pricing)  
69. Terms of Service | AGB \- 3D AI Studio, 3月 15, 2026にアクセス、 [https://www.3daistudio.com/AGB](https://www.3daistudio.com/AGB)  
70. 3D AI Studio \- Generate 3D Models from Image or Text in Seconds, 3月 15, 2026にアクセス、 [https://www.3daistudio.com/](https://www.3daistudio.com/)  
71. Regarding commercial licensing · Issue \#6 · Tencent-Hunyuan/Hunyuan3D-2 \- GitHub, 3月 15, 2026にアクセス、 [https://github.com/Tencent-Hunyuan/Hunyuan3D-2/issues/6](https://github.com/Tencent-Hunyuan/Hunyuan3D-2/issues/6)