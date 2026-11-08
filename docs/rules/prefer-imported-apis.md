# prefer-imported-apis

狀態：**已實作**。lint：`kiki_prefer_imported_apis`。

## 目的

讓指定 API 在各 repositories 使用固定且能辨認來源的形式：一般指定項目直接匯入，其他項目依來源語意保留指定 module 或完整 crate 前綴。這是本 repo 的風格政策，不是 Rust 語意要求，也不追求所有 API 都有相同的路徑段數。

## 適用範圍

- 檢查下列 API 與 namespace 在型別、運算式、關聯項目與原始巨集呼叫路徑中的使用點；不回報或修改既有 imports（包含公開 re-export）。
- 遵守[共用分析範圍](../architecture.md#分析範圍)，涵蓋檔案頂層、各層 module、函式、方法、closure、async block 與其他巢狀區塊；依啟用中的 compiler facts 分析，不檢查 cfg 停用項目。
- 以 rustc 實際解析結果辨識 API；crate 別名、依賴改名與 re-export 不改變其身分，其他來源的同名項目不受影響。巨集參數內僅處理 rustc 已解析、且能精確對應原始輸入範圍的 API 路徑，不自行猜測其他 token trees 的語意。
- 只限制明列的 API／家族，不將自由函式政策外推至同 module 的型別、traits、方法或巨集。

## 規則

家族清單指定檢查範圍，範例不是完整成員表；API 依公開項目的解析身分辨識，不依 `from_*`／`to_*` 等文字模式猜測。

### 使用形式與匯入位置

| 政策 | 要求 |
| --- | --- |
| 直接匯入 | 匯入指定項目並使用原名；無法安全引入短名時退回完整路徑。 |
| 指定 namespace | 必須保留表列的 namespace，不得將成員直接匯入為裸名稱或型別 alias。 |

- 需要的 binding 必須在使用點可見，不檢查既有 import 的上下位置，也不補齊未使用的項目。新增 import 通常置於所在 module 開頭，遇到 `#[cfg]` 時留在該邊界內。crate root 已可由 extern prelude 使用時，不要求冗餘的 `use anyhow;`、`use serde_json;` 或 `use toml;`。
- 指定形式同時適用於型別標註、泛型、關聯項目存取與非呼叫用法，不限於 `fn()` 語法；不以有無 `.await` 判斷 API 來源。
- 使用點須採表列的完整形式，而不是只要包含該 namespace 即可。例如 `std::process::id()`、`std::io::Error` 須改為 `process::id()`、`io::Error`；`confique::toml::template` 則保留表列的來源與格式前綴。此要求只作用於受管 API，不限制 `tokio::process::Command` 等未列項目。
- 受管 API 禁止任意改名，包括使用點 namespace 的任意 alias；既有 import 的冗餘同名 `as` 不另行診斷。module alias 若只用於未受管項目（例如 `use std::process as proc; proc::Command::new(...)`），不受此限制；透過該 alias 使用受管 API（例如 `proc::id()`）則仍違規。
- 政策只套用在使用點。既有 alias、公開 re-export 與 glob 宣告原樣保留；經解析確認的合規 binding 可直接重用。
- 本規則不修改 Cargo manifest，不自行發明 alias；短名不可用時的 fallback 見[政策例外](#政策例外)。

### 直接匯入

| API／家族 | 指定名稱／使用例 |
| --- | --- |
| `anyhow::{Context, Result}` | `use anyhow::{Context, Result};`，使用 `Context`／其 trait 方法與 `Result<T>` |
| anyhow bang macros | `use anyhow::{anyhow, bail, ensure};`，使用 `anyhow!()`、`bail!()`、`ensure!()` |
| `tokio::time` 自由函式 | `sleep()`、`sleep_until()`、`timeout()`、`timeout_at()`、`interval()`、`interval_at()`；其餘自由函式依同一直接匯入政策，不強制 `tokio_time::` |

`a::bail!()` 即使經 alias 或 re-export 解析至 anyhow，仍須改為直接匯入後的 `bail!()`。其他未列入的巨集不套用此政策。

### 統一 Result 來源

- 禁止直接使用標準庫的 `std::result::Result`／`core::result::Result`，包含 prelude 的裸 `Result<T, E>` 與匯入別名；統一先匯入 `anyhow::Result`，再使用 `Result<T, E>`。
- 這是專案的強制來源政策，不因 anyhow 是否已使用、已安裝或受 feature 限定而豁免；依賴及 feature 配置由目標專案維護。
- 保留明確的錯誤型別參數，不將 `Result<T, E>` 任意改成預設錯誤為 `anyhow::Error` 的 `Result<T>`。`Ok`／`Err` 的一般使用不受限制；使用點的 `Result::Ok`／`Result::Err` 保留其後綴。
- `std::io::Result`、`std::fmt::Result` 與其他 crate 自有的 Result aliases 仍依各自政策處理；使用者自訂的同名型別不視為標準 Result。
- 不修改 Cargo manifest；anyhow 依賴或安全可見的來源名稱不可用時不修。已宣告但尚未使用的 `--extern anyhow` 可用於新增 import，不要求先存在 anyhow 使用點。
- 來源 enum 與目標 anyhow type alias 分開追蹤，舊 import 不修改。例如已有 `use std::result::Result;` 時，先改為 `anyhow::Result<T, E>`；rustc 移除 unused import 後，下一輪才可新增 `use anyhow::Result;` 並使用短名。

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
| `std::fmt::{Error, Result}` | `fmt::Error`、`fmt::Result`，不使用 `FmtError`／`FmtResult` |

`Command`、`Display`、`Formatter` 等未列型別不受限制。標準庫 re-export 自 core 的 API 依實際身分辨識，不因 DefId 位於 core 而漏查。

### 指定 crate namespace

| API／家族 | 統一形式 |
| --- | --- |
| `tokio::task` 自由函式 | `tokio::task::spawn()`、`tokio::task::spawn_blocking()`、`tokio::task::yield_now()` |
| `std::sync::mpsc` 自由函式與型別 | `std::sync::mpsc::channel()`、`std::sync::mpsc::Sender<T>`、`std::sync::mpsc::RecvError`；不直接匯入為 `channel`、`Sender` |
| `tokio::sync::mpsc` 自由函式與型別，含 error 子 module | `tokio::sync::mpsc::channel()`、`tokio::sync::mpsc::UnboundedReceiver<T>`、`tokio::sync::mpsc::error::TryRecvError`；不直接匯入為 `channel`、`Sender` |
| `tokio::sync::oneshot` 自由函式與型別，含 error 子 module | `tokio::sync::oneshot::channel()`、`tokio::sync::oneshot::Sender<T>`、`tokio::sync::oneshot::error::RecvError`；不直接匯入為 `channel`、`Sender` |
| `tracing_subscriber::fmt` 入口函式、module 與直屬自由函式／具體型別 | `tracing_subscriber::fmt()`、`tracing_subscriber::fmt::layer()`、`tracing_subscriber::fmt::Layer<S>` |
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

標準 Result 的來源替換見上方「統一 Result 來源」；`io::Result` 與 `fmt::Result` 不改成 anyhow。`Serialize`、`Deserialize`、`DeserializeOwned`、`ToHex` 等 traits 不因轉換函式政策而要求限定路徑。

anstream 此項只涵蓋 `stdout`／`stderr` 函式，不連帶限制同名型別或輸出巨集；公開 API 見 [anstream 文件](https://docs.rs/anstream/latest/anstream/)。使用 crate namespace 不要求額外的 `use anstream;`。

confique 保留完整的 `confique::<格式>::` 前綴，不直接匯入 `template`／`FormatOptions`，也不將格式 module 匯入為 `toml`、`yaml`、`json5` 或另設 alias，以區分同名格式 crates。此項只涵蓋表列函式與型別（含其關聯項目存取），不限制 `Config`／`Layer` traits 或其他未列 API；格式 features 見 [confique 文件](https://docs.rs/confique/0.4.0/confique/#cargo-features)。

`tokio::spawn` 與 `tokio::task::spawn` 若解析為相同 API，均使用 `tokio::task::spawn`。此項不限制 `JoinHandle` 等型別命名。

`std::fmt` 與 `tracing_subscriber::fmt` 是不同的 module，同時匯入為 `fmt` 會衝突。因此標準庫保留 `fmt::…`，tracing 一律保留完整 crate 前綴；`FormatEvent`／`FormatFields` 等 traits 可正常匯入，不擴大限制其他 tracing API。

### 系統與非同步 I/O namespace

| API／家族 | 統一形式 |
| --- | --- |
| `rustix::io` 自由函式與 rustix 自有具體型別 | `rustix::io::write()`、`rustix::io::Errno`、`rustix::io::Result<T>` |
| `tokio::io` 自由函式與 Tokio 自有具體型別 | `tokio::io::duplex()`、`tokio::io::BufReader<T>` |
| `libc` 自由函式、常數與 libc 自有具體型別 | `libc::fork()`、`libc::syscall()`、`libc::AT_EMPTY_PATH` |

不將這些 module 先匯入為 `io` 或其他 alias。traits（包含 `AsyncReadExt`／`AsyncWriteExt`）可正常匯入。標準庫／core 定義的共享 re-exports 不納入新家族：Tokio 的 `Error`／`ErrorKind`／`Result` 沿用標準庫政策；rustix 的 std `IoSlice`／`IoSliceMut`、libc 的 core `c_int`／`c_void` 等不因新家族而被要求改成該 crate 的路徑。是否屬於共享項目依編譯器解析身分判定。

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

use tokio::time::sleep;

async fn send_later() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<u8>(1);
    tokio::task::spawn(async move {
        sleep(Duration::from_millis(10)).await;
        let _ = sender.send(1).await;
    });
    let _ = receiver.recv().await;
}
```

## 錯誤範例

以下使用點違反指定形式；原有 imports 保留，不是診斷對象：

```rust
use std::process::id;
use anyhow::Result as AnyhowResult;

fn fail() -> AnyhowResult<()> { // 改用 Result<()>，必要時新增 use anyhow::Result;
    let _ = id(); // 改用 process::id()，必要時新增 use std::process;
    anyhow::bail!("failed"); // 改用 bail!()，必要時新增 use anyhow::bail;
}
```

## 政策例外

- prelude 的 `drop()`、`size_of()`、`size_of_val()`、`align_of()`、`align_of_val()` 不要求加上 `mem::`，不限制其限定／未限定形式。
- namespace 的規定短名無法安全新增或重用時，允許使用該 API 的完整來源路徑，例如本地有 `mod io` 時使用 `std::io::Result`、本地有 `mod process` 時使用 `std::process::id()`。這是所有 qualified 家族共用的 fallback，不專屬於 I/O；短名可用時仍要求規定形式。來源 crate root 本身亦無法安全解析時不修，不猜測其他名稱。直接匯入項目亦採相同完整路徑 fallback。

## 實作限制

- 無法可靠解析來源時不猜測。名稱檢查包含 generic、value binding 與 `macro_rules!` 文字作用域；不自行展開巨集或猜測未解析的名稱。
- 巨集輸入支援依呼叫端 edition 解析的一般 Rust 運算式，以及 `matches!` 的「運算式、pattern（可含 guard）」形式，包含巢狀呼叫。僅依 compiler facts 重讀對應的原文路徑及 attributes，保留原始呼叫所在的作用域；不跨入輸入內的巢狀 module，也不展開巨集、不掃描巨集定義本體，或根據 `stringify!` 等未執行輸入中的文字推斷 API。

## Auto-fix

沿用[共用自動修正機制](../architecture.md#自動修正)與[安全邊界](../architecture.md#安全邊界)：

- 回報與修正共用 `decide(access)`：規定形式 `Canonical`、不插入 import 的完整路徑 `FullPath`、不修的 `Skip`。符合決策的使用點不再回報；同一位置修正後再次分析應為零 issue。
- 只改使用點、只新增 `use 路徑;` 或 `use 路徑 as 別名;`；既有 `use` 一律不碰。unused import 交給 rustc，排序、合併與排版交給其他規則及 rustfmt。
- target 通常是所在檔案或 inline module，沒有 cfg 的函式內使用亦提升到 module。遇到最近的 `#[cfg]` 邊界時，取邊界內包含使用點的最外層函式本體或 block：cfg fn 取本體，cfg block 取該 block，cfg impl／trait 取所在方法本體；沒有 block（例如 initializer 不是 block 的 cfg let）則退回完整路徑。`cfg_attr` 不算此邊界。
- 固定插在 target 開頭（檔頭或 `{` 之後），僅略過開頭的 inner attributes／inner docs。不分析既有 use 排列，不拆開 item 與其前導註解。同 target 的多條 imports 合併為一筆插入。
- 新增或重用名稱前，驗證同 namespace 的定義（CST 所有 cfg 分支及 compiler 展開產物）、整個 target 與 `use super::*` 子 module 的既有引用，以及使用點和 target 間更近的 item／use、generic、let 與 macro 遮蔽。來源 crate root 必須對應 extern prelude 的直接依賴（可透過已解析、指向該直接依賴的本地 crate alias），且未被其他本地同名定義遮蔽；不安全則退回完整路徑，連 root 都不安全則不修。
- 所有使用點改寫與新增 imports 合成同檔唯一的 multipart Fix，掛在第一個 issue，確保互相依賴的修改一同套用。與其他規則編輯重疊時可能需下一輪才能套用。
- 保留泛型與關聯路徑後綴；使用點範圍含註解或位於 derive／attribute 輸入內時不修。所在 item 的已知 builtin／derive／helper attributes 可由 compiler facts 放行；未知 attribute macro 仍保留屏障。
- 能精確對回原文的巨集參數使用點依原始呼叫外圍的 CST 決定 target，不在 token tree 插入 use，也不處理 macro 定義本體。相同原文被多次展開時，名稱安全檢查必須對每個 compiler origin 成立。
- 同名來源替換可能需多輪完成：先使用完整路徑，待 rustc 清除舊 unused import 後，再引入短名。每輪修復均須維持可編譯與既有引用語意；未啟用 cfg 分支的使用點留待相應平台／設定處理。

## 驗證與安全性依據

- 固定工具鏈見 [`rust-toolchain.toml`](../../rust-toolchain.toml)。單元測試依政策與分析分組於 [`policy/tests.rs`](../../src/rules/prefer_imported_apis/policy/tests.rs)、[`analysis/tests/`](../../src/rules/prefer_imported_apis/analysis/tests/mod.rs)；整合測試依 API 家族與安全邊界分組於 [`tests/rules/prefer_imported_apis/`](../../tests/rules/prefer_imported_apis/mod.rs)。測試檔案的存在不等同於特定 compiler／依賴版本已通過驗證。
- 工具鏈固定為 `nightly-2026-10-15`；家族 fixtures 使用 anyhow `1.0.104`、Tokio `1.54.1`、tracing-subscriber `0.3.23`（`default-features = false`、`features = ["fmt"]`）、anstream `1.0.0`、serde_json `1.0.154`、toml `1.2.1`、rmp-serde `1.3.1`、postcard `1.1.3`、ciborium `0.2.2`、hex `0.4.3` 與 confique `0.4.0`（TOML／YAML／JSON5）；另以 rusqlite `0.41.1`（bundled）驗證 Error 政策；系統家族 fixtures 使用 rustix `1.1.6`、libc `0.2.191`，derive 路徑修復包含 thiserror `2.0.21`。不因此宣稱其他平台、版本或所有 feature 組合皆已驗證。
- 政策參考：標準庫 [prelude](https://doc.rust-lang.org/std/prelude/v1/index.html)、[io::Result](https://doc.rust-lang.org/std/io/type.Result.html)，以及 Tokio [time](https://docs.rs/tokio/latest/tokio/time/)／[task](https://docs.rs/tokio/latest/tokio/task/)。這些文件用於核對 API 與慣例，不代表本專案的命名政策或所有改寫已獲安全驗證。
- 修正的驗證項目包括修正前後編譯／行為、完整修正原文比對與再次 lint 的冪等性；不將個別案例外推為所有 API 均可安全改寫。升級 compiler 或依賴時須重新查核解析身分與修正結果。

## 未決事項或後續規劃

- 待決策：若擴充 `std::fs`／`tokio::fs` 政策，需先釐清與 `prefer-pathkit` 的分工。
- 其他候選依實際需求再評估：future／iter／slice／str、rand、tempfile、base64、zstd 的額外政策；broadcast／watch、鎖／JoinHandle／Instant 的固定命名，以及其他格式 crates。不外推至所有第三方函式或巨集。
