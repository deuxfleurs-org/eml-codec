use std::io::{Result, Write};
use crate::text::ascii;

// TODO: rename this file

pub trait Print {
    fn print(&self, fmt: &mut impl Formatter) -> Result<()>;
}

pub fn print_seq<T, Fmt, F>(fmt: &mut Fmt, s: &[T], sep: F) -> Result<()>
where
    T: Print,
    Fmt: Formatter,
    F: Fn(&mut Fmt) -> Result<()>
{
    if !s.is_empty() {
        s[0].print(fmt)?;
        for x in &s[1..] {
            sep(fmt)?;
            x.print(fmt)?;
        }
    }
    Ok(())
}

// Cow<'a, [u8]> is our base bytes type
impl<'a> Print for std::borrow::Cow<'a, [u8]> {
    fn print(&self, fmt: &mut impl Formatter) -> Result<()> {
        fmt.write_bytes(&self)
    }
}

/// An output formatter that can perform line folding.
///
/// - `write_fws` outputs folding white space which can be used for folding;
/// - `write_bytes` outputs text which cannot be used for folding;
/// - `write_crlf` outputs a line break.
///
/// Requirements for callers of a Formatter:
/// - A line *must never start* with whitespace. This includes both whitespace
///   written using `write_bytes` or `write_fws`.
/// - Text written with `write_bytes` *must never contain CRLF*. /!\ Successive
///   calls to `write_bytes` that result in a CRLF when concatenated are also
///   forbidden! /!\
///
/// Guarantees that a Formatter must provide:
/// - does not output "folds" that contain only folding whitespace;
/// - maximizes the length of folds within the line limit;
/// - keeps folds under the line limit, unless there is no space to fold on;
///   in that case, fold as soon as possible after the line limit.
///
/// Note that the line limit (if any) is determined by each Formatter
/// implementation.
pub trait Formatter {
    // XXX could we provide more safety to ensure that callers of a Formatter
    // obey the requirements above, instead of panicking or being silently
    // incorrect?

    /// Write bytes from `buf`; they cannot be used for line folding.
    /// It is fine for `buf` to include whitespace characters.
    fn write_bytes(&mut self, buf: &[u8]) -> Result<()>;

    /// Write whitespace bytes from `buf`; they can be used for line folding.
    /// `buf` *must only* contain whitespace characters ' ' and '\t'.
    fn write_fws_bytes(&mut self, buf: &[u8]) -> Result<()>;

    /// Terminate the current line, writing CRLF ("\r\n").
    fn write_crlf(&mut self) -> Result<()>;

    /// Write a single folding white space character.
    fn write_fws(&mut self) -> Result<()> {
        self.write_fws_bytes(b" ")
    }
}

/// Implementation of `Formatter` for any writer.
///
/// This implementation *does not* perform line folding, i.e. there is no
/// line limit.
impl<W: Write> Formatter for W {
    fn write_bytes(&mut self, buf: &[u8]) -> Result<()> {
        self.write_all(buf)
    }

    fn write_fws_bytes(&mut self, buf: &[u8]) -> Result<()> {
        self.write_all(buf)
    }

    fn write_crlf(&mut self) -> Result<()> {
        self.write_all(ascii::CRLF)
    }
}

/// `LineFolder` implements `Formatter` and performs line folding.
///
/// The line limit is 80 chars (including CRLF) as per RFC5322.
///
/// On top of `Formatter` methods, a user of `LineFolder` must call
/// its `flush` method after it is done writing. Flushing must only
/// happen after all writing has been done; once a `LineFolder` has
/// been flushed it cannot be written to again.
pub struct LineFolder<W: Write> {
    // Edge case: at the end of the file, if the remaining data of the final
    // fold is only spaces, we must not put it on its own fold (as per the RFC).
    // Instead, we should add it to the previous fold.
    // To account for that edge case, we buffer both the current and the
    // previous fold of the current line.
    prev_fold: Option<Vec<u8>>,
    // invariant: prev_fold.is_some() ==> !cur_fold.is_empty()
    cur_fold: Vec<u8>,
    cur_fold_is_only_fws: bool,
    last_cut_candidate: Option<usize>,
    // We only handle flushing once at the end. Once the LineFolder has been
    // flushed, attempting to write or flush will panic.
    is_flushed: bool,
    inner: W,
}

const LINE_LIMIT: usize = 78;

impl<W: Write> LineFolder<W> {
    pub fn new(inner: W) -> LineFolder<W> {
        Self {
            prev_fold: None,
            cur_fold: Vec::new(),
            cur_fold_is_only_fws: true,
            last_cut_candidate: None,
            is_flushed: false,
            inner,
        }
    }

    // NOTE: flushing is only allowed as the last operation on the LineFolder
    // XXX if flushing fails, calling it again will do nothing; data in buffers is lost.
    pub fn flush(&mut self) -> Result<()> {
        if self.is_flushed {
            return Ok(())
        }
        self.is_flushed = true;
        self.flush_line()?;
        self.inner.flush()
    }

}

