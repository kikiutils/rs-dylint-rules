# prefer-imported-apis

## 目的

指定 API 必須先匯入再使用，並統一可用的名稱與 aliases。目前涵蓋 anyhow；之後可逐項擴充其他 crate 的政策。

## 適用範圍

- 檢查 anyhow crate 本身及下列指定 API 的 imports（所有可見性），以及 imports 以外的型別、運算式與巨集呼叫路徑。
- 檢查 Cargo／rustc 在當次 target、features 與 cfg 下實際載入的自有 Rust 來源，涵蓋各層 module、函式、方法、closure、async block、控制流程與其他巢狀區塊。
- 依 rustc 實際解析結果辨識來源，不只比對 `anyhow` 字樣；其他 crate 的同名項目不受影響。
- 未啟用的程式碼、rustdoc 程式碼區塊、巨集輸入與展開產物先不處理；僅處理能對應至所屬 module 原始定義檔的實體來源；獨立 `include!` item／expression 片段略過，避免把運算式檔案當成 item source file 解析。

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

## 例外

- imports 內允許書寫來源限定路徑，但仍須遵守 alias 限制。
- 名稱衝突不是政策上的例外；無法安全修正時仍診斷，由使用者調整。
- 無法可靠解析來源的情況不猜測，也不依文字名稱提供修正。未限定名稱只有在可對應原始 import 時才檢查 import 是否位於上方；不因找不到原始 import 就猜測巨集或其他片段建立的 binding。

## Auto-fix

- 優先重用使用點已可見、位於上方、名稱合規且不帶條件 attributes 的 import；巨集缺少 import 時，優先併入同一 module 上方、同一路徑且無 attributes／註解的既有私有分組 import；其他情況在最近可合法加入 `use` 的作用域開頭新增必要的 imports。函式內的使用點可新增函式內 import，不強制提升至 module。
- 不跨越使用點與所在 module 之間的 `cfg` 邊界提升巨集 import；條件函式或 block 內缺少巨集 import 時，於該條件作用域內新增。module 自身的 `cfg` 同時涵蓋其 imports 與內容，不阻擋 module 內合併。
- 缺少 `Result` import 時，預設使用 `Result`；若有名稱衝突則僅診斷，不自行改選 alias。已存在的 `AnyhowResult` 可重用；`Error` 一律使用 `AnyhowError`。
- 只新增實際需要的 imports 並替換相關使用點，不自動匯入全部 API。
- 已使用短名稱但對應 import 位於下方時，僅診斷，不搬動既有 imports。
- 違規 aliases、公開重新匯出與含有不合規名稱的 glob 目前僅診斷，不自動重新命名或展開，避免漏改其他檔案、巨集及對外 API。
- 若原限定路徑使用了重新匯出的別名，且新增 import 需要引入被禁止的 alias，僅診斷；已有可重用的合法 import 時仍可替換使用點。
- 替換範圍有註解、使用點或外層帶有共用安全清單以外的 attributes，或新增檔案頂層 import 會插到既有前置註解與 item 之間時，僅診斷。
- 共用安全 attribute 清單（見[架構](../architecture.md#安全邊界)）不阻擋修正；其他 attributes 可能是 attribute macro，仍只診斷。
- 依 rustc 已解析的名稱空間、作用域、imports、型別參數與巨集展開產物檢查衝突；其他作用域的同名項目、不同名稱空間的 binding，以及無關的 derives／巨集呼叫不直接阻擋修正。不支援自行定義或以 `#[macro_use]` 引入與指定 anyhow 巨集同名的其他巨集，不追蹤其 textual scope 或匯出傳播。
- 檢查新增 import 是否捕獲原有型別路徑；無法確認安全時僅診斷。修正只替換已解析的 API 路徑並新增必要 imports，其餘排版交由 rustfmt。
- module 被其他作用域以 glob 匯入時，不在該 module 新增或補入 import，避免改變 glob 使用端的名稱解析；單純含有子模組不阻擋修正。重用既有 import 不受此限制。
- 依實際插入作用域或目標 use tree 合併修正，同一 module 的同一巨集 API 只新增一個 binding，不把不同來源的 imports 混入同一分組。
- 可安全套用的修正使用 `MachineApplicable` suggestion，由 `cargo dylint --fix` 套用；不自行寫檔或呼叫 rustfmt。

### 完成進度

- [x] 確認 anyhow API 清單、alias 政策及新增 imports 的作用域策略。
- [x] 以 crate 身分與定義辨識指定 API，支援依賴改名、qualified paths 與原始巨集呼叫。
- [x] 實作 imports／aliases 診斷、合法 imports 重用與最近作用域的新增修正。
- [x] 驗證名稱衝突、固定 aliases、公開重新匯出、glob、巨集輸入排除及修正後行為。
- [ ] 違規 aliases 的跨使用點重新命名與 glob 展開，目前僅診斷。

已在 `nightly-2026-10-15` 與 anyhow `1.0.104` 驗證；未鎖定使用端的 anyhow 版本，若未來版本改變定義結構，須重新查核。

API 來源辨識與 `Api` 定義位於規則的 `mod.rs`，collector 與名稱解析共用此入口。facts 依檔案保存並保留 module 歸屬，analysis 借用逐檔 facts 並在分析時去重。macro call 由呼叫起點的 CST 祖先定位，保留 call-site 包含分號的情況。
