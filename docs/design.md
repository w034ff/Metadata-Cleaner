# 詳細設計書

対応する要件は [requirements.md](requirements.md) の ID（FR-xx / NFR-xx）で示す。画面の見た目は [mockup/project/](mockup/project/)（`BList.dc.html` と `BResult.dc.html`）を正とする。方式の根拠は [spike-report.md](spike-report.md) にある。

PDF Converter（https://github.com/w034ff/PDF-Converter）と同じ作りのところは、「PDF Converter 設計 §n と同じ」と書き、そのコードを写して使う。

## 1. 全体構成

```
┌──────────────── メインプロセス（Tauri アプリ） ────────────────┐
│  WebView（React + TypeScript）                                  │
│   ・一覧、詳細、進捗の表示。ファイルパスを扱わない（ID のみ）    │
│            │ invoke / event（IPC）                              │
│  Rust（src-tauri）                                              │
│   ・ダイアログ、D&D、パスと ID の表、設定、処理の実行管理        │
│   ・画像（JPEG / PNG / WebP）の調査と除去（crates/core を呼ぶ）  │
│   ・ファイルの書き込み、ワーカーの起動・監視・強制終了          │
│            │ 標準入出力（§5.1 のメッセージ）                    │
└────────────┼────────────────────────────────────────────────────┘
             │  1〜N 個
┌────────────┴──────── ワーカープロセス（同じ実行ファイル） ──────┐
│  `metadata-cleaner --pdf-worker` で起動。WebView を作らない      │
│   ・lopdf で PDF を読み、メタデータを調べる                      │
│   ・メタデータを除いた PDF を作って確かめ、バイト列で返す        │
│   ・ファイルを書かない。読むのは渡された PDF 1 つだけ             │
└──────────────────────────────────────────────────────────────────┘
```

- PDF はすべてワーカーで読む。lopdf が異常な入力でメモリを使い切ったりスタックをあふれさせたりしても、終了するのはワーカーだけで、アプリは動き続ける（NFR-01、スパイク §4）。無限ループとメモリの使いすぎも、時間とメモリの上限（§5.3）でワーカーごと止める。
- 画像は、自前の小さな読み取り（§4）で扱い、メインプロセスで処理する。
- メタデータの処理は Tauri に依存しない crate に置く。画像は `crates/core`、PDF は `crates/worker`。WebKitGTK なしに `cargo test` できる。
- フロントエンドはファイルパスを Rust に渡さない。ダイアログと D&D は Rust で受け、パスは Rust の表に登録して ID だけを返す（PDF Converter 設計 §1 と同じ）。
- pdfium は使わない。OS ごとのネイティブのライブラリを同梱しない。

## 2. 技術選定

| 領域 | 採用 | 理由 |
| --- | --- | --- |
| アプリ基盤 | Tauri 2 | 要件で決定済み |
| 画像の読み書き | 自前（`crates/core` の `jpeg.rs`・`png.rs`・`webp.rs`） | スパイク §2.1。img-parts は EOI の後ろとスキャンの間を見落とす |
| EXIF の読み取り | `kamadak-exif` | スパイクで決定。種類の分類と向きの取得 |
| PDF の読み書き | `lopdf`（既定の機能を切る） | スパイクで決定。純 Rust、過去の版を残さずに書き直せる |
| 並列処理 | `std::thread` とチャネル | PDF Converter と同じ |
| 一時ファイル | `tempfile` | 原子的な保存（§6.5） |
| IPC 型共有 | `ts-rs` | PDF Converter と同じ |
| フロントエンド | React + TypeScript + Vite、素の CSS + CSS カスタムプロパティ | PDF Converter と同じ。部品と仕組みを写して使う |
| フロントエンドのテスト | Vitest + Testing Library + jsdom、`@tauri-apps/api/mocks` | PDF Converter と同じ |
| テストでの PDF の描画 | `hayro`（dev-dependencies のみ） | 除去の前と後の描画が同じかを比べる（§11.2）。純 Rust なので CI で何も取得しなくてよい。暗号化された PDF は扱わない（§4.6 で断る）ので、hayro の制約に当たらない |
| ライセンス | `cargo-about`、`cargo-deny`、`license-checker-rseidelsohn` | NFR-05 |

- ライブラリは実装開始時点の最新安定版とし、`Cargo.lock` と `package-lock.json` をコミットして固定する。スパイクでは lopdf 0.45.0、kamadak-exif 0.6.1 を使った。
- lopdf は `default-features = false` にする。既定の `chrono-clock` と `rayon` は使わない（日時を書かず、並列は §5.2 のプロセスで行うため）。
- Tauri のプラグインは `tauri-plugin-dialog` だけを使い、Rust 側からだけ呼ぶ（§9）。
- 表にない小さな依存: `crates/core` は、書き直した `eXIf` のチャンク（§4.3）の CRC に `crc32fast` を使う。フィクスチャの生成は、暗号化された PDF の鍵に `md-5`（dev-dependencies）を使う（§11.1、PDF Converter と同じ）。

## 3. ディレクトリ構成

