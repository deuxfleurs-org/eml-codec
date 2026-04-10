# eml-codec

This library implements parsing and printing of emails. Its aim is to be a swiss army knife to encode and decode emails, whether it is to build an IMAP/JMAP server, an email filter (like an antispam), or an email client.

`eml-codec` is a child project of [Aerogramme](https://aerogramme.deuxfleurs.fr), a distributed and encrypted IMAP server developed by the non-profit organization [Deuxfleurs](https://deuxfleurs.fr).

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

let email = eml_codec::parse_message(input);
println!(
    "{} just sent you an email with subject \"{}\"",
    email.imf.from_or_sender().unwrap().to_string(),
    email.imf.subject.unwrap().to_string(),
);

let bytes = eml_codec::print_message(email, None);
println!(
    "reformatted email:\n{}",
    String::from_utf8_lossy(&bytes),
);
```

[See more examples and helper tools in the examples/ folder.](./examples/README.md)

## Goals

- Maintainability - modifying the code does not create regression and is possible for someone exterior to the project.
- Compatibility - always try to parse something, do not panic or return an error.
- Exhaustivity - serve as a common project to encode knowledge about emails (existing mime types, existing headers, etc.).
- Type safe - do not manipulate only strings/bytes but leverage Rust type system instead so you benefit from its safety checks at compile time.

[Read more about the design of this library.](./doc/DESIGN.md)

## Missing / known bugs

Current known limitations/bugs:

 - **Part transfer-decoding is not implemented yet**
 - Resent Header Fields are not implemented
 - Comments contained in the email headers are dropped during parsing
 - No support is provided for message/external-body (read data from local computer) and message/partial (aggregate multiple fragmented emails) as they seem obsolete and dangerous to implement.

## Testing methodology

We have been testing `eml-codec` using different complementary techniques:
- extensive unit tests, for both parsing and printing functions;
- fuzzing/property testing, checking for absence of crashes and serialization/deserialization roundtrip properties;
- testing on real-world email corpuses, to improve the parser recovery strategies on non-RFC-compliant emails.

This crate is also tested as part of
[Aerogramme](https://git.deuxfleurs.fr/deuxfleurs/aerogramme) where its parsing
capabilities are compared at the IMAP level against Dovecot, Cyrus, Maddy and
other IMAP servers.

[Read more about our testing methodology.](doc/TESTING.md)

## RFC and IANA references

RFC

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
| 🟩 |6532	| ↳ Internationalized Email Headers |
| 🔴 |9228  | ↳ Delivered-To Email Header Field |
|    |      | **MIME extensions** |
| 🔴 |1847  | ↳ Security Multiparts for MIME: Multipart/Signed and Multipart/Encrypted |
| 🔴 |2231  | ↳ MIME Parameter Value and Encoded Word Extensions: Character Sets, Languages, and Continuations |
| 🔴 |2387  | ↳ The MIME Multipart/Related Content-type |
| 🔴 |3462  | ↳ The Multipart/Report Content Type for the Reporting of Mail System Administrative Messages |
| 🔴 |3798  | ↳ Message Disposition Notification |
| 🔴 |6838  | ↳ Media Type Specifications and Registration Procedures |

IANA

| Name | Description | Note |
|------|-------------|------|
| [Media Types](https://www.iana.org/assignments/media-types/media-types.xhtml) | Registered media types for the Content-Type field | Currently only the media types in the MIME RFC have dedicated support in `eml-codec`. |
| [Character sets](https://www.iana.org/assignments/character-sets/character-sets.xhtml) | Supported character sets for the `charset` parameter | They should all be supported through the `encoding_rs` crate |

## State of the art / alternatives

The following review is not an objective, neutral, impartial review. Instead, it's an attempt
to explain why I wrote this library. If you find something outdated or objectively wrong, feel free to open a PR or an issue to fix it.
In no case, I think `eml-codec` is superior, it's just another approach to the problem, and I see it as another stone to the edifice.

[mail\_parser](https://github.com/stalwartlabs/mail-parser), [mailparse](https://github.com/staktrace/mailparse) and [rust-email](https://github.com/deltachat/rust-email) 
are 3 handwritten parsers. Such handwritten parsers do not encourage separation of concerns: `mail_parser` and `mailparse` feature large functions with hundreds of lines
with a high cylomatic complexity. Due to this complex logic, I have failed to debug/patch such code in the past. 
`rust-email` code is easier to read but its mime part implementation is marked as unstable. `mail_parser` is used in the IMAP/JMAP/SMTP server project [stalwartlabs/mail-server](https://github.com/stalwartlabs/mail-server) and `rust-email` is used in the email-based chat application [Deltachat](https://github.com/deltachat) (however `rust-email` MIME parsed is not used, a custom MIME parser is reimplemented in Delta Chat instead). It must be noted that `mail_parser` supports a large amount of extensions (UTF-8 headers, UTF-7 encoding, continuation, many custom fields, etc.) and would better cope with malformed emails than other libraries. **A goal of `eml_codec` is to be open to contribution and maintainable over time, which is made possible trough the parser combinator pattern that encourages writing small, reusable, independently testable functions.**

[rustyknife](https://github.com/jothan/rustyknife) uses the same design pattern (parser combinator) and the same library ([nom](https://github.com/rust-bakery/nom)) as `eml_codec`. However, `rustyknife` is more targeted to SMTP servers (MTA) than IMAP (MDA) and email clients (MUA).
It thus only supports parsing headers and not emails' body. Also, an acquaintance warned me that this library is a bit slow,
it might be due to the fact that the library does some processing while parsing the email (like rebuilding and allocating strings).
If it happens that this part is not used later, the allocation/processing has been wasted.
**A goal of `eml_codec` is to produce an AST of the email with as few processing as possible, so that the parsing remains efficient,
and then the allocation/processing is made lazily, on demand, when the corresponding function is called. It is often referred as zero-copy.**
 
## Support

`eml-codec`, as part of the [Aerogramme project](https://nlnet.nl/project/Aerogramme/), was funded through the NGI Assure Fund, a fund established by NLnet with financial support from the European Commission's Next Generation Internet programme, under the aegis of DG Communications Networks, Content and Technology under grant agreement No 957073.

![NLnet logo](https://aerogramme.deuxfleurs.fr/images/nlnet.svg)

## License

eml-codec
Copyright (C)  The eml-codec Contributors

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
