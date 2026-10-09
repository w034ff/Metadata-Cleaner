# Metadata Cleaner

[日本語 (Japanese)](README.ja.md)

Metadata Cleaner is an open-source desktop application that removes metadata from images (JPEG, PNG, WebP) and PDF files locally on your machine.
All processing runs entirely on your computer without uploading any files to external online services. Supported operating systems are Windows and Linux (x64).

## Installation

Download the appropriate installer or package for your operating system from the [Releases](https://github.com/w034ff/Metadata-Cleaner/releases) page.

### Windows

- **Formats**: The NSIS installer (`.exe`) is recommended for individual users. The MSI installer (`.msi`) is intended for administrators performing enterprise deployments. Do not install both formats.
- **Windows SmartScreen Notice**: Because the binaries are not code-signed, Windows Defender SmartScreen may display a warning ("Windows protected your PC") on first run or installation. To continue, click "**More info**", then click "**Run anyway**".
- **WebView2 Runtime**: If the Microsoft Edge WebView2 runtime is not already installed on your system, the installer will automatically download and install it.
- **No Network Communication**: The application itself makes no network requests (contains no telemetry and no update checks). On Windows, the Microsoft Edge WebView2 runtime used to display the UI may connect to Microsoft services on its own; the application cannot disable this.

### Linux

- **Formats**: Available as an AppImage or a Debian package (`.deb`).
- **Requirements**: WebKitGTK 4.1 (e.g. `libwebkit2gtk-4.1-0` on Ubuntu 22.04 or later).
- **AppImage**:
  AppImages require FUSE 2, which Ubuntu 22.04 and later do not install by default. Install it first (`libfuse2t64` on Ubuntu 24.04 and later, `libfuse2` on 22.04):
  ```bash
  sudo apt install libfuse2t64
  ```
  Make the downloaded AppImage executable and run it:
  ```bash
  chmod +x Metadata*Cleaner_*_amd64.AppImage
  ./Metadata*Cleaner_*_amd64.AppImage
  ```
- **.deb Package**:
  Install via `apt` to ensure all system dependencies are resolved:
  ```bash
  sudo apt install ./Metadata*Cleaner_*_amd64.deb
  ```

## What is Removed and What is Kept

### Removed

- **Images (JPEG, PNG, WebP)**:
  - EXIF (GPS location, date and time, camera/device make and model, serial numbers, maker notes, embedded thumbnails, etc.)
  - XMP
  - IPTC and Photoshop information
  - Comments (JPEG COM, PNG `tEXt`, `zTXt`, `iTXt`)
  - PNG `tIME`
  - Data appended after JPEG EOI (motion photo videos, subsequent MPF images, etc.)
  - Other non-visual metadata
- **PDF**:
  - Document information dictionary (`/Info`: Title, Author, Subject, Keywords, Creator, Producer, CreationDate, ModDate)
  - Document identifier (`/ID`)
  - Document-, page-, and stream-level XMP metadata (`/Metadata`)
  - Private application data (`/PieceInfo`)
  - Embedded page thumbnails (`/Thumb`)
  - Earlier revisions from incremental updates
  - EXIF and XMP metadata embedded in image streams using solely the `DCTDecode` filter

### Kept

To preserve visual fidelity without altering pixels or pages:

- **Image Orientation**: EXIF Orientation (normalized TIFF containing only orientation and resolution when present)
- **Color Profiles**: Embedded ICC profiles
- **Color and Display Metadata**:
  - JPEG: Adobe APP14 (required for CMYK and YCCK color conversion)
  - PNG: `gAMA`, `cHRM`, `sRGB`, `cICP`, `mDCV`, `cLLI`, `sBIT`, `bKGD`, `pHYs`, `acTL`, `fcTL`, `fdAT`, `tRNS`
  - WebP: `VP8 `, `VP8L`, `VP8X`, `ALPH`, `ANIM`, `ANMF`
  - Print resolution / density: JFIF density, PNG `pHYs`, and EXIF resolution
- **Image Pixel Data**: Compressed image bitstreams (JPEG DCT scans, PNG `IDAT`, WebP image bitstreams) are preserved byte-for-byte without re-encoding.

### What Cannot Be Removed

- **Visible Content**: Information visible in pixels or page content (landscapes, text, people, watermarks, or text in PDF pages)
- **File Names**: Information contained in file names
- **PDF Content Not Stripped**:
  - Author names in annotations (`/T`)
  - Form field values
  - Embedded file attachments
  - JavaScript actions
  - Bookmark (outline) titles
  - JPEG images using filters other than solely `DCTDecode` (such as `FlateDecode` combinations), or XMP inside JPEG 2000 images
- **Unsupported Files**:
  - Password-protected or encrypted PDFs and digitally signed PDFs cannot be processed (they are rejected with an error message to prevent breaking encryption or signatures).
  - Formats other than JPEG, PNG, WebP, and PDF (such as HEIC, AVIF, TIFF, BMP, GIF, Office documents, audio, and video) are not supported.

## Building from Source

### Prerequisites

- **Rust**: Toolchain pinned in `rust-toolchain.toml`
- **Node.js**: Version specified in `.nvmrc` (v24, or `>=24.15.0` per `package.json`)
- **npm**: 11 or higher
- **Linux Build Dependencies** (when building on Linux):
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
- Note: This repository's `.npmrc` enforces supply-chain security by disallowing packages published less than 7 days ago (`min-release-age=7`) and disabling installation scripts (`ignore-scripts=true`).

### Build Steps

1. Install project dependencies:
   ```bash
   npm ci
   ```
2. Start the application in development mode:
   ```bash
   npm run tauri dev
   ```
3. Build production release packages:
   ```bash
   npm run tauri build
   ```
   Bundled packages will be generated under `target/release/bundle/`.

## License

This project is licensed under the [MIT License](LICENSE).

Third-party dependencies and their licenses can be viewed within the application via the "About this app" button in the top bar.
