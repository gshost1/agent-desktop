use super::is_auxiliary_subrole;

#[test]
fn panels_and_unknown_windows_are_auxiliary_and_documents_and_dialogs_are_not() {
    for subrole in ["AXFloatingWindow", "AXSystemFloatingWindow", "AXUnknown"] {
        assert!(is_auxiliary_subrole(subrole), "{subrole}");
    }
    for subrole in ["AXStandardWindow", "AXDialog", "AXSystemDialog"] {
        assert!(!is_auxiliary_subrole(subrole), "{subrole}");
    }
}
