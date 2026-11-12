#!/usr/bin/env python3
"""Connect generated Skell Studio icon files to the app without rewriting old project formats."""
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]

def one(path,old,new):
    target=ROOT/path
    txt=target.read_text(encoding="utf-8")
    count=txt.count(old)
    if count<1:
        raise RuntimeError(f"Missing icon anchor in {path}: {old}")
    target.write_text(txt.replace(old,new),encoding="utf-8")
    print(path,old,"->",new, "("+str(count)+" occurrences)")

one("crates/os-app/build.rs","assets/lemonstudio-nightly.ico","assets/skell-studio-nightly.ico")
one("crates/os-app/build.rs","assets/lemonstudio.ico","assets/skell-studio.ico")
one("crates/os-app/src/linux_launcher.rs",
    'include_bytes!("../assets/lemonstudio.png")',
    'include_bytes!("../assets/skell-studio.png")')
one("crates/os-app/src/linux_launcher.rs",
    'applications/lemonstudio.desktop','applications/skell-studio.desktop')
one("crates/os-app/src/linux_launcher.rs",
    '== Some("lemonstudio")','== Some("skell-studio")')
one("crates/os-app/src/linux_launcher.rs",
    'icons.join("lemonstudio.png")','icons.join("skell-studio.png")')
one("crates/os-app/src/linux_launcher.rs",
    'format!(".lemonstudio-{}-{stamp}.tmp"','format!(".skell-studio-{}-{stamp}.tmp"')
desktop=ROOT/"crates/os-app/assets/lemonstudio.desktop"
skell=ROOT/"crates/os-app/assets/skell-studio.desktop"
if skell.exists():raise RuntimeError("Original Skell desktop entry already exists; no overwrite")
s=desktop.read_text(encoding="utf-8")
for a,b in [("Name=AQUILA","Name=Skell Studio"),("Exec=lemonstudio","Exec=skell-studio"),
            ("Icon=lemonstudio","Icon=skell-studio"),("StartupWMClass=lemonstudio",
             "StartupWMClass=skell-studio")]:
    if a not in s:raise RuntimeError("Missing desktop entry "+a)
    s=s.replace(a,b)
skell.write_text(s,encoding="utf-8")
# Android/mac packaging remains disabled and is not silently rewritten.
print("Skell Studio resource link update complete")
