use std::collections::{HashMap, HashSet};
use std::fs::{read, read_to_string};
use std::sync::{Mutex, Arc};
use std::path::PathBuf;

use rouille::Response;
use glob::glob;

#[derive(Clone)]
enum FileData {
    Text(String),
    Bin(Vec<u8>)
}

impl FileData {
    fn text(self) -> String {
        match self {
            Self::Text(value) => value,
            Self::Bin(_) => "ERROR. FILE IS BINARY.".to_string()
        }
    }

    fn bin(self) -> Vec<u8> {
        match self {
            Self::Bin(value) => value,
            Self::Text(value) => value.as_bytes().to_vec(),
        }
    }
}

#[derive(Default)]
struct AssetsCollection {
    files: HashMap<String, FileData>,
}

impl AssetsCollection {
    pub fn get_cloned(&self, path: &str) -> Option<FileData> {
        self.files.get(path).cloned()
    }
}

type SharedAssetsCollection = Arc<Mutex<AssetsCollection>>;
const ASSETS_EXTENSIONS_TO_LOAD: &[&str] = &["html", "css", "svg", "js", "wasm", "ttf", "wgsl", "txt"];

fn preload_files() -> SharedAssetsCollection {
    let mut collection = AssetsCollection::default();

    for ext in ASSETS_EXTENSIONS_TO_LOAD {
        let pattern = format!("./build/*.{}", ext);
        if let Ok(paths) = glob(&pattern) {
            for entry in paths.flatten() {
                if entry.is_file() {
                    let key = entry.to_string_lossy().to_string().replace('\\', "/");
                    let file_type = match *ext {
                        "wasm" | "ttf" => FileData::Bin(read(&entry).unwrap_or_default()),
                        _ => FileData::Text(read_to_string(&entry).unwrap_or_default()),
                    };
                    collection.files.insert(key, file_type);
                }
            }
        }
    }

    Arc::new(Mutex::new(collection))
}

fn watch_files(assets: &SharedAssetsCollection) {
    use std::sync::mpsc;
    use std::path::Path;
    use notify::{Event, Error, Config, RecommendedWatcher, RecursiveMode, Watcher};

    const WAIT: ::std::time::Duration = ::std::time::Duration::from_millis(200);

    fn must_reload_file<'a>(event_result: &'a Result<Event, Error>) -> Option<&'a PathBuf> {
        let event = match event_result {
            Ok(event) => event,
            Err(_) => { return None; } 
        };
        
        if !matches!(event.kind, notify::EventKind::Modify(_)) {
            return None;
        }

        if event.paths.len() == 0 {
            return None;
        }

        let path = event.paths.first().unwrap();
        if let Some(ext) = path.extension().and_then(|ext| ext.to_str() ) {
            if !ASSETS_EXTENSIONS_TO_LOAD.contains(&ext) {
                return None;
            }
        }

        Some(path)
    }

    let assets_guard = Arc::clone(assets);
    ::std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();

        let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();
        let build_directory = "./build";

        watcher.watch(Path::new(build_directory), RecursiveMode::Recursive).unwrap();

        let mut dedup: HashSet<PathBuf> = HashSet::default();

        loop {
            while let Ok(event_result) = rx.recv_timeout(WAIT) {
                if let Some(path) = must_reload_file(&event_result) {

                    dedup.insert(path.clone());
                }
            }

            if dedup.len() > 0 {
                let mut assets_mut = assets_guard.lock().unwrap();
                for path in dedup.iter() {
                    let local_path = format!("build{}", path.to_string_lossy().split_once(build_directory).unwrap_or_default().1);
                    let local_path = local_path.replace('\\', "/");
                    if !assets_mut.files.contains_key(&local_path) {
                        continue;
                    }

                    let ext = path.extension().and_then(|ext| ext.to_str()).unwrap();
                    let file_type = match ext {
                        "wasm" | "ttf" => {
                            FileData::Bin(std::fs::read(&path).unwrap_or_default())
                        },
                        _ => {
                            FileData::Text(read_to_string(&path).unwrap_or_default())
                        },
                    };

                    assets_mut.files.insert(local_path, file_type);
                }
            }

            dedup.clear();
        }
    });
}

fn response_from_url(url: &str, data: FileData) -> Response {
    let path_extension = ::std::path::Path::new(url).extension().and_then(|ext| ext.to_str() ).unwrap_or("");
    match path_extension {
        "" | "html" => Response::html(data.text()),
        "svg"       => Response::svg(data.text()),
        "txt"       => Response::text(data.text()),
        "css"       => Response::from_data("text/css; charset=utf-8", data.bin()),
        "js"        => Response::from_data("text/javascript; charset=utf-8", data.bin()),
        "wgsl"      => Response::from_data("text/wgsl; charset=utf-8", data.bin()),
        "wasm"      => Response::from_data("application/wasm", data.bin()),
        "png"       => Response::from_data("image/png", data.bin()),
        _           => Response::from_data("application/octet-stream", data.bin())
    }
}

fn get_asset(assets: &SharedAssetsCollection, url: &str) -> Option<FileData> {
    if url == "/" {
        assets.lock().unwrap().get_cloned("build/index.html")
    } else {
        let local_path = format!("build{url}");
        assets.lock().unwrap().get_cloned(&local_path)
    }
}


fn main() {
    let assets = preload_files();

    watch_files(&assets);

    println!("Starting server on localhost:8001");

    rouille::start_server("localhost:8001", move |request| {
        match request.method() {
            "GET" => {
                let url = request.url();
                match get_asset(&assets, &url) {
                    Some(data) => response_from_url(&url, data),
                    None => Response::empty_404()
                }
            },

            _ => Response::empty_204()
        }
    });
}
