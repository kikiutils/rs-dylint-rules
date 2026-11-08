# 架構

## 專案定位

個人使用定位與維護取捨以根目錄 [AGENTS.md](../AGENTS.md#專案定位) 為準；本文件只說明共用架構與執行機制。

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
| `src/derives.rs` | 共用 derive 登記表，依定義 crate 與 derive 名稱提供 helper metadata；各規則仍分別決定路徑修改及排序政策。 |
| `src/diagnostic.rs` | 語法層的診斷與 edits，以及共用的 crate 層級 emit。 |
| `src/rules/*/collect.rs`／`collect/` | 收集 rustc 語意並銜接各規則的分析。`statement_padding` 不需 facts，沒有 collector，其分析入口在 `mod.rs`。 |
| `src/rules/*/analysis.rs`／`analysis/` | 依 CST、原文與 compiler facts 產生診斷與修正要求。 |
| `src/rules/prefer_imported_apis/policy/` | 分檔宣告 API 家族，由公開 exports 建立 DefId 索引，集中直接名稱、指定 module 與完整 crate 前綴政策。 |
| `src/rules/prefer_imported_apis/names/` | 收齊 rustc bindings 後解析原始路徑各段的 lexical／qualified references（包含 import 與已解析的 bang／attribute／derive 巨集前綴），依名稱空間、作用域與 glob 使用端判斷新增／重用名稱是否安全。 |
| `src/rules/prefer_imported_apis/fix.rs` | 將 analysis 的共用決策轉為使用點替換及新增 imports；依 target／binding 去重，同檔所有 edits 合成一筆 multipart suggestion。 |
| `src/test_support.rs` | analysis 單元測試套用 edits 的 helper。 |
| `tests/rules/main.rs` | 規則專屬整合測試入口；`prefer_imported_apis/` 依家族與安全邊界分組，其他規則案例位於同目錄。跨規則與來源情境測試保留於 `tests/`。 |
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
- `statement_padding` 檢查已載入模組檔案的整份 CST，包含其中 cfg 停用的程式碼；不因此載入停用的外部模組檔案。其他規則的診斷根據啟用中的 compiler facts；imports 排序可將原文即可確定分類的停用宣告一併原樣搬移，但不單獨診斷它們。停用項目不列為 unresolved。
- rustdoc 程式碼區塊、巨集輸入 token trees 與展開產物不直接當成一般原始程式碼分析；`prefer_imported_apis` 可依 rustc 已解析且精確對應原始輸入的 facts 處理巨集參數 API 路徑，不自行推斷其他 tokens 的語意。原始巨集呼叫與 derive 的處理方式見各規則文件。

## 診斷層級

診斷統一以 `CRATE_HIR_ID` 輸出，遵守 crate 層級與命令列的 lint 設定，不查找 item 或 module 的 lint level。因此局部 `allow`／`expect` 不會套用至這些診斷。名稱解析所需的 HirId 仍保留。

## 安全邊界

- 相同實體範圍被不同 module 載入時，語意規則可提供診斷但不提供 fix；純語法的 `statement_padding` 不受這項限制。一般 inline module 的範圍不同，不會因此停用語意規則的 fix。
- import 與 derive 規則使用 `syntax::protected` 共用已知 builtin／tool attribute 安全清單（另含不讀取 derive 的 serde_with `skip_serializing_none`，以及原樣保留函式本體的 `tokio::test`／`tokio::main`）；`cfg_attr` 遞迴檢查所有分支，未知 attributes 保留屏障。derive helpers 另要求相同 item 有對應的已解析 derive。僅補空行的 `statement_padding` 不使用這項 attribute 判定。
- `prefer_imported_apis` 的路徑替換另以 compiler-resolved derive facts 放行已知 derive／helper，包含 `cfg_attr` 包住的 derive 與其 helpers；這不放寬 derive 排序或未知 attribute macro。derive/helper metadata 由共用登記表提供，排序名單與路徑修改的放行範圍仍分開決定。imports 排序則僅放行原樣附著的 cfg／不含 expect 的 lint attributes，依修改操作分別判斷安全性。
- API 家族對共享 re-exports 依 DefId 與來源 crate 篩選；std／core 替代路徑有明確優先級，不以宣告迭代順序決定衝突政策。
- 標準 Result → anyhow Result 這類來源替換區分原 API 與目標 binding 的身分，不覆蓋或移除舊 binding；短名衝突時先使用完整路徑，不能把相同文字名稱當成相同型別定義。尚未載入 metadata 的直接依賴可依確切 `--extern` 名稱新增 import，但不以未知 DefId 當作既有 binding 的相等依據。
- 搬移或拆分 imports 仍須保留各規則的註解、cfg、名稱解析與 `expect` 保護。安全 attribute 不代表可以任意搬移帶 attributes 的 item。

## 自動修正

- 可安全套用的 edits 以 `MachineApplicable` multipart suggestions 提供，由 `cargo dylint --fix` 交給 Cargo fix 套用；規則不直接寫檔。
- `prefer_imported_apis` 只改使用點並新增 imports，既有 `use` 不修改；決策、插入位置與名稱安全條件見[規則文件](rules/prefer-imported-apis.md#auto-fix)。
- 無法確認安全的修正不提供 suggestion。未產生診斷或未提供修正，可能是支援範圍、安全條件或執行失敗所致，不代表程式碼符合規則或屬於政策例外。
- 排版交給使用者的 rustfmt；規則不呼叫 rustfmt 或 Clippy。`prefer_imported_apis` 新增 import 時只附上 LF 換行；舊 imports 的 unused 清理由 rustc 處理，合併與排序交給其他規則及 rustfmt。其他規則的原文保留方式見各規則文件。建議流程為 Dylint → 既有 fmt/lint。

## 執行限制

固定使用 [`rust-toolchain.toml`](../rust-toolchain.toml) 指定的 nightly 與 rustc private API。升級 nightly 需適配 API 並重新編譯。分析仍會執行目標專案的 build scripts 與 proc macros；目標專案須具備所選 target 的標準函式庫及原生依賴。workspace、target 與 features 由使用者指定，不自動窮舉其他環境。

來源讀取、編譯或 CST 解析失敗時，不保證能產生各規則診斷。

測試以獨立功能、診斷、安全屏障與 edit 結果為單位，不建立 lint fix 與 rustfmt 交替執行的驗收流程。
