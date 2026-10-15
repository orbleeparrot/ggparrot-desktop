use super::*;

#[test]
fn debug_never_prints_secrets() {
    let mut c = Credentials { member_key: "ggp_member_secret".into(), ..Default::default() };
    c.exchanges.insert("upbit".into(), KeyPair { api_key: "AKEY123".into(), api_secret: "SSECRET456".into() });
    let text = format!("{c:?}");
    for secret in ["ggp_member_secret", "AKEY123", "SSECRET456"] {
        assert!(!text.contains(secret), "{text}");
    }
    assert!(text.contains("<redacted>"));
}
