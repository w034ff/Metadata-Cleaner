# 手動確認の手順書（受け入れ確認）

この手順書は、リリース前の受け入れ確認（`docs/work-plan.md` T16、T17）において、Windows（オーナー）および Linux（実装担当、WSLg）の実環境で、インストーラーから導入したアプリが要件定義書（`docs/requirements.md` §7）の受け入れ基準を満たしているかを手動で確認するための手順です。

## 結果の記録方法

確認結果は、確認完了後に `docs/acceptance/v0.1.0.md` に記録します（`docs/acceptance/` ディレクトリは確認時に作成します）。
記録の書式は以下の表を用い、項目ごとに OS・合否・不合格時の詳細な様子を記述します。

```markdown
# 受け入れ確認結果 (v0.1.0)

| 項目 | OS | 結果 (合格 / 不合格) | 備考・不合格時の様子 |
| --- | --- | --- | --- |
| 項目 2: 画像（JPEG・PNG・WebP） | Windows | 合格 | |
| 項目 2: 画像（JPEG・PNG・WebP） | Linux | 合格 | |
| 項目 3: PDF | Windows | 合格 | |
| 項目 3: PDF | Linux | 合格 | |
| 項目 4: 異常系のエラー表示 | Windows | 合格 | |
| 項目 4: 異常系のエラー表示 | Linux | 合格 | |
| 項目 5: 出力名の重複防止 | Windows | 合格 | |
| 項目 5: 出力名の重複防止 | Linux | 合格 | |
| 項目 6: 一括処理のキャンセル | Windows | 合格 | |
| 項目 6: 一括処理のキャンセル | Linux | 合格 | |
| 項目 7: 両 OS でのインストールと起動 | Windows | 合格 | |
| 項目 7: 両 OS でのインストールと起動 | Linux | 合格 | |
| 項目 8: 外部通信の確認 | Windows | 合格 | |
| 項目 8: 外部通信の確認 | Linux | 合格 | |
| 項目 9: ライセンス表示と CI 検査 | Windows | 合格 | |
| 項目 9: ライセンス表示と CI 検査 | Linux | 合格 | |
```

## 事前準備

### 1. 手動確認用ファイルの作成

リポジトリ直下で次のコマンドを実行し、フィクスチャに含まれない手動確認用ファイル（上限超過ファイル、キャンセル確認用ファイル）を作業用フォルダに生成します。

```bash
npm run manual-test:files -- <出力先フォルダ>
```

これにより、次のファイルおよびディレクトリが生成されます:
- `too-large.jpg`: 268,435,457 バイト（`MAX_IMAGE_FILE_BYTES` 256 MiB 超過）
- `too-large.pdf`: 536,870,913 バイト（`MAX_PDF_FILE_BYTES` 512 MiB 超過）
- `cancel-batch/`: 100 ファイル（`cancel_001.pdf`〜`cancel_050.pdf`、`cancel_051.jpg`〜`cancel_100.jpg`。キャンセル確認用の一括ファイル群）

### 2. フィクスチャの配置場所

リポジトリ内の `crates/core/tests/fixtures/` にあるテスト用ファイルを使用します（design §11.1）:
- `full.jpg`: EXIF（位置情報、日時、機種、シリアル番号、メーカーノート、作成ソフト、作者、著作権、IFD1 サムネイル、向き 6、解像度 300 dpi）、XMP、IPTC、ICC、MPF、COM、JFIF サムネイル、EOI 後ろのデータ
- `orient1.jpg`〜`orient8.jpg`: 向き 1〜8 と GPS を持つ JPEG（向き 1 は EXIF ごと消去）
- `cmyk.jpg`: Adobe APP14 付きの CMYK JPEG
- `progressive.jpg`: プログレッシブ JPEG（スキャン間に COM と APP1）
- `no_jfif_dpi.jpg`: JFIF なし、EXIF のみ解像度
- `full.png`: `tEXt`（IDAT の前と後ろ）、`zTXt`、XMP の `iTXt`、`eXIf`（GPS と向き）、`tIME`、`iCCP`、`pHYs`、IEND 後ろのデータ
- `anim.png`: APNG（`acTL`、`fcTL`、`fdAT`）と `tEXt`
- `full.webp`: VP8X、透過のある VP8L、EXIF（GPS と向き）、XMP、ICCP
- `anim.webp`: アニメーション WebP（ANIM、ANMF）と XMP
- `clean.jpg` / `clean.png` / `clean.webp`: メタデータのない画像
- `webp_named.jpg`: 中身が WebP の `.jpg`
- `full.pdf`: `/Info`、カタログとページの XMP、`/PieceInfo`、`/Thumb`、`full.jpg` 画像、追記保存の 2 つ目の版
- `linearized.pdf`: Web 最適化 PDF（過去の版の誤検出なし）
- `encrypted.pdf`: 閲覧パスワード付き PDF
- `restricted.pdf`: 閲覧パスワードなし・権限制限付き暗号化 PDF
- `signed.pdf`: 電子署名付き PDF
- `corrupt.jpg` / `corrupt.png` / `corrupt.webp` / `corrupt.pdf`: 破損ファイル

