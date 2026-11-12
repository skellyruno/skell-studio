//! Headless, synthetic Windows/Mac/Linux acceptance test. No audio device or personal project.
use os_collab::{assets::Store, *};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
fn validate(d: &Document) -> Result<()> {
    if d.len() > 8 {
        Err("Invalid probe".into())
    } else {
        Ok(())
    }
}
fn main() -> Result<()> {
    let a: Vec<_> = std::env::args().collect();
    let dir = PathBuf::from(
        a.get(2)
            .ok_or("Usage: host DIR ADDRESS | join DIR INVITATION_FILE")?,
    );
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stop = AtomicBool::new(false);
    if a.get(1).map(String::as_str) == Some("host") {
        let package = dir.join("probe.zip");
        std::fs::write(&package, b"synthetic project").map_err(|e| e.to_string())?;
        let host = Host::start(
            "0.0.0.0:32191".parse().unwrap(),
            a[3].clone(),
            package,
            Document::from([("host".into(), json!(1))]),
            validate,
        )?;
        std::fs::write(dir.join("invitation.txt"), host.invitation.encode()?)
            .map_err(|e| e.to_string())?;
        let mut local = Client::connect(
            &host.invitation.with_address("127.0.0.1:32191".into()),
            &random_id()?,
            "Host probe",
        )?;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(120) {
            local.poll(None)?;
            if local.document.get("guest") == Some(&json!(2)) {
                let cache = Store::new(dir.join("received"))?;
                let id = local.document["audio"]
                    .as_str()
                    .ok_or("Missing audio")?
                    .to_string();
                local.download_audio(&id, &cache, &stop)?;
                assert_eq!(
                    std::fs::read(cache.root.join(id)).unwrap(),
                    vec![41; 500_000]
                );
                let changed = Document::from([
                    ("host".into(), json!(3)),
                    ("guest".into(), json!(2)),
                    ("audio".into(), local.document["audio"].clone()),
                ]);
                let changes = diff(&local.document, &changed);
                assert_eq!(
                    local.poll(Some(Proposal { seq: 1, changes }))?.accepted,
                    Some(true)
                );
                std::thread::sleep(Duration::from_secs(2));
                println!("PASS: remote edit and 500 KB audio integrity; host response delivered");
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err("Timed out waiting for the remote computer".into())
    } else {
        let invite =
            Invitation::parse(&std::fs::read_to_string(&a[3]).map_err(|e| e.to_string())?)?;
        let peer = random_id()?;
        let mut c = Client::connect(&invite, &peer, "Guest probe")?;
        c.download(&dir.join("download.zip"), &stop)?;
        assert_eq!(
            std::fs::read(dir.join("download.zip")).unwrap(),
            b"synthetic project"
        );
        let cache = Store::new(dir.join("assets"))?;
        let wav = dir.join("synthetic.wav");
        std::fs::write(&wav, vec![41; 500_000]).unwrap();
        let id = cache.import(&wav, &stop)?;
        c.upload_audio(&id, &cache, &stop)?;
        let mut changed = c.document.clone();
        changed.insert("guest".into(), json!(2));
        changed.insert("audio".into(), json!(id));
        let changes = diff(&c.document, &changed);
        let proposal = Proposal { seq: 1, changes };
        assert_eq!(c.poll(Some(proposal.clone()))?.accepted, Some(true));
        drop(c);
        let mut c = Client::connect(&invite, &peer, "Guest probe")?;
        assert_eq!(c.poll(Some(proposal))?.accepted, Some(true));
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(15) {
            c.poll(None)?;
            if c.document["host"] == json!(3) {
                println!(
                    "PASS: encrypted remote join, package, upload, reconnect deduplication and bidirectional edits"
                );
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err("Host response not received".into())
    }
}