```
/
├── crates/core/              画像の処理（Tauri 非依存）。パッケージ名は mcleaner-core
│   ├── src/
│   │   ├── lib.rs
│   │   ├── detect.rs         中身の先頭バイトによる形式の判定（§4.1）
│   │   ├── jpeg.rs           JPEG のセグメントの読み書き（§4.2）
│   │   ├── png.rs            PNG のチャンクの読み書き（§4.3）
│   │   ├── webp.rs           WebP（RIFF）のチャンクの読み書き（§4.4）
│   │   ├── exif.rs           EXIF の分類と詳細、残す情報だけの EXIF（§4.5）
│   │   ├── xmp.rs            XMP の分類（§4.5）
│   │   ├── iptc.rs           IPTC の分類と詳細（§4.5）
│   │   ├── report.rs         見つかった情報の型（§4.7）
│   │   ├── image_file.rs     画像 1 つの調査・詳細・除去（§4.2〜§4.5 をつなぐ）
│   │   ├── naming.rs         出力名の決定（§6.4。PDF Converter の naming.rs を写す）
│   │   └── error.rs
│   ├── examples/gen_fixtures.rs   フィクスチャの生成（§11.1）
│   └── tests/
├── crates/worker/            PDF の処理とワーカー（lopdf を使う唯一の crate）。パッケージ名は mcleaner-worker
│   └── src/
│       ├── pdf.rs            PDF の調査と除去（§4.6）
│       ├── protocol.rs       メッセージの型と読み書き（§5.1）
│       ├── server.rs         ワーカーの本体
│       ├── client.rs         メインプロセスの側の 1 つのワーカー
│       ├── windows_job.rs    Windows のジョブオブジェクト（§5.3。PDF Converter から写す）
│       └── main.rs           テスト用の単独のワーカー（`mcleaner-worker`）
├── src-tauri/
│   ├── src/
│   │   ├── main.rs           `--pdf-worker` なら worker を実行、それ以外はアプリ
│   │   ├── commands.rs       IPC コマンド（§7）
│   │   ├── items.rs          ファイルの ID の表と一覧の項目
│   │   ├── worker_pool.rs    ワーカーの数の管理と貸し出し（§5.2）
│   │   ├── jobs.rs           処理の実行・進捗・キャンセル（§6）
│   │   └── settings.rs       設定の読み書き（§6.7）
│   ├── capabilities/main.json
│   └── tauri.conf.json
├── src/                      フロントエンド
│   ├── features/cleaner/     一覧、詳細、保存先の帯
│   ├── features/about/
│   ├── features/job/、features/items/、features/output/、features/settings/   PDF Converter から写す
│   ├── components/
│   ├── ipc/、ipc/generated/
│   ├── i18n/
│   ├── licenses/
│   └── styles/
├── scripts/                  generate-licenses.ts、exiftool-check.ts（§11.2）
├── docs/
├── about.toml / deny.toml
├── GEMINI.md
└── .github/workflows/        ci.yml / release.yml / audit.yml / dependency-review.yml
```

## 4. メタデータの処理

### 4.1 形式の判定（FR-01）

- 対象の拡張子は `jpg` `jpeg` `png` `webp` `pdf`（大文字小文字を区別しない）。定数 `SUPPORTED_EXTENSIONS` にまとめる。拡張子は、フォルダと D&D から追加するものを選ぶときだけに使う。
- 形式は中身の先頭バイトで判定する（JPEG は `FF D8 FF`、PNG は 8 バイトの署名、WebP は `RIFF....WEBP`、PDF は先頭 1024 バイト以内の `%PDF-`）。どれでもなければ `UnsupportedFormat`。
- 拡張子と中身が食い違うファイル（中身が WebP の `.jpg` など。Web から保存した画像によくある）は、中身の形式として処理し、名前はそのまま書き出す。中身は変えないので、書き出したファイルも元と同じく食い違ったままになる。表の「形式」の欄には中身の形式を出す。
- ファイルの大きさの上限: 画像は `MAX_IMAGE_FILE_BYTES`（256 MiB）、PDF は `MAX_PDF_FILE_BYTES`（512 MiB）。超えたら `TooLarge`。画像はメインプロセスで入力と出力の両方をメモリに持つので、PDF より小さくする。PDF の上限は、ワーカーのメモリの上限（§5.3、2 GiB）の中で lopdf が読み込めるように決めた。

### 4.2 JPEG（FR-03）

スパイクの `spike/strip-bench/src/jpeg.rs` を元にする。

- SOI から読み、すべてのセグメントとスキャン（プログレッシブのスキャンを含む）を読み、EOI で止まる。EOI がない、長さが合わない、マーカーの位置に `FF` がないファイルは `DecodeFailed`。セグメントの前の詰め物の `FF` は読み飛ばす。
- EOI より後ろのバイト列（モーションフォトの動画、MPF の 2 枚目以降の画像など）は書き出さない。
- 残すセグメント（それ以外の APPn と COM は消す。APPn・COM 以外のセグメントはすべて残す）:

| セグメント | 扱い |
| --- | --- |
| APP0 `JFIF\0` | 先頭 14 バイト（版、密度の単位、密度）だけを残し、サムネイルの大きさを 0 にする |
| APP0 `JFXX\0` | 消す（JFIF の拡張のサムネイル） |
| APP1 `Exif\0\0` | 消して、残す情報だけの EXIF（§4.5）に置き換える。残す情報がなければ書かない。最初の 1 つだけを見る |
| APP2 `ICC_PROFILE\0` | そのまま残す（色のプロファイル） |
| APP14 `Adobe` | そのまま残す（CMYK と YCCK の色の変換に要る） |

- 残すセグメントは、元のバイト列をそのまま書き出す。
- `image_segments`（APPn・COM 以外と、残した ICC・Adobe）が、入力と出力で同じバイト列であることを、書き出す前に確かめる（§6.3）。

### 4.3 PNG（FR-03）

- 署名から IEND まで、チャンクを読む。長さが合わない、IEND がないファイルは `DecodeFailed`。CRC は確かめない（元のバイト列をそのまま写すので、書き出しで壊すことはない）。IEND の後ろは書き出さない。
- 残すチャンク: `IHDR` `PLTE` `IDAT` `IEND` `tRNS` `gAMA` `cHRM` `sRGB` `iCCP` `cICP` `mDCV` `cLLI` `sBIT` `bKGD` `pHYs` `acTL` `fcTL` `fdAT`。どれも画素、色、表示の大きさ、APNG のアニメーションに関わるもので、利用者を特定する情報を持たない。`pHYs` は印刷の大きさに関わるので残す（スパイクでは消していた）。
- `eXIf` は、残す情報だけの EXIF（§4.5）に置き換える。
- それ以外のチャンク（`tEXt` `zTXt` `iTXt` `tIME` `caBX` など、未知のチャンクを含む）は消す。
- 残すチャンクは、長さ・種類・データ・CRC の元のバイト列をそのまま書き出す。

