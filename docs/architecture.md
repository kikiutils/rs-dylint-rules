# 架構

## 專案定位

`kiki-rs-dylint-rules` 是單一 Dylint 動態規則庫，產物為 `cdylib`，不提供獨立執行檔或 compiler driver。檢查與自動修正使用 `cargo dylint`／`cargo dylint --fix`。

各規則的 lint 名稱、實作狀態與 auto-fix 支援程度見[規則索引](rules/README.md)。本文件集中說明已實作規則的共用機制；草案的設計與未決事項留在各規則文件。

## 模組分工

| 模組 | 責任 |
|---|---|
| `src/lib.rs` | 從 `rules::Rule::VARIANTS` 註冊五個 lint，並註冊唯一的 `KikiRules` late pass。 |
| `src/pass.rs` | 收集模組來源，將 HIR 事件分派給 collectors，於 crate 結束時逐檔分析與 emit。 |
| `src/workspace.rs` | `Workspace`：快取來源路徑，記錄 crate 載入的檔案與模組來源（含共用檔案判定）。 |
| `src/source.rs` | 讀取並解析單一來源檔，處理 BOM／CRLF 位移與 span 換算。 |
| `src/facts.rs` | 依實體路徑分檔保存各規則的 compiler facts，每筆附 module 編號與 HirId。 |
| `src/syntax.rs` | 共用 attributes 保護、條件邊界、註解與涵蓋節點 helper。 |
| `src/diagnostic.rs` | 語法層的診斷與 edits，以及共用的 crate 層級 emit。 |
| `src/rules/*/collect.rs` | 收集 rustc 語意並銜接各規則的分析。`statement_padding` 不需 facts，沒有 collector，其分析入口在 `mod.rs`。 |
| `src/rules/*/analysis.rs` | 依 CST、原文與 compiler facts 產生診斷及安全 edits。 |
| `src/rules/prefer_imported_apis/resolution.rs` | 使用 rustc 的 binding、reference、namespace 與 glob 資訊判斷名稱安全性。 |
| `src/test_support.rs` | analysis 單元測試套用 edits 的 helper。 |
| `tests/rules/main.rs` | 規則專屬整合測試入口，各規則案例位於同目錄；跨規則與來源情境測試保留於 `tests/`。 |
| `tests/support/mod.rs` | 整合測試共用 fixture、Cargo 執行及 JSON 診斷判讀；放在子目錄避免被 Cargo 當成獨立測試 target。 |

## 規則註冊與順序

`src/rules/mod.rs` 的 `Rule::VARIANTS` 是 lint 註冊及逐檔分析、emit 順序的唯一來源：

1. `prefer_imported_apis`
2. `import_path_prefix`
3. `import_module_order`
4. `grouped_derive_order`
5. `statement_padding`

順序決定診斷的輸出先後，以及同一輪修正重疊時誰先套用；重疊的修正可能需要分多輪分析與套用。

`Rule` 提供 lint、suggestion 說明及逐檔分析。HIR 收集由 `Collectors` 上與 lint pass 同名的 `check_*` 方法轉交給需要的 collector，與 `Rule::VARIANTS` 順序無關。

## 執行流程

語意以 rustc 為準；`ra_ap_syntax` 僅解析原文 CST，不建立 rust-analyzer 語意資料庫，也不自行探索模組關係。

1. Dylint 以相同 nightly 的 driver 執行 Cargo 分析，rustc 決定 target、features 與 cfg 下實際載入的來源。
2. `check_mod` 記錄模組來源。item、path、expression、generic parameter 與 foreign item 的 HIR 事件交給 `Collectors` 的對應 `check_*` 方法，供各 collector 收集語意。
3. 來源路徑以 rustc `SourceFile` 的起始位置為 key 快取，包含無法使用的結果。工作目錄只 canonicalize 一次，僅納入位於該目錄內的實體來源。
4. facts 依實體路徑分檔，每筆保留 module 編號與 HirId。分析前逐筆檢查來源是否屬於該模組。
5. `check_crate_post` 逐檔以 `Source::load` 讀取並解析，每個路徑只處理一次，再按 `Rule::VARIANTS` 順序執行各規則的 analysis 與 emit。
6. `RuleOutput` 統一由 crate 層級產生標準 rustc lint 與 `MachineApplicable` multipart suggestions。指定 `--fix` 時，由 Cargo fix 套用。

