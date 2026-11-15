use agent_desktop_core::LocatorField;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NodeIdentifiers {
    pub(crate) ax_identifier: LocatorField<String>,
    pub(crate) ax_dom_identifier: LocatorField<String>,
}

impl NodeIdentifiers {
    pub(crate) fn from_fields(
        ax_identifier: LocatorField<String>,
        ax_dom_identifier: LocatorField<String>,
    ) -> Self {
        Self {
            ax_identifier: normalize(ax_identifier),
            ax_dom_identifier: normalize(ax_dom_identifier),
        }
    }
}

impl Default for NodeIdentifiers {
    fn default() -> Self {
        Self {
            ax_identifier: LocatorField::Absent,
            ax_dom_identifier: LocatorField::Absent,
        }
    }
}

fn normalize(field: LocatorField<String>) -> LocatorField<String> {
    match field {
        LocatorField::Known(value) => {
            crate::tree::properties::native_id::meaningful_native_id(Some(value))
                .map(LocatorField::Known)
                .unwrap_or(LocatorField::Absent)
        }
        LocatorField::Absent => LocatorField::Absent,
        LocatorField::Unknown => LocatorField::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_identifiers_remain_distinct() {
        for (native, dom) in [("native-save", "dom-save"), ("_NS:42", "compose")] {
            let identifiers = NodeIdentifiers::from_fields(
                LocatorField::Known(native.into()),
                LocatorField::Known(dom.into()),
            );

            assert_eq!(
                identifiers.ax_identifier,
                LocatorField::Known(native.into())
            );
            assert_eq!(
                identifiers.ax_dom_identifier,
                LocatorField::Known(dom.into())
            );
        }
    }
}
