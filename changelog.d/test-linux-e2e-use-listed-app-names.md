### Testing

* Resolve owned Linux E2E applications by PID in list-apps before menu waits, rather than passing toolkit labels or launcher names that are not product application names.
* Allow only the identified listener_hold holder from crates/linux/src/system/bus/listener_hold.rs up to 1.5 seconds from process start during Qt cleanup; reject other extra processes immediately.
