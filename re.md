prefer_imported_apis 自動修復重寫：實作清單，依實作順序排列。

一、核心規則（後面每一步都依此實作）

1. 只改使用點、只插入 use，舊的 use 一律不碰。沒人用的舊 import 交給 rustc 的 unused_imports，排版、合併、排序交給 rustfmt 和其他規則。
2. 每一輪修完都必須能編譯。名稱檢查不通過時，退回完整路徑或不修。
3. 回報和修正共用同一份決策。修完再跑一次，同一個位置不能再被回報。

二、實作順序

重寫期間允許短暫無法編譯、測試失敗或 lint 有錯，不為了讓每一步都能編譯而先寫之後會被改掉的過渡程式碼。

1. 共用決策 decide(access) -> Decision（analysis 層）

決策結果有三種：

- Canonical { target, binding }：改成規定寫法；若 access 處的名稱還沒有解析到同一個定義，就在 target 插入 use。
- FullPath：改成完整路徑，不插入 use。
- Skip：不修。例如在 derive 或 attribute 內，或範圍內有註解。

已經符合 decide 結果的寫法就不回報。原本的 fallback.rs 併入這裡。

- 原因：目前回報端和修正端各自判斷，必須互相同步，這是補丁一直疊起來的根源。其餘每一步都依賴這份決策，所以最先做。

2. 插入目標 target（純 CST）

- 從 access 往上找到所在的 module（檔案或 inline mod）。
- 途中沒有 #[cfg]：target 就是這個 module。沒有 cfg 的 fn 內使用也一樣，例如 ECP 的 build.rs 會插在檔案頂端。
- 途中有 #[cfg]：以離 access 最近的 cfg 節點為邊界，在邊界內取包含 access、最外層的 fn 本體或 block。
  - cfg fn：該 fn 的本體。
  - cfg block：該 block。
  - cfg impl / trait：access 所在方法的本體。
  - 邊界內沒有 block（例如 #[cfg(..)] let x = ..;）：改用 FullPath。
- 只有 #[cfg] 算邊界，cfg_attr 不算。
- 原因：use 不能跑出 cfg 作用範圍；在邊界內取最外層，同一個 fn 的多個使用點可以共用一條 use。

3. 插入點

- 一律插在 target 的開頭，不看既有的 use。
  - module：檔案開頭，或 inline mod 的 { 之後。
  - fn 本體 / block：{ 之後。
- 唯一例外：若開頭有 inner attribute 或 inner doc（#![..]、//!），插在它們之後。use 放到它們前面會編譯失敗。
- 內容只有 use 路徑; 或 use 路徑 as 別名;。同一個 target 的多條 use 合成一筆編輯。
- 原因：位置固定、不需要分析既有 import 和註解歸屬。插在開頭也不會拆開某個 item 和它上方的註解。合併、排序、空行交給 rustfmt 和 import_module_order。

4. 名稱安全

以下任何一項不通過就改用 FullPath；連路徑根都有問題就 Skip。

- a. target 裡沒有同名的其他定義：
  - 相同 namespace 中，沒有同名的 item、use、extern crate 或 mod。例如自己建了 io.rs（mod io;）就在這裡擋下。
  - 同時查 CST（看得到所有 cfg 分支，例如 #[cfg(windows)] mod io;）和編譯器資料（看得到 macro 產生的 item）。
- b. 不會改變現有引用的意義：
  - 範圍：整個 target（block 內的 use 對整個 block 可見，包括 use 之前的語句和巢狀 item），以及用 use super::* 引入 target 的子 module。
  - 不能有我們沒改寫、目前經由 glob 或 prelude 解析到其他定義的同名引用。新插入的 use 會蓋掉它們。
- c. 使用點會解析到新綁定：使用點和 target 之間沒有更近的遮蔽，包括內層 item / use、generic 參數、let（value namespace）、macro_rules! 文字作用域（macro namespace）。
- d. 路徑根是預期的 crate：在 target 沒有被本地同名 item 遮蔽，且是直接依賴（在 extern prelude 裡）。
- 原因：這是保證「每一輪都能編譯、不改變意義」的唯一防線。
- 做法：Names::safe 依這四項重新整理，NameRequest 只保留決策用得到的欄位。

5. 新的 fix/（單一檔案，預估 150–250 行）

1. 收集每個 access 的 decide 結果。
2. 依 (target, binding) 去重。
3. 產生編輯：改寫使用點，並在每個 target 開頭插入需要的 use。
4. 這個檔案的所有編輯合成一個 Fix，掛在第一個 issue 上。

- 原因：
  - 改寫使用點和插入 use 互相依賴，必須一起套用。
  - rustfix 會把不同建議在同一位置的插入當成衝突，合成一個 Fix 就不會發生。
  - 所有檢查都在決策時做完，規劃不會中途失敗，不需要交易和相依矩陣。

6. 刪除舊實作

- 刪除整個檔案：fix/plan.rs、fix/rewrite_imports.rs、fix/imports.rs、analysis/fallback.rs、analysis/tests/transactions.rs。
- 刪除的欄位與 API：
  - Checks 的 remove、depends、reference。
  - RemovalRequest、ReplacedName。
  - Names 的 removable、depends、reference_ranges。
  - FactIndex 的 import_origins、source_imports。
- 不再回報 import 本身：拿掉 import diagnostic 和 reported_imports，只回報使用點。
- names/paths.rs 的路徑解析有多少只為刪除 import 服務，逐項確認後再刪。

7. 測試

- 既有整合測試：更新和舊 import 刪除相關的預期結果。
- 新增：
  - 同名 mod（io.rs）。
  - use super::* 的子 module。
  - cfg 的 fn、block、impl 方法；沒有 block 的 cfg 語句（FullPath）；巢狀 cfg（以內層為準）。
  - 沒有 cfg 的 fn 內使用（插在 module 開頭）。
  - 開頭有 #![..] / //! 的檔案。
  - 只被間接依賴的 crate（被 4.d 擋下）。
- 冪等：修完再跑一次必須是 0 個 issue。

8. 文件與實測

- 更新 docs/rules/prefer-imported-apis.md 和 docs/architecture.md。
- 在 ECP 跑 fix.sh 驗證。(最後由用戶自行執行)

三、注意事項

- 同名替換要跑多輪才完成：例如已有 use std::result::Result;，第一輪改成 anyhow::Result<T>，rustc 刪掉沒人用的舊 import 後，下一輪才改成 Result<T> 並插入 use。
- 和其他規則改到同一段時，這一輪會被跳過，下一輪再套用。
- macro 輸入裡的 access 依呼叫點外圍的 CST 決定 target；macro 定義本體內的路徑不處理。
- 編譯器只看得到這輪啟用的 cfg 分支；其餘分支的使用點留到在對應平台跑 fix 時處理。
