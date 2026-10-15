//! 매크로 폴더 — 앱 데이터 폴더의 `ggparrot-desktop/macros`에 있는 매크로 파일(`.ggm.json`)을 목록으로 읽고, 다른 곳의 파일을 복사해 넣음.
//!
//! - 목록에는 웹이 내려주는 형식(`<거래소>_<종목>_<전략>_<방향>_<날짜>.ggm.json`)인 `.ggm.json`만 올림.
//!   다른 `.json`은 세기만 함(가져오기로 넣으면 `.ggm.json`으로 바꿔 저장함).
//! - 파일 안도 봄: JSON 객체·종목·매크로 유형(A~K)·레버리지. 서버가 시작을 거절할 유형(적립식·SAR·
//!   여러 종목)은 목록에서 미리 알림.
//! - 성능: 크기·수정 시각이 그대로인 파일은 다시 읽지 않음(캐시). 최근 수정한 파일부터 300개까지만 읽고,
//!   한 파일은 256KB까지만 봄(웹이 주는 매크로는 몇 KB).
//!
//! 가져온 쪽 파일은 지우거나 바꾸지 않음. 같은 이름은 `이름 (2).ggm.json`으로 둠.

use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::macro_info::{rule_type, strategy_label, unsupported_reason};
use crate::types::{exchange_label, exchange_name_of, macro_leverage, macro_side, macro_symbol, Exchange};

/// 이보다 큰 파일은 매크로로 읽지 않음.
pub const MAX_FILE_BYTES: u64 = 256 * 1024;
/// 목록에 올리는 최대 파일 수(최근 수정 순).
pub const MAX_LISTED: usize = 300;
pub const MACRO_EXT: &str = ".ggm.json";
pub const NOT_A_MACRO: &str = "올바른 껄무새 매크로 파일이 아니에요.";

pub fn default_dir() -> PathBuf {
    crate::paths::macros_dir()
}

#[derive(Clone, Debug, PartialEq)]
pub struct LibraryEntry {
    pub file_name: String,
    pub valid: bool,
    pub error: String,
    pub title: String,
    pub exchange_name: String,
    pub exchange_label: String,
    pub symbol: String,
    pub side: String,
    /// 웹과 같은 전략 이름(예: "RSI 조건 매매"). 모르면 빈 문자열.
    pub strategy: String,
    /// 서버가 시작을 거절할 이유(지원하지 않는 유형·여러 종목·모르는 거래소). 돌릴 수 있으면 빈 문자열.
    pub unsupported: String,
    pub summary: String,
    pub signed: bool,
    pub modified_ms: i64,
}

/// 한 번의 폴더 읽기 결과.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    pub files: Vec<LibraryEntry>,
    /// 매크로 형식 이름이 아니라 목록에서 뺀 `.json` 파일 수.
    pub skipped: usize,
    /// 너무 많아 목록에 올리지 않은 `.ggm.json` 파일 수.
    pub truncated: usize,
}

/// (크기, 수정 시각)이 같으면 다시 읽지 않음.
#[derive(Default)]
pub struct LibraryCache(HashMap<String, (u64, i64, LibraryEntry)>);

fn lower(name: &str) -> String {
    name.to_lowercase()
}

/// 목록에 올리는 매크로 파일인지 — `.ggm.json`(숨김 파일 제외).
pub fn is_macro_file(name: &str) -> bool {
    !name.starts_with('.') && lower(name).ends_with(MACRO_EXT)
}

/// 가져올 수 있는 이름인지 — `.json`이면 받아서 안을 봄.
pub fn is_importable(name: &str) -> bool {
    !name.starts_with('.') && lower(name).ends_with(".json")
}

/// 파일 이름 하나만 받음(경로·상위 폴더 이동 금지).
fn safe_name(name: &str) -> Result<&str, String> {
    let ok = !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && name != "."
        && name != ".."
        && !name.contains('\0')
        && is_macro_file(name);
    if ok {
        Ok(name)
    } else {
        Err("매크로 폴더에 있는 파일이 아니에요.".into())
    }
}

