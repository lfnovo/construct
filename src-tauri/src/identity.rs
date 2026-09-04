use serde::Serialize;
use std::path::{Path, PathBuf};

pub(crate) const CHANNEL: &str = env!("CONSTRUCT_CHANNEL");
#[cfg(construct_release)]
pub(crate) const IS_RELEASE: bool = true;
#[cfg(not(construct_release))]
pub(crate) const IS_RELEASE: bool = false;
#[cfg(construct_release)]
pub(crate) const PRODUCT_NAME: &str = "Construct";
#[cfg(not(construct_release))]
pub(crate) const PRODUCT_NAME: &str = "Construct Dev";
#[cfg(construct_release)]
pub(crate) const BUNDLE_IDENTIFIER: &str = "com.luisnovo.construct";
#[cfg(not(construct_release))]
pub(crate) const BUNDLE_IDENTIFIER: &str = "com.luisnovo.construct.dev";
#[cfg(construct_release)]
pub(crate) const CLI_COMMAND: &str = "construct";
#[cfg(not(construct_release))]
pub(crate) const CLI_COMMAND: &str = "construct-dev";
#[cfg(construct_release)]
pub(crate) const MCP_SERVER_NAME: &str = "construct";
#[cfg(not(construct_release))]
pub(crate) const MCP_SERVER_NAME: &str = "construct-dev";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeIdentity {
    pub(crate) channel: &'static str,
    pub(crate) product_name: &'static str,
    pub(crate) bundle_identifier: &'static str,
    pub(crate) cli_command: &'static str,
    pub(crate) default_data_dir: PathBuf,
}

pub(crate) fn data_dir_from(root: &Path) -> PathBuf {
    root.join(BUNDLE_IDENTIFIER)
}

pub(crate) fn default_data_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|path| data_dir_from(&path))
        .ok_or_else(|| "Could not locate the operating system data directory.".to_string())
}

pub(crate) fn runtime_identity() -> Result<RuntimeIdentity, String> {
    Ok(RuntimeIdentity {
        channel: CHANNEL,
        product_name: PRODUCT_NAME,
        bundle_identifier: BUNDLE_IDENTIFIER,
        cli_command: CLI_COMMAND,
        default_data_dir: default_data_dir()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_identity_and_profile_use_the_same_channel() {
        let expected = if IS_RELEASE {
            (
                "release",
                "Construct",
                "com.luisnovo.construct",
                "construct",
            )
        } else {
            (
                "dev",
                "Construct Dev",
                "com.luisnovo.construct.dev",
                "construct-dev",
            )
        };
        assert_eq!(CHANNEL, expected.0);
        assert_eq!(PRODUCT_NAME, expected.1);
        assert_eq!(BUNDLE_IDENTIFIER, expected.2);
        assert_eq!(CLI_COMMAND, expected.3);
        assert_eq!(MCP_SERVER_NAME, expected.3);
        assert_eq!(
            data_dir_from(Path::new("/synthetic/Application Support")),
            PathBuf::from(format!("/synthetic/Application Support/{}", expected.2))
        );
    }
}
