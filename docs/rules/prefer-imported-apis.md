# prefer-imported-apis

狀態：**已實作**。lint：`kiki_prefer_imported_apis`。

## 目的

讓指定 API 在各 repositories 使用固定且能辨認來源的形式：一般指定項目直接匯入，來源語意不可省略者保留 namespace，容易混淆的實作使用固定 module alias。這是本 repo 的風格政策，不是 Rust 語意要求，也不追求所有 API 都有相同的路徑段數。

## 適用範圍

- 檢查下列 API 與 namespace 的 imports（所有可見性），以及 imports 以外的型別、運算式、關聯項目與原始巨集呼叫路徑。
- 遵守[共用分析範圍](../architecture.md#分析範圍)，涵蓋檔案頂層、各層 module、函式、方法、closure、async block 與其他巢狀區塊；依啟用中的 compiler facts 分析，不檢查 cfg 停用項目。
- 以 rustc 實際解析結果辨識 API；crate 別名、依賴改名與 re-export 不改變其身分，其他來源的同名項目不受影響。巨集參數內僅處理 rustc 已解析、且能精確對應原始輸入範圍的 API 路徑，不自行猜測其他 token trees 的語意。
- 只限制明列的 API／家族，不將自由函式政策外推至同 module 的型別、traits、方法或巨集。

## 規則

家族清單指定檢查範圍，範例不是完整成員表；API 依公開項目的解析身分辨識，不依 `from_*`／`to_*` 等文字模式猜測。

### 使用形式與匯入位置

| 政策 | 要求 |
| --- | --- |
| 直接匯入 | 必須先匯入指定項目，再使用原名；禁止使用點的限定路徑及任意 alias。 |
| 指定 namespace | 必須保留表列的 namespace，不得將成員直接匯入為裸名稱或型別 alias。 |
| 固定 module alias | 必須以指定名稱匯入 module，成員仍保留該 namespace；即使只使用一種實作也不省略 alias。 |

- 需要的 import 必須在使用點上方且可見，不要求全部提升至檔案頂層，也不補齊未使用的項目。crate root 已可由 extern prelude 使用時，不要求冗餘的 `use anyhow;`、`use serde_json;` 或 `use toml;`。
- 指定形式同時適用於型別標註、泛型、關聯項目存取與非呼叫用法，不限於 `fn()` 語法；不以有無 `.await` 判斷 API 來源。
- 使用點須採表列的完整形式，而不是只要包含該 namespace 即可。例如 `std::process::id()`、`std::io::Error` 須改為 `process::id()`、`io::Error`；`confique::toml::template` 則保留表列的來源與格式前綴。此要求只作用於受管 API，不限制 `tokio::process::Command` 等未列項目。
- 受管 API 禁止任意改名，包括使用點 namespace 的任意 alias 與冗餘的同名 `as`；固定 module alias 是明列的允許形式。module alias 若只用於未受管項目（例如 `use std::process as proc; proc::Command::new(...)`），不受此限制；透過該 alias 使用受管 API（例如 `proc::id()`）則仍違規。
- imports 內可書寫來源路徑，但建立的 binding 仍須符合政策。規則同樣檢查公開 re-export；不因 `pub use` 放行。glob 若引入違規 binding 仍須診斷，但不全面禁止其他 glob；經解析確認的合規 binding 仍須符合上方位置要求。
- 本規則不修改 Cargo manifest。若依賴改名或名稱衝突使規定形式無法安全引入，仍診斷，不自行選擇其他名稱。

### 直接匯入

| API／家族 | 指定名稱／使用例 |
| --- | --- |
| `anyhow::{Context, Result}` | `use anyhow::{Context, Result};`，使用 `Context`／其 trait 方法與 `Result<T>` |
| anyhow bang macros | `use anyhow::{anyhow, bail, ensure};`，使用 `anyhow!()`、`bail!()`、`ensure!()` |
| `tokio::time` 自由函式 | `sleep()`、`sleep_until()`、`timeout()`、`timeout_at()`、`interval()`、`interval_at()`；其餘自由函式依同一直接匯入政策，不強制 `tokio_time::` |

`a::bail!()` 即使經 alias 或 re-export 解析至 anyhow，仍須改為直接匯入後的 `bail!()`。其他未列入的巨集不套用此政策。

### 標準庫 namespace

| API／家族 | 統一形式 |
| --- | --- |
| `std::process` 自由函式 | `process::id()`、`process::exit()`、`process::abort()` |
| `std::env` 自由函式 | `env::var()`、`env::args_os()`、`env::current_dir()` |
| `std::mem` 非 prelude 自由函式 | `mem::take()`、`mem::replace()`、`mem::swap()`、`mem::zeroed()` |
| `std::ptr` 自由函式 | `ptr::null()`、`ptr::null_mut()`、`ptr::read()` |
| `std::thread` 自由函式 | `thread::spawn()`、`thread::sleep()`、`thread::available_parallelism()` |
| `std::io` 自由函式 | `io::stdin()`、`io::stdout()`、`io::stderr()`、`io::copy()` |
| `std::io::{Error, ErrorKind, Result}` | `io::Error`、`io::ErrorKind`、`io::Result<T>`，不使用 `IoError` 等 aliases |
| `std::result::Result`（含 prelude 的 `Result<T, E>`） | `use std::result;` 後使用 `result::Result<T, E>`；`no_std` 使用 `core::result` |
| `std::fmt::{Error, Result}` | `fmt::Error`、`fmt::Result`，不使用 `FmtError`／`FmtResult` |

`Command`、`Display`、`Formatter` 等未列型別不受限制。標準庫 re-export 自 core 的 API 依實際身分辨識，不因 DefId 位於 core 而漏查。

### 指定 crate namespace

| API／家族 | 統一形式 |
| --- | --- |
| `anyhow::Error` | `anyhow::Error`；不使用裸 `Error` 或 `AnyhowError` |
| `rusqlite::Error` | `rusqlite::Error`；不直接匯入為裸 `Error`，也不使用 `SqliteError`／`RusqliteError` 等 aliases；不連帶限制 `Result`、`Connection` 等未列 API |
| `anstream::{stdout, stderr}` 自由函式 | `anstream::stdout()`、`anstream::stderr()`；不直接匯入為 `stdout`／`stderr`，以區分 `std::io` 的同名函式 |
| `serde_json` 序列化／反序列化與值轉換函式 | `serde_json::from_str()`、`serde_json::from_slice()`、`serde_json::from_value()`、`serde_json::to_string()`、`serde_json::to_vec()`、`serde_json::to_value()` 等 |
| `toml` 序列化／反序列化函式 | `toml::from_str()`、`toml::to_string()`、`toml::to_string_pretty()` 等 |
| `confique::toml::{template, FormatOptions}` | `confique::toml::template::<T>()`、`confique::toml::FormatOptions`；保留來源與格式 namespace |
| `confique::yaml::{template, FormatOptions}` | `confique::yaml::template::<T>()`、`confique::yaml::FormatOptions`；保留來源與格式 namespace |
| `confique::json5::{template, FormatOptions}` | `confique::json5::template::<T>()`、`confique::json5::FormatOptions`；保留來源與格式 namespace |
| `rmp_serde` 序列化／反序列化函式 | `rmp_serde::from_slice()`、`rmp_serde::to_vec()`、`rmp_serde::to_vec_named()` 等 |
| `postcard` 序列化／反序列化函式 | `postcard::from_bytes()`、`postcard::to_allocvec()` 等 |
| `ciborium` 序列化／反序列化函式 | `ciborium::from_reader()`、`ciborium::into_writer()` |
| `hex` 編解碼函式 | `hex::encode()`、`hex::decode()`、`hex::decode_to_slice()` 等 |
| `serde_json::{Value, Map, Error, Result}` | `serde_json::Value`、`serde_json::Map`、`serde_json::Error`、`serde_json::Result<T>`；不使用裸名稱或 `JsonValue` 等 aliases |
| `toml::{Value, Table}` | `toml::Value`、`toml::Table`，不使用裸名稱或另造 aliases |

標準庫的 `Result<T, E>` 保留 `result::` namespace，以區分直接匯入的 `anyhow::Result`；此限制包含 prelude 的裸 `Result`，但不限制 `Ok`／`Err` 的一般用法。`Serialize`、`Deserialize`、`DeserializeOwned`、`ToHex` 等 traits 不因轉換函式政策而要求限定路徑。

anstream 此項只涵蓋 `stdout`／`stderr` 函式，不連帶限制同名型別或輸出巨集；公開 API 見 [anstream 文件](https://docs.rs/anstream/latest/anstream/)。使用 crate namespace 不要求額外的 `use anstream;`。

confique 保留完整的 `confique::<格式>::` 前綴，不直接匯入 `template`／`FormatOptions`，也不將格式 module 匯入為 `toml`、`yaml`、`json5` 或另設 alias，以區分同名格式 crates。此項只涵蓋表列函式與型別（含其關聯項目存取），不限制 `Config`／`Layer` traits 或其他未列 API；格式 features 見 [confique 文件](https://docs.rs/confique/0.4.0/confique/#cargo-features)。

### Tokio task 與 Channel 的固定 module aliases

| 來源／範圍 | 固定匯入 | 統一使用例 |
| --- | --- | --- |
| `tokio::task` 自由函式 | `use tokio::task as tokio_task;` | `tokio_task::spawn()`、`tokio_task::spawn_blocking()`、`tokio_task::yield_now()`、`tokio_task::id()` |
| `std::sync::mpsc` 自由函式與型別 | `use std::sync::mpsc as std_mpsc;` | `std_mpsc::channel()`、`std_mpsc::Sender<T>`、`std_mpsc::SyncSender<T>`、`std_mpsc::RecvError` |
| `tokio::sync::mpsc` 自由函式與型別，含 error 子 module | `use tokio::sync::mpsc as tokio_mpsc;` | `tokio_mpsc::channel()`、`tokio_mpsc::UnboundedReceiver<T>`、`tokio_mpsc::error::TryRecvError` |
| `tokio::sync::oneshot` 自由函式與型別，含 error 子 module | `use tokio::sync::oneshot as tokio_oneshot;` | `tokio_oneshot::channel()`、`tokio_oneshot::Sender<T>`、`tokio_oneshot::error::RecvError` |

`tokio::spawn` 與 `tokio::task::spawn` 若解析為相同 API，均使用 `tokio_task::spawn`。Channel 不使用裸 `channel()`、`Sender<T>`／`Receiver<T>`，也不另造 `AsyncSender` 等 aliases。此表不限制 `JoinHandle`、`Instant` 或鎖型別的命名。

### 與其他規則的分工

本規則只決定指定 API 的使用形式。自身 crate 的 import 前綴交由 `import-path-prefix` 處理，imports 分區交由 `import-module-order` 處理，排版交由 rustfmt 處理。未來的一般匯入粒度規則不得對已由本規則接管的 API 產生相反要求。

## 正確範例

以下假設已安裝相關 dependencies。

```rust
use std::{io, process};

use anyhow::{Context, Result, ensure};

fn parse(text: &str) -> Result<serde_json::Value> {
    ensure!(!text.is_empty(), "設定不可為空");
    serde_json::from_str(text).context("JSON 格式錯誤")
}

fn io_error() -> io::Error {
    io::Error::other(format!("process {} failed", process::id()))
}
```

```rust
use std::time::Duration;

use tokio::{sync::mpsc as tokio_mpsc, task as tokio_task, time::sleep};

async fn send_later() {
    let (sender, mut receiver) = tokio_mpsc::channel::<u8>(1);
    tokio_task::spawn(async move {
        sleep(Duration::from_millis(10)).await;
        let _ = sender.send(1).await;
    });
    let _ = receiver.recv().await;
}
```

## 錯誤範例

以下各項均違反指定使用形式：

```rust
use std::{io::Error as IoError, process::id}; // 應保留 io::Error、process::id。
use anyhow::{Error as AnyhowError, Result as AnyhowResult}; // 應使用 anyhow::Error，並以原名匯入 Result。
use anstream::{stdout, stderr}; // 應直接使用 anstream::stdout()／anstream::stderr()。
use serde_json::{Value as JsonValue, from_slice}; // 應保留 serde_json:: 前綴。
use confique::toml::{template, FormatOptions}; // 應保留 confique::toml:: 前綴。
use tokio::sync::mpsc::{Sender, channel}; // 應匯入 module 為 tokio_mpsc。
use tokio::task::spawn; // 應使用 tokio_task::spawn。

fn fail() -> anyhow::Result<()> { // 應先 use anyhow::Result，再使用 Result<()>。
    anyhow::bail!("failed"); // 應先 use anyhow::bail，再使用 bail!。
}
```

## 政策例外

- prelude 的 `drop()`、`size_of()`、`size_of_val()`、`align_of()`、`align_of_val()` 不要求加上 `mem::`，不限制其限定／未限定形式。
- 名稱衝突不是政策例外；無法安全修正時仍診斷，由使用者調整。

## 實作限制

- 無法可靠解析來源時不猜測。未限定名稱須能對應原始 import 才能檢查上方位置；不猜測由巨集或其他片段建立的 binding。不支援同名其他巨集的 textual scope／`#[macro_use]` 傳播分析。
- 巨集輸入須能依呼叫端 edition 解析為一般 Rust 運算式，僅依 compiler facts 重讀對應的原文路徑及 attributes，保留原始呼叫所在的作用域；不跨入輸入內的巢狀 module，也不展開巨集、不掃描巨集定義本體，或根據 `stringify!` 等未執行輸入中的文字推斷 API。

## Auto-fix

沿用[共用自動修正機制](../architecture.md#自動修正)與[安全邊界](../architecture.md#安全邊界)：

- 優先重用可見、位於上方、名稱合規且無條件 attributes 的 binding；否則只在最近可合法插入的作用域開頭新增獨立 import，不跨越使用點的 cfg 邊界提升。不合併或整理既有 imports，排版交由使用者的 rustfmt。
- 同作用域的新 imports 統一規劃、相同 binding 不重複新增；新的 binding 與所有依賴它的替換同屬一份 multipart suggestion。內層重用計畫中的外層 import 時仍驗證遮蔽。
- 依解析後的名稱空間、作用域、泛型參數、bindings、references 與展開產物檢查名稱捕獲；包含 module namespace，不只檢查 leaf 名稱。module 有 glob 使用端時，不新增可能改變其 bindings 的 import；重用既有 binding 仍須安全。
- 保留註解、attributes、泛型與存取語意；不將 import 插入前導註解與 item 之間。新增 import 置於 inner attributes／inner docs 之後、第一個 item／statement 及其附屬註解之前；檔頭 banner 與 inner attributes 內的註解不阻擋新增 import。此機制共用於所有 API 家族，不拆開文件歸屬，也不任意改走其他 re-export 路徑。
- 可逐一將裸名稱或 leaf alias 的使用點改為指定 namespace，例如 `id()` → `process::id()`、`IoError::other(...)` → `io::Error::other(...)`，仍須通過相同名稱解析與 cfg 安全檢查。
- 能對回原文的巨集參數使用點沿用相同修正流程，例如 `path!(current_dir().ok()?)` → `path!(env::current_dir().ok()?)`；新增 import 仍在原始程式碼作用域內，不插入巨集 token tree。
- 同一原文被巨集重複使用時，須確認各展開位置的原始解析身分與名稱安全性；無法同時保留所有使用點語意時僅診斷。
- 既有 `use`／公開 re-export 宣告不改寫、不搬移、不移除，也不展開 glob；即使改寫使用點後原 import 已無用途仍保留。因此修正後可能仍有原 import 的違規診斷或 `unused_imports`，需另行清理；啟用 `deny(unused_imports)`／`deny(warnings)` 時亦可能因此無法通過編譯。已使用合規名稱但 import 位於下方時同樣僅診斷；無法確認安全的使用點不提供修正。

## 驗證與安全性依據

- 固定工具鏈見 [`rust-toolchain.toml`](../../rust-toolchain.toml)。單元測試依政策、分析與 import edits 分組於 [`policy/tests.rs`](../../src/rules/prefer_imported_apis/policy/tests.rs)、[`analysis/tests/`](../../src/rules/prefer_imported_apis/analysis/tests/mod.rs) 與 [`fix/tests/imports.rs`](../../src/rules/prefer_imported_apis/fix/tests/imports.rs)；整合測試依 API 家族與安全邊界分組於 [`tests/rules/prefer_imported_apis/`](../../tests/rules/prefer_imported_apis/mod.rs)。測試檔案的存在不等同於特定 compiler／依賴版本已通過驗證。
- 已在 Linux host、`nightly-2026-10-15` 驗證單元、規則整合、跨規則與來源情境測試。家族 fixtures 使用 anyhow `1.0.104`、Tokio `1.54.1`、anstream `1.0.0`、serde_json `1.0.154`、toml `1.2.1`、rmp-serde `1.3.1`、postcard `1.1.3`、ciborium `0.2.2`、hex `0.4.3` 與 confique `0.4.0`（TOML／YAML／JSON5）；另以 rusqlite `0.41.1`（bundled）驗證 Error 政策。不因此宣稱其他平台、版本或所有 feature 組合皆已驗證。
- 政策參考：標準庫 [prelude](https://doc.rust-lang.org/std/prelude/v1/index.html)、[io::Result](https://doc.rust-lang.org/std/io/type.Result.html)，以及 Tokio [time](https://docs.rs/tokio/latest/tokio/time/)／[task](https://docs.rs/tokio/latest/tokio/task/)。這些文件用於核對 API 與慣例，不代表本專案的固定 alias 或所有改寫已獲安全驗證。
- 修正的驗證項目包括修正前後編譯／行為、完整修正原文比對與再次 lint 的冪等性；不將個別案例外推為所有 API 均可安全改寫。升級 compiler 或依賴時須重新查核解析身分與修正結果。

## 未決事項或後續規劃

- 待決策：若擴充 `std::fs`／`tokio::fs` 政策，需先釐清與 `prefer-pathkit` 的分工。
- 其他候選依實際需求再評估：future／iter／slice／str、rand、tempfile、base64、zstd 的額外政策；broadcast／watch、鎖／JoinHandle／Instant 的固定命名，以及其他格式 crates。不外推至所有第三方函式或巨集。
