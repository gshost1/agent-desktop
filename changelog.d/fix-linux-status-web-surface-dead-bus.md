### Bug Fixes

* `status` reports an empty `supported_surfaces` again when the platform answers with no surfaces, such as a Linux session whose accessibility bus did not answer, instead of `["web"]` for a web surface that fails `PERM_DENIED` there
