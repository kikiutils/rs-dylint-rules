# grouped-derive-order

## 目的

以功能分組統一每個 `#[derive(...)]` 列表內的順序，讓 `Clone`／`Copy`、`PartialEq`／`Eq` 與 `PartialOrd`／`Ord` 保持相鄰，避免全部按字典排序拆散相關項目。此順序是本 repo 的風格政策，不是 Rust 官方標準。

## 適用範圍

- 檢查對象為 struct、enum 與 union 定義上的原始 derive 列表；僅處理 rustc 確認為支援之內建或第三方 derive 的項目，不以名稱猜測巨集來源。
- 檢查 Cargo／rustc 在當次 target、features 與 cfg 下實際載入、canonicalize 後位於 compiler 工作目錄內的實體來源，包含各層 module 與函式等區塊內的局部定義。
- 支援直接書寫的 `#[derive(...)]`、`#[core::derive(...)]` 與 `#[std::derive(...)]`，包含後兩者的絕對路徑寫法；列表內的限定路徑與支援之 derive aliases 依實際解析結果辨識。
- 每個 derive attribute 獨立處理；不檢查其他 attributes 的整體排列，不跨列表要求全域順序。
- 未啟用的項目、`cfg_attr` 內的 derive 列表、rustdoc 程式碼區塊、巨集輸入及展開產生的 derive 列表目前不處理。符合來源條件的 `include!` 實體來源不因使用 include 而排除，仍須能解析為 Rust item source file。

## 規則

純已知列表先按下表分組，再依各組固定順序排列，最後按分組順序組合；實作使用 `(分組順位, 組內順位)` 作為排序鍵，不採組內字母排序。未使用的項目與空分組省略，不自動補齊 traits：

| 順序 | 功能分組 | 組內固定順序 |
| --- | --- | --- |
| 1 | 除錯顯示 | `Debug` |
| 2 | 複製 | `Clone` → `Copy` |
| 3 | 預設值 | `Default` |
| 4 | 相等、比較與雜湊 | `PartialEq` → `Eq` → `PartialOrd` → `Ord` → `Hash` |
| 5 | Serde 序列化 | `Serialize` → `Serialize_repr` → `Deserialize` → `Deserialize_repr` |
| 6 | 狀態查詢 | `IsEmpty` → `EnumIs` |
| 7 | 列舉資訊 | `EnumCount` → `VariantNames` → `VariantArray` |

- 分組與排序根據 rustc 確認的巨集來源，不只比較最後一段名稱。內建 derive 須由 compiler 確認；Serde derive 須解析至 `serde_derive` crate 的 `Serialize` 或 `Deserialize`，可經由 `serde` re-export、限定路徑或 alias 使用。`IsEmpty` 須來自 `is_empty_derive`；`Serialize_repr`／`Deserialize_repr` 須來自 `serde_repr`；上述 Strum derives 須來自 `strum_macros`，支援經由 `strum` re-export。其他 crate 的同名巨集不列入支援清單。
- 每個 `#[derive(...)]` 只整理自己的列表，不合併、拆分、交換 attributes，也不在列表之間搬動 derive。
- 列表含有任何未知或未支援的 derive 時，整個列表略過，不提供排序診斷或修正，也不要求未知項目移至最後或按字典排序。
- 某個列表被略過，不阻擋同一 item 上其他純已知列表獨立檢查。
- 已查核版本：Serde `1.0.229`、is_empty `0.3.0`（is_empty_derive `0.1.0`）、serde_repr `0.1.21`、strum／strum_macros `0.28.0`。實作依巨集來源辨識，不限制使用端版本；其他版本若改變展開結構，須重新驗證。
- `Serialize_repr`／`Deserialize_repr` 分別是一般 Serde derives 的替代方案，不應與實作相同 trait 的一般 derive 重複使用。Strum 僅支援表內四種 derives，其餘仍按未知列表處理。
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

