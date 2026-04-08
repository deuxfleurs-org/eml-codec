# Testing on the synthetic [mirage/hamlet](https://github.com/mirage/hamlet) email corpus.

Run `./restore.sh` to download the corpus, then `./test.sh` to run the tests.
At the end of the test, the file `trace.json` should be empty.

## What is this testing?

We test that, on this corpus, our parser is able to interpret all the email
contents: it does not drop parts of the input or applies recovery strategies,
*with a number of exceptions*. These exceptions are captured in the `grep` line
of the `test.sh` script, which filters the output trace.

Exceptions: 
- the hamlet corpus was programatically generated, and some specific headers use
a much more unconstrained syntax than what the RFC allows (typically, they use
`unstructured` instead of the more precise RFC grammar). Our parser reads the
RFC grammar and thus drops these headers; we filter out the corresponding
headers from the trace.
- the corpus also does not obey the RFC restriction on content-encoding for
multiparts, which results in a recovery event on our end (`to_part_encoding:
invalid mechanism`); we also filter it out.

Outside of these exceptions, there should be no remaining trace events: this
shows that emails generated from the mirage email library
(https://github.com/mirage/mrmime) can be successfully parsed by eml-codec.
