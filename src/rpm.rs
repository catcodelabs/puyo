use anyhow::{bail, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn process_rpm_file(rpm_path: &Path) -> Result<()> {
    if !rpm_path.exists() {
        bail!("File not found: {:?}", rpm_path);
    }

    let absolute_rpm_path = fs::canonicalize(rpm_path)?;
    let file_stem = rpm_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let temp_dir = std::env::temp_dir().join("puyo_rpm_build");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let pkgbuild_content = format!(
        r#"# Generated dynamically by puyo from .rpm
pkgname="{pkgname}"
pkgver=1.0.0
pkgrel=1
pkgdesc="Converted from .rpm package via puyo"
arch=('x86_64')
depends=('glibc')
options=('!strip')

package() {{
    # bsdtar natively unpacks RPM payloads directly into pkgdir
    bsdtar -xf "{rpm_path}" -C "$pkgdir"
}}
"#,
        pkgname = file_stem.to_lowercase().replace('_', "-"),
        rpm_path = absolute_rpm_path.to_str().unwrap()
    );

    fs::write(temp_dir.join("PKGBUILD"), pkgbuild_content)?;

    let orig_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_dir)?;
    let status = Command::new("makepkg").args(["-si"]).status()?;
    std::env::set_current_dir(orig_dir)?;

    if !status.success() {
        bail!("makepkg failed to build converted .rpm package");
    }

    Ok(())
}