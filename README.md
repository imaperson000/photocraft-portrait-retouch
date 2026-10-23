<p align="center">
  <img src="assets/app-icon/hicolor/256x256/apps/ai.storyteller.photocraft.png" width="128" height="128" alt="PhotoCraft icon: an engraved nine-tailed fox on a blue tile">
</p>

<h1 align="center">PhotoCraft 人像修圖版</h1>

<p align="center">
  以開源影像編輯器 <a href="https://github.com/storytold/photocraft">PhotoCraft</a> 0.14.0 為基礎修改的版本，<br>
  加入一整組「<b>濾鏡 › 人像修圖</b>」功能。
</p>

> 這是個人修改版，**不是 ArtCraft 官方產品**，也和原專案沒有隸屬或背書關係。
> 原版 PhotoCraft 由 ArtCraft 團隊與社群開發：<https://github.com/storytold/photocraft>。

## 新增功能

全部在選單 **濾鏡 › 人像修圖**（英文介面 Filter › Portrait Retouch）：

- **頻率分離**：一鍵產生「低頻／高頻」兩層，疊起來和原圖一樣，紋理與顏色分開修。
- **加深減淡圖層**：50% 中性灰（柔光／覆蓋）、加深／加亮曲線層、黑白觀察組。
- **磨皮**：保留邊緣與毛孔的磨皮圖層，可以自動偵測膚色產生遮色片。
- **皮膚細節檢視**：一鍵開關的黑白高對比（或曝光過度）檢視，斑駁與不均一目了然。
- **光暈**：從亮部產生漫反射或 2／4／6／8 道條紋光芒，有色散、旋轉、上色等參數，可隨時重新編輯。
- **局部光暈**：「開始畫」→ 用筆刷畫出要發光的地方 →「套用」，只有畫到的地方會發光。

## 原版 PhotoCraft 內建工具

<h4>🧰 67 tools</h4>
Move · Artboard · Rectangular and Elliptical Marquee · Single Row Marquee · Single Column Marquee · Lasso · Polygonal Lasso · Magnetic Lasso · Object Selection · Quick Selection · Magic Wand · Crop · Perspective Crop · Slice · Slice Select · Frame · Eyedropper · Color Sampler · Ruler · Note · Count · Remove · Spot Healing Brush · Healing Brush · Patch · Content-Aware Move · Red Eye · Brush · Pencil · Mixer Brush · Color Replacement · Clone Stamp · Pattern Stamp · History Brush · Eraser · Background Eraser · Magic Eraser · Gradient · Paint Bucket · Blur · Sharpen · Smudge · Dodge · Burn · Sponge · Pen · Freeform Pen · Curvature Pen · Add Anchor Point · Delete Anchor Point · Convert Point · Horizontal and Vertical Type · Horizontal and Vertical Type Mask · Path Selection · Direct Selection · Rectangle · Ellipse · Triangle · Polygon · Line · Custom Shape · Hand · Rotate View · Zoom

## 文件

**功能說明、每個功能的操作步驟、開發環境、改了哪些檔案，以及從零開始的保母級安裝教學，都在
[PORTRAIT_RETOUCH.zh-TW.md](PORTRAIT_RETOUCH.zh-TW.md)。**

PhotoCraft 本身的架構、開發與貢獻規範見 [AGENTS.md](AGENTS.md) 與 [`docs/`](docs/)。

## 快速開始（已經裝好 Rust 與 C++ 編譯工具的話）

```powershell
git clone https://github.com/imaperson000/photocraft-portrait-retouch.git
cd photocraft-portrait-retouch
cargo run --release -p photocraft -- --new-instance
```

第一次編譯需要 20–60 分鐘，以及大約 10 GB 的硬碟空間。

## 授權

程式碼採用 **MIT 或 Apache-2.0**（擇一），見 [LICENSE-MIT](LICENSE-MIT)、[LICENSE-APACHE](LICENSE-APACHE)。
Copyright (c) 2026 ArtCraft Team and the PhotoCraft contributors；必要的聲明在 [NOTICE](NOTICE)。
這個版本新增的人像修圖功能以相同授權釋出。

內附的字型、圖示、圖片等素材保留各自的開源授權，作者、來源與授權列在 [ATTRIBUTION.md](ATTRIBUTION.md)。

**商標**：ArtCraft 的名稱、文字標誌與 logo 是 ArtCraft 團隊的商標，不在上述開源授權範圍內。依照其商標條款
（[`docs/brand/LICENSE-brand.txt`](docs/brand/LICENSE-brand.txt)），這個修改版已經移除這些商標檔案，以及程式中以 ArtCraft 名義的社群連結。

<sub>Adobe、Photoshop 等是 Adobe Inc. 的商標或註冊商標。PhotoCraft 是獨立的開源專案，與 Adobe Inc. 沒有隸屬、贊助或背書關係；這些名稱只用來說明相容的工作流程。</sub>
