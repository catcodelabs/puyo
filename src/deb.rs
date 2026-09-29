use anyhow::{bail, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub struct DebMetadata {
    pub name: String,
    pub version: String,
    pub arch: String,
    pub description: String,
}

pub fn parse_control_file(control_content: &str) -> DebMetadata {
    let mut name = "deb-package".to_string();
    let mut version = "1.0.0".to_string();
    let mut arch = "x86_64".to_string();
    let mut description = "Converted from .deb by puyo".to_string();

    for line in control_content.lines() {
        if let Some((key, val)) = line.split_once(':') {
            let key = key.trim().to_lowercase();
            let val = val.trim();
            match key.as_str() {
                "package" => name = val.to_lowercase().replace('_', "-"),
                "version" => version = val.replace('-', "_"),
                "architecture" => {
                    arch = match val {
                        "amd64" => "x86_64".to_string(),
                        "all" => "any".to_string(),
                        other => other.to_string(),
                    };
                }
                "description" => description = val.to_string(),
                _ => {}
            }
        }
    }

    DebMetadata {
        name,
        version,
        arch,
        description,
    }
}

pub fn process_deb_file(deb_path: &Path) -> Result<()> {
    if !deb_path.exists() {
        bail!("File not found: {:?}", deb_path);
    }

    let absolute_deb_path = fs::canonicalize(deb_path)?;
    let temp_dir = std::env::temp_dir().join("puyo_deb_build");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    // Unpack all contents of .deb silently
    let _ = Command::new("bsdtar")
        .args([
            "-xf",
            absolute_deb_path.to_str().unwrap(),
            "-C",
            temp_dir.to_str().unwrap(),
        ])
        .stderr(std::process::Stdio::null())
        .status();

    // Check for control archive and unpack if present
    let control_path = temp_dir.join("control");
    if !control_path.exists() {
        for entry in fs::read_dir(&temp_dir)? {
            if let Ok(entry) = entry {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("control.tar") {
                    let _ = Command::new("bsdtar")
                        .args([
                            "-xf",
                            entry.path().to_str().unwrap(),
                            "-C",
                            temp_dir.to_str().unwrap(),
                        ])
                        .stderr(std::process::Stdio::null())
                        .status();
                    break;
                }
            }
        }
    }

    let meta = if control_path.exists() {
        let content = fs::read_to_string(&control_path).unwrap_or_default();
        parse_control_file(&content)
    } else {
        DebMetadata {
            name: deb_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            version: "1.0.0".to_string(),
            arch: "x86_64".to_string(),
            description: "Converted from .deb via puyo".to_string(),
        }
    };

    let pkgbuild_content = format!(
        r#"# Generated dynamically by puyo from .deb
pkgname="{pkgname}"
pkgver={pkgver}
pkgrel=1
pkgdesc="{pkgdesc}"
arch=('{arch}')
depends=('glibc')
options=('!strip')

package() {{
    for payload in "{deb_dir}"/data.tar.*; do
        if [ -f "$payload" ]; then
            bsdtar -xf "$payload" -C "$pkgdir"
        fi
    done
}}
"#,
        pkgname = meta.name,
        pkgver = meta.version,
        pkgdesc = meta.description,
        arch = meta.arch,
        deb_dir = temp_dir.to_str().unwrap()
    );

    fs::write(temp_dir.join("PKGBUILD"), pkgbuild_content)?;

    let orig_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_dir)?;
    let status = Command::new("makepkg").args(["-si"]).status()?;
    std::env::set_current_dir(orig_dir)?;

    if !status.success() {
        bail!("makepkg failed to build converted .deb package");
    }

    Ok(())
}