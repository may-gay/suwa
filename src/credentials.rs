use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, path::Path, process::Stdio, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Credentials {
    pub password: String,
    pub headers: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credentials { redacted }")
    }
}

#[derive(Clone)]
pub struct Key {
    profile: String,
    account: String,
}

impl Key {
    pub fn new(root: &Path, account: String) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_owned());
        Self {
            profile: format!("{:x}", Sha256::digest(root.as_os_str().as_encoded_bytes())),
            account,
        }
    }

    async fn command(&self, action: &str, input: Option<Vec<u8>>) -> Result<Option<Vec<u8>>> {
        let operation = async {
            let mut command = Command::new("secret-tool");
            command.arg(action);
            if action == "store" {
                command.args(["--label", "Suwa sign-in"]);
            }
            let mut child = command
                .args(["app", "suwa", "profile", &self.profile, "account", &self.account])
                .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .context("Cannot open the Linux keyring; install libsecret and enable a Secret Service keyring")?;
            if let Some(input) = input {
                let mut stdin = child.stdin.take().context("Cannot write to the keyring")?;
                stdin
                    .write_all(&input)
                    .await
                    .context("Cannot write to the keyring")?;
            }
            let output = child
                .wait_with_output()
                .await
                .context("Keyring process failed")?;
            if !output.status.success() {
                if matches!(action, "lookup" | "clear")
                    && output.status.code() == Some(1)
                    && output.stderr.is_empty()
                {
                    return Ok(None);
                }
                bail!("Cannot access the Linux keyring; unlock it and try again");
            }
            Ok(Some(output.stdout))
        };
        tokio::time::timeout(Duration::from_secs(30), operation)
            .await
            .context("The Linux keyring did not respond; unlock it and try again")?
    }

    pub async fn load(&self) -> Result<Option<Credentials>> {
        self.command("lookup", None)
            .await?
            .map(|bytes| {
                let bytes = STANDARD
                    .decode(bytes.trim_ascii())
                    .context("Saved sign-in is unreadable; enter it again")?;
                serde_json::from_slice(&bytes)
                    .context("Saved sign-in is unreadable; enter it again")
            })
            .transpose()
    }

    pub async fn save(&self, credentials: &Credentials) -> Result<()> {
        ensure!(
            self.command(
                "store",
                Some(
                    STANDARD
                        .encode(serde_json::to_vec(credentials)?)
                        .into_bytes()
                )
            )
            .await?
            .is_some(),
            "Sign-in was not saved"
        );
        Ok(())
    }

    pub async fn forget(&self) -> Result<()> {
        let _ = self.command("lookup", None).await?;
        self.command("clear", None).await?;
        ensure!(
            self.command("lookup", None).await?.is_none(),
            "Saved sign-in could not be removed from the keyring"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires access to an unlocked desktop keyring; uses only temporary test entries"]
    async fn keyring_roundtrip_is_scoped_and_removable() {
        let profile = tempfile::tempdir().unwrap();
        let other_profile = tempfile::tempdir().unwrap();
        let key = Key::new(profile.path(), "test-account".into());
        let other = Key::new(profile.path(), "other-account".into());
        let isolated = Key::new(other_profile.path(), "test-account".into());
        let result: Result<()> = async {
            ensure!(key.load().await?.is_none(), "test entry already exists");
            key.save(&Credentials {
                password: "fixture-only\npassword".into(),
                headers: "X-Test: fixture".into(),
            })
            .await?;
            let saved = key.load().await?.context("missing saved fixture")?;
            ensure!(
                saved.password == "fixture-only\npassword",
                "password changed"
            );
            ensure!(saved.headers == "X-Test: fixture", "headers changed");
            ensure!(other.load().await?.is_none(), "account scope leaked");
            ensure!(isolated.load().await?.is_none(), "profile scope leaked");
            key.save(&Credentials {
                password: "updated".into(),
                headers: String::new(),
            })
            .await?;
            ensure!(
                key.load()
                    .await?
                    .context("missing updated fixture")?
                    .password
                    == "updated",
                "update failed"
            );
            key.command("store", Some(b"invalid fixture".to_vec()))
                .await?;
            ensure!(key.load().await.is_err(), "invalid entry accepted");
            Ok(())
        }
        .await;
        key.forget().await.unwrap();
        result.unwrap();
        assert!(key.load().await.unwrap().is_none());
    }
}
