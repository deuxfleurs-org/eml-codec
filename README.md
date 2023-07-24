# eml-codec

**⚠️ Work in progress, do not use in production**  
**⚠️ This is currently only a decoder (parser), encoding is not yet implemented.**

## Example

```rust
let input = br#"Date: 7 Mar 2023 08:00:00 +0200
From: deuxfleurs@example.com
To: someone_else@example.com
Subject: An RFC 822 formatted message
MIME-Version: 1.0
Content-Type: text/plain; charset=us-ascii

This is the plain text body of the message. Note the blank line
between the header information and the body of the message."#;

let email = eml_codec::email(input).unwrap();
println!(
    "{} just sent you an email with subject \"{}\"",
    email.imf.from[0].to_string(),
    email.imf.subject.unwrap().to_string(),
);
```

## About the name

This library does not aim at implementing a specific RFC, but to be a swiss-army knife to decode and encode ("codec") what is generaly considered an email (generally abbreviated "eml"), hence the name: **eml-codec**.

## Goals

- Maintainability - modifying the code does not create regression and is possible for someone exterior to the project. Keep cyclomatic complexity low.
- Composability - build your own parser by picking the relevant passes, avoid work that is not needed.
- Compatibility - always try to parse something, do not panic or return an error.
- Exhaustivity - serve as a common project to encode knowledge about emails (existing mime types, existing headers, etc.).

## Non goals

  - Parsing optimization that would make more complicated to understand the logic.
  - Optimization for a specific use case, to the detriment of other use cases.
  - Pipelining/streaming/buffering as the parser can arbitrarily backtrack + our result contains reference to the whole buffer, eml-codec must keep the whole buffer in memory. Avoiding the sequential approach would certainly speed-up a little bit the parsing, but it's too much work to implement currently.

## Missing / known bugs

Current known limitations/bugs:

 - Resent Header Fields are not implemented
 - Return-Path/Received headers might be hard to use as their order is important, and it's currently lost in the final datastructure.
 - Datetime parsing of invalid date might return `None` instead of falling back to the `bad_body` field
 - Comments contained in the email headers are dropped during parsing
 - No support is provided for message/external-body (read data from local computer) and message/partial (aggregate multiple fragmented emails) as they seem obsolete and dangerous to implement.

## Design

Speak about parser combinators.

## Testing strategy

eml-codec aims to be as much tested as possible against real word data.

### Unit testing: parser combinator independently (done)

### Selected full emails (expected)

### Existing datasets

**Enron 500k** - Took 20 minutes to parse ~517k emails and check that 
RFC5322 headers (From, To, Cc, etc.) are correctly parsed.
From this list, we had to exclude ~50 emails on which
the From/To/Cc fields were simply completely wrong, but while
some fields failed to parse, the parser did not crash and
parsed the other fields of the email correctly.

Run it on your machine:

```bash
cargo test -- --ignored --nocapture enron500k
```

Planned: jpbush, my inbox, etc.

### Fuzzing (expected)

### Across reference IMAP servers (dovevot, cyrus) (expected)

## Targeted RFC and IANA references

| 🚩 | # | Name |
|----|---|------|
| 🟩 |822	| ARPA INTERNET TEXT MESSAGES| 
| 🟩 |2822	| Internet Message Format (2001) | 	
| 🟩 |5322	| Internet Message Format (2008) | 	
| 🟩 |2045	| ↳ Multipurpose Internet Mail Extensions (MIME) Part One: Format of Internet Message Bodies |
| 🟩 |2046	| ↳ Multipurpose Internet Mail Extensions (MIME) Part Two: Media Types | 
| 🟩 |2047	| ↳ MIME (Multipurpose Internet Mail Extensions) Part Three: Message Header Extensions for Non-ASCII Text | 
| 🟩 |2048	| ↳ Multipurpose Internet Mail Extensions (MIME) Part Four: Registration Procedures | 
| 🟩 |2049	| ↳ Multipurpose Internet Mail Extensions (MIME) Part Five: Conformance Criteria and Examples |
|    |      | **Headers extensions** |
| 🔴 |2183  | ↳ Communicating Presentation Information in Internet Messages: The Content-Disposition Header Field |
| 🔴 |6532	| ↳ Internationalized Email Headers |
| 🔴 |9228  | ↳ Delivered-To Email Header Field |
|    |      | **MIME extensions** |
| 🔴 |1847  | ↳ Security Multiparts for MIME: Multipart/Signed and Multipart/Encrypted |
| 🔴 |2387  | ↳ The MIME Multipart/Related Content-type |
| 🔴 |3462  | ↳ The Multipart/Report Content Type for the Reporting of Mail System Administrative Messages |
| 🔴 |3798  | ↳ Message Disposition Notification |
| 🔴 |6838  | ↳ Media Type Specifications and Registration Procedures |

IANA references :
 - (tbd) MIME subtypes
 - [IANA character sets](https://www.iana.org/assignments/character-sets/character-sets.xhtml)

## State of the art / alternatives

`stalwartlab/mail_parser`