/// 매크로로 읽히는지. 읽히면 매크로 원문, 아니면 사람이 읽을 이유.
/// 서버의 매크로 형식(gg_parrot engine/schema.py Macro)에서 꼭 있어야 하는 것만 봄.
pub fn parse_macro(text: &str) -> Result<Value, String> {
    let value: Value = serde_json::from_str(text).map_err(|_| NOT_A_MACRO.to_string())?;
    if !value.is_object() || macro_symbol(&value).is_empty() || rule_type(&value).is_none() {
        return Err(NOT_A_MACRO.into());
    }
    macro_leverage(&value).map_err(|_| NOT_A_MACRO.to_string())?;
    Ok(value)
}

fn title_of(file_name: &str) -> String {
    let l = lower(file_name);
    let cut = if l.ends_with(MACRO_EXT) {
        file_name.len() - MACRO_EXT.len()
    } else if l.ends_with(".json") {
        file_name.len() - 5
    } else {
        file_name.len()
    };
    file_name[..cut].to_string()
}

fn read_limited(path: &Path) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|_| "파일을 읽지 못했어요.".to_string())?;
    if meta.len() > MAX_FILE_BYTES {
        return Err("파일이 너무 커요(256KB 초과). 웹에서 받은 매크로 파일이 맞는지 확인하세요.".into());
    }
    fs::read_to_string(path).map_err(|_| "파일을 읽지 못했어요(UTF-8 JSON 파일이 아니에요).".to_string())
}

fn modified_ms(meta: &fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn entry(path: &Path, file_name: &str, modified_ms: i64) -> LibraryEntry {
    let mut e = LibraryEntry {
        file_name: file_name.to_string(),
        valid: false,
        error: String::new(),
        title: title_of(file_name),
        exchange_name: String::new(),
        exchange_label: String::new(),
        symbol: String::new(),
        side: "long".into(),
        strategy: String::new(),
        unsupported: String::new(),
        summary: String::new(),
        signed: false,
        modified_ms,
    };
    match read_limited(path).and_then(|t| parse_macro(&t)) {
        Ok(m) => {
            let name = exchange_name_of(&m);
            e.valid = true;
            e.exchange_label = exchange_label(&name);
            if Exchange::parse(&name).is_none() {
                e.unsupported = crate::types::unsupported_exchange_message(&name);
            }
            e.exchange_name = name;
            e.symbol = macro_symbol(&m);
            e.side = macro_side(&m).as_str().into();
            e.strategy = rule_type(&m).map(strategy_label).unwrap_or_default().to_string();
            if let Some(why) = unsupported_reason(&m) {
                e.unsupported = why.to_string();
            }
            e.summary = m.get("human_summary").and_then(Value::as_str).unwrap_or("").trim().to_string();
            e.signed = m.get("_sig").is_some_and(Value::is_object);
        }
        Err(reason) => e.error = reason,
    }
    e
}

/// 폴더의 매크로 파일 목록(최근 수정 순). 폴더가 없으면 만듦.
pub fn scan(dir: &Path) -> Result<Scan, String> {
    scan_cached(dir, &mut LibraryCache::default())
}

/// 캐시를 쓰는 폴더 읽기 — 바뀐 파일만 다시 엶. 사라진 파일은 캐시에서 뺌.
pub fn scan_cached(dir: &Path, cache: &mut LibraryCache) -> Result<Scan, String> {
    fs::create_dir_all(dir)
        .map_err(|_| "매크로 폴더를 만들지 못했어요. 디스크 공간과 앱 데이터 폴더 권한을 확인하세요.".to_string())?;
    let rd = fs::read_dir(dir).map_err(|_| "매크로 폴더를 읽지 못했어요.".to_string())?;
    let mut skipped = 0;
    // 1) 이름·크기·수정 시각만 모음(파일을 열지 않음).
    let mut found: Vec<(String, PathBuf, u64, i64)> = vec![];
    for item in rd.flatten() {
        let Ok(meta) = item.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let name = item.file_name().to_string_lossy().to_string();
        if is_macro_file(&name) {
            found.push((name, item.path(), meta.len(), modified_ms(&meta)));
        } else if is_importable(&name) {
            skipped += 1;
        }
    }
    // 2) 최근 수정 순으로 MAX_LISTED개만 읽음.
    found.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| a.0.cmp(&b.0)));
    let truncated = found.len().saturating_sub(MAX_LISTED);
    found.truncate(MAX_LISTED);
    let mut files = Vec::with_capacity(found.len());
    let mut fresh = HashMap::with_capacity(found.len());
    for (name, path, len, mtime) in found {
        let e = match cache.0.get(&name) {
            Some((l, m, e)) if *l == len && *m == mtime => e.clone(),
            _ => entry(&path, &name, mtime),
        };
        fresh.insert(name, (len, mtime, e.clone()));
        files.push(e);
    }
    cache.0 = fresh;
    Ok(Scan { files, skipped, truncated })
}

