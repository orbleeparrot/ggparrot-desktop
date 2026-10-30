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
//!
//! 웹 [실행기 열기]로 받은 매크로도 이 폴더에 파일로 남김([`save_macro`]) — 이름은 웹 내려받기와 같은 규칙
//! ([`macro_file_name`]). 같은 매크로 파일이 이미 있으면 새로 쓰지 않고 그 파일을 씀.

use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::macro_info::{rule_type, strategy_label, unsupported_reason};
use crate::types::{Exchange, exchange_label, exchange_name_of, macro_leverage, macro_side, macro_symbol};

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
    if ok { Ok(name) } else { Err("매크로 폴더에 있는 파일이 아니에요.".into()) }
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

/// (이름, 경로, 크기, 수정 시각).
type Found = (String, PathBuf, u64, i64);

/// 폴더 안 `.ggm.json` 목록 — 최근 수정 순. 파일을 열지 않음. 둘째 값은 매크로 형식 이름이 아닌 `.json` 수.
fn macro_files(dir: &Path) -> Result<(Vec<Found>, usize), String> {
    let rd = fs::read_dir(dir).map_err(|_| "매크로 폴더를 읽지 못했어요.".to_string())?;
    let mut skipped = 0;
    let mut found: Vec<Found> = vec![];
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
    found.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| a.0.cmp(&b.0)));
    Ok((found, skipped))
}

/// 폴더의 매크로 파일 목록(최근 수정 순). 폴더가 없으면 만듦.
pub fn scan(dir: &Path) -> Result<Scan, String> {
    scan_cached(dir, &mut LibraryCache::default())
}

/// 캐시를 쓰는 폴더 읽기 — 바뀐 파일만 다시 엶. 사라진 파일은 캐시에서 뺌.
pub fn scan_cached(dir: &Path, cache: &mut LibraryCache) -> Result<Scan, String> {
    fs::create_dir_all(dir)
        .map_err(|_| "매크로 폴더를 만들지 못했어요. 디스크 공간과 앱 데이터 폴더 권한을 확인하세요.".to_string())?;
    // 1) 이름·크기·수정 시각만 모음(파일을 열지 않음). 2) 최근 수정 순으로 MAX_LISTED개만 읽음.
    let (mut found, skipped) = macro_files(dir)?;
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
    if is_macro_file(&file_name)
        && let (Ok(a), Ok(b)) = (src.parent().map(fs::canonicalize).transpose(), fs::canonicalize(dir))
        && a.as_deref() == Some(b.as_path())
    {
        return Ok(file_name);
    }
    write_new(dir, &target_base, text.as_bytes())
}

const WRITE_FAILED: &str = "매크로 폴더에 파일을 쓰지 못했어요. 디스크 공간과 앱 데이터 폴더 권한을 확인하세요.";

/// 새 파일로만 씀 — 이미 있는 파일을 덮어쓰지 않음. 이름을 고른 사이 같은 이름이 생기면 다음 이름으로.
/// 쓰다 실패하면 방금 만든 빈 파일·반쪽 파일은 지움(목록에 깨진 파일로 남지 않게). 쓴 파일 이름을 돌려줌.
fn write_new(dir: &Path, wanted: &str, bytes: &[u8]) -> Result<String, String> {
    for _ in 0..5 {
        let target_name = unique_name(dir, wanted);
        let path = dir.join(&target_name);
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut out) => {
                if std::io::Write::write_all(&mut out, bytes).is_err() {
                    drop(out);
                    let _ = fs::remove_file(&path);
                    return Err(WRITE_FAILED.into());
                }
                return Ok(target_name);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => break,
        }
    }
    Err(WRITE_FAILED.into())
}

/// 웹 매크로 파일 이름의 전략 부분 — 웹(frontend/src/lib/macroFileName.js STRATEGY_SLUGS)과 같음.
/// 모르는 유형은 유형 글자를 소문자로(없으면 "custom").
fn strategy_slug(rule_type: &str) -> String {
    let slug = match rule_type.to_uppercase().as_str() {
        "A" => "tp-sl-reentry",
        "B" => "limit-band",
        "C" => "dca",
        "D" => "grid",
        "E" => "trailing-stop",
        "F" => "rsi",
        "G" => "bollinger",
        "H" => "martingale",
        "I" => "volatility-breakout",
        "J" => "ma-cross",
        "K" => "sar-defense",
        _ => return name_part(&rule_type.to_lowercase(), "custom"),
    };
    slug.into()
}

/// 이름 한 칸 — 영문·숫자·'-'만 남김(Windows가 못 쓰는 글자·공백·한글·경로 구분자 제거). 앞뒤 '-'는 뗌.
/// 비면 `fallback`. 이상하게 긴 값은 48자에서 자름(웹은 자르지 않지만 파일 이름 길이 제한을 넘지 않게).
fn name_part(value: &str, fallback: &str) -> String {
    let kept: String = value.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(48).collect();
    let trimmed = kept.trim_matches('-');
    if trimmed.is_empty() { fallback.into() } else { trimmed.into() }
}