同一次編譯中的各規則均分析 rustc 載入的原文；規則不自行套用 edits 或將中間修正結果交給下一條規則。

## 原文與診斷位置

- 來源比對會移除 UTF-8 BOM 並將 CRLF 正規化為 LF，與 rustc 載入的文字比較。
- CST 解析將開頭 BOM 替換成等長的三個空白，保留原始 byte offsets。`Source.text` 保存原文，供 analysis 直接切片。
- facts 透過 `original_relative_byte_pos` 對應 CST；emit 透過 `normalized_byte_pos` 轉回 rustc spans。
- 讀檔失敗、來源已變動或 CST 解析失敗時，輸出 warning 並略過該檔案。每個路徑只載入一次，同一檔案不會因五個規則而重複警告。

## 分析範圍

- 來源由當次 Cargo／rustc 在指定 target、features 與 cfg 下實際載入的模組決定，包含 crate root；不自行探索未載入的檔案或其他編譯設定。
- 僅處理 canonicalize 後位於 compiler 工作目錄內、已登記為 module 原始定義檔的實體來源。不依固定目錄、`.rs` 副檔名或 Git 忽略規則篩選。
- 獨立 `include!` item／expression 片段不屬於模組原始定義檔，因此略過。compiler facts 另須符合該檔案的 module 歸屬。
- `statement_padding` 檢查已載入模組檔案的整份 CST，包含其中 cfg 停用的程式碼；不因此載入停用的外部模組檔案。其他四條規則僅根據啟用中的 compiler facts 分析，停用項目不列為 unresolved。
- rustdoc 程式碼區塊、巨集輸入 token trees 與展開產物不直接當成原始程式碼分析；原始巨集呼叫與 derive 的處理方式見各規則文件。

## 診斷層級

診斷統一以 `CRATE_HIR_ID` 輸出，遵守 crate 層級與命令列的 lint 設定，不查找 item 或 module 的 lint level。因此局部 `allow`／`expect` 不會套用至這些診斷。名稱解析所需的 HirId 仍保留。

## 安全邊界

- 相同實體範圍被不同 module 載入時可提供診斷，但整份檔案不提供 fix。一般 inline module 的範圍不同，不會因此停用 fix。
- import 與 derive 規則使用 `syntax::protected` 共用已知 builtin／tool attribute 安全清單（另含不讀取 derive 的 serde_with `skip_serializing_none`，以及原樣保留函式本體的 `tokio::test`／`tokio::main`）；`cfg_attr` 遞迴檢查所有分支，未知 attributes 保留屏障。derive helpers 另要求相同 item 有對應的已解析 derive。僅補空行的 `statement_padding` 不使用這項 attribute 判定。
- 搬移或拆分 imports 仍須保留各規則的註解、cfg、名稱解析與 `expect` 保護。安全 attribute 不代表可以任意搬移帶 attributes 的 item。

## 自動修正

- 可安全套用的 edits 以 `MachineApplicable` multipart suggestions 提供，由 `cargo dylint --fix` 交給 Cargo fix 套用；規則不直接寫檔。
- 無法確認安全的修正不提供 suggestion。未產生診斷或未提供修正，可能是支援範圍、安全條件或執行失敗所致，不代表程式碼符合規則或屬於政策例外。
- 排版交給使用者的 rustfmt；規則不呼叫 rustfmt 或 Clippy。LF／CRLF、縮排及尾逗號的保留方式見各規則文件。建議流程為 Dylint → 既有 fmt/lint。

## 執行限制

固定使用 `nightly-2026-10-15` 與 rustc private API。升級 nightly 需適配 API 並重新編譯。分析仍會執行目標專案的 build scripts 與 proc macros；目標專案須具備所選 target 的標準函式庫及原生依賴。workspace、target 與 features 由使用者指定，不自動窮舉其他環境。

來源讀取、編譯或 CST 解析失敗時，不保證能產生各規則診斷。

測試以獨立功能、診斷、安全屏障與 edit 結果為單位，不建立 lint fix 與 rustfmt 交替執行的驗收流程。
