use crate::{settings::SettingsStore, AppState};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const UPDATE_STATUS_EVENT: &str = "update-status-changed";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatusPayload {
    pub status: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub error: Option<String>,
}

impl UpdateStatusPayload {
    fn idle(current_version: String) -> Self {
        Self {
            status: "idle".to_string(),
            current_version,
            latest_version: None,
            notes: None,
            pub_date: None,
            downloaded_bytes: None,
            total_bytes: None,
            error: None,
        }
    }
}

pub struct UpdateManager {
    status: UpdateStatusPayload,
    pending_update: Option<Update>,
    downloaded_package: Option<Vec<u8>>,
    check_in_progress: bool,
    download_in_progress: bool,
    install_in_progress: bool,
}

impl UpdateManager {
    pub fn new(current_version: String) -> Self {
        Self {
            status: UpdateStatusPayload::idle(current_version),
            pending_update: None,
            downloaded_package: None,
            check_in_progress: false,
            download_in_progress: false,
            install_in_progress: false,
        }
    }

    fn current_version(&self) -> String {
        self.status.current_version.clone()
    }

    fn reset_to_idle(&mut self) {
        let current_version = self.current_version();
        self.status = UpdateStatusPayload::idle(current_version);
        self.pending_update = None;
        self.downloaded_package = None;
        self.check_in_progress = false;
        self.download_in_progress = false;
        self.install_in_progress = false;
    }

    fn set_error(&mut self, message: String) {
        self.status.status = "error".to_string();
        self.status.error = Some(message);
        self.check_in_progress = false;
        self.download_in_progress = false;
        self.install_in_progress = false;
    }

    fn refresh_transient_status(&mut self) {
        if self.install_in_progress {
            self.status.status = "installing".to_string();
            return;
        }
        if self.download_in_progress {
            self.status.status = "downloading".to_string();
            return;
        }
        if self.downloaded_package.is_some() {
            self.status.status = "downloaded".to_string();
            self.status.error = None;
            return;
        }
        if self.pending_update.is_some() {
            self.status.status = "available".to_string();
            self.status.error = None;
            return;
        }
        self.status.status = "upToDate".to_string();
        self.status.latest_version = None;
        self.status.notes = None;
        self.status.pub_date = None;
        self.status.downloaded_bytes = None;
        self.status.total_bytes = None;
        self.status.error = None;
    }
}

fn emit_status(app: &AppHandle, status: &UpdateStatusPayload) {
    let _ = app.emit(UPDATE_STATUS_EVENT, status);
}

fn with_manager<R>(app: &AppHandle, updater: impl FnOnce(&mut UpdateManager) -> R) -> Result<R, String> {
    let state = app.state::<AppState>();
    let mut manager = state
        .updater_manager
        .lock()
        .map_err(|_| "更新状态锁获取失败".to_string())?;
    Ok(updater(&mut manager))
}

fn update_metadata(update: &Update) -> (Option<String>, Option<String>) {
    let notes = update.body.clone().filter(|value| !value.trim().is_empty());
    let pub_date = update
        .raw_json
        .get("pub_date")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    (notes, pub_date)
}

fn should_auto_check(store: &SettingsStore) -> bool {
    store
        .load()
        .map(|settings| settings.startup.auto_check_updates)
        .unwrap_or(true)
}

pub fn get_status(app: &AppHandle) -> Result<UpdateStatusPayload, String> {
    with_manager(app, |manager| manager.status.clone())
}

pub fn dismiss_error(app: &AppHandle) -> Result<(), String> {
    let next_status = with_manager(app, |manager| {
        manager.status.error = None;
        manager.refresh_transient_status();
        manager.status.clone()
    })?;
    emit_status(app, &next_status);
    Ok(())
}

pub fn schedule_update_check(app: AppHandle, store: SettingsStore, force: bool) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = check_for_updates(app.clone(), store, force).await {
            if let Ok(status) = with_manager(&app, |manager| {
                manager.set_error(error.clone());
                manager.status.clone()
            }) {
                emit_status(&app, &status);
            }
        }
    });
}

async fn check_for_updates(app: AppHandle, store: SettingsStore, force: bool) -> Result<(), String> {
    if !force && !should_auto_check(&store) {
        let status = with_manager(&app, |manager| {
            if manager.pending_update.is_none()
                && manager.downloaded_package.is_none()
                && !manager.download_in_progress
                && !manager.install_in_progress
            {
                manager.reset_to_idle();
            }
            manager.status.clone()
        })?;
        emit_status(&app, &status);
        return Ok(());
    }

    let current_status = with_manager(&app, |manager| {
        if manager.check_in_progress || manager.download_in_progress || manager.install_in_progress {
            return Some(manager.status.clone());
        }
        manager.check_in_progress = true;
        manager.status.status = "checking".to_string();
        manager.status.error = None;
        Some(manager.status.clone())
    })?;
    if let Some(status) = current_status {
        emit_status(&app, &status);
        if status.status != "checking" {
            return Ok(());
        }
    }

    let deferred_version = store.load_deferred_update_version().unwrap_or(None);
    let update = app
        .updater()
        .map_err(|err| err.to_string())?
        .check()
        .await
        .map_err(|err| err.to_string())?;

    match update {
        Some(update) => {
            let latest_version = update.version.clone();
            let (notes, pub_date) = update_metadata(&update);
            store
                .save_deferred_update_version(Some(&latest_version))
                .map_err(|err| err.to_string())?;

            let should_install_after_download = deferred_version
                .as_deref()
                .map(|version| version == latest_version)
                .unwrap_or(false);

            let status = with_manager(&app, |manager| {
                manager.pending_update = Some(update);
                manager.downloaded_package = None;
                manager.check_in_progress = false;
                manager.download_in_progress = false;
                manager.install_in_progress = false;
                manager.status.status = "available".to_string();
                manager.status.latest_version = Some(latest_version);
                manager.status.notes = notes;
                manager.status.pub_date = pub_date;
                manager.status.downloaded_bytes = Some(0);
                manager.status.total_bytes = None;
                manager.status.error = None;
                manager.status.clone()
            })?;
            emit_status(&app, &status);
            download_pending_update(app, should_install_after_download);
        }
        None => {
            store
                .save_deferred_update_version(None)
                .map_err(|err| err.to_string())?;
            let status = with_manager(&app, |manager| {
                manager.reset_to_idle();
                manager.status.status = "upToDate".to_string();
                manager.status.clone()
            })?;
            emit_status(&app, &status);
        }
    }

    Ok(())
}

