use crate::tree::{AtspiElement, Field};

#[derive(Clone, Debug)]
pub(in crate::tree) enum WrapperParent {
    Known(Field<String>),
    Listing(AtspiElement),
}