### 3. 手元の実用写真と PDF

手元のスマートフォン等で撮影した一般的な写真（GPS 位置情報や撮影日時を含むもの）および文字を含む一般的な PDF（文書やマニュアルなど）を 1 つずつ用意します（第三者権利保護のためリポジトリには含めません）。

---

## 項目 2: 画像（JPEG・PNG・WebP。EXIF、XMP、IPTC、コメント、ICC、向き）

要件: FR-02、FR-03、FR-06、NFR-03。各種形式の画像から画質を落とさず（再エンコードなし）メタデータを除去し、向きや色のプロファイルが正しく残ることを確認します。

### 準備（使うファイル）

- `full.jpg`、`orient6.jpg`、`cmyk.jpg`、`progressive.jpg`、`no_jfif_dpi.jpg`
- `full.png`、`anim.png`
- `full.webp`、`anim.webp`
- `clean.jpg`
- 手元の実用写真

### 操作

1. アプリを起動する。一覧が空のとき、ドロップ領域に「ファイルまたはフォルダをここにドロップ」「JPEG、PNG、WebP、PDF」「ファイルを追加」「フォルダを追加」が表示され、右側の詳細欄に「写っているもの、PDF の本文、ファイル名は消せません」が表示されていることを確認する。
2. 「ファイルを追加」またはドラッグ＆ドロップで、上記のフィクスチャ画像（計 10 ファイル）を追加する。
3. 一覧の表示を確認する:
   - 「ファイル名」「形式」「大きさ」「見つかった情報」「状態」の各列が表示されていること。
   - `full.jpg`、`orient6.jpg`、`full.png`、`full.webp` などの位置情報を含む画像には、オレンジ色の「位置情報」チップ（ピン記号付き）が表示されること。
   - その他のメタデータに応じて「日時」「機器」「作成者」「ソフトウェア」「コメント」「サムネイル」などのチップが表示されること。
   - `clean.jpg` には「メタデータなし」と表示されること。
   - 各行の「状態」が「待機」と表示されていること。
4. 詳細の確認:
   - `full.jpg` の行をクリックする。行が選択状態（ハイライト）になり、右側の詳細欄に各グループと項目・値（緯度・経度、撮影日時、カメラの機種、メーカーノート、作成者、著作権など）が表示されることを確認する。
   - 詳細欄の「残す情報」に「向き（右に 90 度回転）」「解像度（300 dpi）」「色のプロファイル」が表示されることを確認する。
   - もう一度同じ行をクリックすると選択が解除され、右側が「写っているもの、PDF の本文、ファイル名は消せません」の 1 行に戻ることを確認する。
5. 保存先フォルダの選択:
   - 「保存先フォルダ」の「フォルダを選ぶ」をクリックし、空の保存先フォルダを指定する。
   - 下部バーの左側に「10 件から情報を消して保存します」と予告が表示されていることを確認する。
6. 実行:
   - 下部バーの「消して保存」をクリックする。
7. 完了後の表示確認:
   - 一覧の上に緑色のバナーで「保存 10 件」が表示されること。
   - 下部バーに「✓ すべて完了しました」と表示されること。
   - 表の各行の「状態」が緑の「✓ 完了」になること。
   - 行をクリックすると、右側の詳細欄に「✓ 完了」、「消した情報」、「残した情報」が表示されること。
8. 手元の実用写真の確認:
   - 一覧の「すべて外す」をクリックして一覧を空にする。
   - 手元の実用写真を追加し、同様に「消して保存」を実行して正常に完了することを確認する。

### 期待する結果

