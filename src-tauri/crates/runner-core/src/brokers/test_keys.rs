//! 테스트용 가짜 거래소 키 — 테스트를 돌릴 때마다 새로 만듦.
//! 코드에 고정 키 문자열을 두지 않음: 가짜 서버로만 쓰는 값이어도 서명 키로 흘러가면 보안 분석(CodeQL)이
//! '하드코딩된 암호 값'으로 잡고, 테스트 코드도 분석 대상에 그대로 두기 위함. 한 번 실행하는 동안은 같은 값.

use std::sync::LazyLock;

pub(crate) struct FakeKeys {
    pub access: String,
    pub secret: String,
}

pub(crate) static KEYS: LazyLock<FakeKeys> = LazyLock::new(|| FakeKeys {
    access: format!("test-access-{}", uuid::Uuid::new_v4().simple()),
    secret: format!("test-secret-{}", uuid::Uuid::new_v4().simple()),
});
