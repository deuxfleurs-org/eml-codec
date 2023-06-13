use imf_codec::headers;

fn main() {
    let header = r#"Date: Fri, 21 Nov 1997 09:55:06 -0600
Subject: Hello
 World
From: <quentin@example.com>
Sender: imf@example.com

Hello world
"#;

    println!("{:?}", headers::header_section(header));
}
