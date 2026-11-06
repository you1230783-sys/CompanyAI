# 0.9.4 純搬移對照

前置修正：91a1797（批次panic）、b0a7374（Word位置比較提示）。下列搬移不包含這兩項行為修改。

此對照只驗證v0.9.4的來源。後續版本會在拆分後的檔案繼續修改功能；重新核對本表時，請在v0.9.4標籤的獨立checkout執行，不能把最新main與0.9.4基準不同直接判成當時搬移失敗。

核對方式：先執行scripts/Enter-DevShell.ps1，再執行`python scripts/Verify-SourceMoves.py docs/refactors/0.9.4-files.json docs/refactors/0.9.4-ui.json docs/refactors/0.9.4-runner.json`。工具以固定rustfmt正規化後比較詞元，保留字串與註解；不是以刪除所有空白或逗號放寬比對。

三份JSON保存搬移前提交、來源行號、本體SHA256與必要路徑替換。父模組保留的98個函式也核對原文；87個搬移函式在對照內逐一列出。可見性、公開重匯出、型別與模組宣告仍需搭配git diff審查，不能只憑本體雜湊宣稱所有行為已證明相同。

| 原檔 | 搬移前／後行數 | 拆分提交 |
| --- | --- | --- |
| src/projects/files.rs | 4844／1663 | aa36296 |
| src/ui.rs | 2645／594 | 74d80d1 |
| src/projects/runner.rs | 1932／287 | 146199f |

保留原公開入口；子模組私有輔助方法使用pub(super)，原projects層的pub(super)改為pub(in crate::projects)。UI的ComApartment保留ui層可見範圍。include_str因目錄改變而修正相對路徑，資源內容不變。

run_for與大型工具match仍完整保留在專責模組；本次不將函式內控制流程拆成新抽象，以免將邏輯重構混入純搬移。panic來源檔案位置自然隨搬移改變，診斷仍保存對應版本與新位置。

## files

來源提交：`5544415e57e96ff47f22d506b6ca14d3c4f2bd0d`。

