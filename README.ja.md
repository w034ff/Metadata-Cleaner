# Metadata Cleaner

[English](README.md)

Metadata Cleaner は、画像（JPEG、PNG、WebP）および PDF ファイルからメタデータを取り除くオープンソースのデスクトップアプリケーションです。
外部のオンラインサービスにファイルをアップロードすることなく、すべての処理がお使いの PC 上で完結します。対応 OS は Windows および Linux（x64）です。

## インストール

[Releases](https://github.com/w034ff/Metadata-Cleaner/releases) ページから、お使いの OS に合わせたインストーラーまたは実行ファイルをダウンロードしてください。

### Windows

- **インストーラー形式**: 通常は NSIS インストーラー（`.exe`）をおすすめします。MSI インストーラー（`.msi`）は組織内での一括配布を行う管理者向けです。両方をインストールしないでください。
- **SmartScreen の警告について**: 本アプリはコード署名を行っていないため、初回起動時やインストール時に Windows Defender SmartScreen の警告（「Windows によって PC が保護されました」）が表示される場合があります。その場合は「**詳細情報**」をクリックし、「**実行**」を選択して進めてください。
- **WebView2 ランタイム**: システムに Microsoft Edge WebView2 ランタイムがインストールされていない場合、インストーラーによって自動的に導入されます。
- **外部通信なし**: アプリ自体はネットワーク通信を行いません（テレメトリやアップデート確認もありません）。ただし Windows では、画面の表示に使う Microsoft Edge WebView2 ランタイムが、自身で Microsoft のサービスに接続することがあります。これはアプリから止められません。

### Linux

- **形式**: AppImage および Debian パッケージ（`.deb`）を用意しています。
- **動作要件**: WebKitGTK 4.1（`libwebkit2gtk-4.1-0` 等）が必要です（Ubuntu 22.04 以降対応）。
- **AppImage**:
  AppImage の実行には FUSE 2 が必要ですが、Ubuntu 22.04 以降には標準で入っていません。先にインストールしてください（Ubuntu 24.04 以降は `libfuse2t64`、22.04 は `libfuse2`）。
  ```bash
  sudo apt install libfuse2t64
  ```
  そのうえで、ダウンロードした AppImage に実行権限を付与して起動してください。
  ```bash
  chmod +x Metadata*Cleaner_*_amd64.AppImage
  ./Metadata*Cleaner_*_amd64.AppImage
  ```
- **.deb パッケージ**:
  `apt` を使って依存関係を含めてインストールします。
  ```bash
  sudo apt install ./Metadata*Cleaner_*_amd64.deb
  ```

## 消すもの・残すもの・消せないもの

### 消すもの

- **画像（JPEG / PNG / WebP）**:
  - EXIF（位置情報、撮影・更新日時、カメラ・機器の機種やシリアル番号、メーカーノート、埋め込みサムネイルなど）
  - XMP
  - IPTC および Photoshop 情報
  - コメント（JPEG の COM、PNG の `tEXt` / `zTXt` / `iTXt`）
  - PNG の `tIME`
  - JPEG の EOI より後ろのデータ（モーションフォトの動画、MPF の 2 枚目以降の画像など）
  - そのほか見た目に関わらない付加情報
- **PDF**:
  - 文書情報（タイトル、作成者、件名、キーワード、作成・更新日時、作成ソフト、PDF 作成ソフトなどの `/Info`）
  - 文書の識別子（`/ID`）
  - 文書・ページ・オブジェクトの XMP メタデータ（`/Metadata`）
  - アプリケーション固有情報（`/PieceInfo`）
  - 埋め込みサムネイル（`/Thumb`）
  - 追記保存で残った以前の版（過去の版）
  - `DCTDecode` フィルターのみが使われている画像（JPEG）に含まれる EXIF・XMP など

### 残すもの

見た目や画質を保つため、次を残します:

- **画像の向き**: EXIF の向き（Orientation。向きと解像度のみを残した EXIF を書き出します）
- **色のプロファイル**: 埋め込まれた ICC プロファイル
- **表示や色の再現に必要な情報**:
  - JPEG: Adobe APP14（CMYK / YCCK の色変換に必要）
  - PNG: `gAMA`、`cHRM`、`sRGB`、`cICP`、`mDCV`、`cLLI`、`sBIT`、`bKGD`、`pHYs`、`acTL`、`fcTL`、`fdAT`、`tRNS` など
  - WebP: `VP8 `、`VP8L`、`VP8X`、`ALPH`、`ANIM`、`ANMF`
  - 印刷解像度・密度: JFIF の密度、PNG の `pHYs`、EXIF の解像度
- **画像の画素データ**: JPEG の圧縮データ、PNG の `IDAT`、WebP の画像データは再エンコードせず、元のバイト列のまま保持します。

### 消せないもの

- **見た目に現れている情報**: 画素やページの内容そのもの（写真に写り込んだ景色や文字・人物、透かし、PDF の本文）
- **ファイル名**: ファイル名に含まれる情報
- **PDF 内の消さない項目**:
  - 注釈の作成者名（`/T`）
  - フォームの値
  - 添付ファイル
  - JavaScript
  - しおりの文字
  - `DCTDecode` 以外のフィルター（`FlateDecode` など）が使われている JPEG、および JPEG 2000 内の XMP
- **対象外のファイル・形式**:
  - 暗号化（閲覧パスワードや権限制限）された PDF、および電子署名付きの PDF は処理できません（暗号化や署名を壊さないため、対象外としてエラーを表示します）。
  - JPEG、PNG、WebP、PDF 以外の形式（HEIC、AVIF、TIFF、BMP、GIF、Office 文書、動画、音声など）は対象外です。

## ソースコードからのビルド

### 必要な環境

- **Rust**: `rust-toolchain.toml` で指定されているツールチェーン
- **Node.js**: `.nvmrc` で指定されているバージョン（v24、または `package.json` の要件 `>=24.15.0`）
- **npm**: 11 以上
- **Linux 依存パッケージ**（Linux 環境でビルドする場合）:
  ```bash
  sudo apt-get update
  sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libxdo-dev \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev
  ```
- なお、本リポジトリの `.npmrc` ではセキュリティ確保のため、公開後 7 日未満のバージョンを導入しない設定（`min-release-age=7`）と、インストール時スクリプトを実行しない設定（`ignore-scripts=true`）が有効になっています。

### 手順

1. 依存関係のインストール:
   ```bash
   npm ci
   ```
2. 開発モードでの起動:
   ```bash
   npm run tauri dev
   ```
3. リリース用パッケージのビルド:
   ```bash
   npm run tauri build
   ```
   ビルド成果物は `target/release/bundle/` 配下に生成されます。

## ライセンス

本ソフトウェアは [MIT License](LICENSE) のもとで公開されています。

使用している第三者ライブラリのライセンス一覧は、アプリ内の上部バーにある「このアプリについて」ボタンから確認できます。
