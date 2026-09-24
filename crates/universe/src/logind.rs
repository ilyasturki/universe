use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Suspend,
    Reboot,
    PowerOff,
}

impl PowerAction {
    pub const ALL: [PowerAction; 3] = [PowerAction::Suspend, PowerAction::Reboot, PowerAction::PowerOff];

    pub fn id(self) -> &'static str {
        match self {
            PowerAction::Suspend => "suspend",
            PowerAction::Reboot => "reboot",
            PowerAction::PowerOff => "power_off",
        }
    }

    pub fn parse(id: &str) -> Option<PowerAction> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }

    fn method(self) -> &'static str {
        match self {
            PowerAction::Suspend => "Suspend",
            PowerAction::Reboot => "Reboot",
            PowerAction::PowerOff => "PowerOff",
        }
    }
}

async fn manager() -> zbus::Result<zbus::Proxy<'static>> {
    let conn = zbus::Connection::system().await?;
    zbus::Proxy::new(&conn, "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager").await
}

/// logind's `Can*` answer (`yes`, `no`, `na`, `challenge`, `inhibited`), `None` when logind cannot be asked.
pub async fn can(action: PowerAction) -> Option<String> {
    let call = async { manager().await?.call::<_, _, String>(&*format!("Can{}", action.method()), &()).await };
    match tokio::time::timeout(Duration::from_secs(5), call).await {
        Ok(Ok(answer)) => Some(answer),
        Ok(Err(e)) => {
            tracing::warn!("Can{}: {e}", action.method());
            None
        }
        Err(_) => {
            tracing::warn!("Can{}: timeout", action.method());
            None
        }
    }
}

/// Interactive: where polkit wants a password, the desktop's agent asks for it.
pub async fn request(action: PowerAction) -> Result<(), String> {
    let call = async { manager().await?.call::<_, _, ()>(action.method(), &(true,)).await };
    // Long enough for a password typed into the polkit agent's prompt.
    match tokio::time::timeout(Duration::from_secs(120), call).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(zbus::Error::MethodError(_, Some(message), _))) => Err(message),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("logind did not answer".into()),
    }
}
