use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::LABELLED_BY;
use super::tests::{Accessible, entry, label, labels_of, serve};
use super::{Address, EDITABLE};
use crate::tree::LabelOutcome;
use crate::tree::vocabulary::label_read::RelationSet;
use crate::tree::vocabulary::role_numbers::FILLER;
use zbus::connection::Builder;

struct FieldParentReads(Arc<AtomicUsize>);

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl FieldParentReads {
    fn get_relation_set(&self) -> RelationSet {
        Vec::new()
    }

    fn get_state(&self) -> Vec<u32> {
        vec![1 << EDITABLE, 0]
    }

    fn get_interfaces(&self) -> Vec<&str> {
        Vec::new()
    }

    #[zbus(property)]
    fn parent(&self) -> Address {
        self.0.fetch_add(1, Ordering::SeqCst);
        super::tests::address("/composite")
    }
}

fn serve_fields(
    fields: &[&'static str],
    parent: Accessible,
    reads: &[Arc<AtomicUsize>],
) -> [zbus::Connection; 2] {
    let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = Builder::async_io_unix_stream(far)
        .server(zbus::Guid::generate())
        .unwrap()
        .p2p()
        .serve_at("/composite", parent)
        .unwrap();
    for (path, reads) in fields.iter().zip(reads) {
        server = server
            .serve_at(*path, FieldParentReads(reads.clone()))
            .unwrap();
    }
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(near).p2p().build(),
        server.build(),
    ));
    [client.unwrap(), server.unwrap()]
}

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
    let reads = fields.map(|_| Arc::new(AtomicUsize::new(0)));
    let [client, _server] = serve_fields(&fields, composite(fields.to_vec(), &calls), &reads);
    assert_eq!(
        labels_of(&client, &fields, false),
        [const { LabelOutcome::Unlabelled }; 4]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(reads.map(|reads| reads.load(Ordering::SeqCst)), [1; 4]);
}

#[test]
fn a_single_child_wrapper_reuses_composite_parent_evidence() {
    let calls = Arc::new(AtomicUsize::new(0));
    let reads = [Arc::new(AtomicUsize::new(0))];
    let parent = Accessible {
        role: FILLER,
        children: vec!["/entry"],
        calls: calls.clone(),
        ..Accessible::default()
    };
    let [client, _server] = serve_fields(&["/entry"], parent, &reads);
    assert_eq!(
        labels_of(&client, &["/entry"], false),
        [LabelOutcome::WrapperDescription(
            "Wrapper description".into()
        )]
    );
    assert_eq!(reads[0].load(Ordering::SeqCst), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 5);
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