### 內建與 Serde derive 混用

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Record {
    value: u32,
}
```

Serde 使用固定的 `Serialize` → `Deserialize` 次序，不改成字典序。

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

- 無法確認所有項目均為支援之 derive 時，略過該列表，不猜測巨集來源，也不排序剩餘的已知子區段。
- 列表內有註解時仍可檢查純已知項目的順序，但不自動修正，避免改變註解歸屬。
- 允許 `cfg`、lint levels（`allow`／`warn`／`deny`／`forbid`／`expect`）、`doc`、`deprecated`、`must_use`、`non_exhaustive` 與 `repr` attributes。定義自身的 `serde`、`is_empty`、`strum` helper attributes，須有同一定義上經 rustc 確認、會註冊該 helper 的已支援 derive 才放行；可位於另一個 derive 列表。`cfg_attr` 內含 attributes 全部為安全內建項目時可修正；未知 attributes 與外層 helper 同名 attributes 仍只診斷、不修正，避免改變 attribute macro 的輸入。
- 無法編譯或無法可靠對應原文與 compiler expansion 的程式碼，不保證能產生本規則的診斷。

## Auto-fix

- 只有所有項目皆確認為支援之 derive，且沒有上述修正障礙的列表，才提供 `MachineApplicable` suggestion。
- 僅替換該 attribute 括號內的列表，保留各 derive 路徑與 alias 的原文、原有尾隨逗號，以及 attribute 外的內容；列表內以逗號與空格重新串接，換行與縮排之後交由 rustfmt 處理。
- 不呼叫 rustfmt，不調整 attributes 的位置，不合併或拆分列表，也不對未列入支援清單的第三方／未知 derive 提供自動排序。
- 此獨立處理保證僅限本規則；後續 rustfmt 若啟用 `merge_derives`，仍可能合併多個 attributes。本 repo 目前啟用該選項，如需整條工具鏈保留列表邊界，須另行調整 formatter 設定，本規則不代為修改。
- 由 `cargo dylint --fix` 套用修正；標準 Cargo fix 流程也可能套用 rustc 的其他修正建議。

### 安全性依據

Rust 允許同一 item 有多個 derive attributes；[Reference](https://doc.rust-lang.org/reference/attributes/derive.html) 並未指定本文件的風格順序。[rustc #124794](https://github.com/rust-lang/rust/issues/124794) 展示的差異涉及分開書寫的 `Clone`／`Copy` attributes，不能推論成同一列表中兩者互換必然有相同問題；本規則不改變跨 attribute 的排列。

[rustfmt #6574](https://github.com/rust-lang/rustfmt/issues/6574) 的通用排序提案仍涉及巨集與編譯器行為的討論，因此不把內建 derive 的測試結果外推為所有 procedural derives 都能任意重排。修正範圍限定在固定 nightly 與已查核的支援清單，升級 compiler 或擴充清單時須重新驗證。

Serde `1.0.229` 的 [Serialize 產生器](https://docs.rs/crate/serde_derive/1.0.229/source/src/ser.rs) 與 [Deserialize 產生器](https://docs.rs/crate/serde_derive/1.0.229/source/src/de.rs) 分別產生各自的實作；[容器 attributes 解析](https://docs.rs/crate/serde_derive/1.0.229/source/src/internals/attr.rs) 不以 derive 列表順序決定序列化設定。已驗證混合內建與 Serde、巨集 aliases、限定路徑，以及修正前後的 struct／enum JSON 往返；不因此宣稱所有第三方 derive 都可重排。

is_empty_derive `0.1.0` 的 [產生器](https://docs.rs/crate/is_empty_derive/0.1.0/source/src/is_empty.rs) 依欄位產生 `IsEmpty` impl；serde_repr `0.1.21` 的 [產生器](https://docs.rs/crate/serde_repr/0.1.21/source/src/lib.rs) 依整數 repr 分別產生序列化與反序列化 impl。Strum `0.28.0` 本次支援的 [四種產生器](https://docs.rs/crate/strum_macros/0.28.0/source/src/macros/) 均直接為原 enum 產生 impl，不依 derive 列表順序決定內容；其他涉及轉換目標或額外型別的 derives 暫不納入。

### 完成進度

- [x] 純內建 derive 的功能分組與固定順序。
- [x] 各 attribute 獨立處理、未知列表略過、alias 來源辨識與安全修正邊界。
- [x] 加入 Serde `Serialize`／`Deserialize`，驗證來源、aliases、helper attribute 邊界及序列化往返。
- [x] 加入 is_empty `IsEmpty`、serde_repr derives 及已知內建／helper attributes 的安全修正。
- [x] 加入 Strum `EnumIs`、`EnumCount`、`VariantNames`、`VariantArray`。
- [ ] 經查核後逐項擴充其餘第三方 derives。
