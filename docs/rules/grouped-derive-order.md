# grouped-derive-order

## 目的

以功能分組統一每個 `#[derive(...)]` 列表內的順序，讓 `Clone`／`Copy`、`PartialEq`／`Eq` 與 `PartialOrd`／`Ord` 保持相鄰，避免全部按字典排序拆散相關項目。此順序是本 repo 的風格政策，不是 Rust 官方標準。

## 適用範圍

- 檢查對象為 struct、enum 與 union 定義上的原始 derive 列表；僅處理 rustc 確認為支援之內建 derive 的項目，不以名稱猜測巨集來源。
- 檢查 Cargo／rustc 在當次 target、features 與 cfg 下實際載入、canonicalize 後位於 compiler 工作目錄內的實體來源，包含各層 module 與函式等區塊內的局部定義。
- 支援直接書寫的 `#[derive(...)]`、`#[core::derive(...)]` 與 `#[std::derive(...)]`，包含後兩者的絕對路徑寫法；列表內的限定路徑與內建 derive aliases 依實際解析結果辨識。
- 每個 derive attribute 獨立處理；不檢查其他 attributes 的整體排列，不跨列表要求全域順序。
- 未啟用的項目、`cfg_attr` 內的 derive 列表、rustdoc 程式碼區塊、巨集輸入及展開產生的 derive 列表目前不處理。符合來源條件的 `include!` 實體來源不因使用 include 而排除，仍須能解析為 Rust item source file。

## 規則

純已知列表依下表排序，未使用的項目與空分組省略，不自動補齊 traits：

| 順序 | 功能分組 | 組內固定順序 |
| --- | --- | --- |
| 1 | 除錯顯示 | `Debug` |
| 2 | 複製 | `Clone` → `Copy` |
| 3 | 預設值 | `Default` |
| 4 | 相等、比較與雜湊 | `PartialEq` → `Eq` → `PartialOrd` → `Ord` → `Hash` |

- 分組與排序根據 rustc 確認的內建 derive 身分，不只比較最後一段名稱；自訂或第三方的同名巨集不屬於內建分組。
- 每個 `#[derive(...)]` 只整理自己的列表，不合併、拆分、交換 attributes，也不在列表之間搬動 derive。
- 列表含有任何未知或未支援的 derive 時，整個列表略過，不提供排序診斷或修正，也不要求未知項目移至最後或按字典排序。
- 某個列表被略過，不阻擋同一 item 上其他純已知列表獨立檢查。
- 目前支援清單只有上述九種 Rust 內建 derive。serde、strum 等第三方巨集與自訂巨集均視為未支援；未來須逐項查核來源、版本與安全性，不能僅因 crate 知名就加入。
- 不新增、移除或去重 derive，不改寫限定路徑、aliases、型別內容、helper attributes 或其他 attributes。重複 derive 與缺少 trait 相依的錯誤仍由 rustc 或其他 lint 處理。

## 正確範例

### 同一列表依功能分組

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Value(u32);
```

### 多個列表各自獨立

```rust
#[derive(Clone, Copy)]
#[derive(Debug, PartialEq, Eq)]
struct Value(u32);
```

不要求把第二個列表的 `Debug` 搬到第一個列表。

### 未支援的列表保持原樣

```rust
#[derive(Eq, CustomDerive, PartialEq)]
#[derive(Debug, Clone)]
struct Value;
```

假設 `CustomDerive` 可解析且產生合法程式碼，第一個列表不參與排序；第二個列表仍獨立檢查。這不表示對所有未知 derive 的原始順序提供安全保證，只表示本規則不改動它。

## 錯誤範例

### 相關內建 derive 未按固定順序排列

```rust
#[derive(Hash, Eq, Copy, PartialEq, Clone, Debug)]
struct Value(u32);
```

應整理為：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Value(u32);
```

### 另一個列表正確不會抵銷違規

```rust
#[derive(Eq, PartialEq)]
#[derive(Clone, Copy)]
struct Value(u32);
```

只需將第一個列表改為 `#[derive(PartialEq, Eq)]`，不改變兩個 attributes 的先後順序。

## 例外

- 無法確認所有項目均為支援之內建 derive 時，略過該列表，不猜測巨集來源，也不排序剩餘的已知子區段。
- 列表內有註解時仍可檢查純已知項目的順序，但不自動修正，避免改變註解歸屬。
- 定義或其外層帶有非 derive 的 attributes 時，採保守策略：可辨識的純已知列表仍提供診斷，但不自動修正，避免外層 attribute macro 觀察到不同 tokens。
- 無法編譯或無法可靠對應原文與 compiler expansion 的程式碼，不保證能產生本規則的診斷。

## Auto-fix

- 只有所有項目皆確認為支援之內建 derive，且沒有上述修正障礙的列表，才提供 `MachineApplicable` suggestion。
- 僅替換該 attribute 括號內的列表，保留各 derive 路徑與 alias 的原文、原有尾隨逗號，以及 attribute 外的內容；列表內以逗號與空格重新串接，換行與縮排之後交由 rustfmt 處理。
- 不呼叫 rustfmt，不調整 attributes 的位置，不合併或拆分列表，也不對第三方／未知 derive 提供自動排序。
- 此獨立處理保證僅限本規則；後續 rustfmt 若啟用 `merge_derives`，仍可能合併多個 attributes。本 repo 目前啟用該選項，如需整條工具鏈保留列表邊界，須另行調整 formatter 設定，本規則不代為修改。
- 由 `cargo dylint --fix` 套用修正；標準 Cargo fix 流程也可能套用 rustc 的其他修正建議。

### 安全性依據

Rust 允許同一 item 有多個 derive attributes；[Reference](https://doc.rust-lang.org/reference/attributes/derive.html) 並未指定本文件的風格順序。[rustc #124794](https://github.com/rust-lang/rust/issues/124794) 展示的差異涉及分開書寫的 `Clone`／`Copy` attributes，不能推論成同一列表中兩者互換必然有相同問題；本規則不改變跨 attribute 的排列。

[rustfmt #6574](https://github.com/rust-lang/rustfmt/issues/6574) 的通用排序提案仍涉及巨集與編譯器行為的討論，因此不把內建 derive 的測試結果外推為所有 procedural derives 都能任意重排。修正範圍限定在固定 nightly 與已驗證的內建清單，升級 compiler 或擴充清單時須重新驗證。

### 完成進度

- [x] 純內建 derive 的功能分組與固定順序。
- [x] 各 attribute 獨立處理、未知列表略過、alias 來源辨識與安全修正邊界。
- [ ] 經查核後逐項擴充第三方 derive 支援清單，目前不包含 serde／strum。
