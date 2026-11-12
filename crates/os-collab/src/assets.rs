//! Content-addressed audio transfer. Peer supplied paths never reach the filesystem.
use super::*;
const QUOTA: u64 = 4 * 1024 * 1024 * 1024;
pub fn valid_id(id: &str) -> bool {
    let Some((hash, ext)) = id.split_once('.') else {
        return false;
    };
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && [
            "wav", "aif", "aiff", "flac", "mp3", "ogg", "m4a", "aac", "opus",
        ]
        .contains(&ext)
}
fn digest(path: &Path) -> Result<String> {
    let mut f = File::open(path).map_err(err)?;
    let mut hash = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = f.read(&mut b).map_err(err)?;
        if n == 0 {
            break;
        }
        hash.update(&b[..n]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
pub struct Store {
    pub root: PathBuf,
}
impl Store {
    pub fn new(root: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&root).map_err(err)?;
        Ok(Self { root })
    }
    fn path(&self, id: &str) -> Result<PathBuf> {
        if !valid_id(id) {
            return Err("Invalid audio identity".into());
        }
        Ok(self.root.join(id))
    }
    pub fn info(&self, id: &str) -> Result<(u64, bool)> {
        let path = self.path(id)?;
        if let Ok(m) = std::fs::metadata(&path) {
            return Ok((m.len(), true));
        }
        Ok((
            std::fs::metadata(path.with_extension(format!(
                "{}.part",
                path.extension().unwrap().to_string_lossy()
            )))
            .map_or(0, |m| m.len()),
            false,
        ))
    }
    pub fn write(&self, id: &str, total: u64, offset: u64, data: &[u8]) -> Result<(u64, bool)> {
        let path = self.path(id)?;
        if total > MAX_PACKAGE
            || data.len() > CHUNK
            || offset
                .checked_add(data.len() as u64)
                .is_none_or(|n| n > total)
        {
            return Err("Audio transfer limit exceeded".into());
        }
        let (size, complete) = self.info(id)?;
        if complete {
            return Ok((size, true));
        }
        if offset != size {
            return Ok((size, false));
        }
        let used = std::fs::read_dir(&self.root)
            .map_err(err)?
            .filter_map(|e| e.ok()?.metadata().ok().map(|m| m.len()))
            .sum::<u64>();
        if used.saturating_add(data.len() as u64) > QUOTA {
            return Err("Session audio cache reached 4 GB".into());
        }
        let part = path.with_extension(format!(
            "{}.part",
            path.extension().unwrap().to_string_lossy()
        ));
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&part)
            .map_err(err)?;
        f.write_all(data).map_err(err)?;
        let size = offset + data.len() as u64;
        if size == total {
            f.sync_all().map_err(err)?;
            drop(f);
            if digest(&part)? != id[..64] {
                let _ = std::fs::remove_file(part);
                return Err("Audio integrity check failed".into());
            }
            std::fs::rename(part, path).map_err(err)?;
        }
        Ok((size, size == total))
    }
    pub fn read(&self, id: &str, offset: u64) -> Result<Vec<u8>> {
        let mut f = File::open(self.path(id)?).map_err(err)?;
        let size = f.metadata().map_err(err)?.len();
        if offset > size {
            return Err("Invalid audio offset".into());
        }
        f.seek(SeekFrom::Start(offset)).map_err(err)?;
        let mut b = vec![0; CHUNK.min((size - offset) as usize)];
        f.read_exact(&mut b).map_err(err)?;
        Ok(b)
    }
    pub fn import(&self, path: &Path, stop: &AtomicBool) -> Result<String> {
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        let size = std::fs::metadata(path).map_err(err)?.len();
        if size > MAX_PACKAGE {
            return Err("An audio file exceeds 512 MB".into());
        }
        let id = format!("{}.{}", digest(path)?, ext);
        self.path(&id)?;
        let (offset, complete) = self.info(&id)?;
        if complete {
            return Ok(id);
        }
        let mut f = File::open(path).map_err(err)?;
        f.seek(SeekFrom::Start(offset)).map_err(err)?;
        let mut offset = offset;
        loop {
            if stop.load(Ordering::Acquire) {
                return Err("Cancelled".into());
            }
            let mut b = vec![0; CHUNK];
            let n = f.read(&mut b).map_err(err)?;
            b.truncate(n);
            let (next, done) = self.write(&id, size, offset, &b)?;
            offset = next;
            if done {
                break;
            }
            if n == 0 {
                return Err("Audio file changed during transfer".into());
            }
        }
        Ok(id)
    }
}

