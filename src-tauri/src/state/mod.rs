//! 앱 상태(Core)와 웹뷰로 보내는 스냅샷(src/types.ts v2와 1:1).
//! 실행 슬롯(Run) 여러 개 + 설정(키 칸·회원 키) + 매크로 폴더 + 업데이트 + 연결 도우미.

use runner_core::bot::{BotHandle, RunHealth, StopMode};
use runner_core::connection::{binance_env, check_view, Check, CheckNotes, CheckView, PreflightResult, PublicIpCache};
use runner_core::credentials::{LoadStatus, KEY_STORE_NAME};
use runner_core::library::LibraryEntry;
use runner_core::money::pnl_amount;
use runner_core::setup_progress::SetupProgressReporter;
use runner_core::types::{
    base_asset, exchange_label, exchange_name_of, macro_leverage, macro_side, macro_symbol, market_of, quote_of,
    Credentials, Exchange, KeyPair, KeySlot, Market, RunMode, Side,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

pub const ACTIVITY_LIMIT: usize = 1000;
/// 활동 한 줄의 글자 수 한도(제목·설명·원문) — 거래소 오류 본문 같은 긴 글이 통째로 쌓이지 않게.
pub const ACTIVITY_TITLE_CHARS: usize = 200;
pub const ACTIVITY_DETAIL_CHARS: usize = 1000;
pub const ACTIVITY_RAW_CHARS: usize = 2000;
/// [전체 기록 복사] 한도(글자) — 문의 게시판·메신저 한 번에 붙여 넣을 수 있는 크기. 넘으면 최근 기록만.
pub const ACTIVITY_COPY_CHARS: usize = 60_000;

/// 글자 수로 자르고 잘랐으면 '…'를 붙임(UTF-8 경계 안전).
pub fn clip_chars(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

/// 활동 기록 복사 글 — 맨 위 머리말, 줄마다 날짜·시각. 한도를 넘으면 오래된 줄부터 빼고 뺀 수를 적음.
pub fn activity_copy_text(head: &str, items: &[ActivityDto], max_chars: usize) -> String {
    let lines: Vec<String> = items
        .iter()
        .map(|a| {
            let t = chrono::DateTime::from_timestamp_millis(a.at)
                .map(|d| d.with_timezone(&chrono::Local).format("%m-%d %H:%M:%S").to_string())
                .unwrap_or_default();
            let mut line = format!("{t}  {}", a.title);
            if !a.detail.is_empty() {
                line.push_str(&format!(" — {}", a.detail));
            }
            if !a.raw.is_empty() && a.raw != a.title {
                line.push_str(&format!("\n                {}", a.raw.trim()));
            }
            line
        })
        .collect();
    // 최근 줄부터 한도 안에서 담음.
    let mut used = head.chars().count() + 40;
    let mut keep = 0;
    for l in lines.iter().rev() {
        let n = l.chars().count() + 1;
        if used + n > max_chars {
            break;
        }
        used += n;
        keep += 1;
    }
    let skipped = lines.len() - keep;
    let mut out = vec![head.to_string()];
    if skipped > 0 {
        out.push(format!("(앞의 {skipped}줄은 길어서 뺐어요 — 최근 {keep}줄)"));
    }
    out.extend(lines.into_iter().skip(skipped));
    out.join("\n")
}
pub const WATCH_STAGES: [&str; 3] = ["perm", "keys", "existing"];

#[derive(Clone)]
pub struct KeyCheckRecord {
    pub fingerprint: String,
    pub reason: String,
    pub view: CheckView,
    /// 확인한 시각(ms). 오래된 '연결됨'은 지금도 맞다는 보장이 없어 따로 알림.
    pub checked_at: i64,
}

/// 키 칸의 '연결됨'을 이만큼 지나면 오래된 결과라고 덧붙임(시작할 때는 늘 다시 확인함).
pub const KEY_CHECK_STALE_MS: i64 = 30 * 60_000;

/// 설정의 키 칸 하나.
#[derive(Default)]
pub struct SlotState {
    pub keys: KeyPair,
    pub remember: bool,
    pub dirty: bool,
    pub check: Option<KeyCheckRecord>,
    pub busy_fp: Option<String>,
    pub scheduled: bool,
    pub timer: u64,
    pub generation: u64,
}

/// 연결 도우미가 안내할 곳 — 키 칸, 바이낸스 사이트(testnet·demo·live, 국내는 ""), 시장.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WizardTarget {
    pub slot: KeySlot,
    pub env: &'static str,
    pub market: Market,
}

pub struct Wizard {
    pub slot: KeySlot,
    /// 바이낸스 사이트(testnet·demo·live). 국내는 "".
    pub env: &'static str,
    pub market: Market,
    pub stage: String,
    pub path: String,
    pub got_access: bool,
    pub got_secret: bool,
    pub advance_on_return: String,
    pub clip_seen: String,
    pub generation: u64,
}

#[derive(Clone, Default)]
pub struct RunData {
    pub started_at: i64,
    pub session_id: Option<i64>,
    pub online: bool,
    /// 연결·시세·정리 상태(봇이 매 틱 보냄).
    pub health: RunHealth,
    pub snapshot: Value,
    /// 마지막 상태 스냅샷을 받은 시각(ms). 0이면 아직 못 받음(시작 직후).
    pub updated_at: i64,
    pub finished: Option<(String, String, i64)>,
    /// 서버가 시작 응답에 준 매크로 출처 글자(macro_origin_label). 없으면 "".
    pub macro_origin: String,
}

/// 지난 실행이 앱이 꺼지며 끝맺지 못했을 때 남기는 종료 이유.
pub const INTERRUPTED_NOTE: &str = "지난 실행이 정상적으로 끝나지 않았어요(앱 강제 종료 · 전원 꺼짐 등)";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDto {
    pub id: u64,
    pub run_id: String,
    pub at: i64,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub raw: String,
    pub dev: bool,
    pub tone: String,
}

/// 실행 슬롯 하나 = 매크로 하나 + 모드 하나 + 서버 세션 하나 + 봇 하나.
pub struct Run {
    pub id: String,
    pub macro_: Value,
    pub title: String,
    pub source: String,
    pub sig: Option<Value>,
    pub modified: bool,
    /// 사용자가 투자 금액을 바꿨다면 바꾸기 전(원본) 금액 — 되돌리기·'수정됨' 표시에 씀.
    pub original_capital: Option<f64>,
    pub user_macro_id: Option<i64>,
    pub library_file: Option<String>,
    pub web_linked: bool,
    pub server_base: String,
    pub mode: RunMode,
    pub preflight: Option<PreflightResult>,
    pub generation: u64,
    pub checking: bool,
    pub start_pending: bool,
    /// 시작하는 동안 누른 종료 — 세션이 만들어지면 주문 없이 끝냄.
    pub cancel_requested: Option<StopMode>,
    pub bot: Option<BotHandle>,
    pub stopping: bool,
    /// 실전 시작 확인을 띄웠고 아직 답을 받지 않음 — 확인 명령은 이 표시를 써야만 시작함(화면이 오염돼도
    /// 확인 창 없이 실전을 시작하지 못하게).
    pub live_confirm_issued: bool,
    pub data: Option<RunData>,
    pub activity: Vec<ActivityDto>,
    pub reporter: Option<Arc<SetupProgressReporter>>,
}

/// runs.json 한 줄 — 키는 들어가지 않음.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SavedRun {
    pub id: String,
    pub title: String,
    pub source: String,
    #[serde(rename = "macro")]
    pub macro_: Value,
    #[serde(default)]
    pub sig: Option<Value>,
    #[serde(default)]
    pub user_macro_id: Option<i64>,
    #[serde(default)]
    pub library_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_capital: Option<f64>,
    #[serde(default)]
    pub web_linked: bool,
    #[serde(default)]
    pub server_base: String,
    pub mode: RunMode,
    /// 실행 중에 저장됐으면 시작 시각. 다음에 켰을 때 남아 있으면 앱이 실행 도중 꺼진 것임.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_since: Option<i64>,
}