- 保存先フォルダに保存された各画像を開いて目視確認する:
  - 画像の画質が劣化せず、元の表示と一致していること。
  - `orient6.jpg`: 向きが回転せず、正しい向きで表示されること。
  - `cmyk.jpg`: 色が崩れず正常に描画されること（Adobe APP14 の保持）。
  - `anim.png`、`anim.webp`: アニメーションが損なわれず再生できること。
  - 手元の実用写真: ExifTool またはプロパティで確認し、GPS 位置情報や撮影日時、機種情報が消去されていること。
  - 自動テストにおいて、画素データが再エンコードされず元のバイト列のまま保持されていること。

### 自動テストでの確認

- `crates/core/tests/images.rs`:
  - `test_full_jpg_cleaning`
  - `test_progressive_jpg_cleaning`
  - `test_cmyk_jpg_cleaning`
  - `test_full_png_cleaning`
  - `test_anim_png_cleaning`
  - `test_full_webp_cleaning`
  - `test_anim_webp_cleaning`
- `crates/core/tests/quality.rs`:
  - `test_image_fixtures_quality`（全画像フィクスチャで画素・向き・ICC が一致し、除去後の分類が 0 件であることを確認）
- `scripts/exiftool-check.ts`（`npm run exiftool:check`）:
  - CI の Linux 環境で ExifTool を用いて全フィクスチャを照合し、許可された残存情報（向き・解像度・ICC 等）以外が完全に排除されていることを確認。

---

## 項目 3: PDF（文書情報、XMP、過去の版、JPEG 内の EXIF）

要件: FR-02、FR-03、FR-06、NFR-03。PDF の文書情報、XMP、過去の版（追記保存）、画像内の EXIF を除去し、ページの描画が完全に一致することを確認します。

### 準備（使うファイル）

- `full.pdf`
- `linearized.pdf`
- 確認者が手元に用意した文字を含む実用 PDF

### 操作

1. 「ファイルを追加」またはドラッグ＆ドロップで `full.pdf` を追加する。
2. 一覧で「形式」が「PDF」となり、「見つかった情報」に「日時」「作成者」「ソフトウェア」「コメント」「サムネイル」「過去の版」「その他」が表示されていることを確認する。
3. `full.pdf` をクリックして選択し、右側の詳細欄を確認する:
   - 「作成者」「作成ソフト」「追記保存」（「1 回分の以前の内容が残っています」）「サムネイル」などが表示されていること。
4. 空の保存先フォルダを指定し、下部バーの「消して保存」をクリックする。
5. 完了後、上部バナーに「保存 1 件」、下部バーに「✓ すべて完了しました」、行の状態に「✓ 完了」が表示されることを確認する。
6. `full.pdf` を選択し、詳細欄に「✓ 完了」および「消した情報」が表示されることを確認する。
7. 手元の実用 PDF を追加し、同様に「消して保存」を実行する。

### 期待する結果

- 保存された `full.pdf` および手元の実用 PDF をブラウザー（Chrome / Edge / Firefox）または PDF ビューアーで開いて目視確認する:
  - 全ページが文字化けやレイアウト崩れなく鮮明に描画されること。
  - プロパティ（文書情報）を確認し、タイトル、作成者、サブジェクト、キーワード、作成日時・更新日時、作成ソフトが消去されていること。
- 自動テストにおいて、`hayro` による 72 dpi 描画結果が画素単位で完全一致し、フィクスチャの架空の文字列（`Example Author` など）が生のバイト列に残っていないこと。

### 自動テストでの確認

- `crates/worker/tests/pdf_quality.rs`:
  - `test_pdf_quality_full_and_linearized`（`full.pdf` および `linearized.pdf` の全ページで描画画素が完全一致し、見つかった情報が空で、架空の値が残っていないことを確認）
- `crates/worker/tests/worker.rs`:
  - `clean_full_pdf_returns_cleaned_bytes_and_removed_kinds`
- `src-tauri/tests/pool.rs`:
  - `cleans_full_pdf_through_pool`

---

## 項目 4: 異常系のエラー表示（破損ファイル、拡張子食い違い、暗号化・署名、上限超過、異常終了・時間切れ、書き込み権限なし）

要件: FR-09、NFR-01。異常なファイルやエラー発生時に適切なメッセージが表示され、アプリが異常終了せずに継続操作できることを確認します。

### 準備（使うファイル）

