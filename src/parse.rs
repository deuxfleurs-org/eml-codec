//use imf_codec::header;

fn main() {
    let hdr = r#"Return-Path: <gitlab@framasoft.org>
Delivered-To: quentin@dufour.io
Received: from smtp.deuxfleurs.fr ([10.83.2.2])
	by doradille with LMTP
	id IKPyOvS8iGTxBAEAvTd7DQ
	(envelope-from <gitlab@framasoft.org>)
	for <quentin@dufour.io>; Tue, 13 Jun 2023 19:01:08 +0000
Date: Fri, 21 Nov 1997 10:01:10 -0600
From: Mary Smith 
 <mary@example.net>
Sender: imf@example.com
Reply-To: "Mary Smith: Personal Account" <smith@home.example>
To: John Doe <jdoe@machine.example>
Cc: imf2@example.com
Bcc: (hidden)
Subject: Re: Saying Hello
Comments: A simple message
Comments: Not that complicated
comments : not valid but should be accepted
    by the parser.
Keywords: hello, world
Héron: Raté
 Raté raté
raté raté
Keywords: salut, le, monde
Message-ID: <3456@example.net>
In-Reply-To: <1234@local.machine.example>
References: <1234@local.machine.example>

This is a reply to your hello.
"#;

    //println!("{:?}", header::section(hdr));
}