/// 폴더 안의 파일 하나를 매크로로 읽음.
pub fn read(dir: &Path, file_name: &str) -> Result<Value, String> {
    let name = safe_name(file_name)?;
    let path = dir.join(name);
    let text = read_limited(&path)?;
    parse_macro(&text)
}

/// 겹치지 않는 이름 — `이름.ggm.json`, `이름 (2).ggm.json`, …
fn unique_name(dir: &Path, wanted: &str) -> String {
    if !dir.join(wanted).exists() {
        return wanted.to_string();
    }
    let stem = &wanted[..wanted.len() - MACRO_EXT.len()];
    for n in 2..10_000 {
        let candidate = format!("{stem} ({n}){MACRO_EXT}");
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{stem} ({}){MACRO_EXT}", uuid::Uuid::new_v4().simple())
}

/// 다른 위치의 매크로 파일을 폴더로 복사함. 매크로로 읽히지 않으면 가져오지 않음. 가져온 파일 이름을 돌려줌.
/// `.json`은 `.ggm.json`으로 이름을 바꿔 저장함. 이미 폴더 안에 있는 `.ggm.json`이면 복사하지 않음.
pub fn import(dir: &Path, src: &Path) -> Result<String, String> {
    let file_name = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| is_importable(n))
        .ok_or_else(|| "'.json' 또는 '.ggm.json' 파일만 가져올 수 있어요.".to_string())?;
    let text = read_limited(src)?;
    parse_macro(&text)?;
    fs::create_dir_all(dir).map_err(|_| "매크로 폴더를 만들지 못했어요.".to_string())?;
    let target_base = format!("{}{MACRO_EXT}", title_of(&file_name));
    if is_macro_file(&file_name) {
        if let (Ok(a), Ok(b)) = (src.parent().map(fs::canonicalize).transpose(), fs::canonicalize(dir)) {
            if a.as_deref() == Some(b.as_path()) {
                return Ok(file_name);
            }
        }
    }
    // 새 파일로만 만듦 — 이미 있는 파일을 덮어쓰지 않음. 이름을 고른 사이 같은 이름이 생기면 다음 이름으로.
    for _ in 0..5 {
        let target_name = unique_name(dir, &target_base);
        match fs::OpenOptions::new().write(true).create_new(true).open(dir.join(&target_name)) {
            Ok(mut out) => {
                std::io::Write::write_all(&mut out, text.as_bytes())
                    .map_err(|_| "매크로 폴더에 파일을 쓰지 못했어요.".to_string())?;
                return Ok(target_name);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => break,
        }
    }
    Err("매크로 폴더에 파일을 쓰지 못했어요.".to_string())
}

#[cfg(test)]
mod tests;
