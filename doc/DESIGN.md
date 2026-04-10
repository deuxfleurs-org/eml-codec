# Design 

This document describes design decisions, guidelines and principled followed by eml-codec.

## High-level Goals

We first describe the *goals*: what the library should do, and how it should do it.
Then, in the next section, we describe what we do, when it achieves these goals and which compromises we have to make.

### Use-cases

We want eml-codec to be usable in the following scenarios: 

- **parsing -> analysis.** Examples: display a summary of an email, implement features of an IMAP server like BODYSTRUCTURE, or implement an antispam filter.
- **parsing -> modification -> reprinting.** Example: implement an advanced mailing-list server which optimizes the structure of emails.
- **email composition using the library API -> printing.** Example: send a password recovery email.

*Note*: the second scenario is the most challenging one in terms of library design. 
In this scenario, we might simultaneously want to: 1) be able to ingest most real-world emails even when they are malformed without losing information; 2) ensure that modifying an email does not make it "more invalid" than it is; 3) ensure that final printed emails are always valid. It is generally hard or even impossible to fully satisfy all of these goals, even though they are all desirable in isolation. 

### Library Goals for Consumers

*Ideal* goals for users of the library (we detail later in which cases they cannot 100% be attained):

- **[A] Always read emails successfully**: be able to parse any input without failing (even invalid inputs)
- **[B] Always produce valid emails**: when printing an email, ensure the output is always valid according to email RFCs, with respect to:
  + **email structure**. For example, the `From` and `Date` fields are mandatory.
  + **email serialization**. For example, use correct text encoding and obey line length limits (78 characters + CRLF). 
- **[C] Recover as much information as possible** when encountering invalid emails.
- **[D] Must be usable in production software**: should be fast enough, complete enough, robust enough to serve common email use-cases (e.g. be part of an IMAP server, a mailing-list server, or a web service that needs to send emails).

### Library Goals for Implementors

*Ideal* goals for implementors of the library:

- **[E] Maintainable & composable software architecture**. For example, adding support for new RFCs should be easy and non-intrusive.
- **[F] Easily inspectable**: it should be easy to match code and RFC definitions (e.g. parsing grammars).
- **[G] Easily testable on email corpuses**: make it easy to test the behavior of the library on sets of emails.
- **[H] General-purpose library**: the library should serve most email-related purposes, and not be excessively specialized for e.g. SMTP, IMAP or the composition of transactional emails over other use-cases.
- **[I] Functional idiomatic Rust**: idiomatic Rust code with a preference for functional over procedural style; be compatible with the existing library ecosystem. 

### Non-goals

- **[Z] Hardcore parsing optimization** at the cost of readability
- **[Y] Pipelining/streaming/buffering**: our AST contains references to the input buffer (to perform zero-copy parsing). It may be possible to implement streaming parsing or printing, but it would be a lot of work for unclear benefits (emails are typically small and easily fit in RAM).

## High-level Strategy

Given these goals, we describe the design principles that we follow. Also importantly, we describe *the compromises we have to make*, as some goals are difficult to perfectly satisfy at the same time.

### Design Principles

**Richly-typed internal AST**. Encode as much as possible the constraints from the RFC as part of the AST type definition.
*Implements goals [B].*

**Always recover**. No panic, no Result for the toplevel parsing function. We return the data we think we were able to extract from the input.
*Implements goals [A] and [C].*

**"In-depth parsing"**. We parse the full mail, up to the RFC atoms. We do not return sequences of bytes or unstructured data that are not parsed.
*Implements goal [B].*

**Use parser combinators** to build parsing functions from small composable pieces. Avoid hand-rolled parsing functions.
*Contributes to goals [E], [F] and [I].*

**Zero-copy parsing**. Avoid avoidable overhead when parsing; the AST references the input data but does not copy it.
*Contributes to goal [D].*


We try our best to follow these principles, but there is tension between some of the goals.
We now discuss some of these tensions in the design space, from a high-level perspective.

### Discussion

**Handling of obsolete, malformed or missing data.** 
Goals [A], [B] and [C] conflict when we consider the scenario of "parsing -> modification -> reprinting" (cf "Use-cases" above). Goals [A] and [C] mean that we should parse and recover information from emails containing obsolete, malformed or missing data. But goal [B] requires that reprinting such emails should use strictly RFC-compliant syntax and structure. This is not always possible!

1. In some happy cases it is possible to turn invalid syntax into valid syntax without loss of information. This satisfies all the goals. Examples: turning an invalid Content-Type `text` into the valid `text/plain`; removing a header field `To: ` whose empty body is invalid; turning the email address `"abc"."def"@example.com` (`obs-local-part` syntax) into `abc.def@example.com` or `"abc.def"@example.com` (`local-part` syntax).
2. In some cases, the only reasonable option is to discard malformed data. This satisfies goals [A] and [B], but loses some information, contradicting goal [C].
Example: RFC5322 specifies that there must not be more than one `Subject` header; if we receive an email with several `Subject` headers, we keep the first one and discard the others. 
Other example: as specified in RFC5322, headers like `Subject` may contain NULL characters (or other control characters), but these characters must not be used when printing an RFC-compliant email; we thus accept these characters during parsing but drop them afterwards. Similarly, in domain literals (the `xxx` in an email address `foo@[xxx]`), some characters are allowed when parsing but not in printing, and are thus discarded if they appear. 
3. Finally, there are some cases where there is no clear way of "repairing" invalid data. For instance, data that is required by the RFC may be missing (e.g. the `Date` header), or it may be important to keep but ill-formatted in a way that cannot be correctly reprinted. In those cases, we add exceptions and allow non-compliant syntax to be parsed and reprinted as-is, satisfying goal [A] and [C] but compromising on goal [B]. We document such exceptions in the next sub-section. 

**The Right(c) AST.**

**Security & Sanitization.**

**Missing world knowledge & use cases.**

## Arbitration and Compromises