- `corrupt.jpg`、`corrupt.pdf`（破損ファイル）
- `webp_named.jpg`（拡張子と中身の食い違い）
- `encrypted.pdf`、`restricted.pdf`（暗号化 PDF）
- `signed.pdf`（電子署名付き PDF）
- `too-large.jpg`、`too-large.pdf`（上限超過ファイル）
- 書き込み権限のない保存先フォルダ（後述の手順で作成）

### 操作と期待する結果

#### 1. 破損ファイルの読み込み
- `corrupt.jpg` および `corrupt.pdf` を追加する。
- **期待結果**:
  - 行の状態に赤字で「✕ 対象外」が表示される。
  - 2 行目の理由に次のメッセージが表示される:
    - 画像: 「画像を読み込めませんでした。ファイルが壊れている可能性があります」
    - PDF: 「PDF を読み込めませんでした。ファイルが壊れている可能性があります」
  - 「形式」は拡張子が大文字で表示され（`corrupt.jpg` は「JPG」、`corrupt.pdf` は「PDF」）、「見つかった情報」は空欄となる。
  - 「消して保存」の対象件数（下部バー）にカウントされず、処理対象から除外される。

#### 2. 拡張子と中身が食い違うファイル
- `webp_named.jpg`（中身は WebP だが拡張子が `.jpg`）を追加する。
- **期待結果**:
  - エラーにならず正常に一覧に追加される。
  - 「形式」欄には中身の形式である「WebP」が表示される。
  - 「消して保存」を実行すると正常に完了し、保存先には元の名前のまま `webp_named.jpg` として保存される。

#### 3. 暗号化された PDF と電子署名付き PDF
- `encrypted.pdf` および `restricted.pdf` を追加する。
  - **期待結果**: 状態に赤字で「✕ 対象外」、理由に「暗号化された PDF は扱えません」が表示される。
- `signed.pdf` を追加する。
  - **期待結果**: 状態に赤字で「✕ 対象外」、理由に「電子署名付きの PDF は扱えません」が表示される。行をクリックして選択すると、右側詳細欄に「電子署名付きの PDF は扱えません。消すと署名が無効になるためです」と表示される。

#### 4. 上限超過
- `too-large.jpg`（256 MiB 超）および `too-large.pdf`（512 MiB 超）を追加する。
- **期待結果**:
  - `too-large.jpg`: 状態に赤字で「✕ 対象外」、理由に「ファイルが大きすぎます（上限 256 MB）」が表示される。
  - `too-large.pdf`: 状態に赤字で「✕ 対象外」、理由に「ファイルが大きすぎます（上限 512 MB）」が表示される。

#### 5. 保存先のフォルダの異常

##### 5a. 元のファイルと同じフォルダ
- 元のファイルが存在するフォルダを保存先に指定し、「消して保存」をクリックする。
- **期待結果**:
  - 処理は開始されない（進捗バーは出ない）。
  - 一覧の上に、赤い帯で「元のファイルと同じフォルダには保存できません。別のフォルダを選んでください」が表示される。

##### 5b. 書き込み権限のないフォルダ
- 書き込み権限のないテスト用フォルダを準備する:
  - **Linux**:
    ```bash
    mkdir -p /tmp/mcleaner-readonly && chmod 555 /tmp/mcleaner-readonly
    ```
  - **Windows**:
    - コマンドプロンプト:
      ```cmd
      mkdir C:\test-readonly
      icacls C:\test-readonly /deny "%USERNAME%:(W)"
      ```
    - PowerShell:
      ```powershell
      mkdir C:\test-readonly
      icacls C:\test-readonly /deny "${env:USERNAME}:(W)"
      ```
- 正常な画像（`clean.jpg` 等）を追加し、上記フォルダを保存先に指定して「消して保存」をクリックする。
- **期待結果**:
  - 処理が実行され、書き込み失敗により行の状態が赤字で「✕ 失敗」、理由が「ファイルの書き込みに失敗しました」になる。
  - 上部バナーに「失敗 1 件」、下部バーに赤字で「✕ 失敗があります」と表示される。
  - アプリがクラッシュすることなく継続操作可能であること。
- 後片付け:
  - **Linux**: `chmod 755 /tmp/mcleaner-readonly && rm -rf /tmp/mcleaner-readonly`
  - **Windows**:
    - コマンドプロンプト: `icacls C:\test-readonly /remove:d "%USERNAME%"`、`rmdir C:\test-readonly`
    - PowerShell: `icacls C:\test-readonly /remove:d "${env:USERNAME}"`、`Remove-Item C:\test-readonly`

