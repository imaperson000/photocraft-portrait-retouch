# PhotoCraft 人像修圖版：說明與安裝教學

這個 repo 是以 ArtCraft 團隊的開源影像編輯器 **PhotoCraft 0.14.0**（[storytold/photocraft](https://github.com/storytold/photocraft)，MIT / Apache-2.0）為基礎修改的版本，**不是 ArtCraft 官方產品**。
在原本的程式上加了一組「人像修圖」功能，全部放在選單 **濾鏡 › 人像修圖**（英文介面是 Filter › Portrait Retouch）。

目錄：

1. [新增了什麼功能](#1-新增了什麼功能)
2. [每個功能怎麼用](#2-每個功能怎麼用)
3. [開發環境](#3-開發環境)
4. [保母級安裝教學（Windows）](#4-保母級安裝教學windows)
5. [改了哪些檔案](#5-改了哪些檔案)
6. [技術細節](#6-技術細節)
7. [已知限制與還沒做的](#7-已知限制與還沒做的)
8. [授權與注意事項](#8-授權與注意事項)

---

## 1. 新增了什麼功能

| 選單項目 | 做什麼 |
|---|---|
| **頻率分離…** | 一鍵產生「頻率分離」群組：「低頻」（顏色與明暗）＋「高頻」（紋理，線性光源）。兩層疊起來和原圖一樣，可以分開修。 |
| **加深減淡圖層…** | 一鍵產生「加深減淡」群組：50% 中性灰層（柔光或覆蓋）、可選的「加深／加亮」曲線層（黑色遮色片）、以及「觀察組」（黑白＋對比曲線）。 |
| **磨皮…** | 一鍵產生「磨皮」圖層：保留邊緣與毛孔紋理，只撫平中間尺度的斑駁；遮色片可自動偵測膚色。 |
| **皮膚細節檢視** | 開／關一個檢視群組（黑白＋強對比），讓皮膚的斑駁與不平整看得一清二楚。 |
| **皮膚細節檢視（曝光過度）** | 同上，但用曝光過度（solarize）曲線，連很小的明暗變化都會變成明顯條紋。 |
| **光暈…** | 從亮部生成光暈到獨立的「光暈」圖層（濾色模式）。有漫反射與條紋（2／4／6／8 道）兩種模式，以及強度、半徑、閾值、曝光、光散射（色散）、旋轉、條紋細節、上色（色相／飽和度）、亮度等參數。可以重新編輯。 |
| **局部光暈：開始畫** | 清除光暈，用筆刷畫出「要發光的地方」（畫到的地方顯示紅色）。 |
| **局部光暈：套用** | 只讓畫到的區域裡的亮光產生光暈。 |
| **反轉光暈遮色片／清除光暈遮色片** | 一步反轉或重設光暈層的遮色片。 |

所有產生的圖層、群組名稱都是繁體中文（例如「低頻」「高頻」「中性灰」「磨皮」「光暈」）。這些名稱存在檔案裡，不會隨介面語言改變。

每個功能都是 PhotoCraft 的 **engine 指令**，所以除了選單之外，也可以從 CLI、JSON 控制通道或 MCP 呼叫（指令 id 見 [第 6 節](#6-技術細節)）。

---

## 2. 每個功能怎麼用

先用「檔案 › 開啟舊檔」開一張照片（RGB 或灰階；CMYK、Lab 文件目前不支援這組功能）。

### 頻率分離
1. 濾鏡 › 人像修圖 › **頻率分離…**
2. 「半徑」：0 = 依照片大小自動決定（24MP 約 10px）。皮膚紋理越粗，半徑可以越大。
3. 勾「修飾圖層」會多一層空白的「低頻修飾」，可以在上面修顏色、不動到原本的低頻。
4. 按確定後會自動選到「高頻」：用修復筆刷／仿製印章修紋理；切到「低頻」修色塊與明暗。

### 加深減淡（中性灰）
1. 濾鏡 › 人像修圖 › **加深減淡圖層…**
2. 「方法」：
   - **灰色**：一層 50% 中性灰。用**白色**筆刷畫 = 加亮，**黑色**筆刷畫 = 加深（建議筆刷不透明度 5–10%）。
   - **曲線**：「加深」「加亮」兩層曲線，各帶黑色遮色片；在遮色片上用白色畫，哪裡畫就作用在哪裡。
   - **兩者**：兩種都建立。
3. 「混合模式」：柔光（比較柔和）或覆蓋（比較強烈）。
4. 「觀察組」：黑白＋對比曲線，方便看明暗。**修完記得把「觀察組」關掉（點眼睛）或刪除**，不然輸出會是黑白的。

### 磨皮
1. 濾鏡 › 人像修圖 › **磨皮…**
2. 參數：
   - **強度** 0–100（預設 60）
   - **半徑**：要撫平的斑駁大小，0 = 自動
   - **細節** 0–100：保留多少毛孔紋理（越高越自然）
   - **臨界值**：邊緣保護，越低越能保住五官輪廓
   - **遮色片**：膚色（自動偵測，預設）／全部隱藏（自己用白色筆刷畫要磨的地方）／全部顯現
3. 效果太強：直接降低「磨皮」圖層的不透明度。

### 皮膚細節檢視
- 濾鏡 › 人像修圖 › **皮膚細節檢視**（或「皮膚細節檢視（曝光過度）」）：按一次開啟，再按一次關閉。
- 開著時目前選取的圖層不會改變，可以一邊看一邊修。

### 光暈
1. 濾鏡 › 人像修圖 › **光暈…**，調參數後按確定。
2. **重新編輯**：在圖層面板選取「光暈」圖層，再執行一次「光暈…」，對話框會帶出原本的參數；改完會在同一層重新計算，並保留遮色片與不透明度。
3. 一般的遮色片（遮掉部分光暈）：直接在光暈層的遮色片上用筆刷畫黑色。

### 局部光暈（用筆刷決定哪裡發光）
1. 濾鏡 › 人像修圖 › **局部光暈：開始畫**
   - 光暈會全部清掉，畫面上出現紅色遮色片顯示，工具自動切成**筆刷**。
2. 在想要發光的地方畫，畫到的地方會變**紅色**；畫錯用**橡皮擦（E）**擦掉。
3. 濾鏡 › 人像修圖 › **局部光暈：套用**
   - 只有紅色區域裡的亮光會產生光暈，光暈會自然擴散到筆觸外面；紅色顯示會自動關閉。
4. 想調整強度、大小：選取光暈圖層，再執行「光暈…」，畫好的區域會保留。

---

## 3. 開發環境

這個版本是在下面的環境編譯與測試的：

| 項目 | 版本 / 設定 |
|---|---|
| 作業系統 | Windows 10 Pro 22H2（10.0.19045），x64 |
| C++ 編譯工具 | Visual Studio 2022 Build Tools，工作負載「使用 C++ 的桌面開發」（提供 `link.exe` 與 Windows SDK） |
| Rust | 1.99.1 stable（`x86_64-pc-windows-msvc`），專案要求最低 1.95 |
| Rust 元件 | `clippy`、`rustfmt`、`wasm32-unknown-unknown` target（檢查網頁版能編譯用） |
| Git | 任何近期版本（Git for Windows） |
| Rust 安裝位置 | `D:\rust\cargo`（`CARGO_HOME`）、`D:\rust\rustup`（`RUSTUP_HOME`） |
| 編譯輸出 | `D:\rust\target`（release 版程式）、`D:\rust\target-dev`（跑測試用） |
| 硬碟空間 | 每個編譯輸出資料夾約 10 GB，Rust 工具鏈約 1–2 GB，原始碼約 1 GB。**建議至少留 25 GB。** |

為什麼放 D 槽：C 槽空間不夠（一次完整編譯就要 10 GB 以上），所以 Rust 和編譯輸出都放到空間比較多的 D 槽。你的電腦如果 C 槽空間夠，全部放預設位置也可以（見下面的教學）。

---

## 4. 保母級安裝教學（Windows）

### 只想用、不想編譯：直接下載（最簡單）

1. 打開 <https://github.com/imaperson000/photocraft-portrait-retouch/releases/latest>。
2. 在「Assets」下面點 **`photocraft-portrait-retouch-…-windows-x64.zip`** 下載（約 33 MB）。
3. 在下載的 zip 上按右鍵 →「**解壓縮全部**」，選一個位置（例如桌面）。
4. 打開解壓縮出來的資料夾，點兩下 **`photocraft.exe`**。
5. 出現「Windows 已保護您的電腦」時，按「**其他資訊**」→「**仍要執行**」（程式沒有付費數位簽章，第一次才會出現）。
6. 想要桌面捷徑：在 `photocraft.exe` 上按右鍵 →「傳送到」→「桌面（建立捷徑）」。

資料夾裡的 `portable.txt` 讓這個版本把設定存在旁邊的 `PhotoCraftData` 資料夾。**已經裝過原版 PhotoCraft 也不會互相影響**，兩個可以同時開。請不要刪掉 `portable.txt`。

不需要下面的步驟 1–7，那些是給想自己從原始碼編譯的人。

### 從原始碼編譯

> 以下每一步都可以直接照做。指令請在 **PowerShell** 裡輸入（開始選單搜尋「PowerShell」）。
> 第一次完整編譯大約需要 **20–60 分鐘**（看電腦速度），之後改程式只會重編改到的部分，快很多。

### 步驟 1：安裝 Visual Studio 2022 Build Tools（C++ 編譯工具）

Rust 在 Windows 上需要微軟的連結器（linker）。

1. 打開 <https://visualstudio.microsoft.com/zh-hant/visual-cpp-build-tools/>，下載「Build Tools」。
2. 執行安裝程式，勾選 **「使用 C++ 的桌面開發」**（Desktop development with C++）。右邊預設勾選的「MSVC」和「Windows 11 SDK」（或 Windows 10 SDK）都要保留。
3. 按「安裝」，等它跑完（約 5–8 GB）。
4. 安裝完**重新開機**一次比較保險。

> 已經裝過 Visual Studio 2022（社群版、專業版都可以）且有「使用 C++ 的桌面開發」，就可以跳過這步。

### 步驟 2：安裝 Git

1. 打開 <https://git-scm.com/download/win>，下載並安裝，選項全部用預設即可。
2. 確認裝好了：開一個**新的** PowerShell，輸入：
   ```powershell
   git --version
   ```
   有顯示版本號就成功了。

### 步驟 3：安裝 Rust

#### 3-A：一般情況（C 槽空間夠，至少 25 GB）

1. 打開 <https://rustup.rs/>，下載 `rustup-init.exe` 並執行。
2. 出現選項時直接按 **Enter**（選 1，預設安裝）。
3. 裝完**關掉 PowerShell 再開一個新的**，然後輸入：
   ```powershell
   rustc --version
   cargo --version
   ```
   兩行都有版本號就成功了。
4. 安裝額外元件：
   ```powershell
   rustup component add clippy rustfmt
   rustup target add wasm32-unknown-unknown
   ```

#### 3-B：要把 Rust 裝到 D 槽（C 槽空間不夠時）

1. 建立資料夾，並設定兩個「使用者環境變數」，讓 Rust 裝到 D 槽：
   ```powershell
   New-Item -ItemType Directory -Force D:\rust | Out-Null
   [Environment]::SetEnvironmentVariable("CARGO_HOME", "D:\rust\cargo", "User")
   [Environment]::SetEnvironmentVariable("RUSTUP_HOME", "D:\rust\rustup", "User")
   ```
2. **（很重要）如果 D 槽是另外加裝的硬碟**，它的權限有時只允許「寫入」、不允許「刪除／改名」，rustup 安裝會出現「**存取被拒（os error 5）**」。用下面這行，只對 `D:\rust` 這個資料夾給自己完整權限（不會動到 D 槽其他地方）：
   ```powershell
   icacls D:\rust /grant "${env:USERNAME}:(OI)(CI)F" /T
   ```
3. 關掉 PowerShell、開一個新的（讓環境變數生效），然後下載並執行 rustup：
   ```powershell
   Invoke-WebRequest -Uri "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" -OutFile D:\rust\rustup-init.exe
   D:\rust\rustup-init.exe -y --default-toolchain stable --profile minimal -c clippy -c rustfmt -t wasm32-unknown-unknown
   ```
4. 再開一個新的 PowerShell，確認：
   ```powershell
   rustc --version
   cargo --version
   ```
   如果顯示「找不到 cargo」，代表 PATH 還沒更新：登出 Windows 再登入，或暫時用完整路徑 `D:\rust\cargo\bin\cargo.exe`。
5. 讓編譯輸出也放 D 槽（再加一個使用者環境變數，設完要開新的 PowerShell）：
   ```powershell
   [Environment]::SetEnvironmentVariable("CARGO_TARGET_DIR", "D:\rust\target", "User")
   ```

### 步驟 4：下載這個 repo

```powershell
cd C:\
mkdir build -Force
cd C:\build
git clone https://github.com/imaperson000/photocraft-portrait-retouch.git
cd photocraft-portrait-retouch
```

下載大約 100 MB。

### 步驟 5：編譯

在 repo 資料夾裡（`C:\build\photocraft-portrait-retouch`）執行：

```powershell
cargo build --release -p photocraft
```

- 第一次會下載很多套件並編譯，**20–60 分鐘**都是正常的，過程中會一直出現 `Compiling ...`。
- 最後看到 `Finished release [optimized] target(s)` 就是成功了。
- 中間如果出現 `error: linker 'link.exe' not found`：回到**步驟 1**，確認有裝「使用 C++ 的桌面開發」，並重新開機。

### 步驟 6：執行

```powershell
cargo run --release -p photocraft -- --new-instance
```

或直接點兩下編好的程式：

- 3-A 預設安裝：`C:\build\photocraft-portrait-retouch\target\release\photocraft.exe`
- 3-B 有設 `CARGO_TARGET_DIR`：`D:\rust\target\release\photocraft.exe`

想要桌面捷徑：在 `photocraft.exe` 上按右鍵 → 「傳送到」 → 「桌面（建立捷徑）」。

> **重新編譯前要先關掉 PhotoCraft**：程式開著的時候 Windows 會鎖住 `photocraft.exe`，編譯會出現「failed to remove file … 存取被拒」。
> 注意標題列有 `*` 代表有還沒存的修改，先存檔再關。

### 步驟 7（選做）：跑測試，確認一切正常

```powershell
cargo test -p photocraft-engine --lib -- portrait_cmds glow_cmds
cargo test -p photocraft-algo --lib glow
```

看到 `test result: ok` 就代表新功能的測試全部通過。完整的檢查流程見 [AGENTS.md](AGENTS.md) 第 5 節。

### 之後更新程式

```powershell
cd C:\build\photocraft-portrait-retouch
git pull
cargo build --release -p photocraft
```

---

## 5. 改了哪些檔案

相對於原版 PhotoCraft 0.14.0（commit `ff4738810 Release 0.14.0`）：

**新增的檔案**

| 檔案 | 內容 |
|---|---|
| `crates/engine/src/portrait_cmds.rs` | 頻率分離、加深減淡、磨皮、皮膚細節檢視四組指令 |
| `crates/engine/src/portrait_cmds/tests.rs` | 上面那些指令的測試（8／16／32 位元、灰階、錯誤參數、復原） |
| `crates/algo/src/glow.rs` | 光暈演算法（亮部擷取、多層光暈、條紋、色散、上色），含單元測試 |
| `crates/engine/src/glow_cmds.rs` | 光暈相關指令（產生／重新編輯、局部光暈、遮色片反轉／清除） |
| `crates/engine/src/glow_cmds/tests.rs` | 光暈指令的測試 |
| `PORTRAIT_RETOUCH.zh-TW.md` | 這份說明 |

**修改的檔案**

| 檔案 | 改了什麼 |
|---|---|
| `crates/engine/src/lib.rs`、`commands.rs` | 註冊新模組與指令 |
| `crates/algo/src/lib.rs` | 註冊 `glow` 模組 |
| `crates/engine/src/channel_cmds.rs` | `target()` 改成 crate 內可呼叫 |
| `crates/engine/src/mask_view_cmds.rs` | `set_view()` 改成 crate 內可呼叫（局部光暈用來開關紅色遮色片顯示） |
| `crates/ui-egui/src/filter_dialog.rs` | 新指令加入「有參數對話框＋預覽」清單；光暈對話框在選到光暈圖層時帶入原參數 |
| `crates/ui-egui/src/lib.rs` | 「局部光暈：開始畫」後自動切換成筆刷工具 |
| `crates/ui-egui/src/i18n/*.tsv` | 新選單項目在全部 21 種語言的翻譯（繁中、簡中另外翻了參數名稱） |
| `docs/parity-checklist.md` | `cargo xtask parity` 自動重新產生 |
| `README.md` | 改寫成這個修改版的說明（原版 README 有大量 ArtCraft 商標與宣傳，修改版不能沿用），保留原版的工具清單段落（有測試會比對） |
| `docs/brand/*.png`、`*.svg` | 刪除（ArtCraft 商標，修改版必須移除） |
| `NOTICE`、`ATTRIBUTION.md` | 註明這是修改版、商標已移除；保留原本的著作權聲明 |
| `crates/ui-egui/src/links.rs`、`canvas.rs`、`dialogs.rs`、`crates/engine/src/shell_commands.rs` | 移除 ArtCraft Discord／網站連結與按鈕，其餘連結改指向這個 repo |
| `packaging/windows/photocraft.wxs`、`packaging/linux/*` | 安裝程式／套件的首頁與說明連結改為這個 repo |

**Commit 紀錄**（分支 `main`，原版之後的 commit）

1. `Add Filter > Portrait Retouch: frequency separation, dodge & burn, skin smoothing, glow`
2. `Add skin detail view and light-source glow masks`
3. `Local glow: paint the lights, then apply`
4. `Add Chinese guide for the portrait retouch build`（這份文件）
5. `Remove ArtCraft marks and community links for the public modified build`

---

## 6. 技術細節

### 指令 id（CLI／MCP／控制通道都能用）

| 指令 id | 參數 |
|---|---|
| `portrait.frequencySeparation` | `{"radius":0..250=0,"retouchLayer":bool=false}`（radius 0 = 自動） |
| `portrait.dodgeBurn` | `{"method":"gray\|curves\|both"="gray","blend":"softLight\|overlay"="softLight","helper":bool=true}` |
| `portrait.skinSmooth` | `{"strength":0..100=60,"radius":0..100=0,"detail":0..100=50,"threshold":2..255=24,"mask":"skin\|hideAll\|revealAll"="skin"}` |
| `portrait.skinDetailView` | `{"mode":"contrast\|solarize"="contrast"}`（再執行一次 = 關閉） |
| `portrait.skinDetailViewSolarize` | `{}` |
| `portrait.glow` | `{"mode":"bloom\|streaks","rays":"2\|4\|6\|8","intensity":0..100=100,"glowRadius":0..100=20,"threshold":0.01..1=0.25,"range":bool,"exposure":0..255=0,"dispersion":0..100=0,"rotation":-90..90=0,"detail":0..100=0,"colorize":bool,"hue":0..360=0,"saturation":0..100=80,"lightness":-100..0=0,"mask":bool=true,"maskUse":"output\|paint"="output"}` |
| `portrait.glowPaintStart` | `{}` |
| `portrait.glowPaintApply` | `{}` |
| `portrait.glowMaskInvert`／`portrait.glowMaskClear` | `{}` |

CLI 範例（對一張圖做頻率分離後存成 PSD）：

```powershell
cargo run --release -p photocraft-cli -- run in.jpg --cmd portrait.frequencySeparation --params '{\"radius\":8}' --out out.psd
```

（Windows PowerShell 5.1 會吃掉傳給程式的雙引號，所以 JSON 裡的 `"` 要寫成 `\"`；在 Git Bash 裡則直接寫 `'{"radius":8}'`。`run` 子指令的完整用法見 `docs/development.md`。）

### 演算法

- **頻率分離**：低頻 = 目前可見畫面的高斯模糊；高頻 = `(原圖 − 低頻) / 2 + 50% 灰`，混合模式線性光源。線性光源會把 `2·高頻 − 1` 加回去，所以兩層疊起來就是原圖：16／32 位元完全一致，8 位元誤差在 1 階以內（和 Photoshop 用「套用影像」做頻率分離的公式相同）。
- **加深減淡**：50% 灰在柔光、覆蓋下是中性色，所以剛建立時畫面完全不變；曲線層的遮色片是黑色，所以一開始也不作用。
- **磨皮**：`結果 = 原圖 + 強度 × (表面模糊(原圖) − 高斯(原圖, 細節半徑))`。表面模糊保住邊緣（五官輪廓），扣掉的小半徑高斯讓毛孔紋理留下來，被撫平的只有中間尺度（斑駁、膚色不均）。膚色遮色片用 YCbCr 色度範圍（Cb 77–127、Cr 133–173）加柔化邊緣與羽化。
- **光暈**：
  1. 亮部擷取：最亮色版超過「閾值」的像素保留（「範圍」開啟時是柔和過渡），乘上「曝光」增益。
  2. 漫反射：亮部做五層、相差一個八度的高斯模糊後加總（核心集中、尾巴長）；大小是圖片長邊的比例，所以和解析度無關。
  3. 條紋：每道光芒是一條單邊指數衰減的拖尾，用「每次步距乘 4 的四點取樣」重複幾次做出長拖尾；「條紋細節」越高越銳利。
  4. 光散射：每個色版用不同的擴散範圍（紅最遠、藍最近），產生彩色邊緣。
  5. 上色：保持亮度、換成指定的色相與飽和度；「亮度」壓暗；「強度」整體縮放。
  6. 為了速度，光暈在長邊最多 2048 px 的縮小版上計算，再雙線性放大回原尺寸（光暈本來就沒有細節）。
- **可重新編輯**：光暈參數存在光暈圖層自己的 XMP 中繼資料（命名空間 `https://photocraft.app/ns/glow/1.0/`），PSD、分層 TIFF、`.pcraft` 都會保存。
- **局部光暈**：「開始畫」把遮色片設為全白並停用（不影響合成），打開紅色遮色片顯示；在紅色顯示下畫筆會直接畫在遮色片上（黑色＝紅色＝要發光）。「套用」時把遮色片當成「哪些亮光可以發光」的權重，乘在亮部擷取上再計算光暈，所以光暈會自然擴散到筆觸外。

### 遵守的專案規範

- 所有功能都是 engine 指令，一次操作 = 一個復原步驟。
- 不 panic：所有錯誤參數、錯誤文件狀態都回傳錯誤訊息；`panic_hunt` 對抗性測試通過。
- 支援 8／16／32 位元與 RGB／灰階，測試涵蓋各種位元深度。
- `cargo clippy -D warnings`、`cargo xtask layers`（分層規則）、`cargo xtask wasm`（網頁版可編譯）全部通過。

---

## 7. 已知限制與還沒做的

- **光暈預設**（存取、刪除）還沒做。
- 光暈的**色相滴管**還沒做（目前用色相滑桿）。
- 局部光暈畫的時候不會即時預覽光暈，要按「局部光暈：套用」才會計算。
- 很小的光點（幾個像素）產生的光暈比較淡，可以把「曝光」調高。
- 這組功能只支援 RGB 與灰階文件，CMYK、Lab 會顯示「不支援」。
- 光暈的外觀是依照參數意義自己設計的第一版，還在依實際使用回饋調整中。

---

## 8. 授權與注意事項

- PhotoCraft 原始程式碼與這裡新增的程式碼：**MIT 或 Apache-2.0**（擇一），見 `LICENSE-MIT`、`LICENSE-APACHE`、`NOTICE`。
- ArtCraft 的名稱、文字標誌與 logo 是 ArtCraft 團隊的商標，**不是開源的**（條款見 `docs/brand/LICENSE-brand.txt`）。依照條款對修改版的要求，這個版本已經：
  - 刪除 `docs/brand/` 裡所有商標圖檔（只保留條款文字）；
  - 拿掉程式裡以 ArtCraft 名義的社群連結（說明選單的「加入 ArtCraft Discord」「ArtCraft 網站」、起始畫面與「關於」視窗的 Discord 按鈕與 ArtCraft 連結）；說明選單剩下的「網站／GitHub／回報問題」改為指向這個 repo；
  - 安裝程式與 Linux 套件資訊中的首頁、說明連結改為這個 repo；
  - README 改寫，寫明這是以 ArtCraft 團隊的 PhotoCraft 為基礎的修改版，**不是 ArtCraft 官方產品**。
- 新功能是依照公開的修圖技法與觀察到的行為**自己實作**的，沒有使用任何其他軟體或外掛的程式碼、素材或品牌名稱。
