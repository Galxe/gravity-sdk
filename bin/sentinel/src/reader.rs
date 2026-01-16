use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use anyhow::Result;

pub struct Reader {
    files: HashMap<PathBuf, u64>,
}

impl Reader {
    pub fn new() -> Result<Self> {
        Ok(Self {
            files: HashMap::new(),
        })
    }

    pub async fn add_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        if self.files.contains_key(&path) {
            return Ok(());
        }

        let metadata = fs::metadata(&path)?;
        let len = metadata.len();
        self.files.insert(path, len);
        Ok(())
    }

    pub fn poll(&mut self) -> Result<Vec<(PathBuf, String)>> {
        let mut new_lines = Vec::new();

        for (path, offset) in self.files.iter_mut() {
            if let Ok(mut file) = File::open(path) {
                let metadata = file.metadata()?;
                let current_len = metadata.len();

                if current_len < *offset {
                    // Truncated
                    *offset = 0;
                }

                if current_len > *offset {
                    file.seek(SeekFrom::Start(*offset))?;
                    let reader = BufReader::new(file);
                    
                    let mut bytes_read = 0;
                    for line in reader.lines() {
                        if let Ok(l) = line {
                            // +1 for newline character approximation (though could be CRLF)
                            // Better: use lines and count bytes manually or just use current_len
                            // Note: BufReader lines() strips newline.
                            // We need to advance offset correctly.
                            // Simplification: Read to end.
                            // But we need exact bytes.
                             new_lines.push((path.clone(), l));
                        }
                    }
                    *offset = current_len;
                }
            }
        }
        Ok(new_lines)
    }
}
