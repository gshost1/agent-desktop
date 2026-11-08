use crate::NotificationFilter;

#[test]
fn default_is_unfiltered() {
    let filter = NotificationFilter::default();
    assert!(filter.app.is_none());
    assert!(filter.text.is_none());
    assert!(filter.limit.is_none());
}

#[test]
fn app_and_text_predicates_ignore_case_and_text_reads_title_body_and_app() {
    let info = crate::NotificationInfo {
        index: 1,
        app_name: "Slack".into(),
        title: "Hello".into(),
        body: Some("from Ann".into()),
        actions: vec!["Reply".into()],
    };
    let filter = |app: Option<&str>, text: Option<&str>| NotificationFilter {
        app: app.map(String::from),
        text: text.map(String::from),
        limit: None,
    };
    assert!(filter(Some("sla"), None).admits(&info));
    assert!(!filter(Some("mail"), None).admits(&info));
    assert!(filter(None, Some("ANN")).admits(&info));
    assert!(filter(None, Some("slack")).admits(&info));
    assert!(!filter(None, Some("reply")).admits(&info));
}
