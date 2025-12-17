use crate::header;
use crate::imf;
use crate::mime;

pub fn split_and_build<'a>(v: &Vec<header::FieldRaw<'a>>) -> (mime::NaiveMIME<'a>, imf::Imf<'a>) {
    let (mimev, imfv, otherv) = v.iter().fold(
        (
            Vec::<mime::field::Content>::new(),
            Vec::<imf::field::Field>::new(),
            Vec::<header::FieldRaw<'a>>::new(),
        ),
        |(mut mime, mut imf, mut other), f| {
            if let Ok(m) = mime::field::Content::try_from(f) {
                mime.push(m);
            } else if let Ok(i) = imf::field::Field::try_from(f) {
                imf.push(i);
            } else {
                other.push(f.clone().into())
            }
            (mime, imf, other)
        },
    );

    let mut fmime = mimev.into_iter().collect::<mime::NaiveMIME>();
    let fimf = imfv.into_iter().collect::<imf::Imf>();
    let uninterp_headers = otherv
        .into_iter()
        .filter_map(header::Unstructured::from_raw)
        .collect();
    fmime.fields.uninterp_headers = uninterp_headers;
    (fmime, fimf)
}