/// JS `String(v)`에 가까운 글자 — 문자열·숫자만. 나머지는 빈 문자열.
fn text_of(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// JS의 참 값인지(`filter(Boolean)`) — null·false·0·빈 문자열이 아니면 참.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// 웹이 내려받기에 붙이는 이름과 같은 규칙(frontend/src/lib/macroFileName.js macroFileName):
/// `<거래소>_<종목>_<전략>_<방향>_<날짜(KST)>.ggm.json` — 예) `bithumb_KRW-BTR_rsi_long_20261013.ggm.json`.
/// - 거래소: binance·upbit·bithumb 중 하나(모르면 binance).
/// - 종목: `symbol`(없으면 `symbols[0]`), 여러 종목이면 `-plus<나머지 수>`. 비면 "macro".
/// - 방향: `position_side`가 "short"일 때만 short.
pub fn macro_file_name(m: &Value, now_ms: i64) -> String {
    let ex = text_of(m.get("exchange")).to_lowercase();
    let exchange = if matches!(ex.as_str(), "binance" | "upbit" | "bithumb") { ex.as_str() } else { "binance" };
    let symbols: Vec<&Value> = m
        .get("symbols")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter(|v| truthy(v)).collect())
        .unwrap_or_default();
    let first = m.get("symbol").filter(|v| truthy(v)).or(symbols.first().copied());
    let mut symbol = name_part(&text_of(first), "macro");
    if symbols.len() > 1 {
        symbol.push_str(&format!("-plus{}", symbols.len() - 1));
    }
    let strategy = strategy_slug(&text_of(m.get("rule_type")));
    let side = if m.get("position_side").and_then(Value::as_str) == Some("short") { "short" } else { "long" };
    let day = chrono::DateTime::from_timestamp_millis(now_ms + KST_OFFSET_MS)
        .map(|d| d.format("%Y%m%d").to_string())
        .unwrap_or_else(|| "00000000".into());
    format!("{exchange}_{symbol}_{strategy}_{side}_{day}{MACRO_EXT}")
}

const KST_OFFSET_MS: i64 = 9 * 3_600_000;

/// 매크로 내용이 같은지 볼 때 빼는 값 — 서명(`_sig`)과 웹이 붙이는 설명(`human_summary`).
/// 웹에서 내려받은 파일(서명·설명 포함)과 웹 연결로 받은 매크로(둘 다 없음)를 같은 매크로로 봄.
const META_KEYS: [&str; 2] = ["_sig", "human_summary"];

fn without_meta(m: &Value) -> Value {
    let mut v = m.clone();
    if let Some(o) = v.as_object_mut() {
        for k in META_KEYS {
            o.remove(k);
        }
    }
    v
}

/// 폴더에서 내용이 같은 매크로 파일(최근 수정 순으로 MAX_LISTED개까지만 봄). JSON 값으로 비교 — 키 순서·공백은 무시.
fn find_same(dir: &Path, m: &Value) -> Option<String> {
    let want = without_meta(m);
    let (mut found, _) = macro_files(dir).ok()?;
    found.truncate(MAX_LISTED);
    found
        .into_iter()
        .filter(|(_, _, len, _)| *len <= MAX_FILE_BYTES)
        .find(|(_, path, _, _)| {
            fs::read_to_string(path).ok().and_then(|t| parse_macro(&t).ok()).is_some_and(|v| without_meta(&v) == want)
        })
        .map(|(name, ..)| name)
}

/// [`save_macro`] 결과.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedMacro {
    pub file_name: String,
    /// 같은 매크로 파일이 이미 있어 새로 쓰지 않고 그 파일을 씀.
    pub reused: bool,
}

/// 매크로를 폴더에 `.ggm.json`으로 남김(웹 [실행기 열기]로 받은 매크로용).
/// - 파일 내용: 매크로 JSON 객체 그대로(웹 내려받기 파일과 같은 형식, 2칸 들여쓰기). [`read`]로 다시 읽히는 것만 씀.
/// - 같은 매크로 파일이 이미 있으면 새로 쓰지 않고 그 파일 이름을 돌려줌.
/// - 이미 있는 파일은 덮어쓰지 않음 — 같은 이름이면 `이름 (2).ggm.json`.
pub fn save_macro(dir: &Path, m: &Value, now_ms: i64) -> Result<SavedMacro, String> {
    let text = serde_json::to_string_pretty(m).map_err(|_| NOT_A_MACRO.to_string())?;
    parse_macro(&text)?;
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err("매크로가 너무 커서(256KB 초과) 매크로 폴더에 저장하지 않았어요.".into());
    }
    fs::create_dir_all(dir)
        .map_err(|_| "매크로 폴더를 만들지 못했어요. 디스크 공간과 앱 데이터 폴더 권한을 확인하세요.".to_string())?;
    if let Some(file_name) = find_same(dir, m) {
        return Ok(SavedMacro { file_name, reused: true });
    }
    let file_name = write_new(dir, &macro_file_name(m, now_ms), text.as_bytes())?;
    Ok(SavedMacro { file_name, reused: false })
}

#[cfg(test)]
mod tests;
