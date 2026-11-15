### Documentation

* Two Linux skill lines no longer advise a blind retry after input that may have landed: an Overview-blocked `click` or `expand` (`popup_not_observed`) and a shell read that lost a node after the opener was pressed now say to look before pressing again, and `scripts/check-retry-advice.sh` now also fails a skill passage that names a may-be-delivered error kind and advises a retry without a look-first step. ([#211](https://github.com/gshost1/agent-desktop/pull/211))
