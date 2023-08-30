use crate::header;
use crate::imf;
use crate::mime;

pub fn split_and_build<'a>(v: &Vec<header::Field<'a>>) -> (mime::NaiveMIME<'a>, imf::Imf<'a>) {
    let (mimev, imfv) = v.iter().fold(
        (
            Vec::<mime::field::Content>::new(),
            Vec::<imf::field::Field>::new(),
        ),
        |(mut mime, mut imf), f| {
            if let Ok(m) = mime::field::Content::try_from(f) {
                mime.push(m);
            } else if let Ok(i) = imf::field::Field::try_from(f) {
                imf.push(i);
            }
            (mime, imf)
        },
    );

    let fmime = mimev.into_iter().collect::<mime::NaiveMIME>();
    let fimf = imfv.into_iter().collect::<imf::Imf>();
    (fmime, fimf)
}
