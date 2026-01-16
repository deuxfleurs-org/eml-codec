use rand::{Rng, SeedableRng};
// Use a cryto secure RNG which is "portable" (we want the output of our tests
// to be stable across platforms). Chacha20 provides such a RNG.
use rand_chacha::ChaCha20Rng as RNG;
use crate::text::ascii;

// TODO: provide streaming printing

pub trait Print {
    fn print(&self, fmt: &mut impl Formatter);
}

pub fn print_seq<'a, T, Fmt>(fmt: &mut Fmt, s: &[T], sep: impl Fn(&mut Fmt))
where
    T: Print,
    Fmt: Formatter,
{
    if !s.is_empty() {
        s[0].print(fmt);
        for x in &s[1..] {
            sep(fmt);
            x.print(fmt);
        }
    }
}

impl<T: Print> Print for &T {
    fn print(&self, fmt: &mut impl Formatter) {
        (*self).print(fmt)
    }
}

/// An output formatter that can perform line folding and compute multipart
/// boundaries.
///
/// The `Formatter` API is (unfortunately) quite imperative and tricky to use.
/// At a high level, the trickiness comes from two aspects: formatter modes and
/// multipart boundaries.
///
/// ## Formatter modes
///
/// A `Formatter` can switch between two modes: a "line folding" mode
/// (for writing email headers) or a "direct" mode (for writing email bodies).
///
/// Initially, a newly created `Formatter` is in "direct" mode. Switching to
/// and out of "line folding" mode is done using the `begin_line_folding` and
/// `end_line_folding` functions.
///
/// Depending on the mode, some functions of the API cannot be called or come
/// with extra usage restrictions, including basic text-printing functions.
/// (See the per-function documentation for more details.)
///
/// ## Multipart boundaries
///
/// When in "direct" mode, a `Formatter` can generate and output multipart
/// boundaries. These are randomly generated to (probabilistically) ensure that
/// they do not clash with the rest of the output.
///
/// A boundary can be "registered" using `push_new_boundary`, then printed to
/// the output using `write_current_boundary`. Finally, the boundary should be
/// discarded when the corresponding multipart body ends, with `pop_boundary`.
///
/// Because multipart data can be nested, it is possible to have several
/// "active" boundaries at a given time. However, boundary-related functions
/// must be called in a way that is "well-bracketed": conceptually, a
/// `Formatter` maintains a stack of active boundaries, only the boundary on the
/// top of the stack can be written to the output, and `push_new_boundary` and
/// `pop_boundary` must be used in a well-bracketed fashion. (See the
/// per-function documentation for more details.)
///
/// ## Writing to the formatter
///
/// Data can be written to the output using the following functions:
/// - `write_fws_bytes` and `write_fws` output white space; in "line folding" mode,
///    it can be used for folding;
/// - `write_bytes` outputs text; in "line folding" mode, it cannot be used
///    for folding;
/// - `write_crlf` outputs a line break;
/// - `write_current_boundary` outputs the boundary at the top of the boundary stack.
///
/// All other functions of the API modify the internal state of the `Formatter` but
/// do not produce output.
///
/// **In "line folding" mode**, `write_` functions must obey additional requirements:
/// - A line *must never start* with whitespace. This includes both whitespace
///   written using `write_bytes` or `write_fws`.
/// - Text written with `write_bytes` *must never contain CRLF*. /!\ Successive
///   calls to `write_bytes` that result in a CRLF when concatenated are also
///   forbidden! /!\
///
/// In exchange, in line folding mode, a `Formatter` provides the following guarantees:
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

    /// Switches the `Formatter` mode to "line folding". The `Formatter`
    /// must be currently in "direct" mode.
    fn begin_line_folding(&mut self);

    /// Switches the `Formatter` mode to "direct". The `Formatter` must
    /// be currently in "line folding" mode.
    fn end_line_folding(&mut self);

    /// Registers a new boundary.
    /// This pushes the boundary on top of the internal "boundary stack".
    fn push_new_boundary(&mut self);

    /// Write the current declared boundary to the output (the one on top of the
    /// internal boundary stack). The `Formatter` can be either in "direct" or
    /// "line folding" mode.
    ///
    /// A boundary must have been registered previously.
    fn write_current_boundary(&mut self);

    /// Pop the current boundary from the top of the "boundary stack".
    fn pop_boundary(&mut self);

    /// Write bytes from `buf`; they cannot be used for line folding.
    ///
    /// In line folding mode, `buf` must not contain CRLF and consecutive calls
    /// to `write_bytes` must not result in CRLF being emitted in the output
    /// (e.g. `fmt.write_bytes(b"\r"); fmt.write_bytes(b"\n")`).
    ///
    /// It is fine for `buf` to include whitespace characters.
    fn write_bytes(&mut self, buf: &[u8]);

    /// Write whitespace bytes from `buf`. In "line folding" mode, they can be
    /// used for line folding.
    ///
    /// `buf` *must only* contain whitespace characters ' ' and '\t'.
    fn write_fws_bytes(&mut self, buf: &[u8]);

    /// Terminate the current line, writing CRLF ("\r\n").
    fn write_crlf(&mut self);

    /// Write a single folding white space character.
    fn write_fws(&mut self) {
        self.write_fws_bytes(b" ")
    }

    /// Consumes the `Formatter` and returns the data that was printed to it.
    fn flush(self) -> Vec<u8>;
}