#### 6. ワーカーの異常終了・時間切れ（自動テストでの確認）

配布用バイナリにはテスト用のクラッシュフック（`test-hooks` 機能）はセキュリティ上組み込まれていないため、この項目は自動テストスイートで保証されていることを確認します。

- **該当する自動テスト**:
  - `crates/worker/tests/worker.rs`:
    - `hooks::a_crash_is_reported_and_the_worker_is_not_used_again`（ワーカー異常終了時にメインプロセスが検知し、新しいワーカーで継続できること）
    - `hooks::a_hang_times_out_and_kills_the_worker`（時間切れ時にワーカーを強制終了すること）
    - `hooks::allocations_within_the_limit_succeed`（メモリ制限内の確保が成功すること）
    - `hooks::exceeding_the_memory_limit_ends_only_the_worker`（メモリ上限 2 GiB 超過時にワーカーのみが終了すること）
  - `src-tauri/tests/pool.rs`:
    - `recovers_after_worker_crashes`（クラッシュ後にプールが回復すること）
    - `recovers_after_worker_times_out`（タイムアウト後にプールが回復すること）
  - `src-tauri/tests/jobs.rs`:
    - `worker_crashed_isolated_and_continues_batch`（一括処理中に 1 つの PDF でワーカーがクラッシュしても後続の処理が継続すること）
- **確認手順**:
  端末で以下のコマンドを実行する（`test-hooks` 機能を有効にするため、必ず `--features test-hooks` を指定する）:
  ```bash
  cargo test -p mcleaner-worker --test worker hooks --features test-hooks -- --nocapture
  cargo test -p metadata-cleaner --test pool recovers_after --features test-hooks -- --nocapture
  cargo test -p metadata-cleaner --test jobs worker_crashed --features test-hooks -- --nocapture
  ```
- **期待する結果**:
  すべてのテストが PASS すること。

---

## 項目 5: 出力名の重複防止（既存ファイルとの重複、同一処理内での重複）

要件: FR-04、FR-05、design §6.4。既存ファイルや同一処理内でのファイル名衝突時に、番号付き別名で保存され既存ファイルが上書きされないことを確認します。

### 準備

- `clean.jpg`
- 空の出力先フォルダ

### 操作

1. **既存ファイルとの重複**:
   - 保存先フォルダに、手作業で空の `clean.jpg` をあらかじめ作成しておく。
   - アプリで `clean.jpg` を追加し、上記保存先を指定して「消して保存」をクリックする。
2. **同一処理内での重複**:
   - 別々のフォルダにある同じファイル名（例: `folderA/clean.jpg` と `folderB/clean.jpg`）を一覧に追加する。
   - 同じ保存先フォルダを指定して「消して保存」をクリックする。

### 期待する結果

- 既存の `clean.jpg` は上書きされず、新しく `clean (1).jpg` として保存される。
- 行をクリックして選択したとき、右側の詳細欄に「保存した名前: clean (1).jpg」と表示される。
- 同名だった 2 つのファイルは、それぞれ `clean.jpg` と `clean (1).jpg` として保存される。

### 自動テストでの確認

- `src-tauri/tests/jobs.rs`:
  - `naming_collision_and_noclobber`
  - `saved_name_only_reported_when_different`
- `src/AppIntegration.test.tsx`:
  - `criterion 7: displays savedName only when present (savedName があるときだけ「保存した名前」が出る)`

---

## 項目 6: 一括処理のキャンセル（不完全なファイルが残らないことの確認）

要件: FR-05、design §6.5。処理途中でキャンセルしたとき、書き込み途中の破損ファイルや一時ファイルが保存先に残らないことを確認します。

### 準備（使うファイル）

- `cancel-batch/`（`npm run manual-test:files` で生成した 100 ファイル）
- 空の出力先フォルダ

### 操作

1. 空の出力先フォルダを用意して指定する。
2. 「フォルダを追加」またはドラッグ＆ドロップで `cancel-batch/` 内の全 100 ファイルを追加する。
3. 下部バーに「100 件から情報を消して保存します」と表示されていることを確認し、「消して保存」をクリックする。
4. 下部バーの進捗バーが進み、数件（例: 2〜5 件程度）処理されたタイミングで「キャンセル」ボタンをクリックする。

### 期待する結果

