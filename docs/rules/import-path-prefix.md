# import-path-prefix

## 目的

統一自身 crate 內的 import 路徑前綴：目前模組與子模組使用 `self::`，非 root 祖先模組與其旁支使用 `super::`，必須回到 crate root 才能到達的目標使用 `crate::`。

## 適用範圍

- `.rs` 檔案中，任何允許宣告 `use` 的作用域都適用。
- 包含檔案頂層、inline module（`mod name { ... }`）、函式與方法本體，以及 closure、async block、控制流程或其他巢狀區塊內的局部 imports。
- 遞迴檢查每個作用域內的 imports；路徑判斷以該處所屬的 Rust module 為準，不以檔案目錄或區塊巢狀深度計算。
- 一般 `use` 與所有可見性的 re-exports（例如 `pub use`、`pub(crate) use`）皆適用。
- rustdoc 中的 Rust 程式碼區塊為選配；若實作過於複雜，可以不支援。

## 規則

依目前 module 與匯入路徑的關係，強制使用下列前綴，而非僅提供偏好建議：

- 目標位於目前 module 或其子模組時，必須使用 `self::`，不得從 `crate::` 繞回；省略 `self::` 的自身模組路徑同樣不符合規則。
- 目標位於祖先模組，或必須經由祖先模組進入的其他分支時，若該共同祖先不是 crate root，必須使用一個或多個 `super::`。
- 不得用 `crate::` 表達上述可由 `self::` 或非 root 祖先的 `super::` 到達的路徑，也不得先往上再繞回目前 module 或較近的祖先。
- 若必須回到 crate root 才能到達目標，必須使用 `crate::`，禁止改成一路回到 root 的 `super::` 鏈。
- 目前 module 本身就是 crate root 時，匯入 root 自身或其子模組仍適用 `self::` 規則，因為不需要向上回到 root。
- 函式、closure、async block 與一般區塊不會增加 module 層級；inline module 會增加 module 層級。
- 不因路徑較短就跨過 re-export 改用另一條路徑；必須保留原 import 的目標與語意。
- 標準函式庫與第三方 crate imports 不受此前綴規則限制，不得誤加 `self::`。

### 與其他規則的分工

- 本規則只決定自身 crate 的 import 路徑應使用 `self::`、`super::` 或 `crate::`。
- imports 與模組宣告的分區順序由 `import-module-order` 處理。
- imports 的排版與合併粒度交由 rustfmt 處理。

## 正確範例

假設有下列檔案結構，且各層已透過 `mod` 宣告建立同名 module：

```text
src/
├── lib.rs
└── a/
    ├── mod.rs
    ├── b/
    │   ├── mod.rs
    │   └── zz.rs
    └── c/
        ├── mod.rs
        └── rr.rs
```

以下 imports 均位於 `crate::a::c::rr` 中，並假設目標可見性允許存取。

### 從目前模組或自身子模組匯入

假設 `rr` 內定義了 `LocalType` 與子模組 `child`：

```rust
use self::LocalType as LocalAlias;
use self::child::ChildType;
```

目前 module 內已存在的名稱通常不需要再匯入；此處透過 alias 示範確有匯入需求時的前綴。

### 從父模組匯入

`a/c/mod.rs` 對應 module `crate::a::c`。匯入的是該 module 內的 item，不是 `mod.rs` 檔案本身。

```rust
use super::ParentType;
```

### 從祖先模組的另一個分支匯入

先以兩層 `super::` 回到 `crate::a`，再進入 `b::zz`：

```rust
use super::super::b::zz::SiblingType;
```

### 在函式內匯入

函式本體不增加 module 層級，因此與所屬 module 頂層使用相同路徑：

```rust
fn run() {
    use super::ParentType;
    use super::super::b::zz::SiblingType;
}
```

### 目標只能經由 crate root 到達

假設 `other` 是 crate root 下的另一個 module：

```rust
use crate::other::OtherType;
```

### 公開重新匯出

`pub use` 與其他可見性的 re-exports 使用相同規則：

```rust
pub use self::child::ChildType;
pub use super::ParentType;
pub(crate) use super::super::b::zz::SiblingType;
pub use crate::other::OtherType;
```

## 錯誤範例

以下 imports 同樣位於 `crate::a::c::rr` 中。

### 匯入目前模組或子模組卻未使用 self

```rust
use crate::a::c::rr::LocalType as LocalAlias;
use crate::a::c::rr::child::ChildType;
```

應分別改成 `use self::LocalType as LocalAlias;` 與 `use self::child::ChildType;`。

```rust
use child::ChildType;
```

若 `child` 解析為自身子模組而非第三方 crate，應改成 `use self::child::ChildType;`。

### 可用父模組路徑卻從 crate root 匯入

```rust
use crate::a::c::ParentType;
```

應改成 `use super::ParentType;`。

### 可經由非 root 祖先到達旁支，卻從 crate root 匯入

```rust
use crate::a::b::zz::SiblingType;
```

應改成 `use super::super::b::zz::SiblingType;`。

### 一路使用 super 回到 crate root

```rust
use super::super::super::other::OtherType;
```

應改成 `use crate::other::OtherType;`。

## 例外

目前沒有指定一般例外。無法可靠解析路徑的情況屬於實作限制，不代表允許使用不符合規則的前綴。

## Auto-fix

盡可能支援，但不強求。

- 只在能確認 module 層級與路徑解析結果不變時，替換路徑前綴。
- 保留 aliases、可見性、attributes 與註解，不改變匯入目標。
- 群組 imports 若混合不同目標，只修正能安全處理的部分；必要時拆分，無法確認時只提供診斷。
- 不靠檔案目錄深度猜測 `super::` 數量；遇到 `#[path]`、巨集或其他無法確認 module 結構的情況，不自動修正。