enum FormatterMode {
    Direct,
    Folding(LineFolder),
}

/// `Fmt` implements `Formatter`.
pub struct Fmt {
    mode: FormatterMode,
    boundaries: Boundaries,
    buf: Vec<u8>,
}

impl Fmt {
    /// `seed` is used to seed the internal RNG which generates multipart
    /// boundaries. If set to `None`, the RNG is seeded using randomness from
    /// the operating system.
    pub fn new(seed: Option<u64>) -> Self {
        let rand =
            seed.map(RNG::seed_from_u64)
                .unwrap_or_else(RNG::from_os_rng);
        Self {
            mode: FormatterMode::Direct,
            boundaries: Boundaries::new(rand),
            buf: Vec::new(),
        }
    }
}

impl Formatter for Fmt {
    fn begin_line_folding(&mut self) {
        match self.mode {
            FormatterMode::Direct => {
                self.mode = FormatterMode::Folding(LineFolder::new())
            },
            FormatterMode::Folding(_) =>
                panic!("Formatter::begin_line_folding: already in folding mode")
        }
    }

    fn end_line_folding(&mut self) {
        match self.mode {
            FormatterMode::Folding(ref mut folder) => {
                folder.flush(&mut self.buf);
                self.mode = FormatterMode::Direct
            },
            FormatterMode::Direct => {
                panic!("Formatter::end_line_folding: not in folding mode")
            }
        }
    }

    fn push_new_boundary(&mut self) {
        self.boundaries.push_new_boundary()
    }

    fn write_current_boundary(&mut self) {
        let b = self.boundaries.current_boundary();
        // inline write_bytes to avoid cloning `b`
        match self.mode {
            FormatterMode::Direct =>
                self.buf.extend_from_slice(b),
            FormatterMode::Folding(ref mut folder) =>
                folder.write_bytes(b, &mut self.buf)
        }
    }

    fn pop_boundary(&mut self) {
        self.boundaries.pop_boundary()
    }

    fn write_bytes(&mut self, buf: &[u8]) {
        match self.mode {
            FormatterMode::Direct =>
                self.buf.extend_from_slice(buf),
            FormatterMode::Folding(ref mut folder) =>
                folder.write_bytes(buf, &mut self.buf)
        }
    }

    fn write_fws_bytes(&mut self, buf: &[u8]) {
        match self.mode {
            FormatterMode::Direct =>
                self.buf.extend_from_slice(buf),
            FormatterMode::Folding(ref mut folder) =>
                folder.write_fws_bytes(buf, &mut self.buf)
        }
    }

    fn write_crlf(&mut self) {
        match self.mode {
            FormatterMode::Direct =>
                self.buf.extend_from_slice(ascii::CRLF),
            FormatterMode::Folding(ref mut folder) =>
                folder.write_crlf(&mut self.buf)
        }
    }

    fn flush(mut self) -> Vec<u8> {
        self.boundaries.assert_empty();
        if let FormatterMode::Folding(mut folder) = self.mode {
            folder.flush(&mut self.buf)
        }
        self.buf
    }
}

/// `LineFolder` holds buffers and state used to perform line folding.
///
/// The line limit is 80 chars (including CRLF) as per RFC5322.
///
/// The owner of `LineFolder` MUST call its `flush` method after it is done
/// writing. Flushing must only happen after all writing has been done; once
/// a `LineFolder` has been flushed it cannot be written to again.
struct LineFolder {
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
}

const LINE_LIMIT: usize = 78;

impl LineFolder {
    fn new() -> LineFolder {
        Self {
            prev_fold: None,
            cur_fold: Vec::new(),
            cur_fold_is_only_fws: true,
            last_cut_candidate: None,
            is_flushed: false,
        }
    }

    // NOTE: flushing is only allowed as the last operation on the LineFolder
    // XXX if flushing fails, calling it again will do nothing; data in buffers is lost.
    fn flush(&mut self, inner: &mut Vec<u8>) {
        if self.is_flushed {
            return
        }
        self.is_flushed = true;
        self.flush_line(inner)
    }