### 4.4 WebP（FR-03）

- RIFF のヘッダーと、RIFF の大きさの範囲のチャンクを読む。範囲を超えるチャンクは `DecodeFailed`。RIFF の大きさより後ろは書き出さない。
- 残すチャンク: `VP8 ` `VP8L` `VP8X` `ALPH` `ANIM` `ANMF` `ICCP`。`EXIF` は残す情報だけの EXIF に置き換え、`XMP ` と未知のチャンクは消す。
- `VP8X` のフラグの XMP（`0x04`）を下ろし、EXIF（`0x08`）は EXIF を書くときだけ立てる。EXIF のチャンクは、画像のデータの後ろに置く（WebP の仕様の順序）。
- EXIF のチャンクの先頭に `Exif\0\0` が付いているものも受け付ける（付けるソフトがあるため）。書き出す EXIF には付けない。

### 4.5 EXIF・XMP・IPTC（FR-02、FR-03、FR-06）

**残す情報だけの EXIF**（`exif::kept_only`）

- 元の EXIF の IFD0 から、向き（`Orientation`）と解像度（`XResolution`・`YResolution`・`ResolutionUnit`）だけを写した、リトルエンディアンの TIFF を作る。向きが 1 のときは向きを書かない。解像度は 3 つがそろっているときだけ書く。どれも書かないなら EXIF ごと書かない。
- 解像度を残すのは、JFIF を持たない JPEG では EXIF の解像度だけが印刷の大きさを決めるため（PDF Converter 設計 §4.1 もこれを読む）。どちらも利用者を特定しない。
- EXIF を kamadak-exif で読めないときは、残す情報なしとして EXIF ごと消す（向きが失われうるが、読めない EXIF から何を残すかは決められないため）。

**見つかった情報の分類**（`MetadataKind`）

| 種類 | 表示（ja / en） | 何が当たるか |
| --- | --- | --- |
| `Location` | 位置情報 / Location | EXIF の GPS の IFD のすべての項目、XMP の `exif:GPS*`、IPTC の 2:90〜2:101（都市・州・国など） |
| `DateTime` | 日時 / Date and time | EXIF の `DateTime*`・`SubSecTime*`・`OffsetTime*`、XMP の `xmp:CreateDate`・`ModifyDate`・`MetadataDate`・`photoshop:DateCreated`、IPTC の 2:55・2:60、PNG の `tIME` と `tEXt` の `Creation Time`、PDF の `CreationDate`・`ModDate` |
| `Device` | 機器 / Device | EXIF の `Make`・`Model`・`BodySerialNumber`・`LensMake`・`LensModel`・`LensSerialNumber`・`MakerNote`、XMP の `tiff:Make`・`tiff:Model`・`aux:SerialNumber` |
| `Author` | 作成者 / Author | EXIF の `Artist`・`Copyright`・`CameraOwnerName`、XMP の `dc:creator`・`dc:rights`、IPTC の 2:80・2:116、PNG の `Author`・`Copyright`、PDF の `Author` |
| `Software` | ソフトウェア / Software | EXIF の `Software`・`HostComputer`、XMP の `xmp:CreatorTool`、PNG の `Software`、PDF の `Creator`・`Producer` |
| `Comment` | コメント / Comment | EXIF の `ImageDescription`・`UserComment`、JPEG の COM、XMP の `dc:description`・`dc:title`、IPTC の 2:120・2:05、PNG の `Comment`・`Description`・`Title`、PDF の `Title`・`Subject`・`Keywords` |
| `Thumbnail` | サムネイル / Thumbnail | EXIF の IFD1、JFIF と JFXX のサムネイル、MPF（APP2）と EOI の後ろの画像、PDF の `/Thumb` |
| `History` | 過去の版 / Earlier versions | PDF の追記保存の以前の内容（§4.6） |
| `Other` | その他 / Other | 上のどれにも当たらない、消す対象のもの（未知の APPn、上の表にない EXIF・XMP・PNG のテキスト、`/PieceInfo`、EOI の後ろの画像でないデータなど） |

- 向き・解像度・ICC は残す情報なので、見つかった情報には数えない。
- XMP は XML として解析せず、上の表の要素名・属性名（`exif:GPSLatitude` など）を文字列として探す。分類は表示のためだけに使い、除去は XMP の有無だけで決まる（XMP は丸ごと消す）ので、取りこぼしがあっても消し残しにはならない。どの名前にも当たらない XMP は `Other` とする。
- IPTC（APP13 `Photoshop 3.0` の 8BIM リソース 0x0404）は、データセット（`1C`・記録番号・データセット番号・長さ）を読む。読めなければ `Other`。

**詳細**（FR-06）

- 詳細は、種類ごとに項目（`Field` と値）を並べたもの。`Field` は「緯度」「撮影日時」「カメラの機種」などの表示名のキーで、文言はフロントエンドの辞書に置く（§10.2）。上の表の項目ごとに `Field` を 1 つ決め、それ以外は `Field::Other` と元の名前（`XPKeywords` など）にする。
- 値は Rust が表示用の文字列にする。緯度・経度は度分秒と方角（`12°34′56″ N`）、日時は `2026-01-02 03:04:05` の形、サムネイルは寸法（`160 × 120 px`）。言語によって文が変わる値は、文字列にせず数で渡し、文はフロントエンドの辞書で作る: 大きさ（読めないサムネイル、メーカーノート、圧縮された PNG のテキスト、XMP、EOI の後ろの画像でないデータなど）はバイト数、過去の版は回数（§4.7 の `DetailValue`）。
- XMP は XML として解析しないので、値を項目ごとには出さない。XMP が当たった種類ごとに、その種類の欄へ `Field::Xmp` の項目を 1 つ置き、値は XMP のバイト数にする（画面では「XMP（{n} バイト）」）。どの種類にも当たらない XMP は `Other` の欄に置く。
- 1 つの値は `MAX_DETAIL_VALUE_CHARS`（200 文字）、1 つのファイルの項目は `MAX_DETAIL_ENTRIES`（100）で打ち切り、打ち切ったことを示す。
- 詳細は、行が選ばれたときに `get_details`（§7.1）で毎回ファイルを読み直して作る。値を Rust の表にも設定にも持たず、ログにも書かない（FR-06、NFR-01）。