- 「キャンセル」をクリックした直後にボタンの表示が「キャンセル中…」に変わり、操作が無効化される。
- 処理中だった現在のファイルが保存された後、直ちに処理が停止し、ボタンの表示が「消して保存」に戻る。
- 上部バナーに「キャンセルしました · 保存 {完了数} 件 · 未処理 {未処理数} 件」（例: 「キャンセルしました · 保存 3 件 · 未処理 97 件」）が表示される。
- 表の各行の状態は、処理中だったファイルまで（保存されたファイル）が緑の「✓ 完了」、まだ始まっていなかったファイルが「キャンセル」になる。
- 下部バーに「キャンセルしました」と表示される。
- 保存先フォルダの内容をコマンドで確認する:
  - **Linux**:
    ```bash
    ls -A <保存先フォルダ>
    ```
  - **Windows**（PowerShell）:
    ```powershell
    Get-ChildItem -Force <保存先フォルダ>
    ```
  - 保存されたファイルはビューアーで正常に開くことができ、破損していない。
  - 書き込み途中の不完全なファイルや、ドットで始まる一時ファイル（`.` で始まる隠しファイルなど）は一切残っていない。

### 自動テストでの確認

- `src-tauri/tests/jobs.rs`:
  - `cancellation_stops_next_and_leaves_no_temp_files`
- `src/AppIntegration.test.tsx`:
  - `criterion 9: immediately enters cancelling state when cancel is pressed (キャンセルを押すと直ちに「キャンセル中…」になる)`
- `src/features/job/useJobRunner.test.tsx`:
  - `cancels running job`

---

## 項目 7: 両 OS でのインストーラーによるインストールと起動の確認

要件: NFR-04。GitHub Actions でビルドされたインストーラーパッケージから導入し、正常に起動して各機能が動作することを確認します。

### 操作

#### Windows

1. GitHub Releases（またはワークフローの成果物）から NSIS インストーラー（`Metadata*Cleaner_*_x64-setup.exe`）をダウンロードする。
2. 実行する。Windows Defender SmartScreen の警告（「Windows によって PC が保護されました」）が表示されたら、「詳細情報」→「実行」をクリックして進める。
3. インストールウィザードを完了し、スタートメニューまたはデスクトップから「Metadata Cleaner」を起動する。
4. 項目 2〜6 の各操作を実施し、正常に動作することを確認する。

#### Linux (Ubuntu 22.04 以降 / WSLg)

1. GitHub Releases（またはワークフローの成果物）から AppImage（`Metadata*Cleaner_*_amd64.AppImage`）または `.deb` パッケージ（`Metadata*Cleaner_*_amd64.deb`）をダウンロードする。
2. **AppImage の場合**:
   - Ubuntu 24.04 では `sudo apt install libfuse2t64`、Ubuntu 22.04 では `sudo apt install libfuse2` が導入されていることを確認する。
   - `chmod +x Metadata*Cleaner_*_amd64.AppImage` で実行権限を付与し、起動する。
3. **.deb パッケージの場合**:
   - `sudo apt install ./Metadata*Cleaner_*_amd64.deb` でインストールし、`metadata-cleaner` コマンドまたはアプリケーションメニューから起動する。
4. 項目 2〜6 の各操作を実施し、正常に動作することを確認する。

#### 追加の確認（インストールした実機で）

1. **保存したファイルの権限（Linux）**: 保存したファイルを端末で `ls -l <保存先フォルダ>` で確認し、`-rw-r--r--`（umask 022 通常設定時）になること。
2. **ブラウザーのショートカット無効化**: アプリのウィンドウにフォーカスがある状態で、Ctrl+F、Ctrl+P、Ctrl+Shift+P、Ctrl+R、F5、Ctrl+J、F7 を押す。あわせて Ctrl+C / Ctrl+V（コピー・貼り付け）と Ctrl+＋ / Ctrl+− / Ctrl+0（拡大・縮小）を押す。
   - **期待結果**: Ctrl+F、Ctrl+P、F5 等を押しても何も起きない（検索バーや印刷画面、再読み込みが発生しない）。コピー・貼り付けと拡大・縮小は通常通り機能すること。

### 自動テストでの確認

- `src/features/shortcuts/blockedShortcuts.test.ts`:
  - `treats Command like Ctrl`
- `src/styles/tokens.test.ts`:
  - `does not hard-code color values in component CSS files`
  - `verifies design §10.3 specific contrast ratios in light theme`

---

## 項目 8: 外部通信の確認（通信が発生しないことの確認）

