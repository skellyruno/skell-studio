Lemon Studio - Michael Lane edition / Linux x86-64

Extract the entire .tar.gz folder, then double-click lemonstudio or run ./lemonstudio in that folder.
The executable has its Linux permission bits stored in the archive.
For an application menu entry, place lemonstudio.desktop in $XDG_DATA_HOME/applications and
lemonstudio.png in $XDG_DATA_HOME/icons/hicolor/256x256/apps. XDG_DATA_HOME defaults to ~/.local/share.
If lemonstudio is outside your PATH, set Exec in the desktop entry to its full path followed by %f.
Requires glibc 2.36 or newer, the ALSA shared library (libasound.so.2), and a desktop session with X11 or Wayland libraries and a file-picker portal.
Use your distribution's package manager if a required library is missing. ARM Linux is not included.

This package was cross-compiled on Windows. It has NOT been run or audio-tested on Linux, at the owner's request.
Windows plugins do not become Linux plugins; use compatible Linux plugin binaries.
Choose your audio device in Settings. No personal device configuration or recordings are included.

Update downloads and verifies signed Linux updates, then installs when you close or restart Lemon Studio. Keep the extracted application in a folder your user can write to. Projects and settings are preserved; previous app files remain in a rollback folder. Older releases need one manual upgrade to receive this updater.
Matching editable source is available on that page, alongside the Windows and Mac choices.
See LICENSE and THIRD-PARTY-NOTICES.txt for upstream authors and GPL/AGPL/component terms.