### 4.6 PDF（FR-03）

スパイクの `spike/strip-bench/src/pdf.rs` を元にし、`crates/worker/src/pdf.rs` に置く。

**断る PDF**

- 暗号化された PDF（制限だけを掛けたものを含む）は `PdfEncrypted`。lopdf は空のパスワードで開ける PDF を自動で復号するので、`was_encrypted()` と `is_encrypted()` の両方で見分ける。
- 電子署名付きの PDF は `PdfSigned`。いずれかのオブジェクトが `/Type /Sig` または `/FT /Sig` を持つ、またはカタログの `/Perms` がある、またはカタログの `/AcroForm` の `/SigFlags` が 0 でないもの。消すと署名が無効になるため（要件 §3.2）。
- lopdf で読めない PDF は `PdfOpenFailed`。

**消すもの**

- トレーラーの `/Info` と `/ID`。`/ID` は元のファイルと書き出したファイルを結び付ける識別子になるので、作り直さずに消す（暗号化しない PDF では省略できる）。
- すべての辞書（ストリームの辞書と、入れ子の中を含む）の `/Metadata`、`/PieceInfo`、`/Thumb`。
- フィルターが `DCTDecode` だけ（名前、または要素 1 つの配列）の画像のストリームの JPEG に、§4.2 を当てる。ただし PDF の中では向きを使わないので、残す情報だけの EXIF は書かない。JPEG として読めないストリームは、そのまま残す（描画を変えないため）。
- 参照されなくなったオブジェクト（`prune_objects`）。
- 過去の版は、lopdf が最新の版のオブジェクトだけを持って全体を書き直すことで消える（スパイク §3.2）。

**書き出し**

- `save_modern`（オブジェクトストリームと相互参照ストリーム）で書く。元がオブジェクトストリームを使っていた PDF が大きくならないようにするため（スパイク §3.3）。PDF の版は 1.5 未満なら 1.5 に上がる。
- 消さないもの（版上げの候補。README に書く）: フィルターが `DCTDecode` だけでない JPEG（`FlateDecode` との組み合わせなど）、JPEG 2000 の中の XMP、注釈の作成者名（`/T`）、フォームの値、添付ファイル、JavaScript、しおりの文字。

**調べる**

- 上の「消すもの」が 1 つでもあれば、§4.5 の種類に分ける。`/Info` の各項目は §4.5 の表のとおり。`/Metadata` は XMP として §4.5 の規則で分ける。展開は `MAX_XMP_BYTES`（4 MiB）を上限にし（lopdf の `decompressed_content_with_limit`）、上限を超えるものと展開できないものは、中身を読まずに `Other` とする（値は圧縮されたままのバイト数）。lopdf は上限を超えると途中までの内容を返さないため。`/PieceInfo` は `Other`（値はアプリケーション名の並び）、`/Thumb` は `Thumbnail`、`/ID` は `Other`（元のファイルと書き出したファイルを結び付ける識別子なので、消すことを見せる）、画像の中の JPEG は §4.5 の規則で分ける。
- 過去の版（`History`）: ファイルのバイト列の中の `startxref` の数が 2 以上なら、追記保存があったとみなす。ただし、先頭のオブジェクトが `/Linearized` の辞書で、数がちょうど 2 のときは、Web 表示用の最適化の 2 つ目の相互参照なので数えない。表示のためだけの判定で、除去は常に全体を書き直す。

### 4.7 調べた結果の型（`report.rs`）

- `Inspection { format, kinds: Vec<MetadataKind>, kept: Vec<KeptInfo> }`。`kinds` は重なりなしで、§4.5 の表の順に並べる。
- `Details { groups: Vec<{ kind, entries: Vec<{ field, name?, value }> }>, kept: Vec<KeptInfo>, truncated }`。`name` は `Field::Other` のときだけ入る元の名前。
- `DetailValue`: `Text(String)`（表示用の文字列）、`Bytes(u64)`（大きさ）、`Count(u32)`（過去の版の数）。
- `KeptInfo`: `Orientation(2..=8)`（1 は残さないので出さない。§4.5）、`ColorProfile { description? }`（ICC の `desc` を読めればその文字列）、`Resolution { x, y, unit }`（EXIF の解像度。なければ JFIF の密度、PNG の `pHYs`。単位が不明のものは出さない）。PDF では空。

## 5. ワーカー（NFR-01）

PDF Converter 設計 §5 と同じ仕組みにし、`client.rs`・`windows_job.rs`・メッセージの枠組み（長さ + JSON のヘッダー + バイナリの本体）を写して使う。違うのは要求の中身だけ。

### 5.1 メッセージ

| 要求 | 内容 | 応答 |
| --- | --- | --- |
| `Inspect { path, details }` | PDF を読み、§4.6 のとおり調べる。`details` が真なら詳細も作る | `{ inspection, details? }` またはエラー |
| `Clean { path }` | PDF を読み、§4.6 のとおり消して書き出し、書き出したバイト列を `Inspect` と同じ規則で調べ直す。消す対象が残っていれば `VerifyFailed` | 本体に PDF のバイト列、ヘッダーに `{ removed: Vec<MetadataKind> }` |

- エラーの応答は `{ code, detail }`（§6.6 のコード）。
- 1 つの要求ごとに PDF を読み直し、状態を持たない（PDF Converter の `Open` と `Close` はない）。調べるのと消すのは 1 回ずつで、開いたままにする利点がないため。
- ワーカーはファイルを書かない。書き出した PDF はメインプロセスに返し、メインプロセスが §6.5 の方式で保存する。

### 5.2 ワーカーの管理