impl Run {
    pub fn new(id: String, macro_: Value, title: String, source: &str, server_base: String) -> Run {
        Run {
            id,
            macro_,
            title,
            source: source.into(),
            sig: None,
            modified: false,
            user_macro_id: None,
            library_file: None,
            original_capital: None,
            web_linked: false,
            server_base,
            mode: RunMode::Mock,
            preflight: None,
            generation: 0,
            checking: false,
            start_pending: false,
            cancel_requested: None,
            bot: None,
            stopping: false,
            live_confirm_issued: false,
            data: None,
            activity: vec![],
            reporter: None,
        }
    }

    pub fn from_saved(s: SavedRun, default_base: &str) -> Run {
        // 배포판은 저장된 서버 주소를 믿지 않음 — 파일이 바뀌었거나 개발 빌드가 남긴 로컬 주소로 회원 키가 가지 않게.
        let base =
            if s.server_base.is_empty() || !cfg!(debug_assertions) { default_base.to_string() } else { s.server_base };
        let mut r = Run::new(s.id, s.macro_, s.title, &s.source, base);
        r.sig = s.sig;
        r.user_macro_id = s.user_macro_id;
        r.library_file = s.library_file;
        r.original_capital = s.original_capital;
        r.web_linked = s.web_linked;
        r.mode = s.mode;
        if r.mode == RunMode::Testnet && !r.exchange().is_some_and(Exchange::has_testnet) {
            r.mode = RunMode::Mock;
        }
        if let Some(since) = s.active_since {
            // 실행 중에 앱이 꺼짐 — 포지션·서버 세션이 남았을 수 있으니 '확인 필요'로 보임.
            let mut d = RunData::started(since, None);
            d.finished = Some(("error".into(), INTERRUPTED_NOTE.into(), now_ms()));
            r.data = Some(d);
        }
        r
    }

