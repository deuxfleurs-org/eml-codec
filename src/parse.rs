use imf_codec::fragments::section::Section;
use imf_codec::multipass::segment;
use std::io;
use std::io::Read;

fn parser<'a, F>(input: &'a [u8], func: F) -> ()
where
    F: FnOnce(&Section) -> (),
{
    let seg = segment::new(input).unwrap();
    let charset = seg.charset();
    let fields = charset.fields().unwrap();
    let field_names = fields.names();
    let field_body = field_names.body();
    let section = field_body.section();

    func(&section.fields);
}

fn main() {
    // Read full mail in memory
    let mut rawmail = Vec::new();
    io::stdin().lock().read_to_end(&mut rawmail).unwrap();

    // Parse it
    parser(&rawmail[..], |section| {
        // Checks/debug
        println!("{:?}", section);
        assert!(section.date.is_some());
        assert!(section.from.len() > 0);
        assert!(section.bad_fields.len() == 0);
    });
}