- 数の上限、起動と再利用、異常終了と時間切れの扱い、アプリの終了時の後始末は、PDF Converter 設計 §5.2 と同じ。
- 時間の上限: `Inspect` は `INSPECT_TIMEOUT`（30 秒）、`Clean` は `CLEAN_TIMEOUT`（60 秒）。
- `WorkerCrashed` / `WorkerTimeout` になった PDF は、その処理の中で再試行しない。

### 5.3 メモリの上限

PDF Converter 設計 §5.3 と同じ（`WORKER_MEMORY_LIMIT` = 2 GiB、Linux は `setrlimit(RLIMIT_AS)`、Windows はジョブオブジェクト。unsafe を許すのは `windows_job.rs` だけ）。

### 5.4 ワーカーの見つけ方

メインプロセスは `std::env::current_exe()` を `--pdf-worker` 付きで起動する。pdfium がないので、ほかの引数は要らない。

## 6. 各機能の設計

### 6.1 一覧（FR-01、FR-02）

- 一覧は 1 つ。項目は Rust の表に登録し、フロントエンドには ID と表示用の情報（§7.1 の `FileItem`）だけを渡す。
- 追加の方法: 「ファイルを追加」（複数選択のファイルダイアログ。フィルターは `SUPPORTED_EXTENSIONS`）、「フォルダを追加」（フォルダダイアログ）、D&D（`WindowEvent::DragDrop`）。
- フォルダの扱い（直下だけを見る、サブフォルダ・シンボリックリンク・隠しファイル・対応しない拡張子を追加しない、`skipped` の数え方）、同じファイルを二重に追加しないこと、処理中に一覧を変えられないことは、PDF Converter 設計 §6.1 と同じ。
- 追加のときに調べる（画像は §4、PDF はワーカーの `Inspect { details: false }`）。失敗したファイルも一覧に入れ、行に「✕ 対象外」と理由を表示する。エラーの行は処理の対象にしない。
- サムネイルは作らない（モックアップのとおり）。

### 6.2 詳細（FR-06）

- 行のファイル名のボタンを押すと、その行を選び、`get_details(id)` の結果を右の欄に出す。選んでいる行をもう一度押すと選択を外す。
- 何も選んでいないときは、右の欄に「写っているもの、PDF の本文、ファイル名は消せません」の 1 行だけを出す（NFR-07 の画面の説明）。
- 答えを待つ間に別の行が選ばれたら、古い答えは捨てる（要求ごとに番号を持ち、最新の番号の答えだけを使う）。
- エラーの行を選んだときは、名前、形式、大きさとエラーの理由を出す（`get_details` は呼ばない）。
- 処理のあとは、「✓ 完了」などの状態、保存した名前（元の名前と違うときだけ。§6.4）、消した情報（その行の詳細の項目）、残した情報を出す（モックアップ `BResult`）。

### 6.3 処理の実行（FR-03〜FR-05）

- 「消して保存」で、一覧のエラーのない項目の ID を渡す。保存先のフォルダは Rust の状態のもの（§6.5）。
- 始める前に、項目の元のフォルダのいずれかと保存先のフォルダが同じなら、何も始めずに `SameFolderAsSource` を返す（FR-04。パスは正規化して比べる。Windows では大文字小文字を区別しない）。
- 出力名は始める時点で全件分まとめて決める（§6.4）。
- 画像は、メインプロセスのスレッドで 1 件ずつ: 読む（§4.1 の上限を確かめる）→ 消す（§4.2〜§4.4）→ 確かめる → 保存する（§6.5）→ 保存したファイルを読み直す。
- PDF は、ワーカーを 1 つ借りて `Clean` → 保存する → 読み直す。
- **確かめる**: 書き出す前のバイト列を、追加のときと同じ規則（§4.5 の分類）で調べ直し、見つかった情報が 1 つもないこと。画像では加えて、§4.2〜§4.4 の画像のデータ（JPEG の `image_segments`、PNG の残すチャンク、WebP の画像のチャンク）が入力と出力で同じバイト列であること。満たさなければ `VerifyFailed` とし、何も書かない。
- **読み直す**: 保存したファイルを読み、確かめたバイト列と同じであること。違えばそのファイルを消し、`VerifyFailed` とする（FR-04 の「書き出したあと、そのファイルを読み直して」）。
- 並列の数は §5.2 と同じ式。画像のスレッドと PDF のワーカーを合わせて、その数までにする。
- 進捗は `job-progress`、1 件ごとの結果は `job-item`、最後に `job-finished` を送る（§7.2）。

### 6.4 出力名の決定

PDF Converter 設計 §6.4 と同じ規則（`name (1).jpg` の形、大文字小文字を区別しない重なりの判定、開始時に全件分を決める、保存の瞬間に同名ができていたら次の番号）。候補名は元のファイル名そのもの。`naming.rs` を写して使う。

### 6.5 保存とキャンセル

- 保存の方式（保存先のフォルダの `.` で始まる一時ファイルに全内容を書き、`persist_noclobber` で最終名にする）は PDF Converter 設計 §6.5 と同じ。新しいファイルを作るので、元のファイルの更新日時、属性、代替データストリーム（Windows の Zone.Identifier など）は写らない（FR-04）。
- 保存先のフォルダは Rust の状態に 1 つ持つ。`pick_output_dir` と設定の復元（§6.7）が入れ、`start_clean` はそれを使う。未選択のまま「消して保存」が押されたら、画面が先に `pick_output_dir` を呼び、選ばれたらそのまま始める（PDF Converter 設計 §6.5 と同じ）。
- キャンセルは共有フラグで行い、次のファイルの前に確かめる。処理中のファイルは終わらせて保存する。
- キャンセルを押した直後に「キャンセル中…」を表示し、ボタンを無効にする（NFR-02）。

### 6.6 エラー（FR-09）

IPC では `{ code, detail }` の形で返す（PDF Converter 設計 §6.6 と同じ）。

