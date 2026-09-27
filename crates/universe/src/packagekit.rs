use std::time::Duration;

use futures_util::StreamExt;
use zbus::zvariant::OwnedObjectPath;

use crate::core::Progress;
use crate::{Error, Result};

const NAME: &str = "org.freedesktop.PackageKit";
const PATH: &str = "/org/freedesktop/PackageKit";
const TRANSACTION: &str = "org.freedesktop.PackageKit.Transaction";
// PkFilterEnum NEWEST (16), as a bitfield.
const NEWEST: u64 = 1 << 16;
// PkTransactionFlagEnum ONLY_TRUSTED (1), as a bitfield.
const ONLY_TRUSTED: u64 = 1 << 1;
// PkInfoEnum INSTALLED.
const INFO_INSTALLED: u32 = 1;
// PkExitEnum SUCCESS.
const EXIT_SUCCESS: u32 = 1;
// A download of a few GB over a slow line, and the password prompt in front of it.
const PATIENCE: Duration = Duration::from_secs(3 * 3600);

fn failed(e: impl std::fmt::Display) -> Error {
    Error::Unavailable(format!("PackageKit: {e}"))
}

pub async fn available() -> bool {
    let Ok(conn) = zbus::Connection::system().await else { return false };
    let Ok(dbus) = zbus::fdo::DBusProxy::new(&conn).await else { return false };
    let Ok(name) = zbus::names::BusName::try_from(NAME) else { return false };
    if dbus.name_has_owner(name).await.unwrap_or(false) {
        return true;
    }
    dbus.list_activatable_names().await.is_ok_and(|names| names.iter().any(|n| n.as_str() == NAME))
}

#[derive(Default)]
struct Outcome {
    packages: Vec<(u32, String)>,
    error: String,
    exit: u32,
}

async fn transaction<B>(conn: &zbus::Connection, method: &str, body: &B, label: &str, progress: &mut Option<Progress<'_, '_>>) -> Result<Outcome>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    let pk = zbus::Proxy::new(conn, NAME, PATH, NAME).await.map_err(failed)?;
    let path: OwnedObjectPath = pk.call("CreateTransaction", &()).await.map_err(failed)?;
    let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).path(path.clone()).map_err(failed)?.build();
    let mut signals = zbus::MessageStream::for_match_rule(rule, conn, None).await.map_err(failed)?;
    let tx = zbus::proxy::Builder::<zbus::Proxy>::new(conn)
        .destination(NAME)
        .and_then(|b| b.path(path.clone()))
        .and_then(|b| b.interface(TRANSACTION))
        .map_err(failed)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .map_err(failed)?;
    // Without it PackageKit refuses what needs a password instead of asking polkit.
    let _ = tx.call_method("SetHints", &(vec!["interactive=true", "background=false"],)).await;
    tx.call_method(method, body).await.map_err(failed)?;
    let mut out = Outcome::default();
    let mut report = |percent: u32| {
        if let (Some(p), true) = (progress.as_mut(), percent <= 100) {
            p(percent as u64, 100, &format!("{label} · {percent}%"));
        }
    };
    let read = async {
        while let Some(Ok(msg)) = signals.next().await {
            let header = msg.header();
            let body = msg.body();
            match header.member().map(|m| m.as_str()) {
                Some("Package") => {
                    if let Ok((info, id, _)) = body.deserialize::<(u32, String, String)>() {
                        out.packages.push((info, id));
                    }
                }
                Some("Packages") => {
                    if let Ok(list) = body.deserialize::<Vec<(u32, String, String)>>() {
                        out.packages.extend(list.into_iter().map(|(info, id, _)| (info, id)));
                    }
                }
                Some("ErrorCode") => {
                    if let Ok((_, details)) = body.deserialize::<(u32, String)>() {
                        out.error = details;
                    }
                }
                Some("ItemProgress") => {
                    if let Ok((_, _, percent)) = body.deserialize::<(String, u32, u32)>() {
                        report(percent);
                    }
                }
                Some("PropertiesChanged") => {
                    type Changed = (String, std::collections::HashMap<String, zbus::zvariant::OwnedValue>, Vec<String>);
                    if let Ok((_, changed, _)) = body.deserialize::<Changed>() {
                        if let Some(percent) = changed.get("Percentage").and_then(|v| u32::try_from(v).ok()) {
                            report(percent);
                        }
                    }
                }
                Some("Finished") => {
                    if let Ok((exit, _)) = body.deserialize::<(u32, u32)>() {
                        out.exit = exit;
                    }
                    break;
                }
                Some("Destroy") => break,
                _ => {}
            }
        }
    };
    tokio::time::timeout(PATIENCE, read).await.map_err(|_| failed(format!("{method} did not finish")))?;
    Ok(out)
}

pub async fn install(packages: &[&str], label: &str, mut progress: Option<Progress<'_, '_>>) -> Result<()> {
    if packages.is_empty() {
        return Err(Error::Unavailable(format!("{label}: no package of it for this distribution")));
    }
    let conn = zbus::Connection::system().await.map_err(failed)?;
    let resolved = transaction(&conn, "Resolve", &(NEWEST, packages.to_vec()), label, &mut None).await?;
    if !resolved.error.is_empty() {
        return Err(failed(resolved.error));
    }
    let name_of = |id: &str| id.split(';').next().unwrap_or("").to_string();
    let missing: Vec<&str> =
        packages.iter().copied().filter(|p| !resolved.packages.iter().any(|(_, id)| name_of(id) == p.split([':', '.']).next().unwrap_or(p))).collect();
    if !missing.is_empty() {
        return Err(Error::NotFound(format!("the distribution's repositories have no {}", missing.join(", "))));
    }
    let ids: Vec<String> = resolved.packages.into_iter().filter(|(info, _)| *info != INFO_INSTALLED).map(|(_, id)| id).collect();
    if ids.is_empty() {
        return Ok(());
    }
    let done = transaction(&conn, "InstallPackages", &(ONLY_TRUSTED, ids), &format!("Installing {label}"), &mut progress).await?;
    if done.exit != EXIT_SUCCESS {
        return Err(failed(if done.error.is_empty() { format!("the install ended with exit {}", done.exit) } else { done.error }));
    }
    Ok(())
}