impl<W: Write> Formatter for LineFolder<W> {
    // NOTE: `buf` must not contain line breaks (CRLF).
    // To output line breaks, use `write_crlf`.
    // XXX what are the guarantees in case the underlying writer fails?
    fn write_bytes(&mut self, buf: &[u8]) -> Result<()> {
        assert!(!self.is_flushed);

        // A line must never start with whitespace
        // (otherwise it would be indistinguishable from FWS)
        if self.cur_fold.is_empty() && !buf.is_empty() {
            // XXX turn this into a debug_assert?
            assert!(!ascii::WS.contains(&buf[0]))
        }

        if self.cur_fold.len() + buf.len() <= LINE_LIMIT
            || self.last_cut_candidate.is_none()
        {
            // write `buf`
            self.cur_fold.extend_from_slice(buf);
            if !buf.is_empty() {
                self.cur_fold_is_only_fws = false;
            }
            Ok(())
        } else {
            // fold at `last_cut_candidate`
            self.fold()?;
            // recursive call to actually handle `buf`
            self.write_bytes(buf)
        }
    }

    fn write_fws_bytes(&mut self, buf: &[u8]) -> Result<()> {
        assert!(!self.is_flushed);
        if buf.is_empty() {
            return Ok(())
        }

        // A line must never begin with whitespace.
        // XXX: turn this into debug_assert?
        assert!(!self.cur_fold.is_empty());

        // add buf[0] to `cur_fold`

        if !self.cur_fold_is_only_fws {
            self.last_cut_candidate = Some(self.cur_fold.len());
        }
        self.cur_fold.push(buf[0]);

        // if we are past the line limit, we should fold if we can
        // (possibly on the character we just added)
        if self.cur_fold.len() > LINE_LIMIT
            && self.last_cut_candidate.is_some()
        {
            self.fold()?;
        }

        // recursive call to handle the rest of the buffer
        self.write_fws_bytes(&buf[1..])
    }

    fn write_crlf(&mut self) -> Result<()> {
        assert!(!self.is_flushed);
        // flush the buffers for the current line
        self.flush_line()?;
        self.inner.write_all(ascii::CRLF)?;
        Ok(())
    }
}

impl<W: Write> Drop for LineFolder<W> {
    fn drop(&mut self) {
        let _r = self.flush();
    }
}

impl<W: Write> LineFolder<W> {
    // NOTE: requires `self.last_cut_candidate.is_some()`
    // folds at `last_cut_candidate`
    fn fold(&mut self) -> Result<()> {
        // flush any existing `prev_fold`
        if let Some(prev_fold) = &self.prev_fold {
            // commit `prev_fold` before we split
            self.inner.write_all(prev_fold)?;
            self.inner.write_all(ascii::CRLF)?;
            self.prev_fold = None;
        }
        let cut_pos = self.last_cut_candidate.unwrap();
        // cur_fold  = |aaaaaabbbb|
        //                    ^ cut_pos
        //   becomes
        // prev_fold = |aaaaaa|
        // cur_fold  = |bbbb|
        {
            let mut prev_fold = self.cur_fold.split_off(cut_pos);
            std::mem::swap(&mut self.cur_fold, &mut prev_fold);
            self.prev_fold = Some(prev_fold);
        }
        self.last_cut_candidate = None;
        // `cur_fold` is not FWS since it is after the
        // last cut candidate, and it is non-empty.
        self.cur_fold_is_only_fws = false;
        Ok(())
    }

    // terminate the current line, writing its data
    fn flush_line(&mut self) -> Result<()> {
        if let Some(prev_fold) = &self.prev_fold {
            self.inner.write_all(prev_fold)?;
            if self.cur_fold_is_only_fws {
                // edge case: write `cur_fold` on the same fold
                // as prev_fold to avoid creating a fold with only
                // spaces.
                ()
            } else {
                self.inner.write_all(ascii::CRLF)?;
            }
        }
        self.inner.write_all(&self.cur_fold)?;
        // reset fold state
        self.prev_fold = None;
        self.cur_fold.truncate(0);
        self.cur_fold_is_only_fws = true;
        self.last_cut_candidate = None;
        Ok(())
    }
}

pub fn with_line_folder<F: Fn(&mut LineFolder<&mut Vec<u8>>)>(f: F) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut folder = LineFolder::new(&mut buf);
        f(&mut folder);
        folder.flush().unwrap();
    }
    return buf
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn test_folding() {
        let folded = with_line_folder(|f| {
            // 72 chars
            f.write_bytes(b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx").unwrap();
            f.write_fws().unwrap();
            f.write_bytes(b"yyyyyyyyy").unwrap();
        });
        assert_eq!(folded, b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\r\n yyyyyyyyy");

        let folded = with_line_folder(|f| {
            // 80 chars
            f.write_bytes(b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx").unwrap();
            f.write_fws().unwrap();
            f.write_bytes(b"yyyyyyyyy").unwrap();
        });
        assert_eq!(folded, b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\r\n yyyyyyyyy");

        let folded = with_line_folder(|f| {
            f.write_bytes(b"xxxxxxxxxxxxxxxxx").unwrap();
            f.write_fws_bytes(b"   ").unwrap();
            f.write_bytes(b"xxxxxxxxxxxxxxxx").unwrap();
            f.write_fws().unwrap();
            f.write_bytes(b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx").unwrap();
            f.write_fws().unwrap();
            f.write_bytes(b"yyyyyyyyy").unwrap();
        });
        assert_eq!(folded, b"xxxxxxxxxxxxxxxxx   xxxxxxxxxxxxxxxx xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\r\n yyyyyyyyy");
    }
}
