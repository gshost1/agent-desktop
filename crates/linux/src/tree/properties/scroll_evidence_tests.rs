use super::{decide_bar, of_children};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Range(Arc<AtomicUsize>);

#[zbus::interface(name = "org.a11y.atspi.Value")]
impl Range {
    #[zbus(property)]
    fn minimum_value(&self) -> f64 {
        self.0.fetch_add(1, Ordering::SeqCst);
        0.0
    }

    #[zbus(property)]
    fn maximum_value(&self) -> f64 {
        self.0.fetch_add(1, Ordering::SeqCst);
        80.0
    }
}

#[test]
fn vertical_scroll_ranges_decide_overflow() {
    assert_eq!(decide_bar(true, Some((0.0, 80.0))), Some(Ok(true)));
    assert_eq!(decide_bar(true, Some((4.0, 4.0))), Some(Ok(false)));
    assert_eq!(decide_bar(true, Some((4.0, 0.0))), Some(Ok(false)));
    assert_eq!(decide_bar(true, None), Some(Err(())));
    assert_eq!(decide_bar(false, Some((0.0, 80.0))), None);
    assert_eq!(decide_bar(true, Some((0.0, f64::NAN))), Some(Err(())));
}

#[test]
fn only_readable_vertical_children_decide_the_parent() {
    assert_eq!(of_children([].into_iter()), None);
    assert_eq!(of_children([None].into_iter()), None);
    assert_eq!(of_children([Some(Err(()))].into_iter()), None);
    assert_eq!(of_children([Some(Ok(false))].into_iter()), Some(false));
    assert_eq!(of_children([Some(Ok(true))].into_iter()), Some(true));
    assert_eq!(
        of_children([Some(Ok(false)), Some(Err(()))].into_iter()),
        None
    );
    assert_eq!(
        of_children([Some(Err(())), Some(Ok(true))].into_iter()),
        Some(true)
    );
}

#[test]
fn only_vertical_bars_read_two_named_value_properties_even_when_hidden() {
    use crate::tree::live::test_support::{read_of, words};
    use crate::tree::vocabulary::{
        role_numbers::SCROLL_BAR,
        state_bits::{HORIZONTAL, VERTICAL},
    };
    use crate::tree::{AtspiElement, Field};
    use zbus::connection::Builder;

    let reads = Arc::new(AtomicUsize::new(0));
    let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(near).p2p().build(),
        Builder::async_io_unix_stream(far)
            .server(zbus::Guid::generate())
            .unwrap()
            .p2p()
            .serve_at("/bar", Range(reads.clone()))
            .unwrap()
            .build(),
    ));
    let client = client.unwrap();
    let _server = server.unwrap();
    reads.store(0, Ordering::SeqCst);
    let mut bar = read_of(SCROLL_BAR, "", &[]).node;
    bar.element = AtspiElement::new(":1.1", "/bar", 1);
    bar.traits.states = Field::Present(words(&[HORIZONTAL]));
    assert_eq!(zbus::block_on(super::read_bar(&client, &bar)), None);
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    bar.traits.states = Field::Present(words(&[VERTICAL]));
    assert_eq!(
        zbus::block_on(super::read_bar(&client, &bar)),
        Some(Ok(true))
    );
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    bar.element = AtspiElement::new(":1.1", "/unreadable", 1);
    assert_eq!(
        zbus::block_on(super::read_bar(&client, &bar)),
        Some(Err(()))
    );
    bar.traits.states = Field::Failed("unknown".into());
    assert_eq!(zbus::block_on(super::read_bar(&client, &bar)), None);
}
