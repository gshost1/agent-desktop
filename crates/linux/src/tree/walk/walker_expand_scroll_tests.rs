use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use zbus::connection::Builder;

use super::read_observed;
use crate::tree::AtspiElement;
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::vocabulary::{role_numbers::SCROLL_BAR, state_bits::VERTICAL};

struct Accessible;

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl Accessible {
    fn get_role(&self) -> u32 {
        SCROLL_BAR
    }

    fn get_state(&self) -> Vec<u32> {
        vec![1 << VERTICAL, 0]
    }
}

struct Value(Arc<AtomicUsize>, Option<f64>);

#[zbus::interface(name = "org.a11y.atspi.Value")]
impl Value {
    #[zbus(property)]
    fn minimum_value(&self) -> f64 {
        self.0.fetch_add(1, Ordering::SeqCst);
        0.0
    }

    #[zbus(property)]
    fn maximum_value(&self) -> zbus::fdo::Result<f64> {
        self.0.fetch_add(1, Ordering::SeqCst);
        self.1
            .ok_or_else(|| zbus::fdo::Error::Failed("unavailable".to_owned()))
    }
}

fn observe(purpose: ReadPurpose, maximum: Option<f64>) -> (Option<Result<bool, ()>>, usize) {
    let calls = Arc::new(AtomicUsize::new(0));
    let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(near).p2p().build(),
        Builder::async_io_unix_stream(far)
            .server(zbus::Guid::generate())
            .unwrap()
            .p2p()
            .serve_at("/bar", Accessible)
            .unwrap()
            .serve_at("/bar", Value(calls.clone(), maximum))
            .unwrap()
            .build(),
    ));
    let client = client.unwrap();
    let _server = server.unwrap();
    let element = AtspiElement::new(":1.1", "/bar", 1);
    let (read, _) = zbus::block_on(read_observed(&client, &element, 1024, purpose)).unwrap();
    (read.vertical_bar_range, calls.load(Ordering::SeqCst))
}

#[test]
fn snapshot_reads_scroll_evidence_for_overflow_fitting_and_failed_ranges() {
    for (maximum, expected) in [
        (Some(10.0), Ok(true)),
        (Some(0.0), Ok(false)),
        (None, Err(())),
    ] {
        assert_eq!(observe(ReadPurpose::Snapshot, maximum), (Some(expected), 2));
    }
}

#[test]
fn non_index_purposes_skip_scroll_evidence_reads() {
    for purpose in [
        ReadPurpose::TextSearch,
        ReadPurpose::TypeTarget,
        ReadPurpose::Sampling,
        ReadPurpose::Count,
        ReadPurpose::Identity,
        ReadPurpose::Resolve,
        ReadPurpose::Element,
        ReadPurpose::State,
        ReadPurpose::Bounds,
        ReadPurpose::Value,
        ReadPurpose::Actions,
        ReadPurpose::Frame,
        ReadPurpose::Structure,
        ReadPurpose::Shape,
        ReadPurpose::Gate,
    ] {
        assert_eq!(observe(purpose, Some(10.0)), (None, 0), "{purpose:?}");
    }
}