| 函式 | 原位置 | 新位置 |
| --- | --- | --- |
| `new` | `src/projects/files.rs:1168` | `src/projects/files/state.rs` |
| `saved` | `src/projects/files.rs:1236` | `src/projects/files/state.rs` |
| `restore` | `src/projects/files.rs:1245` | `src/projects/files/state.rs` |
| `enable_memory` | `src/projects/files.rs:1385` | `src/projects/files/state.rs` |
| `memory` | `src/projects/files.rs:1395` | `src/projects/files/state.rs` |
| `archive_results` | `src/projects/files.rs:1964` | `src/projects/files/execution.rs` |
| `recorded_operation` | `src/projects/files.rs:1977` | `src/projects/files/execution.rs` |
| `operation_history` | `src/projects/files.rs:1991` | `src/projects/files/execution.rs` |
| `cached_result` | `src/projects/files.rs:2005` | `src/projects/files/execution.rs` |
| `remember_result` | `src/projects/files.rs:2037` | `src/projects/files/execution.rs` |
| `execute` | `src/projects/files.rs:2094` | `src/projects/files/execution.rs` |
| `execute_progress` | `src/projects/files.rs:2103` | `src/projects/files/execution.rs` |
| `set_chart_chooser` | `src/projects/files.rs:1651` | `src/projects/files/chart_work.rs` |
| `review_chart` | `src/projects/files.rs:1659` | `src/projects/files/chart_work.rs` |
| `add_chart` | `src/projects/files.rs:1680` | `src/projects/files/chart_work.rs` |
| `verify_chart_export` | `src/projects/files.rs:1699` | `src/projects/files/chart_work.rs` |
| `save_dataset` | `src/projects/files.rs:1727` | `src/projects/files/chart_work.rs` |
| `export_chart_png` | `src/projects/files.rs:1789` | `src/projects/files/chart_work.rs` |
| `perform` | `src/projects/files.rs:2392` | `src/projects/files/dispatch.rs` |
| `office_action` | `src/projects/files.rs:3833` | `src/projects/files/dispatch.rs` |
| `office_batch` | `src/projects/files.rs:3843` | `src/projects/files/dispatch.rs` |
| `finish` | `src/projects/files.rs:3886` | `src/projects/files/completion.rs` |
| `finish_cancellable` | `src/projects/files.rs:3890` | `src/projects/files/completion.rs` |
| `work_log_outcome_keeps_failure_before_large_request_pages` | `src/projects/files.rs:3991` | `src/projects/files/tests.rs` |
| `cad_operation_limit_reports_count_without_creating_output` | `src/projects/files.rs:4005` | `src/projects/files/tests.rs` |
| `dataset_checkpoint_reopens_and_rejects_modified_csv_without_touching_sources` | `src/projects/files.rs:4030` | `src/projects/files/tests.rs` |
| `archived_results_keep_operation_identity_and_survive_restore` | `src/projects/files.rs:4073` | `src/projects/files/tests.rs` |
| `new_drawing_context_keeps_failure_without_requiring_active_snapshot` | `src/projects/files.rs:4117` | `src/projects/files/tests.rs` |
| `new_design_hides_drawing_schema_until_saved_and_keeps_state_after_restore` | `src/projects/files.rs:4165` | `src/projects/files/tests.rs` |
| `old_broker_checkpoint_does_not_gain_a_new_drawing_gate` | `src/projects/files.rs:4229` | `src/projects/files/tests.rs` |
| `staged_cad_geometry_survives_broker_restore_without_becoming_a_drawing` | `src/projects/files.rs:4248` | `src/projects/files/tests.rs` |
| `same_task_handoff_before_loading_cad_exposes_drawing_without_another_handoff` | `src/projects/files.rs:4333` | `src/projects/files/tests.rs` |
| `cad_tools_follow_snapshot_and_active_document_lifetime` | `src/projects/files.rs:4363` | `src/projects/files/tests.rs` |
| `tools_require_existing_edit_and_export_targets` | `src/projects/files.rs:4425` | `src/projects/files/tests.rs` |
| `excel_binding_survives_broker_checkpoint_and_blocks_unplanned_export` | `src/projects/files.rs:4481` | `src/projects/files/tests.rs` |
| `png_exports_preserve_names_reuse_verified_files_and_survive_restore` | `src/projects/files.rs:4508` | `src/projects/files/tests.rs` |
| `reader_copy_allows_reader_write_access_without_unlocking_original` | `src/projects/files.rs:4576` | `src/projects/files/tests.rs` |
| `user_chart_save_reuses_conversation_folder_without_overwrite` | `src/projects/files.rs:4634` | `src/projects/files/tests.rs` |
| `names_collide_without_overwrite_and_directory_pins_still_block_changes` | `src/projects/files.rs:4664` | `src/projects/files/tests.rs` |
| `reject_windows_escape_and_device_paths` | `src/projects/files.rs:4767` | `src/projects/files/tests.rs` |
| `historical_diagram_export_keeps_its_original_source_version` | `src/projects/files.rs:4785` | `src/projects/files/tests.rs` |

## ui

來源提交：`aa3629676f41c29158bbf129be316e7cecf447b6`。