/// Local path translations live only in the worker. The network sees hashes, never write destinations.
pub struct Media {
    pub store: Store,
    local: BTreeMap<String, String>,
    wire: BTreeMap<String, String>,
}
impl Media {
    pub fn new(root: PathBuf) -> Result<Self> {
        Ok(Self {
            store: Store::new(root)?,
            local: BTreeMap::new(),
            wire: BTreeMap::new(),
        })
    }
    pub fn encode(
        &mut self,
        d: &Document,
        mut client: Option<&mut Client>,
        stop: &AtomicBool,
    ) -> Result<Document> {
        let mut out = d.clone();
        for (key, value) in &mut out {
            visit(key, value, &mut |path| {
                if path.is_empty() || path.starts_with("mlasset:") {
                    return Ok(());
                }
                let id = if let Some(id) = self.wire.get(path) {
                    id.clone()
                } else {
                    let id = self.store.import(Path::new(path), stop)?;
                    self.wire.insert(path.clone(), id.clone());
                    self.local.entry(id.clone()).or_insert(path.clone());
                    id
                };
                if let Some(c) = client.as_deref_mut() {
                    c.upload_audio(&id, &self.store, stop)?;
                }
                *path = format!("mlasset:{id}");
                Ok(())
            })?;
        }
        Ok(out)
    }
    pub fn decode(
        &mut self,
        d: &Document,
        client: &mut Client,
        stop: &AtomicBool,
    ) -> Result<Document> {
        let mut out = d.clone();
        for (key, value) in &mut out {
            visit(key, value, &mut |path| {
                if path.is_empty() {
                    return Ok(());
                }
                let id = path
                    .strip_prefix("mlasset:")
                    .ok_or("Host sent an unsafe audio reference")?
                    .to_string();
                if !valid_id(&id) {
                    return Err("Invalid audio reference".into());
                }
                if !self.local.contains_key(&id) {
                    client.download_audio(&id, &self.store, stop)?;
                    let local = self.store.root.join(&id).to_string_lossy().into_owned();
                    self.wire.insert(local.clone(), id.clone());
                    self.local.insert(id.clone(), local);
                }
                *path = self.local[&id].clone();
                Ok(())
            })?;
        }
        Ok(out)
    }
}
fn visit(key: &str, v: &mut Value, f: &mut impl FnMut(&mut String) -> Result<()>) -> Result<()> {
    if key.ends_with("/sample_path") {
        if let Value::String(s) = v {
            f(s)?
        }
    }
    if key == "project/audio_warps" || key == "project/vocal_edits" {
        if let Value::Array(items) = v {
            for item in items {
                for k in ["source", "rendered"] {
                    if let Some(Value::String(s)) = item.get_mut(k) {
                        f(s)?
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn verify_references(d: &Document, root: &Path) -> Result<()> {
    for (key, value) in d {
        if !key.ends_with("/sample_path")
            && key != "project/audio_warps"
            && key != "project/vocal_edits"
        {
            continue;
        }
        let mut value = value.clone();
        visit(key, &mut value, &mut |path| {
            if path.is_empty() {
                return Ok(());
            }
            let id = path.strip_prefix("mlasset:").ok_or("Unshared audio file")?;
            if !valid_id(id) || !root.join(id).is_file() {
                return Err("Audio must finish uploading before this edit is shared".into());
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_store_resumes_checks_hash_and_rejects_paths() {
        let dir = std::env::temp_dir().join(format!("collab-assets-{}", random_id().unwrap()));
        let store = Store::new(dir.join("store")).unwrap();
        let bytes = vec![37; CHUNK * 2 + 73];
        let src = dir.join("source.wav");
        std::fs::write(&src, &bytes).unwrap();
        let id = format!("{}.wav", digest(&src).unwrap());
        assert!(store.info("../../outside.wav").is_err());
        assert_eq!(
            store
                .write(&id, bytes.len() as u64, 0, &bytes[..CHUNK])
                .unwrap(),
            (CHUNK as u64, false)
        );
        assert_eq!(
            store
                .write(&id, bytes.len() as u64, 0, &bytes[..CHUNK])
                .unwrap(),
            (CHUNK as u64, false)
        );
        store
            .write(
                &id,
                bytes.len() as u64,
                CHUNK as u64,
                &bytes[CHUNK..2 * CHUNK],
            )
            .unwrap();
        assert!(
            store
                .write(
                    &id,
                    bytes.len() as u64,
                    (2 * CHUNK) as u64,
                    &bytes[2 * CHUNK..]
                )
                .unwrap()
                .1
        );
        assert_eq!(std::fs::read(store.root.join(&id)).unwrap(), bytes);
        let bad = format!("{}.wav", "0".repeat(64));
        assert!(store.write(&bad, 3, 0, b"bad").is_err());
        assert_eq!(store.info(&bad).unwrap(), (0, false));
        let _ = std::fs::remove_dir_all(dir);
    }
}
