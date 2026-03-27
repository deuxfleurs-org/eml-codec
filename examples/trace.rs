use rayon::prelude::*;

use tracing::{Event, Id, Subscriber};
use tracing::span::{Attributes, Record};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing::{Level, span};

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Write, Read};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FieldVisitor {
    fields: Vec<(&'static str, String)>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.push((field.name(), format!("{:?}", value)));
    }
}

struct SpanData {
    name: String,
    fields: Vec<(&'static str, String)>,
}

struct PruningLayer {
    spans: Mutex<HashMap<Id, SpanData>>,
    relevant_spans: Mutex<HashSet<Id>>,
    file: Arc<Mutex<Box<dyn Write + Send>>>,
}

struct PruningGuard {
    file: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl Drop for PruningGuard {
    fn drop(&mut self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

impl PruningLayer {
    fn new(path: Option<&str>) -> (Self, PruningGuard) {
        let writer = match path {
            Some(p) => Box::new(File::create(p).unwrap()) as Box<dyn Write + Send>,
            None => Box::new(std::io::stdout()) as Box<dyn Write + Send>,
        };
        let file = Arc::new(Mutex::new(writer));
        let layer = Self {
            spans: Mutex::new(HashMap::new()),
            relevant_spans: Mutex::new(HashSet::new()),
            file: file.clone(),
        };
        let guard = PruningGuard { file };
        (layer, guard)
    }
}

impl<S> Layer<S> for PruningLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, _ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        attrs.record(&mut visitor);
        let mut spans = self.spans.lock().unwrap();
        spans.insert(id.clone(), SpanData {
            name: attrs.metadata().name().to_string(),
            fields: visitor.fields,
        });
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, _ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        values.record(&mut visitor);

        let mut spans = self.spans.lock().unwrap();
        if let Some(span) = spans.get_mut(id) {
            span.fields.extend(visitor.fields);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        // let message = visitor.fields.iter().find_map(|(k, v)| if *k == "message" { Some(v) } else { None });
        // eprintln!("{} \t {}",
        //           event.metadata().name().to_string(),
        //           match message { Some(msg) => &format!(" {}", msg), None => "" },
        // );

        // collect event ancestors
        let chain: Vec<_> = match ctx.event_scope(event) {
            Some(scope) => scope.from_root().map(|s| s.id()).collect(),
            None => Vec::new(),
        };

        // mark ancestor spans as used
        {
            let mut relevant_spans = self.relevant_spans.lock().unwrap();
            relevant_spans.extend(chain.iter().cloned());
        }

        let mut file = self.file.lock().unwrap();
        let spans = self.spans.lock().unwrap();

        let mut e_spans = Vec::new();
        for span_id in &chain {
            let span = spans.get(span_id).unwrap();
            e_spans.push(LogSpan {
                span: span.name.clone(),
                meta: span.fields.clone().into_iter().collect(),
            })
        }
        serde_json::to_writer(&mut *file, &LogEvent {
            trace: e_spans,
            event: event.metadata().name().to_string(),
            meta: visitor.fields.into_iter().collect(),
        }).unwrap();
        file.write_all(b"\n").unwrap();
    }

    fn on_close(&self, id: Id, _ctx: Context<'_, S>) {
        let mut spans = self.spans.lock().unwrap();
        let relevant_spans = self.relevant_spans.lock().unwrap();
        // discard span if it contains no events
        if !relevant_spans.contains(&id) {
            spans.remove(&id);
        }
    }
}

#[derive(serde::Serialize)]
struct LogEvent {
    event: String,
    #[serde(flatten)]
    meta: HashMap<&'static str, String>,
    trace: Vec<LogSpan>,
}

#[derive(serde::Serialize)]
struct LogSpan {
    span: String,
    #[serde(flatten)]
    meta: HashMap<&'static str, String>,
}

fn parse_mbox(input: &[u8]) -> Vec<Vec<u8>> {
    let mut res = Vec::new();
    let mut cur: Option<Vec<u8>> = None;
    for line in input.split(|b| *b == b'\n') {
        if line.starts_with(b"From ") {
            if let Some(cur) = cur {
                res.push(cur)
            }
            cur = None
        } else {
            if let Some(ref mut cur) = cur {
                cur.extend(line);
                cur.push(b'\n');
            } else {
                let mut line = line.to_vec();
                line.extend(b"\n");
                cur = Some(line)
            }
        }
    }
    if let Some(cur) = cur {
        res.push(cur)
    }
    res
}

fn dir_entries(path: &std::path::Path, v: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry: std::fs::DirEntry = entry?;
            let path = entry.path();
            if path.is_dir() {
                dir_entries(&path, v)?;
            } else {
                v.push(path);
            }
        }
    }
    Ok(())
}

fn main() {
    let (layer, _guard) = PruningLayer::new(None);
    tracing_subscriber::registry()
        .with(layer)
        .init();

    for path in std::env::args().skip(1) {
        let attr = std::fs::metadata(&path).expect(&format!("error reading {}", path));
        if attr.is_dir() {
            let mut entries = Vec::new();
            dir_entries(&std::path::PathBuf::from(path.clone()), &mut entries)
                .expect(&format!("failed listing files in {}", path));
            entries.par_iter().for_each(|path| {
                let span = span!(Level::TRACE, "file email", path = format!("{}", path.display()));
                let _enter = span.enter();
                eprintln!("parsing email {}", path.display());
                let mut input = Vec::new();
                File::open(&path).unwrap().read_to_end(&mut input).unwrap();
                let _eml = eml_codec::parse_message(&input);
            });
        } else {

            if path.ends_with(".mbox") {
                let span = span!(Level::TRACE, "mailbox", path);
                let _enter = span.enter();
                eprintln!("parsing mailbox: {}", path);
                let mut input = Vec::new();
                File::open(&path).unwrap().read_to_end(&mut input).unwrap();
                let raw_emails = parse_mbox(&input);
                eprintln!("{} emails found", raw_emails.len());

                raw_emails.par_iter().enumerate().for_each(|(idx, raw_email)| {
                    let span = span!(Level::TRACE, "mailbox email", idx);
                    let _enter = span.enter();
                    eprintln!("parsing mbox email {}", idx);
                    let _eml = eml_codec::parse_message(&raw_email);
                })
            } else if path.ends_with(".zip") {
                let span = span!(Level::TRACE, "zip", path);
                let _enter = span.enter();
                eprintln!("parsing zip file: {}", path);
                let archive = zip::ZipArchive::new(File::open(&path).unwrap()).unwrap();
                let nb_items = archive.len();
                let archive_lck = Mutex::new(archive);
                (0..nb_items).into_par_iter().for_each(|i| {
                    let mut input = Vec::new();
                    #[allow(unused_assignments)]
                    let mut path = None;
                    {
                        let mut archive = archive_lck.lock().unwrap();
                        let mut file = archive.by_index(i).unwrap();
                        eprintln!("parsing email {}", file.name());
                        path = Some(file.name().to_string());
                        file.read_to_end(&mut input).unwrap();
                    }
                    let span = span!(Level::TRACE, "zip email", path = path.unwrap());
                    let _enter = span.enter();
                    let _eml = eml_codec::parse_message(&input);
                })
            } else {
                let span = span!(Level::TRACE, "eml", path);
                let _enter = span.enter();
                eprintln!("parsing single email: {}", path);
                let mut input = Vec::new();
                File::open(&path).unwrap().read_to_end(&mut input).unwrap();
                let _eml = eml_codec::parse_message(&input);
            }
        }
    }
}