| 函式 | 原位置 | 新位置 |
| --- | --- | --- |
| `command` | `src/ui.rs:761` | `src/ui/commands.rs` |
| `event` | `src/ui.rs:1431` | `src/ui/events.rs` |
| `tick` | `src/ui.rs:1730` | `src/ui/events.rs` |
| `refresh_services` | `src/ui.rs:557` | `src/ui/session.rs` |
| `start_notifications` | `src/ui.rs:576` | `src/ui/session.rs` |
| `fetch_events` | `src/ui.rs:623` | `src/ui/session.rs` |
| `logout` | `src/ui.rs:647` | `src/ui/session.rs` |
| `begin_chat` | `src/ui.rs:685` | `src/ui/session.rs` |
| `read_mail` | `src/ui.rs:731` | `src/ui/session.rs` |
| `confirm_update` | `src/ui.rs:1811` | `src/ui/updates.rs` |
| `start_update_download` | `src/ui.rs:1832` | `src/ui/updates.rs` |
| `apply_ready_update` | `src/ui.rs:1860` | `src/ui/updates.rs` |
| `open_browser` | `src/ui.rs:1934` | `src/ui/bootstrap.rs` |
| `show_window` | `src/ui.rs:1964` | `src/ui/bootstrap.rs` |
| `app_icon` | `src/ui.rs:1983` | `src/ui/bootstrap.rs` |
| `taskbar_created_message` | `src/ui.rs:2008` | `src/ui/bootstrap.rs` |
| `tray` | `src/ui.rs:2013` | `src/ui/bootstrap.rs` |
| `window_proc` | `src/ui.rs:2030` | `src/ui/bootstrap.rs` |
| `drop` | `src/ui.rs:2262` | `src/ui/bootstrap.rs` |
| `run` | `src/ui.rs:2269` | `src/ui/bootstrap.rs` |
| `verify_panic_recovery` | `src/ui.rs:2274` | `src/ui/bootstrap.rs` |
| `run_tutorial_preview` | `src/ui.rs:2282` | `src/ui/bootstrap.rs` |
| `run_local` | `src/ui.rs:2287` | `src/ui/bootstrap.rs` |
| `run_local_scenario` | `src/ui.rs:2292` | `src/ui/bootstrap.rs` |
| `run_with_local` | `src/ui.rs:2297` | `src/ui/bootstrap.rs` |
| `apply_window_theme` | `src/ui.rs:2613` | `src/ui/bootstrap.rs` |
| `show_fatal_error` | `src/ui.rs:2636` | `src/ui/bootstrap.rs` |

## runner

來源提交：`74d80d1c71bd223742daa2f8d81be1636a15ba84`。

| 函式 | 原位置 | 新位置 |
| --- | --- | --- |
| `run_for` | `src/projects/runner.rs:266` | `src/projects/runner/execution.rs` |
| `report` | `src/projects/runner.rs:1603` | `src/projects/runner/execution.rs` |
| `recover_activity` | `src/projects/runner.rs:1614` | `src/projects/runner/recovery.rs` |
| `recover` | `src/projects/runner.rs:1646` | `src/projects/runner/recovery.rs` |
| `recover_charts` | `src/projects/runner.rs:1679` | `src/projects/runner/recovery.rs` |
| `recover_diagrams` | `src/projects/runner.rs:1698` | `src/projects/runner/recovery.rs` |
| `replay_safe` | `src/projects/runner.rs:1720` | `src/projects/runner/recovery.rs` |
| `invalidate_pause` | `src/projects/runner.rs:1757` | `src/projects/runner/recovery.rs` |
| `trim_journal` | `src/projects/runner.rs:1764` | `src/projects/runner/recovery.rs` |
| `recover_review` | `src/projects/runner.rs:1775` | `src/projects/runner/recovery.rs` |
| `recover_analysis` | `src/projects/runner.rs:1791` | `src/projects/runner/recovery.rs` |
| `recover_overview` | `src/projects/runner.rs:1803` | `src/projects/runner/recovery.rs` |
| `recover_local_time` | `src/projects/runner.rs:1815` | `src/projects/runner/recovery.rs` |
| `activity_keeps_early_entries_and_restores_more_than_one_page` | `src/projects/runner.rs:1833` | `src/projects/runner/pause_tests.rs` |
| `saved_design_and_precreation_error_are_visible_without_a_dwg_artifact` | `src/projects/runner.rs:1856` | `src/projects/runner/pause_tests.rs` |
| `segment_budget_is_twenty_four_hours_and_cancellation_stays_separate` | `src/projects/runner.rs:1873` | `src/projects/runner/pause_tests.rs` |
| `tool_and_model_quotas_pause_at_the_exact_boundary` | `src/projects/runner.rs:1879` | `src/projects/runner/pause_tests.rs` |
| `review_journal_restores_only_the_selected_encrypted_artifact` | `src/projects/runner.rs:1886` | `src/projects/runner/pause_tests.rs` |
| `recovery_never_replays_unknown_writes` | `src/projects/runner.rs:1918` | `src/projects/runner/pause_tests.rs` |
