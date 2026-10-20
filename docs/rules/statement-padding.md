# statement-padding

## 目的

跨多行的 statement 與相鄰的 item 若黏在一起，段落邊界不清楚。本規則只補上缺少的空行，讓跨多行的 statement 或 item 結束後，與下一個項目之間有明確分隔。

## 適用範圍

- 檢查對象為同一作用域內 statement 與 item 之間的空行，包括檔案頂層、inline module、函式、closure、async block、控制流程與其他巢狀區塊中的 statement list，以及 `impl`、`trait`、`extern` 區塊內的成員列表。
- 檢查 `mod` 宣告載入的自有 Rust 來源，包含 cfg 停用的程式碼。不依 cfg、target 或 feature 判斷是否啟用，插入空行不影響語意。
- 只依 CST 的結構與原文換行判斷，不需要 rustc 的解析結果，因此不依名稱、型別或路徑判斷。
- 巨集輸入、`macro_rules!` 內容、rustdoc 程式碼區塊與巨集展開產物不檢查；`include!` 引入的片段不是模組擁有的來源，也不檢查。

## 規則

- 完整 statement 跨多行，且同一作用域還有下一項時，其後必須有一個空行。完整 statement 包含多行函式呼叫、method chain、struct literal、`if`、`match`、迴圈與 `let ... else`，不拆開 `else`、method chain 或外層呼叫。
- 作用域中的 item（`fn`、`struct`、`enum`、`union`、`impl`、`trait`、`const`、`static`、`type`、inline `mod`、`macro_rules!` 等）之後若還有下一項，必須有一個空行，單行 item 同樣適用。例如 `fn a() {}` 接 `fn b() {}` 時，需在 `a` 之後補空行。
- 只要求後方空行，不要求前方空行；作用域的首尾不因此增加空行。
- 單行 statement 之間不要求空行，例如 `drop(file);` 接 `Ok(summary)`。
- 已存在的空行不改動；多餘的空行交由 rustfmt 縮減。
- 註解與 attribute 屬於其後的元素。空行插在前一個元素所在行之後，不會把註解或 attribute 與它們的元素拆開。判斷元素是否跨多行時，不計入其前導的 attribute 與註解。

### 與其他規則的分工

- `use`、`extern crate` 與沒有區塊的 `mod x;` 不在本規則範圍內，前後皆不要求空行，交由 rustfmt 處理。
- 本規則只處理空行，不決定 item 的順序、分組或 import 路徑；這些分別由 `import-module-order`、`import-path-prefix` 等規則處理。

## 正確範例

```rust
const A: u8 = 1;
const B: u8 = 2;

fn one() {}

fn two() {}

pub fn run(values: &[u8]) -> Option<u8> {
    let total = values
        .iter()
        .sum::<u8>();

    let count = 3;
    drop(String::new());
    if total > count {
        println!("big");
    }

    let Some(first) = values.first() else {
        return None;
    };

    Some(*first + total)
}
```

成員列表同樣適用：

```rust
impl Value {
    fn a(&self) -> u8 {
        self.a
    }

    fn b(&self) -> u8 {
        self.b
    }
}
```

## 錯誤範例

```rust
const A: u8 = 1;
const B: u8 = 2;
fn one() {}
fn two() {}

pub fn run(values: &[u8]) -> Option<u8> {
    let total = values
        .iter()
        .sum::<u8>();
    let count = 3;
    drop(String::new());
    if total > count {
        println!("big");
    }
    let Some(first) = values.first() else {
        return None;
    };
    Some(*first + total)
}
```

違反的位置如下：

- `const B` 與 `fn one` 相鄰，`const B` 之後需要空行。
- `fn one() {}` 與 `fn two() {}` 相鄰，`fn one` 之後需要空行。
- `let total` 跨多行，其後需要空行。
- `if` 跨多行，其後需要空行。
- `let ... else` 跨多行，其後需要空行。

`const A` 與 `const B` 都是單行常數，因此兩者之間不需要空行。改法即上方正確範例。

## 例外

- 相鄰的單行 `const`、`static`、`type` 之間不要求空行。
- `use`、`extern crate` 與 `mod x;` 不檢查，前後皆不要求空行。
- 同一檔案被多個 module 共用（例如 `#[path]`）時，只回報、不修正。

## Auto-fix

- 支援：多行 statement 或 item 後方缺空行，以及相鄰 item 之間缺空行，皆以 `MachineApplicable` suggestion 在前一元素結尾所在行之後插入一個換行。CRLF 檔案插入 CRLF。由 `cargo dylint --fix` 套用，規則本身不寫檔，也不呼叫 rustfmt。
- 只插入換行，不改動其他 token，不刪除既有空行，不移動註解或 attribute。
- 僅回報、不修正：同一行相鄰的元素，例如 `fn a() {} fn b() {}`。拆行屬於排版決定，不由本規則自動決定。
- 診斷由共用出口在 crate 層級 emit，不依 item、foreign item 或 module 的 `allow` 調整歸屬。
