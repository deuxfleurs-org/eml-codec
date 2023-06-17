# imf-codec

**Work in progress, do not use in production**
**This is currently only a decoder (parser), encoding is not supported.**

## Goals

 - Correctness: do no deviate from the RFC, support edge and obsolete cases
 - Straightforward/maintainable: implement the RFCs as close as possible, minimizing the amount of clever tricks and optimizations
 - Multiple syntax: Write the parser so it's easy to alternate between the strict and obsolete/compatible syntax
 - Never fail: Provide as many fallbacks as possible

## Non goals

  - Parsing optimization (greedy parser, etc.) as it would require to significantly deviate from the RFC ABNF syntax (would consider this case if we could prove that the transformation we make are equivalent)
  - Pipelining/streaming/buffering as the parser can arbitrarily backtrack + our result contains reference to the whole buffer, imf-codec must keep the whole buffer in memory. Avoiding the sequential approach would certainly speed-up a little bit the parsing, but it's too much work to implement currently.
  - Zerocopy. It might be implementable in the future, but to quickly bootstrap this project, I avoided it for now.

## Missing / known bugs

Current known limitations/bugs:

 - Resent Header Fields are not implemented
 - Return-Path/Received headers might be hard to use as their order is important, and it's currently lost in the final datastructure.
 - Datetime parsing of invalid date might return `None` instead of falling back to the `bad_body` field
 - Comments are dropped

## Design

Based on nom, a parser combinator lib in Rust.

## Testing strategy

 - Unit testing: parser combinator independently.
 - Selected full emails
 - Enron 500k

## Development status

Early development. Not ready.
Do not use it in production or any software at all.

## Targeted RFC

| # | Name |
|---|------|
|822	| ARPA INTERNET TEXT MESSAGES| 
|2822	| Internet Message Format (2001) | 	
|5322	| Internet Message Format (2008) | 	
|2045	| ↳ Multipurpose Internet Mail Extensions (MIME) Part One: Format of Internet Message Bodies |
|2046	| ↳ Multipurpose Internet Mail Extensions (MIME) Part Two: Media Types | 
|2047	| ↳ MIME (Multipurpose Internet Mail Extensions) Part Three: Message Header Extensions for Non-ASCII Text | 
|2048	| ↳ Multipurpose Internet Mail Extensions (MIME) Part Four: Registration Procedures | 
|2049	| ↳ Multipurpose Internet Mail Extensions (MIME) Part Five: Conformance Criteria and Examples |
|6532	| Internationalized Email Headers |
|9228   | Delivered-To Email Header Field |

## Alternatives

`stalwartlab/mail_parser`
