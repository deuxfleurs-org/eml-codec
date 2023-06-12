use imf_codec::headers;

fn main() {
    let header = "Date: Fri, 21 Nov 1997 09:55:06 -0600\r\nSubject: Hello\r\n World\r\nFrom: <quentin@deuxfleurs.fr>\r\n\r\nHello world";
    println!("{:?}", headers::header_section(header));
}
