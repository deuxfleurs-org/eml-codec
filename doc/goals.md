## Goals

- Maintainability - modifying the code does not create regression and is possible for someone exterior to the project. Keep cyclomatic complexity low.
- Composability - build your own parser by picking the relevant passes, avoid work that is not needed.
- Compatibility - always try to parse something, do not panic or return an error.
- Exhaustivity - serve as a common project to encode knowledge about emails (existing mime types, existing headers, etc.).

## Non goals

  - Parsing optimization that would make more complicated to understand the logic.
  - Optimization for a specific use case, to the detriment of other use cases.
  - Pipelining/streaming/buffering as the parser can arbitrarily backtrack + our result contains reference to the whole buffer, eml-codec must keep the whole buffer in memory. Avoiding the sequential approach would certainly speed-up a little bit the parsing, but it's too much work to implement currently.


