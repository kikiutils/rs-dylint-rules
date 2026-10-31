# prefer-imported-paths

狀態：**草案，尚未實作**。以下為設計規劃，目前沒有對應 lint、診斷或 auto-fix。

## 目的

自身 crate 的跨層依賴先透過上方 imports 定義區交代來源，使用點再以匯入名稱表達用途。允許匯入個別 item，也允許保留 module 作為命名空間，不強制在使用點加上 `self::`。

## 適用範圍

- 檢查對象為 imports 以外，自身 crate 路徑的使用點，包含運算式、型別、模式與巨集呼叫路徑；所有可見性的 imports 由 `import-path-prefix` 處理。
- 預定遵守[共用分析範圍](../architecture.md#分析範圍)，只根據啟用中的 compiler facts 檢查；尚待確認的細節見[未決事項](#未決事項或後續規劃)。
- 涵蓋檔案頂層、inline module、函式、方法、closure、async block、控制流程與其他巢狀區塊中符合本規則檢查對象的位置。
- 遞迴檢查各作用域；涉及名稱、路徑或型別時，以 rustc 的實際解析結果為準。

## 規則

### 跨層依賴先匯入

- 使用點禁止直接使用 `super::` 或 `crate::` 路徑，包含透過這些前綴存取 module、函式、型別、常數或其他 items。
- 上層或其他分支的依賴必須先在上方 imports 定義區匯入，再於使用點使用匯入名稱。
- 不預先要求所有 imports 搬至檔案頂層。
- 所有可見性的 imports（`use`、`pub use` 與各種 `pub(...) use`）均不受使用點限制；其中的前綴選擇遵守 `import-path-prefix`。
- 目前作用域已可見的自身 items 或子模組可以直接使用，不要求為了滿足本規則而新增多餘 imports。

### 匯入粒度由使用端決定

- 允許匯入個別 item，之後使用短名稱，例如 `data()`、`LIMIT`、`StructA::new()`。
- 允許匯入 module，之後以 module 名稱分組，例如 `output::data()`。
- 若需要保留 domain，可匯入較上層 module，之後使用 `cli::output::data()`。
- 同名 module 可透過 alias 區分，例如 `cli_output::data()` 與 `storage_output::write()`。
- 不強制攤平 imports，也不根據 module 階層猜測使用端應保留多少 domain 名稱。
- 不跨過 re-export 改用另一個定義位置；必須保留原路徑解析結果與語意。

### self 不強制

- `self::` 可保留，但不要求新增或移除。
- `self::` 只表示目前 module 命名空間中的名稱，不代表該名稱實際定義於此或屬於此 domain。
- module-level import 建立的名稱可透過 `self::` 存取；函式或其他一般區塊內的局部 imports 不會把名稱加入所屬 module，不能機械補上 `self::`。
- 型別、trait、泛型參數或 `Self` 的關聯項目路徑不要求加上前綴，例如 `StructA::new()`、`State::Ready`、`T::Item`、`Self::new()`。

### 與其他規則的分工

- `import-module-order`：imports 與模組宣告的分區順序。
- `import-path-prefix`：所有可見性 imports 內的來源路徑前綴。
- 本規則：一般使用點不得直接以 `super::` 或 `crate::` 存取依賴，但不強制 `self::` 或匯入粒度。

## 正確範例

### 匯入個別 item 後使用短名稱

```rust
mod output;
mod types;

use self::output::data;
use self::types::StructA;

fn run() {
    data();
    StructA::new();
}
```

### 保留 module 作為命名空間

```rust
use crate::cli::output;

fn run() {
    output::data();
}
```

### 保留較上層的 domain

以下假設目前 module 位於 `cli` 分支之外，且依 `import-path-prefix` 應使用 `crate::cli` 匯入：

```rust
use crate::cli;

fn run() {
    cli::output::data();
}
```

### 使用 alias 區分同名 module

```rust
use crate::cli::output as cli_output;
use crate::storage::output as storage_output;

fn run() {
    cli_output::data();
    storage_output::write();
}
```

### 自身子模組可直接使用，self 可保留

```rust
mod output;

fn run() {
    output::data();
    self::output::data();
}
```

### 局部匯入也可使用匯入名稱

```rust
fn run() {
    use super::output as printer;

    printer::data();
}
```

`printer` 是局部 alias，不要求也不能直接改成 `self::printer::data()`。

## 錯誤範例

### 使用點直接存取上層或其他分支

```rust
fn run() {
    super::output::data();
    crate::storage::output::write();
}
```

應先匯入，例如：

```rust
use super::output;
use crate::storage::output as storage_output;

fn run() {
    output::data();
    storage_output::write();
}
```

### 使用點直接以 crate 路徑引用型別

```rust
fn run() {
    crate::types::StructA::new();
}
```

應先匯入 `StructA` 再使用 `StructA::new()`，或匯入 `types` 後使用 `types::StructA::new()`。

## 政策例外

- 標準函式庫與第三方 crate 的路徑不受限制，例如 `std::io::Error`、`tokio::spawn()`。

## 實作限制

- 巨集呼叫的路徑可納入檢查，但巨集輸入 token 的內容不能一律當成 Rust 路徑解析或改寫。

## Auto-fix

尚未提供。以下為預定的修正條件，實作時仍須符合[共用自動修正機制](../architecture.md#自動修正)：

- 若已有可用的匯入名稱，且能確認替換後的解析結果與語意不變，可縮短使用點的路徑。
- 沒有可用名稱時只提供診斷，不自動新增或搬動 imports，也不替使用端決定匯入粒度或 alias。
- 保留泛型參數、註解與其他語法，不自動新增或移除 `self::`。
- 遇到名稱遮蔽或無法可靠處理的巨集時，不自動修正。

## 驗證與安全性依據

無

## 未決事項或後續規劃

- 待決策：rustdoc、原始巨集呼叫與展開等支援邊界。
- 待決策：「上方 imports 定義區」的作用域與位置。
