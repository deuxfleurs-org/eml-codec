#![no_main]
use eml_codec::text::misc_token::Phrase;
use libfuzzer_sys::fuzz_target;
use pretty_assertions::Comparison;

fuzz_target!(|phrase_words: Vec<&str>| {
    // Contract of `Phrase::from`, see its documentation.
    let s1 = Phrase::from(phrase_words.as_ref()).data().join(" ");
    let s2 = phrase_words.join(" ");
    if s1 != s2 {
        eprintln!("{}", Comparison::new(&s1, &s2));
        panic!()
    }
});
