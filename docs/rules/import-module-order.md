# import-module-order

## 目的

統一各作用域內 imports 與外部模組宣告的分區順序，禁止不同分區互相穿插。

rustfmt 只會在各段連續的 imports 內整理分組，不會跨越 `mod` 宣告重新安排 imports。本規則負責檢查這些大區塊的相對位置，區塊內的排版與排序仍交由 rustfmt 處理。

## 適用範圍

- `.rs` 檔案中，任何允許宣告 `use` 的作用域都適用。
- 包含檔案頂層、inline module（`mod name { ... }`）、函式與方法本體，以及 closure、async block、控制流程或其他巢狀區塊內的局部 imports。
- 遞迴檢查每個作用域；各作用域獨立計算分區順序，不與父層或子層混合排序。
- rustdoc 中的 Rust 程式碼區塊為選配；若實作過於複雜，可以不支援。

## 規則

依序排列下列分區；沒有內容的分區可以省略：

| 順序 | 分區 | 範例 |
| --- | --- | --- |
| 1 | 標準函式庫 imports | `use std::...;`、`use core::...;`、`use alloc::...;` |
| 2 | 第三方 crate imports | `use tokio::...;` |
| 3 | 外部模組宣告 | `mod config;`、`pub(crate) mod config;` |
| 4 | 自身模組 imports | `use crate::...;`、`use self::...;`、`use super::...;` |

本規則的核心是維持以下三個大區塊的順序：

```text
標準函式庫與第三方 imports
→ 外部模組宣告
→ 自身模組 imports
```

- 不得在外部模組宣告前放置自身模組 imports。
- 不得在外部模組宣告或自身模組 imports 後，再放置標準函式庫或第三方 imports。
- 不得將外部模組宣告穿插在自身模組 imports 之間。
- 空白行或註解不會重設分區順序。
- 同一作用域內穿插其他宣告或執行敘述，也不會重設分區順序。
- 本規則檢查 imports 與外部模組宣告之間的相對順序，不額外要求它們必須位於作用域開頭。
- `mod name { ... }` 本身不屬於「外部模組宣告」分區；它在父模組中的位置由其他規則決定。

### 與 rustfmt 的分工

- 標準函式庫與第三方 imports 之間的分組，以及各段 imports 內部的排序、合併粒度與排版，交由 rustfmt 的 `group_imports`、`imports_granularity`、`imports_layout` 等設定處理。
- 自身模組 imports 內部的 `crate`、`self`、`super` 順序同樣交由 rustfmt 處理，本規則不額外指定它們彼此的先後順序。
- 外部模組宣告區塊內的排序交由 rustfmt 的 `reorder_modules` 處理。
- 本規則不要求分區標記註解；若需要，由其他規則定義。

## 正確範例

```rust
use std::sync::Arc;

use tokio::task::spawn_blocking;

pub(crate) mod config;
pub(crate) mod documents;

use self::config::WorkspaceConfig;
use crate::shutdown::check_shutdown;
```

函式、inline module 與其他巢狀區塊也適用，且各作用域獨立檢查：

```rust
mod worker {
    use std::sync::Arc;

    use crate::shutdown::check_shutdown;

    fn run() {
        use std::time::Duration;

        use tokio::task::spawn_blocking;

        use crate::constants::LIMIT;

        let task = async move {
            use std::io;

            use crate::shutdown::check_shutdown;
        };
    }
}
```

省略外部模組宣告分區時，自身模組 imports 直接接在第三方 imports 後：

```rust
use std::sync::Arc;

use tokio::task::spawn_blocking;

use crate::shutdown::check_shutdown;
```

## 錯誤範例

### 自身模組 imports 出現在外部模組宣告前

```rust
use std::sync::Arc;

use tokio::task::spawn_blocking;

use self::config::WorkspaceConfig;
use crate::shutdown::check_shutdown;

pub(crate) mod config;
pub(crate) mod documents;
```

### 外部模組宣告穿插在自身模組 imports 之間

```rust
use std::sync::Arc;

use tokio::task::spawn_blocking;

use crate::shutdown::check_shutdown;

pub(crate) mod config;

use self::config::WorkspaceConfig;
```

### 外部模組宣告後再次出現標準函式庫或第三方 imports

```rust
use std::sync::Arc;

pub(crate) mod config;

use std::time::Duration;
use tokio::task::spawn_blocking;
```

### 自身模組 imports 後再次出現第三方 imports

```rust
use std::sync::Arc;

use tokio::task::spawn_blocking;

pub(crate) mod config;

use self::config::WorkspaceConfig;
use crate::shutdown::check_shutdown;

use rusqlite::Connection;
```

## 例外

目前未指定一般例外，以下情況待討論，不先假定可任意重排：

- `#[macro_use]`、條件編譯或巨集產生的 items。
- `pub use` 等 re-exports 是否納入相同分區。
- 未以 `crate::`、`self::` 或 `super::` 開頭的自身模組 imports，如何與第三方 imports 區分。

## Auto-fix

希望支援，但不強求。

- 只在能確認安全、分區歸屬明確時搬動 items。
- 搬動時保留所屬 attributes、文件註解與一般註解；歸屬不明時只提供診斷。
- 涉及巨集或其他無法確認安全性的情況，不自動修正。
- 修正後交由 rustfmt 整理區塊內部格式，且再次檢查不應產生相同問題。