| コード | 状況 | 表示文言（ja / en） |
| --- | --- | --- |
| UnsupportedFormat | 対応しない形式 | 対応していない形式です / Unsupported file format |
| DecodeFailed | 画像の構造を読めない（壊れたファイル） | 画像を読み込めませんでした。ファイルが壊れている可能性があります / Couldn't read the image. The file may be damaged. |
| PdfOpenFailed | PDF を読めない（壊れたファイル） | PDF を読み込めませんでした。ファイルが壊れている可能性があります / Couldn't read the PDF. The file may be damaged. |
| PdfEncrypted | 暗号化された PDF | 暗号化された PDF は扱えません / Encrypted PDFs aren't supported |
| PdfSigned | 電子署名付きの PDF | 電子署名付きの PDF は扱えません / Signed PDFs aren't supported |
| TooLarge | ファイルの大きさが上限超過 | ファイルが大きすぎます（上限 {detail}） / File is too large (limit: {detail}) |
| WorkerCrashed | ワーカーが異常終了した | PDF の処理中に問題が起きました。ファイルが壊れている可能性があります / Something went wrong while processing the PDF. The file may be damaged. |
| WorkerTimeout | ワーカーが時間内に応答しない | PDF の処理に時間がかかりすぎたため中止しました / Processing the PDF took too long and was stopped |
| VerifyFailed | 書き出したものに消す対象が残っていた、画像のデータが変わった、保存したファイルが違っていた | 情報を消しきれなかったため、保存しませんでした / Couldn't remove all the information, so the file wasn't saved |
| SameFolderAsSource | 保存先が元のファイルのフォルダ | 元のファイルと同じフォルダには保存できません。別のフォルダを選んでください / Choose a folder other than the one the files are in |
| ReadFailed | 読み込みの失敗 | ファイルの読み込みに失敗しました / Failed to read file |
| WriteFailed | 書き込みの失敗（権限、容量など） | ファイルの書き込みに失敗しました / Failed to write file |
| JobRunning | 処理中に一覧や処理を操作しようとした | 処理中は操作できません / Not available while processing |
| UnknownHandle | 存在しない ID | ファイルをもう一度追加してください / Please add the file again |
| InvalidParams | 引数が範囲外（UI からは通常送られない） | 無効な設定です / Invalid settings |

- `TooLarge` の `detail` には、Rust が上限を `256 MB` の形の文字列にして入れる。
- `VerifyFailed` は、正しく実装されていれば起きない。起きたときに利用者がファイルを使ってしまわないよう、保存しないことを優先する。
- `SameFolderAsSource` は処理全体を止めるエラーなので、行ではなく一覧の上に `ErrorDisplay` で出す。

### 6.7 設定の保存（FR-07、FR-10）

- 保存先、読み書きの方式、壊れた設定や未知の `schemaVersion` で既定値に戻すこと、フォルダの扱いは PDF Converter 設計 §6.7 と同じ。
- 内容: `schemaVersion`（1）、`language`（`"ja"`・`"en"`・`null`）、`outputDir`（絶対パスか `null`）。
- `get_settings` は `{ language, outputDir: { dirLabel } | null }` を返す。`save_settings` は `{ language }` を受け取る。

## 7. IPC

### 7.1 コマンド

| コマンド | 引数 | 戻り値 |
| --- | --- | --- |
| `get_settings` / `save_settings` | – / `{ language }` | §6.7 |
| `add_files` | `source: "files" \| "folder"` | `{ added: FileItem[], skipped: { unsupported, folders, duplicates } } \| null` |
| `remove_items` | `ids` | – |
| `get_details` | `id` | `Details`（§4.7） |
| `pick_output_dir` | – | `{ dirLabel } \| null` |
| `start_clean` | `ids` | – |
| `cancel_job` | – | – |
| `get_about` | – | `{ version }` |

- `FileItem`: `{ id, name, format: "jpeg" \| "png" \| "webp" \| "pdf" \| null, bytes, kinds: MetadataKind[], error }`。`error` は失敗した項目だけに入る `{ code, detail }`。失敗した項目では `format` は `null`、`kinds` は空。
- どのコマンドもパスを引数に取らない。`dirLabel` は表示用で、送り返されない。
- 処理は同時に 1 つだけ。実行中の `start_clean`、`add_files`、`remove_items` は `JobRunning` で拒む。`get_details` は実行中でも受け付ける。

### 7.2 イベント（Rust → フロントエンド）

| イベント | ペイロード |
| --- | --- |
| `items-dropped` | `{ added: FileItem[], skipped, error: IpcError \| null }`（処理中に落とされたときは何も加えず、`error` を `JobRunning` にする） |
| `job-progress` | `{ done, total, current: string \| null }` |
| `job-item` | `{ id, status: "ok" \| "failed" \| "cancelled", savedName: string \| null, removed: MetadataKind[], error? }`（`savedName` は元の名前と違うときだけ入る） |
| `job-finished` | `{ succeeded, failed, unprocessed, cancelled }` |

## 8. ライセンス

- 第三者ライセンス一覧は `scripts/generate-licenses.ts` が `cargo-about` と npm の依存から作る（PDF Converter 設計 §8.3 から pdfium の部分を除いたもの）。
- 許可リストの検査と一覧は、配布物に入る依存だけを対象にする（`deny.toml` の `[graph] exclude-dev = true`）。テストでだけ使う `hayro` と、フィクスチャの生成にだけ使う crate は除く。ExifTool（§11.2）は CI に入れるだけで、配布物にも依存にも入らない。

## 9. セキュリティ（NFR-01）

PDF Converter 設計 §9 と同じにする（capabilities は `core:*` の最小集合、CSP、HTTP クライアントを `cargo-deny` の `bans` で禁止、WebView2 の起動オプション、止められない通信の README への記載、依存の入手経路、リリースの書き込みの権限、既知の脆弱性の検査）。pdfium に関する項目はない。加えて次のとおり。

- PDF はワーカーでだけ読む（§1、§5）。`crates/worker` 以外の crate が `lopdf` に依存していないことを、`cargo-deny` の `bans`（`wrappers = ["mcleaner-worker"]`）で確かめる。
- ワーカーには、読む PDF のパスだけを渡す。ワーカーはファイルを書かない。
- 詳細の値（§4.5）はログに書かない。Rust の `log` の呼び出しに、ファイルの中身から作った文字列を渡さない（レビューの観点にする）。

