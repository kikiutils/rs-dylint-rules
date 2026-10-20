# import-module-order

## 目的

統一 imports 與外部模組宣告的分區順序，補足 rustfmt 不跨越 `mod` 宣告整理 imports 的限制。

## 適用範圍

- 檢查對象為所有可見性的 imports 與外部模組宣告（`mod name;`），包含標準函式庫、第三方 crate 與自身模組 imports。
- 僅處理 Cargo／rustc 在當次 target、features 與 cfg 下實際載入、canonicalize 後位於 compiler 工作目錄內的實體來源。不依固定目錄、`.rs` 副檔名或 Git 忽略規則篩選。
- 涵蓋檔案頂層、inline module、函式、方法、closure、async block、控制流程與其他巢狀區塊中符合本規則檢查對象的位置；imports 的來源分組以 rustc 的實際解析結果為準。
- 未啟用的程式碼、rustdoc 程式碼區塊、巨集輸入與展開產物不檢查；獨立 `include!` 片段略過。

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
- 同一個 `use` 不得混合外部與自身來源；無法安全判定分組時，只提供診斷，不猜測來源。
- 依當次 Cargo 分析的 target、features 與 cfg 判定分組與排序，不自動分析其他平台。
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

## 例外

- 明確標記 `#[cfg(test)]` 或含有必要 `test` 條件之 `#[cfg(all(...))]` 的外部模組宣告不參與分區順序，允許保留在檔案末尾；不依模組名稱判斷，其來源仍正常檢查，也不重設其他 imports 的順序。
- 依各上下文的 `cfg`／`cfg_attr` 與 test 模式檢查啟用範圍；未啟用的項目與其內部作用域不檢查、不修改、不列為 unresolved。
- 使用固定 nightly 的 compiler 分析，會執行 build scripts／proc macros；啟用中的 imports 無法解析或編譯失敗時，不保證能產生規則診斷。

## Auto-fix

- 以同一作用域中第一個至最後一個參與排序的項目為重排區段；所有 imports 分組都須明確，且區段內不得穿插其他定義、執行敘述、巨集或未參與排序的項目。
- 區段內不得有註解或 attributes；緊鄰區段首尾項目的註解也會阻擋修正。作用域自身或外層僅在帶有共用安全清單以外的 attributes 時阻擋修正。
- 重排保留項目原文及各分區內的順序，以 LF 分隔項目，分區之間插入空白行。不推導縮排，同一行的宣告也可修正，排版交給 rustfmt。

符合條件時提供 `MachineApplicable` suggestion，由 `cargo dylint --fix` 套用；否則只保留診斷。本規則不執行 rustfmt／Clippy，建議流程為 Dylint → 既有 fmt/lint。

### 完成進度

- [x] 分區檢查、巢狀作用域、測試模組例外與安全重排。
- [x] 原生 Dylint 診斷與 auto-fix、Cargo 實際載入來源及 module 實例隔離。
- [x] 各種可見性的 imports／re-exports 依來源參與分組與安全重排。

## 實作邊界

- 測試專用外部模組只排除明確要求 `test` 的 cfg：`test` 或含有此條件的 `all(...)`，支援巢狀 `all`；`any(...)`、`not(...)` 與 `feature = "test"` 不因此排除。
- collector 依 compiler facts 預先建立啟用中的 CST 節點集合；inline modules 合併為逐檔一次分析，不重複回報。
- 區段內帶 attributes 的 imports 或 module 宣告不搬移，即使 attribute 屬於共用安全清單。
