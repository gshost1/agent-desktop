use super::is_embedded_runtime;

const BROWSER: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36";
const EMBEDDED: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) SomeApp/4.41.0 Chrome/130.0.6723.191 Electron/33.4.0 Safari/537.36";

#[test]
fn a_web_browser_is_not_an_embedded_runtime() {
    assert!(!is_embedded_runtime(BROWSER));
    assert!(!is_embedded_runtime(""));
}

#[test]
fn an_app_that_embeds_the_runtime_is_recognized_from_its_user_agent() {
    assert!(is_embedded_runtime(EMBEDDED));
}
