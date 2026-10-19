# 架構

## 專案定位

`kiki-rs-dylint-rules` 是單一 Dylint 動態規則庫，產物為 `cdylib`，不提供獨立執行檔或自訂 compiler driver。目前只實作 `import_module_order`；檢查與自動修正直接使用 `cargo dylint`／`cargo dylint --fix`。

## 模組分工

| 模組 | 職責 |
| --- | --- |
| `src/lib.rs` | 提供 Dylint ABI 入口，註冊所有規則。 |
| `src/rules/mod.rs` | 維護規則註冊函式列表 `RULES`。 |
| `src/rules/<rule>/` | 每條規則一個目錄，負責語意收集、檢查、診斷及修正提案。 |
| `src/source.rs` | 驗證來源、解析原文 CST，轉換 rustc spans 與磁碟原文位元組偏移。路徑與檔案讀取統一使用 pathkit。 |
| `src/diagnostic.rs` | 定義規則內部的 `RuleOutput`、`Issue`、`Diagnostic`、`Fix` 與 `Edit`。 |
| `tests/` | 驗證實際 Dylint 診斷、lint 層級及 `--fix` 行為。 |

## 規則註冊

每條規則的 `mod.rs` 匯出註冊函式常數 `RULE`：

```rust
pub(super) const RULE: fn(&mut rustc_lint::LintStore) = self::compiler::register;
```

在 `src/rules/mod.rs` 宣告模組並將 `RULE` 加入 `RULES`。`src/lib.rs` 的 `register_lints` 依序呼叫這些函式，由各規則自行註冊 lint 與 compiler pass；新增規則不需修改共用來源處理。

## 執行流程

語意以 rustc 為準；`ra_ap_syntax` 僅用於原文 CST 解析，不建立 rust-analyzer 語意資料庫，也不自行探索模組關係。

1. Dylint 取得、編譯並載入規則庫，使用相同 nightly 的 driver 執行 Cargo 分析。
2. rustc 決定當次 target、features 與 cfg 下實際載入的來源。規則僅處理 canonicalize 後位於 compiler 工作目錄內的實體來源，不遍歷全部 `.rs` 檔案。
3. 規則收集語意，使用原文 CST 檢查各作用域，將結果轉成標準 rustc lint 診斷。lint attributes 控制診斷層級。
4. 可安全修正時，診斷附上 `MachineApplicable` multipart suggestion；指定 `--fix` 時，由標準 Cargo fix 流程套用。

規則不直接寫檔，不建立額外報告協定，也不進行跨平台或多環境修正仲裁。workspace、target 與 features 由使用者透過 Dylint／Cargo 參數指定，不自動補充其他平台或窮舉 feature 組合。

## 執行限制與副作用

- 固定使用 `nightly-2026-10-01` 與 rustc private API；升級 nightly 需適配 API 並重新編譯規則庫。
- 分析會執行 build scripts 與 proc macros，Cargo 也可能更新 lockfile；只對可信任專案執行。
- 目標專案須具備所選 target 的標準函式庫、原生依賴與 SDK；編譯失敗時，不保證能產生完整規則診斷。
- 不檢查當次未啟用的程式碼，也不保證修正在其他 target 或 feature 組合下相容。
- `include!` 的實體來源目前須能解析為 Rust item source file。
- 不執行 rustfmt 或 Clippy。`--fix` 使用標準 Cargo fix 流程，可能同時套用 rustc 的其他修正建議，並非只限本規則。

## 上游參考

- [Dylint](https://github.com/trailofbits/dylint)
- [dylint_linting API](https://docs.rs/dylint_linting/6.2.0/dylint_linting/)