fn download_pending_update(app: AppHandle, install_after_download: bool) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = download_pending_update_inner(app.clone(), install_after_download).await {
            if let Ok(status) = with_manager(&app, |manager| {
                manager.set_error(error.clone());
                manager.status.clone()
            }) {
                emit_status(&app, &status);
            }
        }
    });
}

async fn download_pending_update_inner(app: AppHandle, install_after_download: bool) -> Result<(), String> {
    let update = with_manager(&app, |manager| {
        if manager.download_in_progress || manager.install_in_progress {
            return None;
        }
        let update = manager.pending_update.take()?;
        manager.download_in_progress = true;
        manager.status.status = "downloading".to_string();
        manager.status.downloaded_bytes = Some(0);
        manager.status.total_bytes = None;
        manager.status.error = None;
        Some(update)
    })?;

    let Some(update) = update else {
        return Ok(());
    };

    let mut downloaded_bytes = 0u64;
    let bytes = update
        .download(
            |chunk_length, total_bytes| {
                downloaded_bytes += chunk_length as u64;
                if let Ok(status) = with_manager(&app, |manager| {
                    manager.status.status = "downloading".to_string();
                    manager.status.downloaded_bytes = Some(downloaded_bytes);
                    manager.status.total_bytes = total_bytes;
                    manager.status.error = None;
                    manager.status.clone()
                }) {
                    emit_status(&app, &status);
                }
            },
            || {},
        )
        .await
        .map_err(|err| err.to_string())?;

    let latest_version = update.version.clone();
    let status = with_manager(&app, |manager| {
        manager.pending_update = Some(update);
        manager.downloaded_package = Some(bytes);
        manager.download_in_progress = false;
        manager.status.status = "downloaded".to_string();
        manager.status.latest_version = Some(latest_version);
        manager.status.downloaded_bytes = manager.status.downloaded_bytes.or(Some(downloaded_bytes));
        manager.status.total_bytes = manager.status.total_bytes.or(manager.status.downloaded_bytes);
        manager.status.error = None;
        manager.status.clone()
    })?;
    emit_status(&app, &status);

    if install_after_download {
        install_downloaded_update(&app)?;
    }

    Ok(())
}

pub fn install_downloaded_update(app: &AppHandle) -> Result<(), String> {
    let install_target = with_manager(app, |manager| {
        if manager.install_in_progress {
            return None;
        }
        let update = manager.pending_update.take()?;
        let bytes = manager.downloaded_package.take()?;
        manager.install_in_progress = true;
        manager.status.status = "installing".to_string();
        manager.status.error = None;
        Some((update, bytes))
    })?;

    let Some((update, bytes)) = install_target else {
        return Err("当前没有可安装的更新".to_string());
    };

    let settings_store = {
        let state = app.state::<AppState>();
        state.settings_store.clone()
    };

    match update.install(&bytes) {
        Ok(()) => {
            settings_store
                .save_deferred_update_version(None)
                .map_err(|err| err.to_string())?;
            app.restart();
        }
        Err(error) => {
            let message = error.to_string();
            let status = with_manager(app, |manager| {
                manager.pending_update = Some(update);
                manager.downloaded_package = Some(bytes);
                manager.install_in_progress = false;
                manager.set_error(message.clone());
                manager.status.clone()
            })?;
            emit_status(app, &status);
            Err(message)
        }
    }
}

pub fn handle_settings_changed(app: AppHandle, store: SettingsStore) {
    if !should_auto_check(&store) {
        if let Ok(status) = with_manager(&app, |manager| {
            if manager.pending_update.is_none()
                && manager.downloaded_package.is_none()
                && !manager.download_in_progress
                && !manager.install_in_progress
            {
                manager.reset_to_idle();
            }
            manager.status.clone()
        }) {
            emit_status(&app, &status);
        }
        return;
    }
    schedule_update_check(app, store, false);
}

pub fn maybe_install_on_quit(app: &AppHandle, store: &SettingsStore) -> Result<bool, String> {
    let settings = store.load().map_err(|err| err.to_string())?;
    if !settings.startup.auto_install_updates_on_quit {
        return Ok(false);
    }
    let has_downloaded_update = with_manager(app, |manager| manager.downloaded_package.is_some())?;
    if !has_downloaded_update {
        return Ok(false);
    }
    install_downloaded_update(app)?;
    Ok(true)
}