要件: NFR-01、design §9。アプリが外部ネットワークと一切通信しないこと（プライバシー保護）を確認します。

### 操作

#### Windows

1. Metadata Cleaner を閉じた状態で、PowerShell で次を実行する（2 分間記録する）。
2. 実行が始まったらすぐに Metadata Cleaner を起動し、ファイルの追加、消去などの一連の操作を行う。
3. 2 分たつと、外への TCP 接続の一覧（`<プロセス名> <IP アドレス>:<ポート>`）が出る。

```powershell
$seen = @{}
$end = (Get-Date).AddSeconds(120)
while ((Get-Date) -lt $end) {
  $ids = @(Get-CimInstance Win32_Process -Filter "Name='metadata-cleaner.exe' OR Name='msedgewebview2.exe'" |
    Where-Object { $_.Name -eq 'metadata-cleaner.exe' -or $_.CommandLine -match 'com\.w034ff\.metadatacleaner' } |
    ForEach-Object { $_.ProcessId })
  Get-NetTCPConnection -ErrorAction SilentlyContinue |
    Where-Object { $ids -contains $_.OwningProcess -and $_.RemoteAddress -notin '0.0.0.0', '::', '127.0.0.1', '::1' } |
    ForEach-Object {
      $name = (Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue).ProcessName
      $seen["$name $($_.RemoteAddress):$($_.RemotePort)"] = $true
    }
  Start-Sleep -Seconds 1
}
$seen.Keys | Sort-Object
```

4. `msedgewebview2` の行が出たら、その IP アドレスの持ち主を調べる:
   ```powershell
   Invoke-RestMethod "https://ipinfo.io/<IP アドレス>/org"
   ```

#### Linux

1. アプリを起動し、各種ファイルを追加・消去する。
2. 端末で以下のコマンドを実行し、アプリ本体およびワーカー（同一実行ファイル `metadata-cleaner`）、画面表示プロセス（WebKitGTK）のネットワークソケットを確認する:
   ```bash
   ss -tunp | grep -E "metadata-cleaner|WebKit"
   ```

### 期待する結果

- **Windows**: `metadata-cleaner` の行が 1 つも出ない（本体とワーカーは外に接続しない）。`msedgewebview2` の行は、持ち主が Microsoft（`AS8075 Microsoft Corporation`）なら、README に書いた WebView2 ランタイム自身の通信として例外に記録する。Microsoft 以外への接続があれば不具合として記録する。
- **Linux**: アプリ本体、ワーカープロセス、および WebKit 表示プロセスが外部のサーバーへ一切接続を行っていないこと。

---

## 項目 9: ライセンス表示と CI 検査の確認

要件: FR-08、NFR-05、design §8.3。依存ライブラリのライセンス検査が CI で通過しており、アプリ画面上で第三者ライセンス一覧が閲覧できることを確認します。

### 操作

1. **CI ワークフローの検査結果確認**:
   - GitHub Actions の `CI` ワークフローのログを確認する。
   - `License Check` ステップ（`npm run licenses:check`）が成功（緑のチェック）していることを確認する。
   - 手元の環境でも以下のコマンドを実行し、成功することを確認する:
     ```bash
     npm run licenses:check
     ```
2. **アプリ画面でのライセンス表示確認**:
   - アプリを起動し、上部バーの右側にある「このアプリについて」ボタンをクリックする。
   - ダイアログが表示されることを確認する:
     - 「このアプリについて」のタイトル
     - 「バージョン 0.1.0」
     - 「このアプリのライセンス（MIT）」
     - 「第三者ライセンスを表示」ボタン
   - 「第三者ライセンスを表示」ボタンをクリックすると、依存ライブラリ一覧が展開される。
   - 任意のライブラリの「ライセンス本文を表示」をクリックし、完全なライセンス条文が表示されることを確認する。
   - 「閉じる」ボタン、またはダイアログ外側をクリックしてダイアログが閉じることを確認する。

### 期待する結果

- CI のライセンス検査（許可リスト外ライセンス・禁止クレートの排除）がパスしている。
- アプリのダイアログ上で、全サードパーティライセンスが欠落なく閲覧できる。

### 自動テストでの確認

- `src/licenses/index.test.ts`:
  - `includes runtime dependencies and excludes dev-only crates`
  - `include Rust crates and npm packages`
- `src/App.test.tsx`:
  - `shows the version and this app's license`
  - `lists the third-party licenses when expanded`
