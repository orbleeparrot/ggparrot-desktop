//! 앱 기록 — 매크로 하나에 묶이지 않는 앱 전체의 일(웹 연결·업데이트·키체인·저장·매크로 폴더 등).
//! - 알림(토스트)은 모두 여기에도 남음([`crate::shared::Shared::notice_from`]) — 토스트가 사라져도 다시 볼 수 있게.
//! - 메모리에만 둠(최근 [`APP_LOG_LIMIT`]줄). 앱을 끄면 사라짐.
//! - 넣기 전에 키 값·인증 값을 가림([`Core::scrub`]) — 복사·문의용 정보에 그대로 나가므로.

use super::{ACTIVITY_DETAIL_CHARS, ACTIVITY_TITLE_CHARS, Core, clip_chars, now_ms};
use serde::Serialize;

/// 앱 기록 줄 수 한도 — 넘으면 오래된 줄부터 뺌.
pub const APP_LOG_LIMIT: usize = 500;
/// 문의용 정보에 넣는 최근 앱 기록 줄 수.
pub const SUPPORT_APP_LOG_LINES: usize = 20;
/// 문의용 정보의 앱 기록 한 줄 글자 수 한도.
const SUPPORT_LINE_CHARS: usize = 300;

/// 앱 기록 한 줄(src/types.ts `AppLogEntry`).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppLogEntry {
    pub id: u64,
    /// epoch ms
    pub at: i64,
    /// "info" | "warn" | "error"
    pub level: &'static str,
    /// 짧은 출처 표시 — "app"·"web"·"update"·"keys"·"storage"·"library"·"server"·"run"·"wizard".
    pub source: &'static str,
    pub title: String,
    pub detail: String,
}

/// 알림 수준을 앱 기록 수준으로 — 모르는 값은 info.
pub fn log_level(level: &str) -> &'static str {
    match level {
        "error" | "danger" => "error",
        "warn" | "warning" => "warn",
        _ => "info",
    }
}

/// 알림 글을 (제목, 설명)으로 나눔 — 첫 줄이 제목, 나머지(앞뒤 빈 줄 제외)가 설명.
pub fn split_message(message: &str) -> (String, String) {
    let message = message.trim();
    match message.split_once('\n') {
        Some((head, rest)) => (head.trim().to_string(), rest.trim().to_string()),
        None => (message.to_string(), String::new()),
    }
}

fn level_label(level: &str) -> &'static str {
    match level {
        "error" => "오류",
        "warn" => "주의",
        _ => "알림",
    }
}

/// 복사·문의용 한 줄 — "2026-10-16 14:03:22  주의 [web] 제목 — 설명". 설명의 줄바꿈은 " / "로 이음.
pub fn app_log_line(e: &AppLogEntry) -> String {
    let t = super::copy_line_time(e.at);
    let mut line = format!("{t}  {} [{}] {}", level_label(e.level), e.source, e.title);
    let detail: Vec<&str> = e.detail.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if !detail.is_empty() {
        line.push_str(" — ");
        line.push_str(&detail.join(" / "));
    }
    line
}

/// [앱 기록 복사] 글 — 머리말 + 줄마다 날짜·시각. 한도를 넘으면 최근 줄만([`super::bounded_copy_text`]).
pub fn app_log_copy_text<'a>(head: &str, items: impl IntoIterator<Item = &'a AppLogEntry>, max_chars: usize) -> String {
    super::bounded_copy_text(head, items.into_iter().map(app_log_line).collect(), max_chars)
}

impl Core {
    /// 앱 기록에 한 줄 넣고 그 번호를 돌려줌. 화면에는 다음 emit이 보냄(`pending_app_log`).
    /// 제목·설명은 키 값을 가리고 활동 기록과 같은 글자 수로 자름.
    pub fn push_app_log(&mut self, level: &str, source: &'static str, title: &str, detail: &str) -> u64 {
        self.app_log_seq += 1;
        let entry = AppLogEntry {
            id: self.app_log_seq,
            at: now_ms(),
            level: log_level(level),
            source,
            title: clip_chars(&self.scrub(title), ACTIVITY_TITLE_CHARS),
            detail: clip_chars(&self.scrub(detail), ACTIVITY_DETAIL_CHARS),
        };
        self.app_log.push_back(entry.clone());
        while self.app_log.len() > APP_LOG_LIMIT {
            self.app_log.pop_front();
        }
        self.pending_app_log.push(entry);
        self.app_log_seq
    }

