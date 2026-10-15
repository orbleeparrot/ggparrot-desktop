//! 웹뷰 명령. 이름·인자는 src/types.ts v2의 명령 목록과 같음.

use runner_core::bot::StopMode;
use runner_core::types::{Exchange, KeySlot, RunMode};
use serde_json::Value;
use std::path::PathBuf;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::shared::S;
use crate::state::{ActivityDto, StartReply};

type R<T> = Result<T, String>;

fn slot_of(slot: &str) -> R<KeySlot> {
    KeySlot::parse(slot).ok_or_else(|| "알 수 없는 키 칸이에요.".to_string())
}

// --- 실행 -------------------------------------------------------------------------

#[tauri::command]
pub fn get_state(s: State<'_, S>) -> Value {
    s.state_value()
}

#[tauri::command]
pub fn get_activity(s: State<'_, S>, run_id: String) -> Vec<ActivityDto> {
    s.activity_of(&run_id)
}

/// 다른 위치의 파일을 골라 매크로 폴더로 가져오고 슬롯을 만듦.
#[tauri::command]
pub async fn add_macro_file(s: State<'_, S>) -> R<Option<String>> {
    let picked =
        s.app.dialog().file().set_title("매크로 파일 선택").add_filter("껄무새 매크로", &["json"]).blocking_pick_file();
    let Some(file) = picked else { return Ok(None) };
    let path = file.into_path().map_err(|_| "매크로 파일을 읽지 못했어요.".to_string())?;
    let (ok, errors) = s.inner().import_paths(&[path]);
    match ok.first() {
        Some(name) => s.inner().add_run_from_library(name).map(Some),
        None => Err(errors.first().cloned().unwrap_or_else(|| "매크로 파일을 가져오지 못했어요.".into())),
    }
}

#[tauri::command]
pub fn add_run_from_library(s: State<'_, S>, file_name: String) -> R<String> {
    s.inner().add_run_from_library(&file_name)
}

#[tauri::command]
pub fn remove_run(s: State<'_, S>, run_id: String) -> R<()> {
    s.inner().remove_run(&run_id)
}

#[tauri::command]
pub fn set_run_mode(s: State<'_, S>, run_id: String, mode: String) {
    s.inner().set_run_mode(&run_id, RunMode::parse_or_mock(&mode));
}

#[tauri::command]
pub fn start_run(s: State<'_, S>, run_id: String) -> StartReply {
    s.inner().start_run(&run_id)
}

#[tauri::command]
pub fn start_run_confirmed(s: State<'_, S>, run_id: String) -> StartReply {
    s.inner().start_run_confirmed(&run_id)
}

#[tauri::command]
pub fn stop_run(s: State<'_, S>, run_id: String, mode: String) -> R<()> {
    let mode = StopMode::parse(&mode).ok_or("알 수 없는 종료 방식이에요.")?;
    s.inner().stop_run(&run_id, mode);
    Ok(())
}

#[tauri::command]
pub fn copy_activity(s: State<'_, S>, run_id: String) -> R<()> {
    if s.copy_activity(&run_id) {
        Ok(())
    } else {
        Err("활동 기록을 복사하지 못했어요.".into())
    }
}

// --- 설정 -------------------------------------------------------------------------

#[tauri::command]
pub fn set_slot_key(s: State<'_, S>, slot: String, field: String, value: String) -> R<()> {
    s.inner().set_slot_key(slot_of(&slot)?, &field, value);
    Ok(())
}

#[tauri::command]
pub fn reveal_slot_key(s: State<'_, S>, slot: String, field: String) -> R<String> {
    Ok(s.reveal_slot_key(slot_of(&slot)?, &field))
}

#[tauri::command]
pub fn set_slot_remember(s: State<'_, S>, slot: String, remember: bool) -> R<()> {
    s.inner().set_slot_remember(slot_of(&slot)?, remember);
    Ok(())
}

#[tauri::command]
pub fn check_slot(s: State<'_, S>, slot: String) -> R<()> {
    s.inner().begin_slot_check(slot_of(&slot)?, true);
    Ok(())
}

#[tauri::command]
pub fn set_member_key(s: State<'_, S>, value: String) {
    s.inner().set_member_key(value);
}

/// 웹 연결이 가져온 다른 계정의 회원 키를 쓸지 정함.
#[tauri::command]
pub fn resolve_member_key(s: State<'_, S>, accept: bool) {
    s.inner().resolve_member_key(accept);
}

#[tauri::command]
pub fn reveal_member_key(s: State<'_, S>) -> String {
    s.reveal_member_key()
}

