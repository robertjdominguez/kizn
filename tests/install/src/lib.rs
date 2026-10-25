//! A staged kizn release: the files a GitHub release holds, laid out the way
//! GitHub serves them, so the install scripts can run against a local HTTP
//! server (`KIZN_RELEASE_BASE`).
//!
//! ```text
//! <root>/download/v<version>/<asset>     a tagged release
//! <root>/latest/download/<asset>         the latest (non-prerelease) one
//! ```
//!
//! Assets come from a directory of real bundles (named as the release
//! workflow names them) when one is given, and are otherwise stubs: small
//! files of the right shape whose bytes differ per version, so "this version
//! is installed" can be checked by comparing bytes.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub const LINUX: &str = "kizn-linux-x86_64.AppImage";
pub const ICON: &str = "kizn-icon.png";
pub const MACOS: &str = "kizn-macos-universal.app.tar.gz";
pub const WINDOWS: &str = "kizn-windows-x86_64.zip";
pub const ASSETS: [&str; 4] = [LINUX, ICON, MACOS, WINDOWS];

/// The binary inside each archive, as the install compares it.
pub const MACOS_BINARY: &str = "kizn.app/Contents/MacOS/kizn";
pub const WINDOWS_BINARY: &str = "kizn.exe";

#[derive(Debug)]
pub struct Stage {
    pub root: PathBuf,
    /// Real bundles to use instead of stubs, by asset name.
    pub real: Option<PathBuf>,
}

impl Stage {
    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.root.join("download").join(format!("v{version}"))
    }

    pub fn latest_dir(&self) -> PathBuf {
        self.root.join("latest").join("download")
    }

    /// Stages `version`; also as the latest release unless it's a prerelease.
    pub fn release(&self, version: &str, latest: bool) -> io::Result<()> {
        let dir = self.version_dir(version);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("VERSION"), format!("{version}\n"))?;
        for asset in ASSETS {
            let real = self.real.as_ref().map(|d| d.join(asset)).filter(|p| p.exists());
            match real {
                Some(path) => {
                    fs::copy(path, dir.join(asset))?;
                }
                None => fs::write(dir.join(asset), stub(asset, version)?)?,
            }
        }
        write_sums(&dir)?;
        if latest {
            let to = self.latest_dir();
            if to.exists() {
                fs::remove_dir_all(&to)?;
            }
            fs::create_dir_all(&to)?;
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                fs::copy(entry.path(), to.join(entry.file_name()))?;
            }
        }
        Ok(())
    }

    /// Makes `asset`'s entry in every staged SHA256SUMS wrong.
    pub fn tamper(&self, asset: &str) -> io::Result<()> {
        for dir in self.release_dirs()? {
            let sums = dir.join("SHA256SUMS");
            let text = fs::read_to_string(&sums)?;
            let bad: String = text
                .lines()
                .map(|l| {
                    if l.ends_with(&format!("  {asset}")) {
                        format!("{}  {asset}\n", "0".repeat(64))
                    } else {
                        format!("{l}\n")
                    }
                })
                .collect();
            fs::write(sums, bad)?;
        }
        Ok(())
    }

    fn release_dirs(&self) -> io::Result<Vec<PathBuf>> {
        let mut dirs = vec![];
        if self.latest_dir().exists() {
            dirs.push(self.latest_dir());
        }
        if let Ok(rd) = fs::read_dir(self.root.join("download")) {
            for e in rd {
                dirs.push(e?.path());
            }
        }
        Ok(dirs)
    }
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// `SHA256SUMS` for every file in `dir`, in `sha256sum` format.
pub fn write_sums(dir: &Path) -> io::Result<()> {
    let mut names: Vec<String> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n != "SHA256SUMS")
        .collect();
    names.sort();
    let mut out = String::new();
    for n in names {
        out.push_str(&format!("{}  {n}\n", sha256(&fs::read(dir.join(&n))?)));
    }
    fs::write(dir.join("SHA256SUMS"), out)
}

/// The bytes of one member of a .tar.gz or .zip archive.
pub fn archive_member(archive: &Path, member: &str) -> io::Result<Vec<u8>> {
    let name = archive.to_string_lossy();
    let mut out = vec![];
    if name.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)?;
        zip.by_name(member)?.read_to_end(&mut out)?;
        return Ok(out);
    }
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(fs::File::open(archive)?));
    for entry in tar.entries()? {
        let mut entry = entry?;
        if entry.path()?.to_string_lossy().trim_start_matches("./") == member {
            entry.read_to_end(&mut out)?;
            return Ok(out);
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, format!("{member} isn't in {name}")))
}

fn stub(asset: &str, version: &str) -> io::Result<Vec<u8>> {
    let binary = format!("#!/bin/sh\necho 'kizn stub {version}'\n").into_bytes();
    Ok(match asset {
        LINUX => binary,
        ICON => include_bytes!("../../../src-tauri/icons/128x128.png").to_vec(),
        MACOS => {
            let plist = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
                 <key>CFBundleName</key><string>kizn</string>\
                 <key>CFBundleExecutable</key><string>kizn</string>\
                 <key>CFBundleShortVersionString</key><string>{version}</string>\
                 </dict></plist>\n"
            );
            let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(vec![], flate2::Compression::fast()));
            for (path, bytes, mode) in
                [("kizn.app/Contents/Info.plist", plist.as_bytes(), 0o644), (MACOS_BINARY, binary.as_slice(), 0o755)]
            {
                let mut h = tar::Header::new_gnu();
                h.set_size(bytes.len() as u64);
                h.set_mode(mode);
                h.set_cksum();
                tar.append_data(&mut h, path, bytes)?;
            }
            tar.into_inner()?.finish()?
        }
        WINDOWS => {
            let mut zip = zip::ZipWriter::new(io::Cursor::new(vec![]));
            zip.start_file(WINDOWS_BINARY, zip::write::SimpleFileOptions::default())?;
            zip.write_all(format!("MZ kizn stub {version}").as_bytes())?;
            zip.finish()?.into_inner()
        }
        other => return Err(io::Error::other(format!("no stub for {other}"))),
    })
}
