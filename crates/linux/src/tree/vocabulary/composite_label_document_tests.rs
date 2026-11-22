use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::LABELLED_BY;
use super::tests::{Accessible, entry, label, labels_of, serve};
use crate::tree::LabelOutcome;

pub(super) fn composite(children: Vec<&'static str>, calls: &Arc<AtomicUsize>) -> Accessible {
    Accessible {
        children,
        relations: vec![(LABELLED_BY, "/label")],
        calls: calls.clone(),
        ..Accessible::default()
    }
}

#[test]
fn fields_of_one_parent_share_one_read_of_it() {
    let fields = ["/entry", "/e2", "/e3", "/e4"];
    let calls = Arc::new(AtomicUsize::new(0));
    let entries = fields.map(|path| (path, entry(&Arc::default())));
    let [client, _server] = serve(
        [
            ("/composite", composite(fields.to_vec(), &calls)),
            ("/label", label(Vec::new())),
        ]
        .into_iter()
        .chain(entries),
    );
    assert_eq!(
        labels_of(&client, &fields, false),
        [const { LabelOutcome::Unlabelled }; 4]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn a_field_inside_a_web_document_reads_no_composite() {
    let calls = Arc::new(AtomicUsize::new(0));
    let [client, _server] = serve([
        ("/composite", composite(vec!["/entry"], &calls)),
        ("/label", label(Vec::new())),
        ("/entry", entry(&calls)),
    ]);
    assert_eq!(
        labels_of(&client, &["/entry"], true),
        [LabelOutcome::Unlabelled]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        labels_of(&client, &["/entry"], false),
        [LabelOutcome::Text("Find:".to_owned())]
    );
}
