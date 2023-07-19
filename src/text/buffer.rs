#[derive(Debug, PartialEq, Default)]
pub struct Text<'a> {
    parts: Vec<&'a [u8]>,
}

impl<'a> Text<'a> {
    pub fn push(&mut self, e: &'a [u8]) {
        self.parts.push(e)
    }

    pub fn to_string(&self) -> String {
        let enc = encoding_rs::UTF_8;
        let size = self.parts.iter().fold(0, |acc, v| acc + v.len());

        self.parts.iter().fold(
            String::with_capacity(size),
            |mut acc, v| {
                let (content, _) = enc.decode_without_bom_handling(v);
                acc.push_str(content.as_ref());
                acc
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text() {
        let mut text = Text::default();
        text.push(b"hello");
        text.push(&[ascii::SP]);
        text.push(b"world");
        assert_eq!(
            text.to_string(),
            "hello world".to_string(),
        );
    }
}
