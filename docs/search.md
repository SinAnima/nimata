# Search

Search looks through every discussion on this device: post text and
discussion titles. It runs entirely locally, against the SQLite full-text
index described in [data-model.md](data-model.md#search-schema-version-6).
Nothing is sent anywhere.

Open it with the field at the top of the discussion list, or press ⌘F
(Ctrl+F on Windows and Linux) or `/`. Results replace the list as you type.
Choosing a post opens its discussion, scrolls to it, and highlights the
matching words; **Close search** or Esc brings the list back and removes the
highlights.

## Query syntax

Words must all appear in a post, in any order. Each word matches the
beginnings of words, so `stratif` finds "stratification"; a single letter
matches only itself. Case and accents never matter, in any script:
`νηματα` finds "Νήματα" and `cafe` finds "Café".

| Write                         | Finds                                                                                 |
| ----------------------------- | ------------------------------------------------------------------------------------- |
| `datalog oracle`              | posts containing both words                                                           |
| `"mapping engine"`            | the exact phrase                                                                      |
| `from:claude`                 | posts by a model or person whose name or alias starts with "claude"; `from:me` is you |
| `after:2026-09-01`            | posts written on or after that day                                                    |
| `before:2026-10-01`           | posts written before that day                                                         |
| `on:2026-09-15`               | posts written on that day                                                             |
| `discussion:"Digital Assets"` | posts in discussions whose title contains this                                        |
| `in:archived`                 | also archived discussions, which are otherwise left out                               |

Filters combine with words and with each other:
`from:claude after:2026-09-01 discussion:"Digital Assets" custody`. Days are
in your local time. Several `from:` filters mean any of those authors.

A filter Nimata cannot use, such as `after:yesterday`, is explained above
the results and otherwise ignored. Other `word:value` text, such as a URL or
`10:30`, is searched as words. Search syntax characters (`OR`, `NEAR`, `*`)
are ordinary words; a query can never change how the index is read.

Discussion titles match on words alone; with `from:` or a date filter only
posts are shown.

## Results

Posts are listed newest first, up to 100; if more match, the list says so
and suggests narrowing the search. Each result shows the discussion, the
words around the first match, the author, and the time. Deleted posts and
deleted discussions are never found.

Searches whose results you open are remembered (the last ten) and offered
when the search field is empty. **Clear** forgets them.

## Speed

Budget: every query answers in **under 250 ms** on an archive of 10,000
discussions and 1,000,000 posts (about 1.2 GB) on an Apple M1 Max. Newest-
first ordering lets SQLite stop after the first 100 matches, which keeps
very common words fast; ranking by relevance would have to score every
match.

Measured on 2026-10-06, the slowest query took 48 ms (`datalog oracle`);
rare words, phrases, and date filters take 1 to 2 ms, and word beginnings
and filters with common words about 25 ms.

The check builds that archive and times a set of queries. It is ignored in
normal test runs because building the archive takes several minutes:

```sh
cargo test --release -p nimata-core --test search_performance -- --ignored --nocapture
```

## Not yet

- Attachments are not searched (they arrive in Stage 8).
- No semantic or "similar meaning" search; matching is by words.
- Saved searches come with Stage 13.