## 10. フロントエンド

### 10.1 画面構成

見た目と配置はモックアップに従う。要点:

- アプリ名は「Metadata Cleaner」。ウィンドウの既定の大きさは 1200×800、最小は 960×640。
- 上部バー: アプリ名、言語の切り替え、「このアプリについて」（PDF Converter と同じ部品と並び。タブはない）。
- 本体: 左に一覧、右に詳細の欄（幅 300〜340 px）。左は上から、保存先の帯（「保存先フォルダ」、フォルダの名前、「フォルダを選ぶ」）、結果の帯（処理のあとだけ）、一覧の見出し（件数、「ファイルを追加」「フォルダを追加」「すべて外す」）、表。一覧が空のときは、表の代わりにドロップ領域を出す。
- 表の列: ファイル名、形式、大きさ、見つかった情報、状態。列は処理の前と後で変えない（レイアウトがずれないようにするため）。表は `table-layout: fixed` にし、状態の欄の幅を固定する。
- 見つかった情報は、種類ごとのラベル（§4.5 の表示）を並べる。位置情報だけは、ピンの記号を付けて別の色にする（§10.3）。何もなければ「メタデータなし」。
- 状態の欄: 処理の前は「待機」、エラーの行は「✕ 対象外」、処理中は「処理中…」、処理のあとは「✓ 完了」「✕ 失敗」「キャンセル」。理由は 2 行目に 1 行で出し、長ければ省略して `title` で全文を見せる（行の高さを変えないため）。
- 下部バー: 処理していない間は「{n} 件から情報を消して保存します」（数えるのはエラーのない項目）と「消して保存」。処理中は進捗と「キャンセル」、処理のあとは結果（PDF Converter 設計 §10.1 と同じ色と記号）。
- 結果の帯と下部バーの結果は、一覧か保存先が変わったら消す（PDF Converter 設計 §10.1 と同じ）。

### 10.2 状態管理、多言語

- PDF Converter 設計 §10.2 と同じ仕組み（`useReducer` と Context、`ja.ts` を基準にした型付きの辞書、`useJobEvents`・`useJobRunner`・`JobFooter`・`JobSummaryBanner`・`useItemsDropped`・`OutputDirField`・`useSettingsAutoSave`）を写して使う。タブがないので、画面ごとの reducer は 1 つにする。
- 選んでいる行の ID と、その詳細の答えは画面の状態に持つ。詳細の値は画面を閉じれば消え、どこにも保存しない。
- `MetadataKind` と `Field` の表示名は辞書に置く。辞書のキーの集合は、`ts-rs` が作る型の全値と一致することをテストで確かめる（種類を足したときに訳し漏れないようにするため）。

### 10.3 テーマ

- 色は `styles/tokens.css` で定義する。PDF Converter の tokens.css を写し、アクセントの色をティールに替える。

| トークン | ライト | ダーク | 確かめる比 |
| --- | --- | --- | --- |
| `--color-accent` | `#0e6b66` | `#3fa79e` | 面の背景に対して、ライト 6.34、ダーク 5.28 |
| `--color-text-on-accent` | `#ffffff` | `#1c1b18` | アクセントに対して、ライト 6.34、ダーク 5.93（ダークで白は 2.90 で足りない） |
| `--color-selected-row` | `#e7f1f0` | `#1f3a38` | 本文・補足の文字・失敗の赤・成功の緑が 4.5 以上（ダークの補足 4.91、失敗 4.88） |
| `--color-location-bg` / `--color-location-text` | `#fbecd0` / `#6b4100` | `#4a3818` / `#f6cf8a` | ライト 7.56、ダーク 7.60 |
| `--color-chip-bg` / `--color-chip-text` | `#efede8` / `#4a4844` | `#3a3833` / `#d6d2ca` | ライト 7.80、ダーク 7.77 |

- 位置情報の色はオレンジ系にする。失敗の赤（PDF Converter と同じ深紅）とアクセントのティールの両方と色相が離れていて、ピンの記号でも見分けられる。
- 上の比と、PDF Converter の `tokens.test.ts` が確かめている比（失敗と成功の色をすべての背景に対して 4.5 以上）を、`tokens.test.ts` で確かめる。
- エラーの見せ方、成功と失敗の記号、フォントは PDF Converter 設計 §10.3 と同じ。

## 11. テスト

### 11.1 フィクスチャ

第三者のファイルを使わず、`crates/core/examples/gen_fixtures.rs` で作ってコミットする。メタデータの値はすべて架空のもの（位置は北緯 12 度 34 分・東経 65 度 43 分、名前は `Example Author` など）にする（NFR-06）。ICC、XMP、IPTC は ExifTool が読める正しい形にする（スパイク §5）。