#[tauri::command]
pub fn set_close_to_tray(s: State<'_, S>, value: bool) {
    s.inner().set_close_to_tray(value);
}

#[tauri::command]
pub fn set_show_dev_log(s: State<'_, S>, show: bool) {
    s.inner().set_show_dev_log(show);
}

/// 투자 금액 바꾸기·되돌리기(amount가 null이면 매크로에 적힌 금액으로).
#[tauri::command]
pub fn set_run_capital(s: State<'_, S>, run_id: String, amount: Option<f64>) -> R<()> {
    s.inner().set_run_capital(&run_id, amount)
}

#[tauri::command]
pub fn reload_storage(s: State<'_, S>) -> R<()> {
    s.inner().reload_storage()
}

#[tauri::command]
pub fn recover_storage(s: State<'_, S>) -> R<()> {
    s.inner().recover_storage()
}

#[tauri::command]
pub fn update_check(s: State<'_, S>) {
    s.inner().update_check(false);
}

#[tauri::command]
pub fn update_install(s: State<'_, S>) -> R<()> {
    s.inner().update_install()
}

#[tauri::command]
pub fn update_restart(s: State<'_, S>) -> R<()> {
    s.inner().update_restart()
}

// --- 매크로 폴더 ---------------------------------------------------------------------

#[tauri::command]
pub fn library_refresh(s: State<'_, S>) -> R<()> {
    s.inner().refresh_library()
}

#[tauri::command]
pub fn library_open_folder(s: State<'_, S>) -> R<()> {
    s.inner().open_library_folder()
}

#[tauri::command]
pub fn open_web(s: State<'_, S>) -> R<()> {
    s.inner().open_web()
}

#[tauri::command]
pub async fn library_import(s: State<'_, S>) -> R<usize> {
    let picked = s
        .app
        .dialog()
        .file()
        .set_title("매크로 폴더로 가져올 파일")
        .add_filter("껄무새 매크로", &["json"])
        .blocking_pick_files();
    let Some(files) = picked else { return Ok(0) };
    let paths: Vec<PathBuf> = files.into_iter().filter_map(|f| f.into_path().ok()).collect();
    let (ok, errors) = s.inner().import_paths(&paths);
    if !errors.is_empty() {
        s.notice("warn", format!("가져오지 못한 파일이 있어요.\n\n{}", errors.join("\n")));
    }
    Ok(ok.len())
}

// --- 연결 도우미 ---------------------------------------------------------------------

#[tauri::command]
pub fn wizard_open(s: State<'_, S>, exchange: String, stage: Option<String>) -> R<()> {
    let ex = Exchange::parse(&exchange).ok_or("알 수 없는 거래소예요.")?;
    s.inner().wizard_open(KeySlot::for_run(ex, RunMode::Live), stage.as_deref().unwrap_or("ask"));
    Ok(())
}

#[tauri::command]
pub fn wizard_choose(s: State<'_, S>, path: String) {
    s.inner().wizard_choose(&path);
}

#[tauri::command]
pub fn wizard_go(s: State<'_, S>, stage: String) {
    s.inner().wizard_go(&stage);
}

#[tauri::command]
pub fn wizard_back(s: State<'_, S>) {
    s.inner().wizard_back();
}

#[tauri::command]
pub fn wizard_close(s: State<'_, S>) {
    s.inner().wizard_close();
}

#[tauri::command]
pub fn wizard_refresh_ip(s: State<'_, S>) {
    s.inner().begin_public_ip(true);
}

#[tauri::command]
pub fn wizard_copy_ip(s: State<'_, S>) {
    s.inner().wizard_copy_ip();
}

#[tauri::command]
pub fn wizard_copy_and_open(s: State<'_, S>) {
    s.inner().wizard_copy_and_open();
}

#[tauri::command]
pub fn wizard_open_api_page(s: State<'_, S>) {
    s.inner().wizard_open_page(true);
}

#[tauri::command]
pub fn wizard_open_home(s: State<'_, S>) {
    s.inner().wizard_open_page(false);
}

#[tauri::command]
pub fn wizard_recheck(s: State<'_, S>) {
    s.inner().wizard_start_check();
}

#[tauri::command]
pub fn wizard_recopy(s: State<'_, S>) {
    s.inner().wizard_recopy();
}

#[tauri::command]
pub fn wizard_set_key(s: State<'_, S>, field: String, value: String) {
    if field == "access" || field == "secret" {
        s.inner().wizard_set_key(&field, value);
    }
}

#[tauri::command]
pub fn wizard_set_remember(s: State<'_, S>, remember: bool) {
    s.inner().wizard_set_remember(remember);
}
