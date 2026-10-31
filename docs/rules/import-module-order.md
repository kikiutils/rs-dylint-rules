# import-module-order

狀態：**已實作**。lint：`kiki_import_module_order`。

## 目的

統一 imports 與外部模組宣告的分區順序，補足 rustfmt 不跨越 `mod` 宣告整理 imports 的限制。

## 適用範圍

- 檢查對象為所有可見性的 imports 與外部模組宣告（`mod name;`），包含標準函式庫、第三方 crate 與自身模組 imports。
- 遵守[共用分析範圍](../architecture.md#分析範圍)，只根據啟用中的 compiler facts 檢查。
- 涵蓋檔案頂層、inline module、函式、方法、closure、async block、控制流程與其他巢狀區塊中符合本規則檢查對象的位置；imports 的來源分組以 rustc 的實際解析結果為準。

## 規則

依序排列下列分區，空分區可省略：

| 順序 | 分區 | 範例 |
| --- | --- | --- |
| 1 | 標準函式庫與第三方 imports | `use std::...;`、`use core::...;`、`use alloc::...;`、`use tokio::...;` |
| 2 | 外部模組宣告 | `mod config;`、`pub(crate) mod config;` |
| 3 | 自身模組 imports | `use crate::...;`、`use self::...;`、`use super::...;`，以及解析為自身模組的省略前綴路徑 |

- 不要求分區位於作用域開頭；空白、註解、其他宣告或執行敘述都不重設順序。
- `mod name { ... }` 本身不參與父作用域的排序，其內部獨立檢查。
- imports 一律依來源分組，不因可見性而另設分區；`pub use` 與各種 `pub(...) use` 適用相同檢查及修正條件。
- 同一個 `use` 不得混合外部與自身來源。
- 本規則不指定分區內的先後順序，重排時保留各分區原有順序。標準函式庫／第三方細分、imports 排序與合併、`crate`／`self`／`super` 的相互順序及模組排序交由 rustfmt 處理，不要求分區標記註解。
- 只檢查分區順序，不檢查 import 路徑前綴或一般使用點；`import-path-prefix` 等其他規則的職責不在此規則內。

## 正確範例

```rust
use std::sync::Arc;

use tokio::task::spawn_blocking;

mod config;

use self::config::WorkspaceConfig;
use crate::shutdown::check_shutdown;

#[cfg(test)]
mod tests;
```

巢狀作用域遵守相同順序：

```rust
mod worker {
    use std::sync::Arc;

    use crate::constants::LIMIT;

    fn run() {
        use std::time::Duration;

        use crate::shutdown::check_shutdown;
    }
}
```

## 錯誤範例

```rust
use crate::shutdown::check_shutdown;

mod config;

use std::sync::Arc;
```

## 政策例外

- 明確標記 `#[cfg(test)]` 或含有必要 `test` 條件之 `#[cfg(all(...))]` 的外部模組宣告不參與分區順序，允許保留在檔案末尾；支援巢狀 `all`，但 `any(...)`、`not(...)` 與 `feature = "test"` 不因此排除。
- 不依模組名稱判斷測試模組；其來源仍正常檢查，也不重設其他 imports 的順序。

## 實作限制

- 啟用中的 import 來源無法解析，或在不同分析上下文中無法一致分類時，僅診斷、不猜測分組。

## Auto-fix

- 以同一作用域中第一個至最後一個參與排序的項目為重排區段；所有 imports 分組都須明確，且區段內不得穿插其他定義、執行敘述、巨集或未參與排序的項目。
- 區段內不得有註解或 attributes；緊鄰區段首尾項目的註解也會阻擋修正。即使 attribute 屬於共用安全清單，也不搬移帶 attribute 的 imports 或 module 宣告；作用域自身或外層則僅在帶有安全清單以外的 attributes 時阻擋修正。
- 重排保留項目原文及各分區內的順序，以 LF 分隔項目，分區之間插入空白行。不推導縮排，同一行的宣告也可修正，排版交給 rustfmt。

## 驗證與安全性依據

測試位置：單元測試見 [`src/rules/import_module_order/tests.rs`](../../src/rules/import_module_order/tests.rs)，整合測試見 [`tests/rules/import_module_order.rs`](../../tests/rules/import_module_order.rs)。

工具鏈固定為 [`nightly-2026-10-15`](../../rust-toolchain.toml)。

## 未決事項或後續規劃

無
