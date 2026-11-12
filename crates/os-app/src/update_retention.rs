//! Bounded rollback retention. Only updater-owned, structurally validated folders are eligible.
use super::*;
const KEEP: usize = 2;

fn plain(path: &Path) -> bool {
    let Ok(m) = fs::symlink_metadata(path) else { return false; };
    if m.file_type().is_symlink() { return false; }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 { return false; } // junction/reparse point
    }
    m.is_file() || m.is_dir()
}

fn numbered(name: &str, prefix: &str, suffix: &str) -> Option<(u64, u128)> {
    let name = name.strip_prefix(prefix)?.strip_suffix(suffix)?;
    let (version, stamp) = name.split_once('-')?;
    if version.len() != 10 || !version.bytes().chain(stamp.bytes()).all(|b| b.is_ascii_digit()) { return None; }
    Some((version.parse().ok()?, stamp.parse().ok()?))
}

// Refuse links, unexpected files and partial rollback transactions. Never traverse user folders.
fn inventory(root: &Path, allowed: &[String]) -> Result<Vec<PathBuf>> {
    if !plain(root) || !fs::symlink_metadata(root)?.is_dir() { return Err("Unsafe cleanup folder".into()); }
    fn visit(root: &Path, path: &Path, allowed: &[String], paths: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
        if depth > 6 || paths.len() > 128 { return Err("Unexpected update folder layout".into()); }
        for e in fs::read_dir(path)? {
            let p = e?.path();
            if !plain(&p) { return Err("Linked update item preserved".into()); }
            let rel = p.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
            if fs::symlink_metadata(&p)?.is_dir() {
                if !allowed.iter().any(|n| n.starts_with(&(rel.clone()+"/"))) { return Err("Unknown folder preserved".into()); }
                visit(root, &p, allowed, paths, depth + 1)?;
            } else if !allowed.contains(&rel) { return Err("Unknown file preserved".into()); }
            paths.push(p);
        }
        Ok(())
    }
    let mut paths = Vec::new(); visit(root, root, allowed, &mut paths, 0)?; Ok(paths)
}

fn remove_owned(root: &Path, parent: &Path, allowed: &[String]) -> Result<()> {
    // Recheck immediately before deletion, including the canonical parent boundary.
    if fs::canonicalize(root)?.parent() != Some(fs::canonicalize(parent)?.as_path()) { return Err("Cleanup path escaped its parent".into()); }
    let paths = inventory(root, allowed)?;
    for p in paths {
        if !plain(&p) { return Err("Cleanup item changed; preserved".into()); }
        if fs::symlink_metadata(&p)?.is_dir() { fs::remove_dir(&p)?; } else { fs::remove_file(&p)?; }
    }
    fs::remove_dir(root)?; Ok(())
}

fn backups(parent: &Path, current: &Path, installed: u64, mac: bool) -> Result<usize> {
    let allowed: Vec<String> = if mac { MAC_FILES.to_vec() } else { vec![EXE,"AQUILA Source.zip","THIRD-PARTY-NOTICES.txt","LICENSE"] }.into_iter().map(str::to_owned).collect();
    let mut found = Vec::new();
    for e in fs::read_dir(parent)? {
        let e = e?; let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        let Some((v, stamp)) = numbered(&name, if mac {".lemon-backup-"} else {".update-backup-"}, if mac {".app"} else {""}) else { continue; };
        if v > installed || p == current || !p.join(if mac {"Contents/MacOS/lemonstudio"} else {EXE}).is_file() || inventory(&p,&allowed).is_err() { continue; }
        found.push((v, stamp, p));
    }
    found.sort_by(|a,b| (b.0,b.1).cmp(&(a.0,a.1)));
    let mut removed = 0;
    for (_,_,p) in found.into_iter().skip(KEEP) {
        if remove_owned(&p,parent,&allowed).is_ok() { removed += 1; }
    }
    Ok(removed)
}

fn process_alive(pid: u128) -> bool {
    let Ok(pid)=u32::try_from(pid) else {return true;};
    if pid==0 {return true;}
    #[cfg(windows)] unsafe {
        use windows_sys::Win32::{Foundation::{CloseHandle,GetLastError},System::Threading::{OpenProcess,GetExitCodeProcess,PROCESS_QUERY_LIMITED_INFORMATION}};
        let handle=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,0,pid);
        if handle.is_null() {return GetLastError()!=87;}
        let mut code=0;let ok=GetExitCodeProcess(handle,&mut code);CloseHandle(handle);
        ok==0 || code==259
    }
    #[cfg(unix)] {
        std::process::Command::new("/bin/kill").arg("-0").arg(pid.to_string()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s|s.success()).unwrap_or(true)
    }
}