    // NOTE: `buf` must not contain line breaks (CRLF).
    // To output line breaks, use `write_crlf`.
    // XXX what are the guarantees in case the underlying writer fails?
    fn write_bytes(&mut self, buf: &[u8], inner: &mut Vec<u8>) {
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
        } else {
            // fold at `last_cut_candidate`
            self.fold(inner);
            // recursive call to actually handle `buf`
            self.write_bytes(buf, inner)
        }
    }

    fn write_fws_bytes(&mut self, buf: &[u8], inner: &mut Vec<u8>) {
        assert!(!self.is_flushed);
        if buf.is_empty() {
            return
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
            self.fold(inner)
        }

        // recursive call to handle the rest of the buffer
        self.write_fws_bytes(&buf[1..], inner)
    }

    fn write_crlf(&mut self, inner: &mut Vec<u8>) {
        assert!(!self.is_flushed);
        // flush the buffers for the current line
        self.flush_line(inner);
        inner.extend_from_slice(ascii::CRLF)
    }

    // internal helpers

    // NOTE: requires `self.last_cut_candidate.is_some()`
    // folds at `last_cut_candidate`
    fn fold(&mut self, inner: &mut Vec<u8>) {
        // flush any existing `prev_fold`
        if let Some(prev_fold) = &self.prev_fold {
            // commit `prev_fold` before we split
            inner.extend_from_slice(prev_fold);
            inner.extend_from_slice(ascii::CRLF);
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
        self.cur_fold_is_only_fws = false
    }

    // terminate the current line, writing its data
    fn flush_line(&mut self, inner: &mut Vec<u8>) {
        if let Some(prev_fold) = &self.prev_fold {
            inner.extend_from_slice(prev_fold);
            if self.cur_fold_is_only_fws {
                // edge case: write `cur_fold` on the same fold
                // as prev_fold to avoid creating a fold with only
                // spaces.
                ()
            } else {
                inner.extend_from_slice(ascii::CRLF);
            }
        }
        inner.extend_from_slice(&self.cur_fold);
        // reset fold state
        self.prev_fold = None;
        self.cur_fold.truncate(0);
        self.cur_fold_is_only_fws = true;
        self.last_cut_candidate = None
    }
}

struct Boundaries {
    active_boundaries: Vec<Vec<u8>>, // behaves as a stack
    rand: RNG,
}

// TODO: check
const BOUNDARY_LEN: usize = 65;

impl Boundaries {
    fn new(rand: RNG) -> Self {
        Self {
            active_boundaries: Vec::new(),
            rand,
        }
    }

    fn push_new_boundary(&mut self) {
        let b = self.random_boundary();
        self.active_boundaries.push(b);
    }

    fn current_boundary(&self) -> &[u8] {
        self.active_boundaries.last().unwrap()
    }

    fn pop_boundary(&mut self) {
        self.active_boundaries.pop();
    }

    // generate a random boundary using characters in DIGIT | ALPHA
    fn random_boundary(&mut self) -> Vec<u8> {
        let mut v = Vec::with_capacity(BOUNDARY_LEN);
        for _ in 0..BOUNDARY_LEN {
            let n = self.rand.random_range(0..(10 + 26 + 26));
            let byte =
                if n < 10 {
                    ascii::N0 + n
                } else if n - 10 < 26 {
                    ascii::LCA + (n - 10)
                } else {
                    ascii::LSA + (n - 10 - 26)
                };
            v.push(byte)
        }
        v
    }

    fn assert_empty(&self) {
        assert!(self.active_boundaries.is_empty());
    }
}

pub fn with_formatter<F>(seed: Option<u64>, f: F) -> Vec<u8>
where
    F: for <'a> Fn(&'a mut Fmt)
{
    let mut fmt = Fmt::new(seed);
    f(&mut fmt);
    fmt.flush()
}

// Cow<'a, [u8]> is our base bytes type
impl<'a> Print for std::borrow::Cow<'a, [u8]> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(&self)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // in tests, fix the formatter seed
    pub fn with_formatter(f: impl Fn(&mut Fmt)) -> Vec<u8> {
        super::with_formatter(Some(0), f)
    }

    #[test]
    fn test_folding() {
        let folded = with_formatter(|f| {
            f.begin_line_folding();
            // 72 chars
            f.write_bytes(b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
            f.write_fws();
            f.write_bytes(b"yyyyyyyyy");
        });
        assert_eq!(folded, b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\r\n yyyyyyyyy");

        let folded = with_formatter(|f| {
            f.begin_line_folding();
            // 80 chars
            f.write_bytes(b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
            f.write_fws();
            f.write_bytes(b"yyyyyyyyy");
        });
        assert_eq!(folded, b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\r\n yyyyyyyyy");

        let folded = with_formatter(|f| {
            f.begin_line_folding();
            f.write_bytes(b"xxxxxxxxxxxxxxxxx");
            f.write_fws_bytes(b"   ");
            f.write_bytes(b"xxxxxxxxxxxxxxxx");
            f.write_fws();
            f.write_bytes(b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
            f.write_fws();
            f.write_bytes(b"yyyyyyyyy");
        });
        assert_eq!(folded, b"xxxxxxxxxxxxxxxxx   xxxxxxxxxxxxxxxx xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\r\n yyyyyyyyy");
    }
}
