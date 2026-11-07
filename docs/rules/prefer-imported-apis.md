# prefer-imported-apis

狀態：**已實作**。lint：`kiki_prefer_imported_apis`。

## 目的

指定 API 必須先匯入再使用，並統一可用的名稱與 aliases。目前涵蓋 anyhow；之後可逐項擴充其他 crate 的政策。

## 適用範圍

- 檢查 anyhow crate 本身及下列指定 API 的 imports（所有可見性），以及 imports 以外的型別、運算式與巨集呼叫路徑。
- 遵守[共用分析範圍](../architecture.md#分析範圍)，涵蓋各層 module、函式、方法、closure、async block、控制流程與其他巢狀區塊。
- 依 rustc 實際解析結果辨識來源，不只比對 `anyhow` 字樣；其他 crate 的同名項目不受影響。

## 規則

### 指定 API 先匯入再使用

禁止在使用點以限定路徑直接存取以下 API，包含 crate 別名或其他可解析至相同 API 的限定路徑：

| API | 允許的匯入名稱 |
| --- | --- |
| `anyhow::anyhow!` | `anyhow` |
| `anyhow::bail!` | `bail` |
| `anyhow::ensure!` | `ensure` |
| `anyhow::Result` | `Result` 或 `AnyhowResult` |
| `anyhow::Error` | 必須為 `AnyhowError` |

使用前須在可見的上方 imports 區域匯入對應 API；只匯入實際需要的項目，不補齊整份清單。`Error` 的型別標註、關聯函式與其他使用方式均適用，不限於呼叫語法。

### 限制 aliases

- 禁止替 anyhow crate 本身設定 alias，例如 `use anyhow as ah;` 或 `extern crate anyhow as ah;`；不禁止未設定 alias 的 `use anyhow;`。Cargo.toml 中的依賴改名不由本規則限制，但改名後的 API 使用仍依真實來源檢查。
- 上表 API 中，僅 `Result`、`Error` 允許設定 alias，且分別只能為 `AnyhowResult`、`AnyhowError`。
- `Result` 可以不設定 alias；`Error` 必須設定 alias，不允許直接以 `Error` 匯入或使用。
- `anyhow`、`bail`、`ensure` 巨集不得設定 alias；冗餘的 `bail as bail` 與 `Result as Result` 同樣不允許。
- 若來源已以合規名稱重新匯出，可直接匯入該名稱，不要求再補上同名 `as`。
- glob 若隱式匯入了上述 API 的不合規名稱（例如 `anyhow::*` 中的 `Error`），仍須診斷，改用符合命名政策的明確 imports；不因此全面禁止其他 glob imports。
- 經 rustc 確認、名稱合規的 glob bindings 同樣參與「匯入須位於使用點上方」檢查；不要求將這些 glob 展開。
- 上述命名限制同樣適用於各種可見性的重新匯出；不因使用 `pub use` 而放行。
- 尚未列入清單的 anyhow API 不新增限制；不把目前政策外推至所有 crate。

### 與其他規則的分工

- 本規則決定指定 API 是否須匯入及允許的匯入名稱。
- 自身 crate 的 import 路徑前綴交由 `import-path-prefix` 處理，imports 分區交由 `import-module-order` 處理，排版交由 rustfmt 處理。

## 正確範例

```rust
use anyhow::{Error as AnyhowError, Result, anyhow, bail, ensure};

fn check(valid: bool) -> Result<()> {
    ensure!(valid, "invalid input");
    let error: AnyhowError = anyhow!("example");
    bail!(error);
}
```

`Result` 也允許使用固定 alias：

```rust
use anyhow::Result as AnyhowResult;

fn run() -> AnyhowResult<()> {
    Ok(())
}
```

## 錯誤範例

```rust
use anyhow as ah;                  // 禁止 crate alias。
use anyhow::bail as fail;          // 巨集不得設定 alias。
use anyhow::Result as AppResult;   // 只能保留 Result 或改為 AnyhowResult。
use anyhow::Error;                 // 必須改為 Error as AnyhowError。
use anyhow::Error as OtherError;   // 只能使用 AnyhowError。

fn run() -> anyhow::Result<()> {   // 必須先匯入 Result 或 AnyhowResult。
    anyhow::bail!("failed");       // 必須先匯入 bail。
}
```

## 政策例外

- imports 內允許書寫來源限定路徑，但仍須遵守 alias 限制。
- 名稱衝突不是政策上的例外；無法安全修正時仍診斷，由使用者調整。

## 實作限制

- 無法可靠解析來源的情況不猜測，也不依文字名稱提供修正。未限定名稱只有在可對應原始 import 時才檢查 import 是否位於上方；不因找不到原始 import 就猜測巨集或其他片段建立的 binding。
- 不支援自行定義或以 `#[macro_use]` 引入與指定 anyhow 巨集同名的其他巨集，不追蹤其 textual scope 或匯出傳播。

## Auto-fix

- 優先重用使用點已可見、位於上方、名稱合規且不帶條件 attributes 的 import；巨集缺少 import 時，優先併入同一 module 上方、同一路徑且無 attributes／註解的既有私有分組 import；其他情況在最近可合法加入 `use` 的作用域開頭新增必要的 imports。函式內的使用點可新增函式內 import，不強制提升至 module。
- 上述重用也包含合規的 glob bindings；仍檢查名稱遮蔽與條件 attributes，不新增或展開 glob。
- 同一輪已規劃新增的外層 import，只有在其可見範圍內且位於使用點上方時才可重用，避免在內層重複新增；新的 binding 與依賴它的替換合併為同一份 multipart suggestion，內層仍須通過名稱衝突檢查。
- 不跨越使用點與所在 module 之間的 `cfg` 邊界提升巨集 import；條件函式或 block 內缺少巨集 import 時，於該條件作用域內新增。module 自身的 `cfg` 同時涵蓋其 imports 與內容，不阻擋 module 內合併。
- 缺少 `Result` import 時，預設使用 `Result`；若有名稱衝突則僅診斷，不自行改選 alias。已存在的 `AnyhowResult` 可重用；`Error` 一律使用 `AnyhowError`。
- 只新增實際需要的 imports 並替換相關使用點，不自動匯入全部 API。
- 已使用短名稱但對應 import 位於下方時，僅診斷，不搬動既有 imports。
- 違規 aliases、公開重新匯出與含有不合規名稱的 glob 目前僅診斷，不自動重新命名或展開，避免漏改其他檔案、巨集及對外 API。
- 若原限定路徑使用了重新匯出的別名，且新增 import 需要引入被禁止的 alias，僅診斷；已有可重用的合法 import 時仍可替換使用點。
- 替換範圍有註解、使用點或外層帶有共用安全清單以外的 attributes，或新增檔案頂層 import 會插到既有前置註解與 item 之間時，僅診斷。
- 新增 import 會置於作用域的 inner attributes 與 inner doc comments（`//!`、`/*! ... */`）之後，不改變文件歸屬；普通前置註解的既有屏障仍保留。
- 依 rustc 已解析的名稱空間、作用域、imports、型別參數與巨集展開產物檢查衝突；其他作用域的同名項目、不同名稱空間的 binding，以及無關的 derives／巨集呼叫不直接阻擋修正。
- 檢查新增 import 是否捕獲原有型別路徑；無法確認安全時僅診斷。修正只替換已解析的 API 路徑並新增必要 imports。
- module 被其他作用域以 glob 匯入時，不在該 module 新增或補入 import，避免改變 glob 使用端的名稱解析；單純含有子模組不阻擋修正。重用既有 import 不受此限制。
- 依實際插入作用域或目標 use tree 合併修正，同一 module 的同一巨集 API 只新增一個 binding，不把不同來源的 imports 混入同一分組。

## 驗證與安全性依據

已在 `nightly-2026-11-01` 與 anyhow `1.0.104` 驗證；未鎖定使用端的 anyhow 版本，若未來版本改變定義結構，須重新查核。

## 未決事項或後續規劃

- 違規 aliases 的跨使用點重新命名與 glob 展開，目前僅診斷，尚未提供修正。

### 擴充更多 API 時的架構方向

以下是下一個 crate 或新類型 API 真正加入時的改寫方向，尚未建立通用框架。目前只有 anyhow 政策；不能只新增一個 `Api` variant，就假設來源辨識、名稱衝突及 auto-fix 均已支援。

- **先集中政策，再擴充清單。** 將 namespace／巨集分類、預設名稱、允許的 aliases、import 命名限制及能否新增 alias，收斂到 `Api` 的方法或小型政策資料。避免在 collector、analysis 與 resolution 各處繼續補 `Api::X` 特例；enum 加少量方法即可，不先建立 trait hierarchy、plugin registry 或新增依賴。
- **衝突候選名稱使用同一份政策。** `resolution::candidate()` 目前另列 anyhow 的名稱；擴充時應由上述政策導出所有允許的名稱及 namespace，避免忘記同步後漏收 bindings／references，誤認新名稱沒有衝突。
- **來源身分與顯示名稱分開。** `api()` 目前只識別 anyhow，非巨集 API 只接受 crate root 下的定義。支援其他 crate 或子 module API 時，按實際 DefId 的來源與定義路徑擴充匹配；不同 crate 的同名 API 仍保有不同身分。診斷中的 crate／API 名稱也應由政策提供，不在修正流程硬寫 anyhow。
- **保留可共用的 edit 機制。** `PlannedImport`、`import_edit()` 與 `import_offset()` 繼續處理作用域、上方位置、插入／合併及 multipart suggestion，不負責猜測特定 crate 的命名政策。新的 binding 與依賴它的替換仍須同屬一份 fix；既有 import 與計畫中的 import 均須位於使用點上方。
- **新的 namespace 另行驗證。** 目前修正涵蓋型別與 bang macros；函式、值、unit／tuple constructors 等 ValueNS 用法不是單純加入清單。須補齊相應的 binding／reference 收集、遮蔽及名稱捕獲檢查，並以實際使用案例驗證。

擴充時沿用完整修正原文比對、修正前後編譯／行為測試及再次 lint 的冪等性驗證；保留 cfg、glob、註解與 inner docs 的既有邊界。不為尚未使用的 API 或假設性組合提前建立框架，也不因新增 API 就複製一套 fix 流程。
