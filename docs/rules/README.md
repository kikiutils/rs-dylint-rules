# 規則索引

以下區分已實作規則與設計草案；草案尚未註冊 lint，不會產生診斷或自動修正。已實作規則以 [`Rule`](../../src/rules/mod.rs) 的註冊為準。

| 規則 | lint 名稱 | 狀態 | Auto-fix |
| --- | --- | --- | --- |
| [prefer-imported-apis](prefer-imported-apis.md) | `kiki_prefer_imported_apis` | 已實作 | 安全的限定路徑替換與 import 新增／重用；違規 aliases 與 glob 僅診斷 |
| [import-path-prefix](import-path-prefix.md) | `kiki_import_path_prefix` | 已實作 | 安全的前綴替換與分組拆分；需改寫成群組形式的 module 匯入僅診斷 |
| [import-module-order](import-module-order.md) | `kiki_import_module_order` | 已實作 | 可安全重排的連續區段；其餘違規僅診斷 |
| [grouped-derive-order](grouped-derive-order.md) | `kiki_grouped_derive_order` | 已實作 | 純已知 derive 列表的安全排序；含未知 derive 的列表略過 |
| [statement-padding](statement-padding.md) | `kiki_statement_padding` | 已實作 | 補上缺少的空行；同一行相鄰元素僅診斷 |
| [prefer-imported-paths](prefer-imported-paths.md) | —（未註冊） | 草案，尚未實作 | 尚未提供 |
| [prefer-pathkit](prefer-pathkit.md) | —（未註冊） | 草案，尚未實作 | 尚未提供 |

共用的[分析範圍](../architecture.md#分析範圍)、[診斷層級](../architecture.md#診斷層級)、[安全邊界](../architecture.md#安全邊界)與[自動修正機制](../architecture.md#自動修正)集中在架構文件，各規則只補充自身政策與限制。

[template.md](template.md) 是撰寫規則文件的模板，不是已註冊或規劃中的規則。
