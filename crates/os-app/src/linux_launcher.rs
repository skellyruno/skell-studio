// SPDX-License-Identifier: GPL-3.0-or-later

//! Refresh the icon used by an existing AQUILA desktop entry in the user's icon theme.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(crate) const ICON: &[u8] = include_bytes!("../assets/skell-studio.png");

pub(crate) fn refresh() {
    let absolute = |name| std::env::var_os(name).map(PathBuf::from).filter(|path| path.is_absolute());
    let Some(data_home) = absolute("XDG_DATA_HOME").or_else(|| absolute("HOME").map(|home| home.join(".local/share"))) else { return };
    let data_dirs = std::env::var_os("XDG_DATA_DIRS").filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let _ = refresh_icon(&data_home, std::env::split_paths(&data_dirs).filter(|path| path.is_absolute()));
}

fn refresh_icon(data_home: &Path, data_dirs: impl Iterator<Item = PathBuf>) -> io::Result<()> {
    let launcher = std::iter::once(data_home.to_owned()).chain(data_dirs)
        .find_map(|base| fs::read_to_string(base.join("applications/skell-studio.desktop")).ok());
    let uses_app_icon = launcher.as_deref().and_then(|text| {
        let group = text.lines().skip_while(|line| line.trim() != "[Desktop Entry]").skip(1);
        group.take_while(|line| !line.trim_start().starts_with('['))
            .find_map(|line| line.trim().strip_prefix("Icon=").map(str::trim))
    }) == Some("skell-studio");
    if !uses_app_icon {
        return Ok(());
    }

    let theme = data_home.join("icons/hicolor");
    let icons = theme.join("256x256/apps");
    let destination = icons.join("skell-studio.png");
    if fs::symlink_metadata(&destination).is_ok_and(|meta| !meta.is_file())
        || fs::read(&destination).is_ok_and(|current| current == ICON)
    {
        return Ok(());
    }

    fs::create_dir_all(&icons)?;
    let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let pending = icons.join(format!(".skell-studio-{}-{stamp}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new().create_new(true).write(true).open(&pending)?;
    let written = file.write_all(ICON);
    drop(file);
    if let Err(error) = written.and_then(|()| fs::rename(&pending, &destination)) {
        let _ = fs::remove_file(pending);
        return Err(error);
    }
    // Icon theme caches are keyed by the top-level theme directory's modification time.
    let _ = fs::File::open(theme).and_then(|directory| directory.set_modified(SystemTime::now()));
    Ok(())
}
