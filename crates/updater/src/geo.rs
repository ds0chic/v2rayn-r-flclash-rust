//! Geo `.dat` file requests for the update flow (SP28-L1-001).
//!
//! Derived from the frozen upstream (commit `7d6a967`)
//! `ServiceLib/Services/UpdateService.cs` (`GetGeoFilesRequest`):
//! the GeoFiles row downloads `geoip.dat` + `geosite.dat` from one URL
//! template (`{0}` = the bare `geoip`/`geosite` name, upstream
//! `Global.GeoUrl` or the stored `ConstItem.GeoSourceUrl`) into the binary
//! directory (`Utils.GetBinPath`). Geo files carry no release version: the
//! check row never reports a remote version and a missing/blank source is an
//! explicit error, never a fabricated version.

use std::path::{Path, PathBuf};

use crate::error::UpdateError;

/// Update-row key for the Geo `.dat` files (upstream
/// `CheckUpdateViewModel._geo` / `CheckUpdateModel.CoreTypeForStorage`).
pub const GEO_FILES_TARGET: &str = "GeoFiles";

/// Bare asset names substituted for `{0}` in the geo source template, in
/// formal-plan order (mirrors `application::dns::GEO_ASSET_FILES`).
pub const GEO_BARE_NAMES: [&str; 2] = ["geoip", "geosite"];

/// One Geo `.dat` download: the resolved URL and its managed destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoFileRequest {
    /// Bare name (`geoip` / `geosite`).
    pub bare_name: String,
    /// File name (`geoip.dat` / `geosite.dat`).
    pub file_name: String,
    /// Template with `{0}` replaced by [`Self::bare_name`].
    pub url: String,
    /// Managed destination (`<bin_dir>/<file_name>`).
    pub target: PathBuf,
}

/// Substitute the bare asset name for `{0}` in a geo source template.
pub fn geo_download_url(template: &str, bare_name: &str) -> String {
    template.replace("{0}", bare_name)
}

/// Build the Geo `.dat` download set for `bin_dir`.
///
/// A blank template is an explicit [`UpdateError::InvalidMetadata`]: the
/// caller maps it onto an honest unconfigured-source error and must never
/// fall back to a fabricated remote version.
pub fn geo_file_requests(
    template: &str,
    bin_dir: &Path,
) -> Result<Vec<GeoFileRequest>, UpdateError> {
    if template.trim().is_empty() {
        return Err(UpdateError::InvalidMetadata(
            "geo source template is empty".to_string(),
        ));
    }
    Ok(GEO_BARE_NAMES
        .into_iter()
        .map(|bare| GeoFileRequest {
            bare_name: bare.to_string(),
            file_name: format!("{bare}.dat"),
            url: geo_download_url(template, bare),
            target: bin_dir.join(format!("{bare}.dat")),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_substitute_the_bare_name_and_target_the_bin_dir() {
        let bin = Path::new("/data/bin");
        let requests = geo_file_requests("https://mirror.example/{0}.dat", bin).unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0],
            GeoFileRequest {
                bare_name: "geoip".to_string(),
                file_name: "geoip.dat".to_string(),
                url: "https://mirror.example/geoip.dat".to_string(),
                target: bin.join("geoip.dat"),
            }
        );
        assert_eq!(requests[1].url, "https://mirror.example/geosite.dat");
        assert_eq!(requests[1].target, bin.join("geosite.dat"));
    }

    #[test]
    fn blank_template_is_an_error_never_a_fake_source() {
        for blank in ["", "   "] {
            let error = geo_file_requests(blank, Path::new("/data/bin")).unwrap_err();
            assert!(
                matches!(error, UpdateError::InvalidMetadata(_)),
                "{error:?}"
            );
        }
    }
}
