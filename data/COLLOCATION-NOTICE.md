# Collocation scoring provenance

The compatibility categories and scoring constants in
prototype/bridge/src/collocation.rs are independently authored project rules,
licensed GPL-3.0-or-later under the project LICENSE. They are grammatical priors,
not measured corpus frequencies, pretrained model weights, or imported sentences.
The separate synthetic evaluation fixtures use the same project license.

The decoder uses only the existing checksum-pinned CC-CEDICT dictionary and
Google Books Ngram unigram-derived weights. Their attribution and terms remain
in LEXICON-NOTICE.md. No new corpus, downloaded model, host context, clipboard
content or personal typing dataset is incorporated into this distribution.
User-confirmed word preferences remain local under the existing version-1 format.