    pub fn saved(&self) -> SavedRun {
        SavedRun {
            id: self.id.clone(),
            title: self.title.clone(),
            source: self.source.clone(),
            macro_: self.macro_.clone(),
            sig: self.sig.clone(),
            user_macro_id: self.user_macro_id,
            library_file: self.library_file.clone(),
            original_capital: self.original_capital,
            web_linked: self.web_linked,
            server_base: self.server_base.clone(),
            mode: self.mode,
            active_since: self.running().then(|| self.data.as_ref().map_or(0, |d| d.started_at)),
        }
    }

    pub fn exchange_name(&self) -> String {
        exchange_name_of(&self.macro_)
    }
    pub fn exchange(&self) -> Option<Exchange> {
        Exchange::parse(&self.exchange_name())
    }
    pub fn symbol(&self) -> String {
        macro_symbol(&self.macro_)
    }
    pub fn running(&self) -> bool {
        self.bot.is_some()
    }
    pub fn key_slot(&self) -> Option<KeySlot> {
        self.exchange().map(|e| KeySlot::for_run(e, self.mode, self.market()))
    }
    /// 거래소의 종목 화면(상세 머리줄의 '○○에서 보기'). 모르는 거래소·종목이면 None.
    pub fn market_page(&self) -> Option<runner_core::market_page::MarketPage> {
        runner_core::market_page::market_page(self.exchange()?, &self.symbol(), self.market(), self.mode)
    }
    /// 껄무새 웹의 이 실행 기록 화면(`/agents?session=…`). 세션 번호가 있을 때만.
    /// 기본 서버에서 만든 세션만 — 다른 서버(개발용 등)의 세션은 껄무새 웹에 없음.
    pub fn web_run_url(&self) -> Option<String> {
        let sid = self.data.as_ref()?.session_id.filter(|s| *s > 0)?;
        if self.server_base.trim_end_matches('/') != runner_core::types::DEFAULT_SERVER_BASE {
            return None;
        }
        Some(format!("{}agents?session={sid}", crate::shared::WEB_URL))
    }
    /// 웹 가이드 보고를 떼어 내고 마지막 상태('closed')를 만듦 — 앱 종료와 목록에서 빼기가 함께 씀.
    /// 상태를 만들지 못하면(모르는 거래소) 보고만 닫으라고 None을 함께 돌려줌.
    pub fn take_closing_report(&mut self) -> Option<(Arc<SetupProgressReporter>, Option<Value>)> {
        let rep = self.reporter.take()?;
        let payload = runner_core::setup_progress::progress_payload(
            "closed",
            &self.exchange_name(),
            self.binance_env(),
            "",
            (false, false),
            None,
            "",
            false,
        )
        .ok();
        Some((rep, payload))
    }
    pub fn market(&self) -> Market {
        market_of(&self.exchange_name(), macro_side(&self.macro_), macro_leverage(&self.macro_).unwrap_or(1))
    }
    /// 바이낸스 연결 도우미가 안내할 사이트(모드·시장으로). 국내·모르는 거래소는 "".
    pub fn binance_env(&self) -> &'static str {
        match self.exchange() {
            Some(Exchange::Binance) => binance_env(self.mode, self.market()),
            _ => "",
        }
    }
    /// 이 실행으로 연 연결 도우미의 대상 — 실행이 쓰는 칸(`key_slot`)과 그 칸의 사이트.
    /// 모의는 키 없이도 돌아 데려갈 칸이 없음(None).
    pub fn wizard_target(&self) -> Option<WizardTarget> {
        if self.mode == RunMode::Mock {
            return None;
        }
        Some(self.target_of(self.key_slot()?))
    }
    /// 웹 가이드에 알릴 대상. 실전·연습은 `wizard_target`과 같음.
    /// 모의는 파이썬 실행기처럼 연습 사이트(`binance_env`)의 칸으로 알림 — 가이드가 연습 키 만들기부터 따라 그림.
    /// 모의 실행이 키를 쓸 때의 칸(`key_slot`, 실거래 칸)과는 다름 — 가이드 표시용일 뿐임.
    pub fn guide_target(&self) -> Option<WizardTarget> {
        if self.mode != RunMode::Mock {
            return self.wizard_target();
        }
        Some(self.target_of(KeySlot::for_run(self.exchange()?, RunMode::Testnet, self.market())))
    }
    fn target_of(&self, slot: KeySlot) -> WizardTarget {
        WizardTarget { slot, env: slot.binance_env(), market: slot.fixed_market().unwrap_or(self.market()) }
    }
    pub fn mode_choices(&self) -> Vec<RunMode> {
        match self.exchange() {
            Some(e) if !e.has_testnet() => vec![RunMode::Mock, RunMode::Live],
            _ => vec![RunMode::Mock, RunMode::Testnet, RunMode::Live],
        }
    }
}

