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
multipass parser
 - extract header block: `&[u8]` (find \r\n\r\n OR \n\n OR \r\r OR \r\n)
 - decode/convert it with chardet + encoding\_rs to support latin-1: Cow<&str>
 - extract header lines iter::&str (requires only to search for FWS + obs\_CRLF)
 - extract header names iter::Name::From(&str)
 - extract header body iter::Body::From(Vec<MailboxRef>)
 - extract header section Section

recovery
 - based on multipass, equivalent to sentinel / synchronization tokens

## Testing strategy

imf-codec aims to be as much tested as possible against reald

### Unit testing: parser combinator independently (done)

### Selected full emails (expected)

### Existing datasets

**Enron 500k** - Took 20 minutes to parse ~517k emails and check that 
RFC5322 headers (From, To, Cc, etc.) are correctly parsed.
From this list, we had to exclude ~50 emails on which
the From/To/Cc fields were simply completely wrong, but while
some fields failed to parse, the parser did not crash and
parsed the other fields of the email correctly.

Planned: jpbush, my inbox, etc.

### Fuzzing (expected)

### Across reference IMAP servers (dovevot, cyrus) (expected)

## Development status

Early development. Not ready.
Do not use it in production or any software at all.

Todo:
 - [X] test over the enron dataset
 - [ ] convert to multipass parser
 - [ ] implement mime part 3 (encoded headers)
 - [ ] implement mime part 1 (new headers)
 - [ ] review part 2 (media types) and part 4 (registration procedure) but might be out of scope
 - [ ] implement some targeted testing as part of mime part 5
 - [ ] implement fuzzing through cargo fuzz
 - [ ] test over other datasets (jpbush, ml, my inbox)
 - [ ] backport to aerogramme
 - [ ] fix warnings, put examples, document the public API a little bit

## Targeted RFC

| 🚩 | # | Name |
|----|---|------|
| 🟩 |822	| ARPA INTERNET TEXT MESSAGES| 
| 🟩 |2822	| Internet Message Format (2001) | 	
| 🟩 |5322	| Internet Message Format (2008) | 	
| 🔴 |2045	| ↳ Multipurpose Internet Mail Extensions (MIME) Part One: Format of Internet Message Bodies |
| 🔴 |2046	| ↳ Multipurpose Internet Mail Extensions (MIME) Part Two: Media Types | 
| 🔴 |2047	| ↳ MIME (Multipurpose Internet Mail Extensions) Part Three: Message Header Extensions for Non-ASCII Text | 
| 🔴 |2048	| ↳ Multipurpose Internet Mail Extensions (MIME) Part Four: Registration Procedures | 
| 🔴 |2049	| ↳ Multipurpose Internet Mail Extensions (MIME) Part Five: Conformance Criteria and Examples |
| 🟩 |6532	| Internationalized Email Headers |
| 🔴 |9228   | Delivered-To Email Header Field |

## Alternatives

`stalwartlab/mail_parser`