| ファイル | 内容 | 目的 |
| --- | --- | --- |
| `full.jpg` | EXIF（GPS、日時、機種、シリアル番号、メーカーノート、ソフト、作者、著作権、IFD1 のサムネイル、向き 6、解像度）、XMP、IPTC、ICC、MPF、COM、JFIF のサムネイル、EOI の後ろのデータ | すべての種類の除去と、向き・解像度・ICC の保持 |
| `orient1.jpg`〜`orient8.jpg` | 向き 1〜8 と GPS を持つ JPEG | 向きの保持（1 では EXIF ごと消える） |
| `cmyk.jpg` | Adobe APP14 付きの CMYK の JPEG と EXIF | Adobe の保持 |
| `progressive.jpg` | プログレッシブの JPEG。スキャンの間に COM と APP1（XMP）を挟む | スキャンの間のセグメント |
| `no_jfif_dpi.jpg` | JFIF がなく、EXIF にだけ解像度がある JPEG | 解像度の保持 |
| `full.png` | `tEXt`（IDAT の前と後ろ）、`zTXt`、XMP の `iTXt`、`eXIf`（GPS と向き）、`tIME`、`iCCP`、`pHYs`、IEND の後ろのデータ | PNG の除去と保持 |
| `anim.png` | APNG（`acTL` `fcTL` `fdAT`）と `tEXt` | アニメーションの保持 |
| `full.webp` | VP8X、透過のある VP8L、EXIF（GPS と向き）、XMP、ICCP | WebP の除去と保持 |
| `anim.webp` | アニメーションの WebP（ANIM、ANMF）と XMP | アニメーションの保持 |
| `clean.jpg` / `clean.png` / `clean.webp` | メタデータのない画像 | 「メタデータなし」 |
| `webp_named.jpg` | 中身が WebP の `.jpg` | 拡張子と中身の食い違い |
| `full.pdf` | `/Info`、カタログとページの XMP、`/PieceInfo`、`/Thumb`、`full.jpg` を `DCTDecode` で入れた画像、追記保存の 2 つ目の版 | PDF の除去 |
| `linearized.pdf` | Web 表示用に最適化した（`startxref` が 2 つある）メタデータなしの PDF | 過去の版の誤検出がないこと |
| `encrypted.pdf` / `restricted.pdf` | 閲覧のパスワード付き、制限だけの暗号化 | `PdfEncrypted` |
| `signed.pdf` | 署名のフィールドと `/Type /Sig` の辞書を持つ PDF（署名の値は架空） | `PdfSigned` |
| `corrupt.jpg` / `corrupt.png` / `corrupt.webp` / `corrupt.pdf` | 途中で切れたファイル | `DecodeFailed` / `PdfOpenFailed` |

- プログレッシブの JPEG は `jpeg-encoder` の `set_progressive` で作り、生成したバイト列のスキャンの間にセグメントを差し込む。
- ICC は、`gen_fixtures` が sRGB の行列と TRC を持つ最小の v2 プロファイルを組み立てる（第三者のプロファイルのファイルを使わないため）。
- WebP のフィクスチャは可逆（VP8L）だけにする。`image` は不可逆の VP8 を書けず、`ALPH` は VP8 と組み合わせるチャンクなので、`ALPH` を持つファイルは作れない。`ALPH` を残すことは、チャンクの並びを組み立てた単体テストで確かめる（T03）。
- 暗号化された PDF は、PDF Converter の `gen_fixtures` の RC4 の実装を写して作る（生成し直しても同じファイルになるようにするため。PDF Converter 設計 §11.1）。
- `crates/core/tests/fixtures.rs` が、生成し直したバイト列とコミット済みのファイルを比べる（PDF Converter と同じ）。

### 11.2 品質テスト（NFR-03）

- **画像の画素**: 各フィクスチャを消したあと、`image` で読んだ画素と向きが元と同じこと。ICC のバイト列が元と同じこと。画像のデータのバイト列（§6.3 の「確かめる」と同じ範囲）が同じこと。
- **PDF の描画**: `full.pdf` と、消したあとの PDF を、`hayro` で全ページ 72 dpi で描き、画素が完全に一致すること。同じ描画エンジンで前後を比べるので、許容の幅は設けない。
- **残っていないこと（自前）**: 消したあとのファイルを §4.5 の規則で調べ、見つかった情報が 0 であること。PDF は加えて、生のバイト列にフィクスチャの架空の値（`Example Author` など）が含まれないこと。ExifTool では、PDF の中の JPEG と過去の版を確かめられないため（スパイク §5）。
- **残っていないこと（ExifTool）**: CI の Linux のジョブで ExifTool（`libimage-exiftool-perl`）を入れ、`scripts/exiftool-check.ts` が、テスト用の単独の実行ファイルで消したフィクスチャを `exiftool -j -a -G1` で読む。許すグループと項目（ファイルの基本情報、JFIF、Adobe、ICC、PNG と WebP の画像の基本情報、残す EXIF の 4 項目）以外が出たら失敗にする。Windows の CI では行わない。
- **書き出した PDF が開けること**: `hayro` で全ページを描けること。ブラウザー内蔵のビューアーで開けることは、手動の確認で確かめる。
- **性能（NFR-02）**: 基準環境は開発者の PC（PDF Converter と同じ）。`--release` のテストで、テストの中で作る 4000×3000 の JPEG（約 2.5 MB、`full.jpg` と同じメタデータ付き）の調査・除去・確認の合計と、100 ページの PDF（各ページに `DCTDecode` の画像と XMP）の `Clean` の時間を測る。目安は基準環境で JPEG が 0.5 秒以内（スパイクでは 6 ms）、PDF が 1 秒以内。CI では、環境の差を見込んで JPEG が 2 秒、PDF が 5 秒を超えたら失敗にする。基準環境での値は手動で確かめる。

### 11.3 テストの一覧

| 対象 | 種類 | 主な内容 |
| --- | --- | --- |
| crates/core | 単体 | 形式の判定、JPEG・PNG・WebP の読み取り（壊れた入力、詰め物、スキャンの間、後ろのデータ）、残すものと消すもの、残す情報だけの EXIF、分類（EXIF・XMP・IPTC・PNG のテキスト）、詳細の値の形と打ち切り、出力名 |
| crates/core | 品質 | §11.2 の画像 |
| crates/worker | 単体 | 断る PDF（暗号化、署名、壊れたもの）、消すもの、過去の版の判定、`DCTDecode` の JPEG |
| crates/worker | 品質 | §11.2 の PDF |
| crates/worker | 結合 | ワーカーの要求と応答、異常終了と時間切れ（テスト用の仕組みで起こす）、メモリの上限 |
| src-tauri | 単体 | 一覧の追加（フォルダ、重複、隠しファイル）、同じフォルダの拒否、保存と読み直し、キャンセル、設定 |
| フロントエンド | 単体・結合 | 一覧と詳細の表示、選択と古い答えの破棄、処理の前後で列が変わらないこと、結果の帯と下部バー、辞書のキーの網羅、コントラスト |
| CI（Linux） | ExifTool | §11.2 |
