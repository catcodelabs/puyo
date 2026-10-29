use anyhow::{bail, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub struct RpmMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
}

pub fn parse_rpm_metadata(rpm_path: &Path) -> RpmMetadata {
    let fallback_name = rpm_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
        .to_lowercase()
        .replace('_', "-");

    // Try extracting metadata using `rpm` query tool if available
    if let Ok(output) = Command::new("rpm")
        .args(["-qp", "--queryformat", "%{NAME}|%{VERSION}|%{SUMMARY}", rpm_path.to_str().unwrap()])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = stdout.split('|').collect();
            if parts.len() >= 2 {
                let name = parts[0].trim().to_lowercase().replace('_', "-");
                let version = parts[1].trim().replace('-', "_");
                let description = if parts.len() >= 3 && !parts[2].trim().is_empty() {
                    parts[2].trim().to_string()
                } else {
                    "Converted from .rpm package via puyo".to_string()
                };
                return RpmMetadata {
                    name,
                    version,
                    description,
                };
            }
        }
    }

    RpmMetadata {
        name: fallback_name,
        version: "0.1.0".to_string(),
        description: "Converted from .rpm package via puyo".to_string(),
    }
}

pub fn process_rpm_file(rpm_path: &Path) -> Result<()> {
    if !rpm_path.exists() {
        bail!("File not found: {:?}", rpm_path);
    }

    let absolute_rpm_path = fs::canonicalize(rpm_path)?;
    let meta = parse_rpm_metadata(&absolute_rpm_path);

    let temp_dir = std::env::temp_dir().join("puyo_rpm_build");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let pkgbuild_content = format!(
        r#"# Generated dynamically by puyo from .rpm metadata parsing
pkgname="{pkgname}"
pkgver={pkgver}
pkgrel=1
pkgdesc="{pkgdesc}"
arch=('x86_64')
depends=('glibc')
options=('!strip')

package() {{
    # bsdtar natively unpacks RPM payloads directly into pkgdir
    bsdtar -xf "{rpm_path}" -C "$pkgdir"
}}
"#,
        pkgname = meta.name,
        pkgver = meta.version,
        pkgdesc = meta.description,
        rpm_path = absolute_rpm_path.to_str().unwrap()
    );

    fs::write(temp_dir.join("PKGBUILD"), pkgbuild_content)?;

    let orig_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_dir)?;
    let status = Command::new("makepkg").args(["-si", "--noconfirm"]).status()?;
    std::env::set_current_dir(orig_dir)?;

    if !status.success() {
        bail!("makepkg failed to build converted .rpm package");
    }

    Ok(())
}