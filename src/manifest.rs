use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct PuyoManifest {
    pub package: PackageSection,
    pub build: BuildSection,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageSection {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildSection {
    pub cmd: String,
    pub binary: String,
    #[serde(default)]
    pub depends: Vec<String>,
    #[serde(default)]
    pub makedepends: Vec<String>,
}

impl PuyoManifest {
    pub fn load_from_dir<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let manifest_path = dir.as_ref().join(".puyo");
        let content = fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest at {:?}", manifest_path))?;

        let manifest: PuyoManifest =
            toml::from_str(&content).with_context(|| "Failed to parse .puyo format")?;

        Ok(manifest)
    }

    pub fn generate_pkgbuild(&self) -> String {
        let depends_str = self.format_array(&self.build.depends);
        let makedepends_str = self.format_array(&self.build.makedepends);
        let desc = self
            .package
            .description
            .as_deref()
            .unwrap_or("Built via puyo");

        let package_cmd = if self.build.cmd.contains("cargo") {
            format!("cargo install --path . --root \"$pkgdir/usr\"")
        } else {
            format!(
                "install -Dm755 \"{}\" \"$pkgdir/usr/bin/{}\"",
                self.build.binary, self.package.name
            )
        };

        format!(
            r#"# Generated dynamically by puyo
pkgname="{pkgname}"
pkgver=puyo
pkgrel=9595
pkgdesc="{desc}"
arch=('x86_64')
depends=({depends})
makedepends=({makedepends})

build() {{
    {build_cmd}
}}

package() {{
    {package_cmd}
}}
"#,
            pkgname = self.package.name,
            desc = desc,
            depends = depends_str,
            makedepends = makedepends_str,
            build_cmd = self.build.cmd,
            package_cmd = package_cmd
        )
    }

    fn format_array(&self, items: &[String]) -> String {
        items
            .iter()
            .map(|s| format!("'{}'", s))
            .collect::<Vec<_>>()
            .join(" ")
    }
}