    /// 알림 글 하나를 앱 기록으로 — 첫 줄은 제목, 나머지는 설명.
    pub fn push_notice_log(&mut self, level: &str, source: &'static str, message: &str) -> u64 {
        let (title, detail) = split_message(message);
        self.push_app_log(level, source, &title, &detail)
    }

    /// 매크로 폴더를 읽은 뒤 — 폴더를 읽지 못했거나 새로 읽지 못한 파일이 생겼으면 한 줄 남김.
    /// 같은 문제는 거듭 남기지 않음(폴더는 자주 다시 읽음). 고쳐졌다가 다시 생기면 다시 남김.
    pub fn log_library_problems(&mut self, scan_error: Option<&str>) {
        if let Some(e) = scan_error {
            if self.library_logged.0 != e {
                self.library_logged.0 = e.to_string();
                self.push_app_log("warn", "library", "매크로 폴더를 읽지 못했어요", e);
            }
            return;
        }
        self.library_logged.0.clear();
        let broken: Vec<(String, String)> =
            self.library.iter().filter(|f| !f.valid).map(|f| (f.file_name.clone(), f.error.clone())).collect();
        let fresh: Vec<String> = broken
            .iter()
            .filter(|(name, _)| !self.library_logged.1.contains(name))
            .map(|(name, why)| format!("{name}: {why}"))
            .collect();
        self.library_logged.1 = broken.into_iter().map(|(name, _)| name).collect();
        if !fresh.is_empty() {
            let title = format!("매크로 폴더에서 읽지 못한 파일이 있어요 ({}개)", fresh.len());
            self.push_app_log("warn", "library", &title, &fresh.join("\n"));
        }
    }

    /// 앱 기록 전체(오래된 것 → 최근).
    pub fn app_log_list(&self) -> Vec<AppLogEntry> {
        self.app_log.iter().cloned().collect()
    }

    /// 문의용 정보에 넣을 최근 앱 기록 줄(오래된 것 → 최근, 줄마다 글자 수 한도).
    pub fn recent_app_log_lines(&self, n: usize) -> Vec<String> {
        let skip = self.app_log.len().saturating_sub(n);
        self.app_log.iter().skip(skip).map(|e| clip_chars(&app_log_line(e), SUPPORT_LINE_CHARS)).collect()
    }

    /// 글 안의 키 값(회원 키·거래소 키·시크릿·기억한 키·받아 둔 회원 키)과 인증 값(Bearer·토큰·URL 쿼리)을 가림.
    pub fn scrub(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        let mut values: Vec<&str> = vec![self.member_key.as_str()];
        values.extend(self.pending_member_key.as_deref());
        let pairs =
            self.slots.values().map(|s| &s.keys).chain(self.remembered.iter().flat_map(|r| r.exchanges.values()));
        for p in pairs {
            values.push(&p.api_key);
            values.push(&p.api_secret);
        }
        if let Some(r) = &self.remembered {
            values.push(&r.member_key);
        }
        runner_core::outbox::redact(&mask_values(text, values), "")
    }
}

/// 짧은 값은 가리지 않음 — 잘못 넣은 한두 글자 키가 기록 글 전체를 망가뜨리지 않게(실제 키는 훨씬 긺).
const MIN_MASK_CHARS: usize = 8;

/// 값들을 "[키 숨김]"으로 바꿈. 긴 값부터(짧은 값이 긴 값의 일부만 가리지 않게).
fn mask_values<'a>(text: &str, values: impl IntoIterator<Item = &'a str>) -> String {
    let mut values: Vec<&str> =
        values.into_iter().map(str::trim).filter(|v| v.chars().count() >= MIN_MASK_CHARS).collect();
    values.sort_by_key(|v| std::cmp::Reverse(v.chars().count()));
    values.dedup();
    let mut text = text.to_string();
    for v in values {
        text = text.replace(v, "[키 숨김]");
    }
    text
}
