# TODO

## 範本的 rustc／clippy 設定

不需寫 dylint，在 `rs-crate-template`、`rs-crates-template` 的 `[lints.rust]`／`[workspace.lints.rust]` 開啟即可。

- [ ] 開啟 `closure_returning_async_block = "warn"`：`move || async { … }` 回傳的 future 不能借用 closure 的捕獲，`async move || { … }` 可以。注意泛型 bound（如 `F: AsyncFn()`）無法要求 async closure 的 future 為 `Send`；接 `Fn() -> Fut, Fut: Send` 的 API 改寫後若借用捕獲會編譯失敗。

## 實作草案

- [ ] prefer-imported-paths：設計見 [rules/prefer-imported-paths.md](rules/prefer-imported-paths.md)。
- [ ] prefer-pathkit：設計見 [rules/prefer-pathkit.md](rules/prefer-pathkit.md)。

## 新規則

決定實作時依 [rules/template.md](rules/template.md) 開規則文件並更新[規則索引](rules/README.md)。

- [ ] guard-must-use：實作 `Drop` 的 `pub` 型別必須標 `#[must_use]`（比照 std 的 `MutexGuard`）。`init_logger(opts)?;` 這類寫法會讓 guard 立刻 drop（如 `kikiutils/rust` 的 `LoggerGuard` 讓日誌 worker 停止），型別沒標 `#[must_use]` 時 `unused_must_use` 不報；clippy `let_underscore_drop` 只擋 `let _ =`，`must_use_candidate` 只管函式。Auto-fix 補 attribute。
  - 待決策：是否只限 `Drop` 有副作用的 guard 類型別；是否要求 `#[must_use = "..."]` 帶訊息。
- [ ] consistent-platform-cfg（僅診斷）：平台條件只用固定寫法（如 `#[cfg(windows)]`、`#[cfg(target_os = "linux")]`），同一 workspace 不混用 `unix`、`not(windows)`。這些寫法範圍不同（`unix` 含 macOS，`not(windows)` 更廣），混用會讓平台程式在非預期平台編譯；也因此不能自動替換。
  - 待決策：允許的寫法清單（是否允許 `unix`、是否改用 `cfg_aliases` 的 `linux` 別名）；是否一併檢查 `cfg_attr` 與 `cfg!()`。
- [ ] prefer-thiserror（僅診斷）：自訂錯誤型別不手寫 `impl Display` + `impl std::error::Error`，改用 `thiserror::Error` derive；包裝其他錯誤用 `#[from]`，不手寫 `From`。Display 內容要改寫成 `#[error(...)]`，無法安全自動轉換。
  - 待決策：crate 未依賴 thiserror 時是否診斷；只實作 `Display`、未實作 `Error` 的型別是否納入。
