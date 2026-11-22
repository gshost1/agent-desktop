use crate::tree::{AtspiElement, Field};
use zbus::zvariant::OwnedObjectPath;

#[derive(Clone, Debug)]
pub(in crate::tree) enum WrapperParent {
    Known(Field<String>),
    Listing(AtspiElement),
    Children(AtspiElement, Vec<(String, OwnedObjectPath)>),
}