fn downloads(parent: &Path, installed: u64, protected: Option<&Path>, key: &VerifyingKey) -> Result<usize> {
    let mut removed = 0;
    for e in fs::read_dir(parent)? {
        let e = e?; let p = e.path();
        let Some((v,pid)) = numbered(&e.file_name().to_string_lossy(),"","") else {continue};
        // Future downloads belong to pending updates; the applying helper's own stage stays intact.
        if v > installed || protected == Some(p.as_path()) || !plain(&p) || process_alive(pid) {continue;}
        let manifest_path=p.join("signed.json");
        if !plain(&manifest_path) || fs::metadata(&manifest_path)?.len()>65536 {continue;}
        let Ok(m)=fs::read(&manifest_path).and_then(|b| verify_newer_than(&b,key,0).map_err(std::io::Error::other)) else {continue};
        if m.version!=v {continue;}
        let mut allowed: Vec<String> = m.files.into_iter().map(|f|f.name).collect();
        allowed.extend(["signed.json","helper-ready","result.txt","lemon-update-helper","lemon-update-helper.exe"].into_iter().map(str::to_owned));
        // Only the known Microsoft runtime files copied by our updater, never arbitrary DLLs.
        allowed.extend(["concrt140.dll","msvcp140.dll","msvcp140_1.dll","msvcp140_2.dll","msvcp140_atomic_wait.dll","msvcp140_codecvt_ids.dll","vccorlib140.dll","vcruntime140.dll","vcruntime140_1.dll","vcruntime140_threads.dll"].into_iter().map(str::to_owned));
        if remove_owned(&p,parent,&allowed).is_ok() {removed+=1;}
    }
    Ok(removed)
}

pub(super) fn run(target: &Path, installed: u64, protected: Option<&Path>) -> Result<usize> {
    let mac=cfg!(target_os="macos");
    let current=if mac {target.parent().and_then(Path::parent).and_then(Path::parent).ok_or("No app bundle")?} else {target};
    let parent=current.parent().ok_or("No installation folder")?;
    let mut n=backups(parent,current,installed,mac)?;
    if let Ok(cache)=cache() { n+=downloads(&cache,installed,protected,&public_key()?)?; }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn folder(label:&str)->PathBuf {
        let p=std::env::temp_dir().join(format!("lemon-retention-{label}-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&p).unwrap();p
    }
    #[test]
    fn retention_keeps_two_and_preserves_partial_future_and_user_files() {
        let root=folder("files");let current=root.join(EXE);fs::write(&current,b"active").unwrap();
        for i in 1..=5 {let p=root.join(format!(".update-backup-202611090{i}-{i}"));fs::create_dir(&p).unwrap();fs::write(p.join(EXE),b"old").unwrap();}
        let partial=root.join(".update-backup-2026110901-99");fs::create_dir(&partial).unwrap();fs::write(partial.join(EXE),b"old").unwrap();fs::write(partial.join("project.lmn"),b"preserve").unwrap();
        assert_eq!(backups(&root,&current,2026110904,false).unwrap(),2);
        assert!(root.join(".update-backup-2026110903-3").exists());assert!(root.join(".update-backup-2026110904-4").exists());assert!(root.join(".update-backup-2026110905-5").exists());assert!(partial.exists());assert_eq!(fs::read(&current).unwrap(),b"active");
        assert_eq!(backups(&root,&current,2026110904,false).unwrap(),0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn retention_mac_bundle_keeps_latest_two_without_touching_active() {
        let root=folder("mac");let current=root.join("AQUILA.app");fs::create_dir(&current).unwrap();
        for i in 1..=4 {let p=root.join(format!(".lemon-backup-202611090{i}-{i}.app"));fs::create_dir_all(p.join("Contents/MacOS")).unwrap();fs::write(p.join("Contents/MacOS/lemonstudio"),b"app").unwrap();}
        assert_eq!(backups(&root,&current,2026110904,true).unwrap(),2);assert!(current.exists());assert!(root.join(".lemon-backup-2026110904-4.app").exists());fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn retention_clears_authenticated_completed_cache_but_protects_pending_stage() {
        use ed25519_dalek::{Signer,SigningKey};
        let root=folder("cache");let key=SigningKey::from_bytes(&[31;32]);
        for i in 1..=4 {
            let p=root.join(format!("202611090{i}-4294967294"));fs::create_dir(&p).unwrap();
            let m=Manifest{product:PRODUCT.into(),version:2026110900+i,label:"test".into(),notes:String::new(),files:expected_files().into_iter().map(|name|File{name:name.into(),sha256:"00".repeat(32),size:1}).collect()};
            let payload=serde_json::to_string(&m).unwrap();let signature=hex(&key.sign(payload.as_bytes()).to_bytes());fs::write(p.join("signed.json"),serde_json::to_vec(&Signed{payload,signature}).unwrap()).unwrap();
        }
        let active=root.join(format!("2026110901-{}",std::process::id()));fs::create_dir(&active).unwrap();
        fs::copy(root.join("2026110901-4294967294/signed.json"),active.join("signed.json")).unwrap();
        let protected=root.join("2026110902-4294967294");
        fs::write(root.join("2026110903-4294967294/user.wav"),b"keep").unwrap();
        assert_eq!(downloads(&root,2026110903,Some(&protected),&key.verifying_key()).unwrap(),1);
        assert!(active.exists());assert!(protected.exists());assert!(root.join("2026110904-4294967294").exists());assert!(root.join("2026110903-4294967294/user.wav").exists());fs::remove_dir_all(root).unwrap();
    }
}
