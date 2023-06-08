mod headers;
mod model;
mod abnf;

fn main() {
    let header = "Subject: Hello\r\n World\r\nFrom: <quentin@deuxfleurs.fr>\r\n\r\n";
    println!("{:?}", headers::header_section(header));
}
