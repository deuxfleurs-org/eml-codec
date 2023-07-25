pub fn parsed<'a>(input: &'a [u8], rest: &'a [u8]) -> &'a [u8] {
    let start = input.as_ptr();
    let offset = rest.as_ptr();
    let idx = (offset as usize - start as usize) / std::mem::size_of::<u8>();
    assert!(idx <= input.len());
    &input[..idx]
}

pub fn rest<'a>(input: &'a [u8], parsed: &'a [u8]) -> &'a [u8] {
    let start = input.as_ptr();
    let offset = (&parsed[parsed.len()..]).as_ptr();
    let idx = (offset as usize - start as usize) / std::mem::size_of::<u8>();
    assert!(idx <= input.len());
    &input[idx..]
}

pub fn with_preamble<'a>(input: &'a [u8], parsed: &'a [u8]) -> &'a [u8] {
    let start = input.as_ptr();
    let offset = (&parsed[parsed.len()..]).as_ptr();
    let idx = (offset as usize - start as usize) / std::mem::size_of::<u8>();
    assert!(idx <= input.len());
    &input[..idx]
}

pub fn with_epilogue<'a>(input: &'a [u8], rest: &'a [u8]) -> &'a [u8] {
    let start = input.as_ptr();
    let offset = rest.as_ptr();
    let idx = (offset as usize - start as usize) / std::mem::size_of::<u8>();
    assert!(idx <= input.len());
    &input[idx..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all() {
        let outer = b"aa bb cc";
        let inner = &outer[3..5];
        assert_eq!(inner, b"bb");

        let p = parsed(outer, inner);
        assert_eq!(p, b"aa ");

        let r = rest(outer, inner);
        assert_eq!(r, b" cc");

        let wp = with_preamble(outer, inner);
        assert_eq!(wp, b"aa bb");

        let we = with_epilogue(outer, inner);
        assert_eq!(we, b"bb cc");
    }
}
