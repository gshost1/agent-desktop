### Testing

* The x86_64 macOS FFI panic guard runs on pushes to `main` and no longer on every pull request; the arm64 macOS guard, which also checks the C header, still runs on each one. Hosted macOS runners take five jobs at a time, and pull requests waited hours for this job.
