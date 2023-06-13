use imf_codec::headers;

fn main() {
    let header = r#"Date: Fri, 21 Nov 1997 10:01:10 -0600
From: Mary Smith 
 <mary@example.net>
Sender: imf@example.com
Reply-To: "Mary Smith: Personal Account" <smith@home.example>
To: John Doe <jdoe@machine.example>
Subject: Re: Saying Hello
Message-ID: <3456@example.net>
In-Reply-To: <1234@local.machine.example>
References: <1234@local.machine.example>

This is a reply to your hello.
"#;

    println!("{:?}", headers::header_section(header));
}