/// 키를 알아볼 수 있게 끝 네 글자만 — "…abcd".
pub fn key_hint(key: &str) -> String {
    let k: Vec<char> = key.trim().chars().collect();
    let tail: String = k[k.len().saturating_sub(4)..].iter().collect();
    format!("…{tail}")
}

pub struct UpdateData {
    pub status: &'static str,
    pub version: String,
    pub notes: String,
    pub progress: f64,
    /// 내려받은 바이트·전체 바이트(모르면 0).
    pub downloaded: u64,
    pub total: u64,
    pub error: String,
    pub checked_at: i64,
    /// 이 위치에서는 설치할 수 없는 이유(macOS 디스크 이미지·격리 위치에서 실행 중 등). 비면 설치 가능.
    pub blocked: String,
    /// 방금 새 버전으로 바뀌었으면 그 전 버전(한 번 알리고 비움).
    pub updated_from: String,
}

pub struct Core {
    pub runs: Vec<Run>,
    pub slots: BTreeMap<KeySlot, SlotState>,
    pub member_key: String,
    /// 웹 연결이 가져온, 지금과 다른 계정의 회원 키 — 사용자가 바꾸기로 할 때까지 쓰지 않음.
    pub pending_member_key: Option<String>,
    pub remembered: Option<Credentials>,
    pub load_status: LoadStatus,
    pub remember_supported: bool,
    pub storage_error: String,
    pub show_dev_log: bool,
    /// 창을 닫으면 트레이로 숨기고 계속 실행함(기본 켬).
    pub close_to_tray: bool,
    pub hidden_to_tray: bool,
    pub server_base: String,
    pub claim_busy: bool,
    pub closed: bool,
    pub ip_cache: PublicIpCache,
    pub wizard: Option<Wizard>,
    pub wizard_generation: u64,
    pub app_away: bool,
    pub library_dir: PathBuf,
    pub library: Vec<LibraryEntry>,
    /// 매크로 형식 이름(.ggm.json)이 아니라 목록에서 뺀 .json 수·너무 많아 올리지 않은 수.
    pub library_skipped: usize,
    pub library_truncated: usize,
    /// 매크로 폴더 목록이 바뀔 때마다 오름(폴더 읽기·실행 목록 추가·빼기 — '실행 목록에 N개' 표시).
    /// 화면에는 바뀐 판만 보냄(목록이 상태의 대부분이라 매 틱 보내면 무거움).
    pub library_rev: u64,
    pub update: UpdateData,
    pub activity_seq: u64,
    /// 아직 화면에 보내지 않은 활동 — emit이 함께 보냄.
    pub pending_activity: Vec<ActivityDto>,
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl Core {
    pub fn new(server_base: String, remember_supported: bool, library_dir: PathBuf) -> Self {
        let mut slots = BTreeMap::new();
        for s in KeySlot::ALL {
            slots.insert(s, SlotState::default());
        }
        Core {
            runs: vec![],
            slots,
            member_key: runner_core::types::dev_env("GGP_MEMBER_KEY").unwrap_or_default(),
            pending_member_key: None,
            remembered: None,
            load_status: LoadStatus::Missing,
            remember_supported,
            storage_error: String::new(),
            show_dev_log: false,
            close_to_tray: true,
            hidden_to_tray: false,
            server_base,
            claim_busy: false,
            closed: false,
            ip_cache: PublicIpCache::new(),
            wizard: None,
            wizard_generation: 0,
            app_away: false,
            library_dir,
            library_skipped: 0,
            library_truncated: 0,
            library: vec![],
            library_rev: 0,
            update: UpdateData {
                status: if cfg!(debug_assertions) { "unsupported" } else { "idle" },
                version: String::new(),
                notes: String::new(),
                progress: -1.0,
                downloaded: 0,
                total: 0,
                error: String::new(),
                checked_at: 0,
                blocked: String::new(),
                updated_from: String::new(),
            },
            activity_seq: 0,
            pending_activity: vec![],
        }
    }

    pub fn run(&self, id: &str) -> Option<&Run> {
        self.runs.iter().find(|r| r.id == id)
    }
    pub fn run_mut(&mut self, id: &str) -> Option<&mut Run> {
        self.runs.iter_mut().find(|r| r.id == id)
    }
    pub fn running_count(&self) -> usize {
        self.runs.iter().filter(|r| r.running()).count()
    }
    /// 끄거나 다시 시작하면 안 되는 매크로 수 — 실행 중 + 시작하는 중 + 시작 전 확인 중.
    pub fn busy_count(&self) -> usize {
        self.runs.iter().filter(|r| r.running() || r.start_pending || r.checking).count()
    }

    pub fn slot(&self, s: KeySlot) -> &SlotState {
        &self.slots[&s]
    }
    pub fn slot_mut(&mut self, s: KeySlot) -> &mut SlotState {
        self.slots.get_mut(&s).expect("all slots exist")
    }

    pub fn slot_pair(&self, s: KeySlot) -> Option<KeyPair> {
        let p = &self.slot(s).keys;
        let (k, sec) = (p.api_key.trim(), p.api_secret.trim());
        (!k.is_empty() && !sec.is_empty()).then(|| KeyPair { api_key: k.into(), api_secret: sec.into() })
    }

    /// 칸 하나만 담은 자격증명(키 확인용). 연습 칸도 거래소 이름(binance)으로 넣음.
    pub fn slot_credentials(&self, s: KeySlot) -> Credentials {
        let mut c = Credentials::default();
        if let Some(p) = self.slot_pair(s) {
            c.exchanges.insert(s.exchange().as_str().into(), p);
        }
        c
    }

    /// 저장용 — 칸 이름 그대로(binance_testnet·binance_demo 포함), 회원 키 포함.
    pub fn storage_values(&self) -> Credentials {
        let mut c = Credentials { member_key: self.member_key.trim().to_string(), ..Default::default() };
        for (s, st) in &self.slots {
            let (k, sec) = (st.keys.api_key.trim(), st.keys.api_secret.trim());
            if !k.is_empty() || !sec.is_empty() {
                c.exchanges.insert(s.as_str().into(), KeyPair { api_key: k.into(), api_secret: sec.into() });
            }
        }
        if let Some(r) = &self.remembered {
            c.connection_history = r.connection_history.clone();
        }
        c
    }

    /// 이 슬롯을 시작할 때 봇에 줄 자격증명 — 모드에 맞는 칸의 키를 거래소 이름으로.
    pub fn run_credentials(&self, run: &Run) -> Credentials {
        let mut c = Credentials { member_key: self.member_key.trim().to_string(), ..Default::default() };
        if let (Some(ex), Some(slot)) = (run.exchange(), run.key_slot()) {
            if let Some(p) = self.slot_pair(slot) {
                c.exchanges.insert(ex.as_str().into(), p);
            }
        }
        c
    }

    pub fn run_fingerprint(&self, run: &Run) -> String {
        runner_core::connection::preflight_fingerprint(&run.macro_, run.mode, &self.run_credentials(run))
    }

    /// 설정 카드에서 연 연결 도우미의 대상 — 칸의 사이트와 확인 시장.
    pub fn wizard_target_for_slot(&self, s: KeySlot) -> WizardTarget {
        WizardTarget { slot: s, env: s.binance_env(), market: self.probe_market(s) }
    }

    /// 칸을 확인할 시장. 연습 칸·국내는 칸이 정함(`KeySlot::fixed_market`).
    /// 바이낸스 실거래 칸은 한 키로 현물·선물을 다 쓰므로 이 규칙 하나만 따름 —
    /// 이 칸을 연 도우미가 있으면 그 시장, 없으면 이 칸을 쓰는 실행(실전·모의) 중 목록 맨 앞 실행의 시장, 그것도 없으면 현물.
    /// 실행 목록·모드·매크로에서 매번 셈 — 웹 연결로 실행이 늘거나 모드가 바뀌어도 따로 맞출 값이 없음.
    pub fn probe_market(&self, s: KeySlot) -> Market {
        if let Some(m) = s.fixed_market() {
            return m;
        }
        if let Some(w) = self.wizard.as_ref().filter(|w| w.slot == s) {
            return w.market;
        }
        self.runs.iter().find(|r| r.key_slot() == Some(s)).map_or(Market::Spot, Run::market)
    }

    /// (기본 확인 매크로, 모드, 칸 자격증명, 지문)
    pub fn slot_check_snapshot(&self, s: KeySlot) -> (Value, RunMode, Credentials, String) {
        let m = s.probe_macro_in(self.probe_market(s));
        let mode = s.check_mode();
        let c = self.slot_credentials(s);
        let fp = runner_core::connection::preflight_fingerprint(&m, mode, &c);
        (m, mode, c, fp)
    }

    pub fn slot_busy(&self, s: KeySlot) -> bool {
        let st = self.slot(s);
        st.scheduled || st.busy_fp.as_deref() == Some(self.slot_check_snapshot(s).3.as_str())
    }

    pub fn current_slot_check(&self, s: KeySlot) -> Option<&KeyCheckRecord> {
        let fp = self.slot_check_snapshot(s).3;
        self.slot(s).check.as_ref().filter(|c| c.fingerprint == fp)
    }

    /// 연결 도우미가 볼 검사 상태 — idle·busy·done.
    pub fn wizard_check(&self, s: KeySlot) -> (&'static str, CheckView, String) {
        let idle = check_view(s.exchange().as_str(), false, "");
        if self.slot_pair(s).is_none() {
            return ("idle", idle, String::new());
        }
        if self.slot_busy(s) {
            return ("busy", idle, String::new());
        }
        match self.current_slot_check(s) {
            Some(c) => ("done", c.view.clone(), c.reason.clone()),
            None => ("idle", idle, String::new()),
        }
    }

    pub fn load_failure_note(&self) -> String {
        match self.load_status {
            LoadStatus::ReadError => format!(
                "{KEY_STORE_NAME} 잠금을 풀거나 접근을 허용한 뒤 [다시 읽기]를 누르세요. 저장 항목은 지우지 않아요."
            ),
            LoadStatus::DecryptError => "다른 사용자 계정이나 기기에서 저장한 키일 수 있어요. 키를 다시 입력하거나 [지우고 새로 만들기]로 저장 항목을 새로 만드세요. Secret Key를 잃어버렸다면 거래소에서 새로 발급해야 해요.".into(),
            LoadStatus::InvalidData => "[지우고 새로 만들기]로 새로 만들 수 있어요. 지금 입력한 키는 그대로 남아요.".into(),
            _ => String::new(),
        }
    }

    /// 같은 거래소·같은 종목을 이미 다른 슬롯이 돌리고 있는지 여부(시작 중 포함).
    pub fn conflicting_run(&self, run: &Run) -> Option<&Run> {
        let (ex, sym) = (run.exchange_name(), run.symbol());
        self.runs.iter().find(|r| {
            r.id != run.id && (r.running() || r.start_pending) && r.exchange_name() == ex && r.symbol() == sym
        })
    }

    pub fn conflict_message(run: &Run) -> String {
        format!(
            "같은 거래소 · 종목({} {})은 한 번에 하나만 실행해요. 지금 다른 매크로가 실행 중이에요.",
            exchange_label(&run.exchange_name()),
            run.symbol()
        )
    }

    // --- 활동 ---------------------------------------------------------------------
    #[allow(clippy::too_many_arguments)]
    pub fn push_activity(
        &mut self,
        run_id: &str,
        kind: &str,
        title: &str,
        detail: &str,
        raw: &str,
        dev: bool,
        tone: &str,
    ) {
        self.activity_seq += 1;
        let item = ActivityDto {
            id: self.activity_seq,
            run_id: run_id.into(),
            at: now_ms(),
            kind: kind.into(),
            title: clip_chars(title, ACTIVITY_TITLE_CHARS),
            detail: clip_chars(detail, ACTIVITY_DETAIL_CHARS),
            raw: clip_chars(raw, ACTIVITY_RAW_CHARS),
            dev,
            tone: tone.into(),
        };
        if let Some(r) = self.run_mut(run_id) {
            r.activity.push(item.clone());
            if r.activity.len() > ACTIVITY_LIMIT {
                let extra = r.activity.len() - ACTIVITY_LIMIT;
                r.activity.drain(..extra);
            }
        }
        self.pending_activity.push(item);
    }

    /// 원문 로그 줄(개발자 로그).
    pub fn push_raw(&mut self, run_id: &str, line: &str) {
        let (kind, _icon, text) = runner_core::log_kind::log_style(line);
        let tone = match kind {
            "error" => "danger",
            "warn" => "warn",
            "fill" => "ok",
            _ => "muted",
        };
        self.push_activity(run_id, "info", &text, "", line, true, tone);
    }

    // --- 스냅샷 ---------------------------------------------------------------------

    /// 웹 가이드에 보낼 지금 단계 — 웹으로 들어온 슬롯마다(키·주소·오류 문장은 싣지 않음).
    pub fn progress_payloads(&self) -> Vec<(Arc<SetupProgressReporter>, Value)> {
        let mut out = vec![];
        for r in &self.runs {
            let Some(rep) = r.reporter.clone() else { continue };
            let Some(target) = r.guide_target() else { continue };
            let (ex, slot) = (target.slot.exchange(), target.slot);
            // 바이낸스는 사이트마다 칸이 따로라 같은 칸을 연 도우미만 이 실행의 진행으로 봄.
            let wizard = self.wizard.as_ref().filter(|w| w.slot == slot);
            let (stage, path, slots) = match wizard {
                Some(w) => (w.stage.clone(), w.path.clone(), (w.got_access, w.got_secret)),
                None => ("ready".to_string(), String::new(), (false, false)),
            };
            let (state, view, _) = self.wizard_check(slot);
            let use_view =
                if wizard.is_some() { matches!(stage.as_str(), "result_ok" | "result_fail") } else { state == "done" };
            let checks = use_view.then_some((view.auth, view.ip, view.order));
            if let Ok(p) = runner_core::setup_progress::progress_payload(
                &stage,
                ex.as_str(),
                target.env,
                &path,
                slots,
                checks,
                if use_view { &view.problem } else { "" },
                use_view && view.connected,
            ) {
                out.push((rep, p));
            }
        }
        out
    }
}

mod health;
mod render;
pub(crate) use render::finish_label;
pub use render::{Notice, StartReply};

#[cfg(test)]
mod tests;
