### Testing

* CI now validates changelog fragments and runs the fragment tool's self-test on every pull request, in the Format job. The fork's Supply Chain workflow does not run, so a malformed fragment used to be caught only at release.
* The hosted macOS E2E lane runs on macos-15 for pull requests and on macos-15 and macos-26 for pushes to main. GitHub runs five macOS jobs at a time on this account, and two E2E jobs per pull request made every parallel pull request wait.
