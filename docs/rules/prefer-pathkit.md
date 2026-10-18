# prefer-pathkit

## 目的

統一路徑型別、建構方式與檔案操作：優先使用 `pathkit::Path`、`pathkit::path!` 與 Path 提供的方法，避免不必要的標準庫路徑轉換與直接 fs 呼叫。

## 適用範圍

- `.rs` 檔案中的自有程式碼，包含 imports、型別宣告、參數、struct 欄位與各種使用點。
- 依名稱與型別實際解析結果判斷，不能只比對 `Path`、`read` 等字面名稱。
- rustdoc 中的 Rust 程式碼區塊為選配；若實作過於複雜，可以不支援。

## 規則

### 路徑型別

- 針對已明確使用路徑型別的自有程式碼，在沒有必要互通需求時，禁止使用 `std::path::Path` 或 `std::path::PathBuf`，改用對應的 `pathkit::Path` 表示。
- 參數允許 `Path`、`&Path`、`impl AsRef<Path>`、`impl Into<Path>`，以及指向相同 pathkit 型別的等效泛型約束；不強制每次傳遞 owned `Path`。
- struct 欄位若使用標準庫路徑型別，包含容器或型別別名包裝，在沒有必要互通需求時，必須改用對應的 `pathkit::Path`，保留原本需要的容器結構。
- 參數採相同判定邊界，檢查已表達為路徑的型別或泛型約束，不推測任意參數是否語意上代表檔案路徑。
- 不以名稱、註解或曾傳入檔案操作函式作為充分依據，將任意 `String`、`OsString` 等參數或欄位判定為路徑；這些線索不足以要求遷移型別。
- 外部 API、trait 或 FFI 強制的型別與簽名可保留，必要轉換限制在互通邊界，不因此豁免整個函式或資料結構。

### 路徑建構

- 非必要不得直接呼叫 `pathkit::Path::new`，改用 `pathkit::path!`。
- 必須區分使用者直接呼叫 constructor 與 `path!` 展開內部的呼叫，不對後者重複報錯。
- `path!` 的字串 literal 採格式化語意；若字串含有大括號，替換時必須保留原本的字面路徑，不得誤當插值。

### 檔案操作

- 有等效 Path 方法的檔案操作函式，禁止直接呼叫，優先使用 `SyncFsOps` 或 `AsyncFsOps` 提供的 Path 方法。
- 例如同步讀取優先使用 `path.read_sync()`，非同步讀取優先使用 `path.read().await`。
- 不得只因當前沒有 Path 物件就自動豁免；應先判斷能否合理使用 `path!` 建構路徑並呼叫等效方法。
- 未涵蓋的操作及其所需型別允許使用，不整個禁止 `std::fs` 或 `tokio::fs` 命名空間。
- 等效性必須考慮同步或非同步、錯誤型別、回傳值、平台與操作行為，不只比對方法名稱。

### 避免多餘的顯式路徑轉換

- `Path` 已實作 `Deref<Target = std::path::Path>`；若可直接借用或由 deref coercion 完成互通，不得額外呼叫可移除的路徑轉換方法。
- 檢查範圍包含 `as_path()`、`to_path_buf()`，以及實際解析為相同性質轉換的 `to_path()` 等方法；不得單憑方法名稱判定違規。
- 例如函式接受 `&std::path::Path` 時，若 `&path` 已足夠，不得寫成 `path.as_path()` 或 `&path.to_path_buf()`。
- 若 receiver 本來就是 `&Path`，應考慮直接傳入該借用，不額外要求增加一層 `&`。
- 只有移除轉換後仍滿足目標型別、泛型約束、ownership 與 lifetime，且保持語意時，才視為多餘轉換。
- 外部 API 確實需要 owned `PathBuf`、獨立副本、不同生命週期或其他無法以借用取代的型別時，允許必要轉換。
- `to_path_buf()` 會產生 owned 副本，不能一律當成 `as_path()` 的等價寫法；也不能為了移除轉換而任意改成 move 原值。

### Dependency 與 feature

- 不因目標專案尚未安裝 pathkit 就停用規則；缺少 dependency 時應回報，由使用者補上。
- 使用非同步方法需要對應的 `async-fs-ops` feature；缺少 feature 不視為直接呼叫等效 fs 函式的理由。
- 不自動修改目標專案的 manifest 或安裝 dependencies。

## 正確範例

### 路徑參數與儲存欄位

```rust
use pathkit::Path;

struct Config {
    directory: Path,
}

fn inspect(path: &Path) {}

fn consume(path: Path) {}

fn inspect_generic(path: impl AsRef<Path>) {}

fn consume_generic(path: impl Into<Path>) {}
```

### 不推測字串欄位的用途

```rust
struct Config {
    directory: String,
    value: String,
}
```

本規則不因 `directory` 的名稱就要求改成 `Path`，也不宣稱能判定 `value` 是否實際儲存路徑；字串參數同樣不依名稱或用途線索強制遷移型別。

### 使用 Path 方法操作檔案

```rust
use pathkit::{SyncFsOps, path};

let file = path!("config.json");
let contents = file.read_to_string_sync()?;
```

### 外部 API 接受標準庫路徑借用

假設外部函式 `external_inspect` 接受 `&std::path::Path`：

```rust
let file = path!("config.json");
external_inspect(&file);
```

不需要先呼叫 `as_path()`，也不需要建立暫時的 `PathBuf`。

### 外部 API 確實需要 owned PathBuf

假設外部函式 `external_store` 接受 owned `std::path::PathBuf`，且呼叫後仍需要原本的 `file`：

```rust
external_store(file.to_path_buf());
inspect(&file);
```

此處需要獨立 owned 值，不能改成 `external_store(&file)`。

## 錯誤範例

### 自有路徑欄位使用標準庫型別

```rust
struct Config {
    directory: std::path::PathBuf,
}
```

沒有外部型別相容需求時，應改成 `pathkit::Path`。

### 直接呼叫有等效方法的 fs 函式

```rust
let file = path!("config.json");
let contents = std::fs::read_to_string(&file)?;
```

應使用 `file.read_to_string_sync()`；修正時仍須確認呼叫端的錯誤型別處理。

### 不必要的 constructor 呼叫

```rust
let file = Path::new("config.json");
```

應使用 `path!("config.json")`。

### 可由借用取代的顯式轉換

假設 `external_inspect` 接受 `&std::path::Path`：

```rust
external_inspect(file.as_path());
external_inspect(&file.to_path_buf());
```

在不改變解析結果與語意的前提下，應使用 `external_inspect(&file)`。

## 例外

- pathkit 未提供等效能力的操作。
- 外部 API、trait 或 FFI 強制的型別與簽名，以及必要的互通轉換。
- ownership、lifetime 或泛型約束使轉換無法移除的情況。
- 不檢查外部 dependencies 的實作，也不把 `path!` 的內部展開當成使用者直接呼叫。

其他「特殊原因」的局部豁免方式仍待討論，不先假定可任意豁免。

## Auto-fix

不要求所有情況都能自動修正。

- 能確認型別、ownership、lifetime、錯誤處理與其他語意不變時，才考慮修正簡單呼叫或移除多餘轉換。
- 不依賴 deref 的存在就假定所有型別或泛型參數都能自動 coercion。
- 不自動遷移參數或欄位型別，不自行改變借用、複製或 ownership 策略。
- 不盲目將字串 literal 的 constructor 呼叫替換成格式化巨集。
- 無法確認安全性時，只提供診斷與建議。
