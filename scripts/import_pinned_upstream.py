#!/usr/bin/env python3
"""Import two audited source revisions matching user-uploaded DAW ZIPs."""
from pathlib import Path, PurePosixPath
from zipfile import ZipFile
import hashlib
import sys

EXPECTED = {
    "ADE": (9641, "f84bf2b0475a776c90984acd3720d57b7c9a95a6"),
    "ML": (9364, "36c6447167313157311e8f98c310f65750561e29"),
}
SKIP = {"LICENSE", "README.md", "SNAPTUNE_INTEGRATION.md", "THIRD-PARTY-NOTICES.txt"}
INSTALLER = "plugins/ml-plasma/vendor/gridline/tools/windows/gridline.iss"
ML_ONLY = [
    "crates/os-engine/src/engine/instrument_workers.rs",
    "crates/os-engine/src/engine/mix_workers.rs",
    "crates/os-engine/tests/fx_null.rs",
    "crates/os-engine/tests/live_note_count.rs",
    "crates/os-flp/tests/corpus.rs",
    "crates/os-flp/tests/project_corpus.rs",
    "docs/assets/screenshot-playlist.webp",
    "tools/os-render/tests/eq_null.rs",
    "tools/os-render/tests/pdc_null.rs",
]

def object_sha(kind, data):
    return hashlib.sha1(kind.encode() + b" " + str(len(data)).encode() + b"\0" + data).digest()

def tree_sha(d):
    items = []
    for name, value in d.items():
        if isinstance(value, dict):
            mode, sha, sort_key = "40000", tree_sha(value), name + "/"
        else:
            mode, sha, sort_key = value[0], value[1], name
        items.append((sort_key, mode, name, sha))
    raw = b"".join(mode.encode() + b" " + name.encode() + b"\0" + sha
                   for _, mode, name, sha in sorted(items, key=lambda t: t[0].encode()))
    return object_sha("tree", raw)

def verify(path, label):
    files, tree = {}, {}
    with ZipFile(path) as archive:
        roots = {i.filename.split("/")[0] for i in archive.infolist() if "/" in i.filename}
        if len(roots) != 1:
            raise RuntimeError("Unexpected archive root")
        prefix = next(iter(roots)) + "/"
        for item in archive.infolist():
            if item.is_dir():
                continue
            if not item.filename.startswith(prefix):
                raise RuntimeError("Unexpected path")
            rel = item.filename[len(prefix):]
            parts = PurePosixPath(rel).parts
            if not parts or ".." in parts or rel.startswith("/"):
                raise RuntimeError("Unsafe archive path")
            if rel in SKIP or rel.startswith("licenses/"):
                continue
            data = archive.read(item)
            if rel == INSTALLER:
                data = data.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")
            mode = "100755" if (item.external_attr >> 16) & 0o111 else "100644"
            files[rel] = (mode, data)
            node = tree
            for part in parts[:-1]:
                node = node.setdefault(part, {})
            node[parts[-1]] = (mode, object_sha("blob", data))
    sha = tree_sha(tree).hex()
    count, expected_sha = EXPECTED[label]
    print(f"{label}: {len(files)} files, tree SHA {sha}")
    if len(files) != count or sha != expected_sha:
        raise RuntimeError(f"Source does not match the EXACT uploaded {label} archive; refusing import.")
    return files

def write(dest, rel, mode, data):
    path = dest / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    path.chmod(0o755 if mode == "100755" else 0o644)

def main():
    if len(sys.argv) != 4:
        raise SystemExit("Usage: python scripts/import_pinned_upstream.py ADE.zip ML.zip target")
    ade_zip, ml_zip, target = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]).resolve()
    ade, ml = verify(ade_zip, "ADE"), verify(ml_zip, "ML")
    for rel, (mode, data) in ade.items():
        if rel.startswith(".github/workflows/"):
            rel = "docs/upstream-workflows-disabled/" + PurePosixPath(rel).name
        write(target, rel, mode, data)
    for rel in ML_ONLY:
        if rel in ade or rel not in ml:
            raise RuntimeError(f"Expected ML-only file missing or overlapping: {rel}")
        mode, data = ml[rel]
        write(target, "reference/ml-only/" + rel, mode, data)
    with ZipFile(ade_zip) as archive:
        license_entry = next((i for i in archive.infolist()
                              if i.filename.endswith("/LICENSE") and i.filename.count("/") == 1), None)
        if license_entry is not None:
            write(target, "LICENSE", "100644", archive.read(license_entry))
        elif not (target / "LICENSE").exists():
            raise RuntimeError("Missing GPL LICENSE. Stop.")
    path = target / "crates/os-app/src/app.rs"
    src = path.read_text(encoding="utf-8")
    old = '''            if crate::updates::ready() {
                if self.transport.playing() || self.audio_rec.is_some() { self.state.message = Some("Stop playback or recording before restarting to update.".into()); self.invalidate(); return; }
                self.ask_discard("restarting to update", crate::dialog::Next::Update);
            } else { crate::updates::request(); self.state.message = Some(crate::updates::display().1); self.invalidate(); }'''
    new = '''            // SKELL STUDIO BASELINE: never check/install Michael Lane / AQUILA updates from this fork.
            self.state.message = Some("Updates are disabled in this Skell Studio development fork.".into());
            self.invalidate();'''
    if src.count(old) != 1:
        raise RuntimeError("Expected upstream update UI anchor missing")
    path.write_text(src.replace(old, new), encoding="utf-8")
    path = target / "crates/os-app/src/main.rs"
    src = path.read_text(encoding="utf-8")
    if src.count("    updates::start();") != 1:
        raise RuntimeError("Expected upstream update startup anchor missing")
    src = src.replace("    updates::start();",
        "    // SKELL STUDIO BASELINE: upstream update service disabled until first-party updater exists.\n"
        "    // Do not call updates::start() in this fork.")
    path.write_text(src, encoding="utf-8")
    path = target / ".gitignore"
    with path.open("a", encoding="utf-8") as f:
        f.write("\n# Skell Studio baseline artifacts\n*.zip\n*.log\n")
    print(f"SUCCESS: imported {len(ade)} ADE files and {len(ML_ONLY)} ML-only reference files.")

if __name__ == "__main__":
    main()
