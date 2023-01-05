use axum::body::Bytes;
use sha2::Digest;
use std::{
    collections::HashMap,
    fs::{File, FileType},
    io::{BufReader, Read},
    path::Path,
};

#[derive(Default)]
pub struct StaticFileRegistry {
    by_key: HashMap<String, String>,
    files: HashMap<String, (Bytes, &'static str)>,
}

fn to_hash_key(bytes: &[u8]) -> String {
    let mut s = "v0-".to_owned();
    for byte in bytes {
        s += &format!("{:02x}", byte);
    }
    s
}

impl StaticFileRegistry {
    pub fn register_files<P: AsRef<Path>>(&mut self, root: P) -> Result<(), std::io::Error> {
        for file in std::fs::read_dir(root.as_ref())? {
            let file = file?;
            if file.file_type()?.is_file() {
                let file = file.file_name();
                let name = Path::new(&file);
                let ext = name.extension().unwrap_or_default().to_string_lossy();
                self.register(&file.to_string_lossy(), &ext, root.as_ref().join(name))?;
            }
        }
        Ok(())
    }

    pub fn register<P: AsRef<Path>>(
        &mut self,
        key: &str,
        extension: &str,
        file: P,
    ) -> Result<(), std::io::Error> {
        let mut reader = BufReader::new(File::open(&file)?);
        let mut buf = Vec::with_capacity(1024);
        reader.read_to_end(&mut buf)?;
        let mime_type = match extension {
            "txt" => "text/plain",
            "css" => "text/css",
            _ => infer::get(&buf)
                .expect(&format!(
                    "File type was not known for {}",
                    file.as_ref().to_string_lossy()
                ))
                .mime_type(),
        };

        let mut hash = sha2::Sha256::new();
        hash.update(&buf);
        let hash: &[u8] = &hash.finalize();

        self.files.insert(
            to_hash_key(hash) + "." + extension,
            (Bytes::from(buf), mime_type),
        );
        self.by_key
            .insert(key.to_owned(), to_hash_key(hash) + "." + extension);

        tracing::info!(
            "Registered '{}' with extension '{}', mime type '{}', and hash '{}'",
            key,
            extension,
            mime_type,
            to_hash_key(hash)
        );

        Ok(())
    }

    pub fn lookup_key(&self, key: &str) -> Option<&str> {
        self.by_key.get(key).map(|x| x.as_str())
    }

    pub fn get_bytes_from_key(&self, key: &str) -> Option<(Bytes, &'static str)> {
        self.files.get(key).map(|x| (x.0.clone(), x.1))
    }
}