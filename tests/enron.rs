use std::path::PathBuf;
use std::fs::File;
use std::io::Read;
use imf_codec::header;
use walkdir::WalkDir;


#[test]
#[ignore]
fn test_enron500k() {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("resources/enron/maildir/");

    let known_bad_fields = [
        "maildir/white-s/calendar/113.", // To: east <7..>
                                         
        "maildir/skilling-j/inbox/223.", // From: pep <performance.>
                                         
        "maildir/jones-t/all_documents/9806.", // To: <"tibor.vizkelety":@enron.com>
        "maildir/jones-t/notes_inbox/3303.", // To: <"tibor.vizkelety":@enron.com>
                                             
        "maildir/lokey-t/calendar/33.", // A second Date entry for the calendar containing
                                        // Date:       Monday, March 12
                                        
        "maildir/zipper-a/inbox/199.", // To: e-mail <mari.>

        "maildir/dasovich-j/deleted_items/128.", // To: f62489 <g>
        "maildir/dasovich-j/all_documents/677.", // To: w/assts <govt.>
        "maildir/dasovich-j/all_documents/8984.", // To: <"ft.com.users":@enron.com>
        "maildir/dasovich-j/all_documents/3514.", // To: <"ft.com.users":@enron.com>
        "maildir/dasovich-j/all_documents/4467.", // To: <"ft.com.users":@enron.com>
        "maildir/dasovich-j/all_documents/578.", // To: w/assts <govt.>
        "maildir/dasovich-j/all_documents/3148.", // To: <"economist.com.readers":@enron.com>
        "maildir/dasovich-j/all_documents/9953.", // To: <"economist.com.reader":@enron.com>
        "maildir/dasovich-j/risk_analytics/3.", // To: w/assts <govt.>
        "maildir/dasovich-j/notes_inbox/5391.", // To: <"ft.com.users":@enron.com>
        "maildir/dasovich-j/notes_inbox/4952.", // To: <"economist.com.reader":@enron.com>
        "maildir/dasovich-j/notes_inbox/2386.", // To: <"ft.com.users":@enron.com>
        "maildir/dasovich-j/notes_inbox/1706.", // To: <"ft.com.users":@enron.com>
        "maildir/dasovich-j/notes_inbox/1489.", // To: <"economist.com.readers":@enron.com>
        "maildir/dasovich-j/notes_inbox/5.", // To: w/assts <govt.>
    ];

    let known_bad_from = [
        "maildir/skilling-j/inbox/223.", // From: pep <performance.>
    ];

    let mut i = 0;
    for entry in WalkDir::new(d.as_path()).into_iter().filter_map(|file| file.ok()) {
        if entry.metadata().unwrap().is_file() {
            //@TODO check list

            // read file
            let mut raw = Vec::new();
            let mut f = File::open(entry.path()).unwrap();
            f.read_to_end(&mut raw).unwrap();

            // parse
            let (email, encoding, malformed) = header::from_bytes(&raw);
            //println!("Encoding: {:?}, Malformed: {:?}", encoding, malformed);

            let (input, hdrs) = header::section(&email).unwrap();
            //println!("{:?}", hdrs);
            let ok_date = hdrs.date.is_some();
            let ok_from = hdrs.from.len() > 0;
            let ok_fields = hdrs.bad_fields.len() == 0;

            let p = entry.path();
            if !ok_date || !ok_from || !ok_fields {
                println!("Issue with: {}", p.display());
            }

            assert!(ok_date);

            if !known_bad_from.iter().any(|&s| p.ends_with(s)) {
                assert!(ok_from);
            }

            if !known_bad_fields.iter().any(|&s| p.ends_with(s)) {
                assert!(ok_fields);
            }

            i += 1;
            if i % 1000 == 0 {
                println!("Analyzed emails: {}", i);
            }
        } 
    }
}
