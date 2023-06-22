use imf_codec::fragments::section;
use imf_codec::multipass;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use walkdir::WalkDir;

fn parser<'a, F>(input: &'a [u8], func: F) -> ()
where
    F: FnOnce(&section::Section) -> (),
{
    let seg = multipass::segment::new(input).unwrap();
    let charset = seg.charset();
    let fields = charset.fields().unwrap();
    let field_names = fields.names();
    let field_body = field_names.body();
    let section = field_body.section();

    func(&section.fields);
}

#[test]
#[ignore]
fn test_enron500k() {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("resources/enron/maildir/");
    let prefix_sz = d.as_path().to_str().unwrap().len();
    //d.push("williams-w3/");

    let known_bad_fields = HashSet::from([
        "white-s/calendar/113.",       // To: east <7..>
        "skilling-j/inbox/223.",       // From: pep <performance.>
        "jones-t/all_documents/9806.", // To: <"tibor.vizkelety":@enron.com>
        "jones-t/notes_inbox/3303.",   // To: <"tibor.vizkelety":@enron.com>
        "lokey-t/calendar/33.",        // A second Date entry for the calendar containing
        // Date:       Monday, March 12
        "zipper-a/inbox/199.",                       // To: e-mail <mari.>
        "dasovich-j/deleted_items/128.",             // To: f62489 <g>
        "dasovich-j/all_documents/677.",             // To: w/assts <govt.>
        "dasovich-j/all_documents/8984.",            // To: <"ft.com.users":@enron.com>
        "dasovich-j/all_documents/3514.",            // To: <"ft.com.users":@enron.com>
        "dasovich-j/all_documents/4467.",            // To: <"ft.com.users":@enron.com>
        "dasovich-j/all_documents/578.",             // To: w/assts <govt.>
        "dasovich-j/all_documents/3148.",            // To: <"economist.com.readers":@enron.com>
        "dasovich-j/all_documents/9953.",            // To: <"economist.com.reader":@enron.com>
        "dasovich-j/risk_analytics/3.",              // To: w/assts <govt.>
        "dasovich-j/notes_inbox/5391.",              // To: <"ft.com.users":@enron.com>
        "dasovich-j/notes_inbox/4952.",              // To: <"economist.com.reader":@enron.com>
        "dasovich-j/notes_inbox/2386.",              // To: <"ft.com.users":@enron.com>
        "dasovich-j/notes_inbox/1706.",              // To: <"ft.com.users":@enron.com>
        "dasovich-j/notes_inbox/1489.",              // To: <"economist.com.readers":@enron.com>
        "dasovich-j/notes_inbox/5.",                 // To: w/assts <govt.>
        "kaminski-v/sites/19.",                      // To: <"the.desk":@enron.com>
        "kaminski-v/sites/1.",                       // To: <"the.desk":@enron.com>
        "kaminski-v/discussion_threads/5082.",       // To: <"ft.com.users":@enron.com>
        "kaminski-v/discussion_threads/4046.",       // To: <"the.desk":@enron.com>
        "kaminski-v/discussion_threads/4187.",       // To: <"the.desk":@enron.com>
        "kaminski-v/discussion_threads/8068.", // To: cats <breaktkhrough.>, risk <breakthrough.>, leaders <breaktkhrough.>
        "kaminski-v/discussion_threads/7980.", // To: dogs <breakthrough.>, cats <breaktkhrough.>, risk <breakthrough.>,\r\n\tleaders <breaktkhrough.>
        "kaminski-v/all_documents/5970.", //To: dogs <breakthrough.>, cats <breaktkhrough.>, risk <breakthrough.>,\r\n\tleaders <breaktkhrough.>
        "kaminski-v/all_documents/5838.", // To + Cc: dogs <breakthrough.>, breakthrough.adm@enron.com, breakthrough.adm@enron.com,\r\n\tbreakthrough.adm@enron.com
        "kaminski-v/all_documents/10070.", // To: <"ft.com.users":@enron.com>
        "kaminski-v/all_documents/92.",   // To: <"the.desk":@enron.com>
        "kaminski-v/all_documents/276.",  // To: <"the.desk":@enron.com>
        "kaminski-v/technical/1.",        // To: <"the.desk":@enron.com>
        "kaminski-v/technical/7.",        // To: <"the.desk":@enron.com>
        "kaminski-v/notes_inbox/140.", // To: dogs <breakthrough.>, cats <breaktkhrough.>, risk <breakthrough.>,\r\n\tleaders <breaktkhrough.>
        "kaminski-v/notes_inbox/95.", // To + CC failed: cats <breaktkhrough.>, risk <breakthrough.>, leaders <breaktkhrough.>
        "kean-s/archiving/untitled/1232.", // To: w/assts <govt.>, mark.palmer@enron.com, karen.denne@enron.com
        "kean-s/archiving/untitled/1688.", // To: w/assts <govt.>
        "kean-s/sent/198.", // To: w/assts <govt.>, mark.palmer@enron.com, karen.denne@enron.com
        "kean-s/reg_risk/9.", // To: w/assts <govt.>
        "kean-s/discussion_threads/950.", // To: w/assts <govt.>, mark.palmer@enron.com, karen.denne@enron.com
        "kean-s/discussion_threads/577.", // To: w/assts <govt.>
        "kean-s/calendar/untitled/1096.", // To: w/assts <govt.>, mark.palmer@enron.com, karen.denne@enron.com
        "kean-s/calendar/untitled/640.",  // To: w/assts <govt.>
        "kean-s/all_documents/640.",      // To: w/assts <govt.>
        "kean-s/all_documents/1095.",     // To: w/assts <govt.>
        "kean-s/attachments/2030.",       // To: w/assts <govt.>
        "williams-w3/operations_committee_isas/10.", // To: z34655 <m>
    ]);

    let known_bad_from = HashSet::from([
        "skilling-j/inbox/223.", // From: pep <performance.>
    ]);

    let mut i = 0;
    for entry in WalkDir::new(d.as_path())
        .into_iter()
        .filter_map(|file| file.ok())
    {
        if entry.metadata().unwrap().is_file() {
            let mail_path = entry.path();
            let suffix = &mail_path.to_str().unwrap()[prefix_sz..];

            // read file
            let mut raw = Vec::new();
            let mut f = File::open(mail_path).unwrap();
            f.read_to_end(&mut raw).unwrap();

            // parse
            parser(&raw, |hdrs| {
                let ok_date = hdrs.date.is_some();
                let ok_from = hdrs.from.len() > 0;
                let ok_fields = hdrs.bad_fields.len() == 0;

                if !ok_date || !ok_from || !ok_fields {
                    println!("Issue with: {}", suffix);
                }

                assert!(ok_date);

                if !known_bad_from.contains(suffix) {
                    assert!(ok_from);
                }

                if !known_bad_fields.contains(suffix) {
                    assert!(ok_fields);
                }

                i += 1;
                if i % 1000 == 0 {
                    println!("Analyzed emails: {}", i);
                }
            })
        }
    }
}
