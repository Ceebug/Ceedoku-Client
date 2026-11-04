use eframe::egui;
use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const RELEASES_URL: &str = "https://api.github.com/repos/ceebug/ceedoku-client/releases/latest";
const GAME_URL: &str = "https://ceedoku.github.io/";

#[derive(Clone, Debug)]
struct Options {
    app_path: PathBuf,
    app_version: String,
    user_data: PathBuf,
    icon_path: PathBuf,
    app_args: Vec<String>,
}

struct UpdatePlan {
    client_update: bool,
    content_update: bool,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

enum UiMessage {
    Status(String),
    Progress { status: String, progress: f32 },
    Warning(String),
    Complete,
    Error(String),
}

struct UpdaterApp {
    receiver: Receiver<UiMessage>,
    launch_state: Arc<AtomicBool>,
    status: String,
    error: Option<String>,
    warning: Option<String>,
    progress: Option<f32>,
    finished: bool,
}

impl UpdaterApp {
    fn new(receiver: Receiver<UiMessage>, launch_state: Arc<AtomicBool>) -> Self {
        Self {
            receiver,
            launch_state,
            status: "Checking for updates...".to_owned(),
            error: None,
            warning: None,
            progress: None,
            finished: false,
        }
    }
}

impl Drop for UpdaterApp {
    fn drop(&mut self) {
        if !self.finished {
            self.launch_state.store(true, Ordering::Relaxed);
        }
    }
}

impl eframe::App for UpdaterApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                UiMessage::Status(status) => self.status = status,
                UiMessage::Progress { status, progress } => {
                    self.status = status;
                    self.progress = Some(progress.clamp(0.0, 1.0));
                }
                UiMessage::Warning(warning) => self.warning = Some(warning),
                UiMessage::Complete => {
                    self.launch_state.store(true, Ordering::Relaxed);
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                UiMessage::Error(error) => {
                    self.error = Some(error);
                    self.finished = true;
                }
            }
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(16, 20, 24))
                    .inner_margin(24),
            )
            .show(context, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(12.0);
                    ui.heading(
                        egui::RichText::new("Ceedoku is updating")
                            .size(24.0)
                            .color(egui::Color32::from_rgb(235, 240, 244)),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("It will launch when it is completed.")
                            .size(15.0)
                            .color(egui::Color32::from_rgb(180, 191, 199)),
                    );
                    ui.add_space(22.0);

                    if self.finished {
                        if let Some(error) = &self.error {
                            ui.label(
                                egui::RichText::new(error)
                                    .color(egui::Color32::from_rgb(255, 150, 140)),
                            );
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                if ui.button("Launch current version").clicked() {
                                    self.launch_state.store(true, Ordering::Relaxed);
                                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                if ui.button("Quit").clicked() {
                                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            });
                        }
                    } else {
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                            if let Some(progress) = self.progress {
                                ui.add(
                                    egui::ProgressBar::new(progress)
                                        .fill(egui::Color32::from_rgb(115, 167, 255))
                                        .desired_height(18.0),
                                );
                                ui.add_space(16.0);
                            }
                            ui.label(
                                egui::RichText::new(&self.status)
                                    .color(egui::Color32::from_rgb(149, 164, 176)),
                            );
                            if let Some(warning) = &self.warning {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(warning)
                                        .color(egui::Color32::from_rgb(255, 190, 130)),
                                );
                            }
                        });
                    }
                });
            });

        context.request_repaint_after(Duration::from_millis(100));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|argument| argument == "--ui-test") {
        let icon_path = args
            .windows(2)
            .find(|pair| pair[0] == "--icon")
            .map(|pair| PathBuf::from(&pair[1]))
            .ok_or("--ui-test requires --icon <path>")?;
        run_ui_test(&icon_path)?;
        return Ok(());
    }

    let options = parse_options()?;
    let update_plan = check_for_updates(&options);
    if !update_plan.client_update && !update_plan.content_update {
        launch_game(&options)?;
        return Ok(());
    }

    let icon = load_icon(&options.icon_path)?;
    let (sender, receiver) = mpsc::channel();
    let worker_options = options.clone();
    let worker_sender = sender.clone();
    let worker_update_plan = update_plan;
    let launch_state = Arc::new(AtomicBool::new(false));
    let app_launch_state = launch_state.clone();

    let worker = thread::spawn(move || {
        if let Err(error) = run_updates(&worker_options, &worker_sender, worker_update_plan) {
            let _ = worker_sender.send(UiMessage::Error(error.to_string()));
            return;
        }
        let _ = worker_sender.send(UiMessage::Complete);
    });

    let native_options = eframe::NativeOptions {
        centered: true,
        viewport: egui::ViewportBuilder::default()
            .with_title("Ceedoku is updating")
            .with_inner_size([440.0, 250.0])
            .with_min_inner_size([440.0, 250.0])
            .with_max_inner_size([440.0, 250.0])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "Ceedoku is updating",
        native_options,
        Box::new(move |_| Ok(Box::new(UpdaterApp::new(receiver, app_launch_state)))),
    )?;
    if worker.join().is_err() {
        return Err("native update worker stopped unexpectedly".into());
    }

    if launch_state.load(Ordering::Relaxed) {
        launch_game(&options)?;
    }
    Ok(())
}

fn run_ui_test(icon_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let icon = load_icon(icon_path)?;
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let _ = sender.send(UiMessage::Status("Downloading update - 0%".to_owned()));
        for downloaded in 0..=100 {
            let progress = downloaded as f32 / 100.0;
            let _ = sender.send(UiMessage::Progress {
                status: format!("Downloading update - {:.0}%", progress * 50.0),
                progress: progress * 0.5,
            });
            thread::sleep(Duration::from_millis(40));
        }
        for installed in 0..=100 {
            let progress = installed as f32 / 100.0;
            let overall_progress = 0.5 + progress * 0.5;
            let _ = sender.send(UiMessage::Progress {
                status: format!("Installing update - {:.0}%", overall_progress * 100.0),
                progress: overall_progress,
            });
            thread::sleep(Duration::from_millis(80));
        }
    });

    let launch_state = Arc::new(AtomicBool::new(false));
    let app_launch_state = launch_state.clone();
    let native_options = eframe::NativeOptions {
        centered: true,
        viewport: egui::ViewportBuilder::default()
            .with_title("Ceedoku Updater UI Test")
            .with_inner_size([440.0, 250.0])
            .with_min_inner_size([440.0, 250.0])
            .with_max_inner_size([440.0, 250.0])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_icon(icon),
        ..Default::default()
    };

    let result = eframe::run_native(
        "Ceedoku Updater UI Test",
        native_options,
        Box::new(move |_| Ok(Box::new(UpdaterApp::new(receiver, app_launch_state)))),
    );
    if worker.join().is_err() {
        return Err("updater UI test worker stopped unexpectedly".into());
    }
    result?;
    Ok(())
}

fn check_for_updates(options: &Options) -> UpdatePlan {
    let cached_index_path = options
        .user_data
        .join("cache")
        .join("current")
        .join("index.html");
    let has_cached_game = cached_index_path.is_file();
    let client = match Client::builder()
        .user_agent("Ceedoku-Client-Updater")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            eprintln!("Could not prepare update checks: {error}");
            return UpdatePlan {
                client_update: false,
                content_update: !has_cached_game,
            };
        }
    };

    let client_update = match client
        .get(RELEASES_URL)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<Release>())
    {
        Ok(release) => match is_newer_version(
            &options.app_version,
            release.tag_name.trim_start_matches('v'),
        ) {
            Ok(is_newer) => is_newer,
            Err(error) => {
                eprintln!("Could not compare client versions: {error}");
                false
            }
        },
        Err(error) => {
            eprintln!("Could not check for a client update: {error}");
            false
        }
    };

    let content_update = match client
        .get(GAME_URL)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.bytes())
    {
        Ok(latest_index) => match fs::read(cached_index_path) {
            Ok(cached_index) => game_index_changed(Some(&cached_index), &latest_index),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                game_index_changed(None, &latest_index)
            }
            Err(error) => {
                eprintln!("Could not read the cached Ceedoku index: {error}");
                true
            }
        },
        Err(error) => {
            eprintln!("Could not check for Ceedoku content updates: {error}");
            !has_cached_game
        }
    };

    UpdatePlan {
        client_update,
        content_update,
    }
}

fn game_index_changed(cached_index: Option<&[u8]>, latest_index: &[u8]) -> bool {
    cached_index != Some(latest_index)
}

fn parse_options() -> Result<Options, Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let mut app_path = None;
    let mut app_version = None;
    let mut user_data = None;
    let mut icon_path = None;
    let mut app_args = Vec::new();

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--app-path" => app_path = args.next().map(PathBuf::from),
            "--app-version" => app_version = args.next(),
            "--user-data" => user_data = args.next().map(PathBuf::from),
            "--icon" => icon_path = args.next().map(PathBuf::from),
            "--app-arg" => {
                if let Some(value) = args.next() {
                    app_args.push(value);
                }
            }
            _ => {}
        }
    }

    Ok(Options {
        app_path: app_path.ok_or("missing --app-path")?,
        app_version: app_version.ok_or("missing --app-version")?,
        user_data: user_data.ok_or("missing --user-data")?,
        icon_path: icon_path.ok_or("missing --icon")?,
        app_args,
    })
}

fn load_icon(path: &Path) -> Result<egui::IconData, Box<dyn std::error::Error>> {
    let image = image::load_from_memory(&fs::read(path)?)?.into_rgba8();
    let (width, height) = image.dimensions();
    Ok(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn run_updates(
    options: &Options,
    sender: &Sender<UiMessage>,
    update_plan: UpdatePlan,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .user_agent("Ceedoku-Client-Updater")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(900))
        .build()?;

    if update_plan.client_update {
        let _ = sender.send(UiMessage::Status(
            "Checking for a client update...".to_owned(),
        ));
        match check_client_update(&client, options, sender) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("Client update could not be completed: {error}");
                let _ = sender.send(UiMessage::Warning(format!(
                    "Client update unavailable; continuing with the installed version: {error}"
                )));
                let _ = sender.send(UiMessage::Status("Preparing Ceedoku...".to_owned()));
            }
        }
    }

    if update_plan.content_update {
        let _ = sender.send(UiMessage::Status(
            "Updating Ceedoku game files...".to_owned(),
        ));
        let status = Command::new(&options.app_path)
            .args(&options.app_args)
            .arg("--ceedoku-content-update")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(format!("Ceedoku content update exited with {status}").into());
        }
    }
    Ok(())
}

fn check_client_update(
    client: &Client,
    options: &Options,
    sender: &Sender<UiMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let release: Release = client
        .get(RELEASES_URL)
        .send()?
        .error_for_status()?
        .json()?;
    let latest = release.tag_name.trim_start_matches('v');
    if !is_newer_version(&options.app_version, latest)? {
        return Ok(());
    }

    let asset_name = asset_name(env::consts::OS, env::consts::ARCH)
        .ok_or("unsupported platform for automatic client updates")?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| format!("Release {} has no {asset_name} asset", release.tag_name))?;

    let response = client
        .get(&asset.browser_download_url)
        .send()?
        .error_for_status()?;
    let total_bytes = response.content_length().filter(|total| *total > 0);
    let _ = sender.send(UiMessage::Progress {
        status: "Downloading update - 0%".to_owned(),
        progress: 0.0,
    });
    let update_dir = options.user_data.join("updates");
    fs::create_dir_all(&update_dir)?;
    let installer = update_dir.join(&asset.name);
    download_asset(
        response,
        &installer,
        asset.digest.as_deref(),
        total_bytes,
        sender,
    )?;

    install_update_with_progress(options, &installer, sender)?;
    Ok(())
}

fn download_asset(
    mut response: reqwest::blocking::Response,
    destination: &Path,
    expected_digest: Option<&str>,
    total_bytes: Option<u64>,
    sender: &Sender<UiMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = fs::File::create(destination)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut downloaded = 0u64;
    let mut next_progress = 0;
    send_update_progress(sender, "Downloading update", 0.0);

    loop {
        let count = response.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])?;
        hasher.update(&buffer[..count]);
        downloaded += count as u64;

        if downloaded >= next_progress {
            let phase_progress = total_bytes
                .map(|total| (downloaded as f64 / total as f64).clamp(0.0, 1.0) as f32)
                .unwrap_or(0.0);
            send_update_progress(sender, "Downloading update", phase_progress * 0.5);
            next_progress = downloaded + 256 * 1024;
        }
    }
    send_update_progress(sender, "Downloading update", 0.5);

    let actual_digest = format!("{:x}", hasher.finalize());
    if let Some(expected) = expected_digest.and_then(|value| value.strip_prefix("sha256:")) {
        if actual_digest != expected {
            let _ = fs::remove_file(destination);
            return Err("Downloaded client update failed its SHA-256 check".into());
        }
    }
    Ok(())
}

fn send_update_progress(sender: &Sender<UiMessage>, phase: &str, progress: f32) {
    let progress = progress.clamp(0.0, 1.0);
    let _ = sender.send(UiMessage::Progress {
        status: format!("{phase} - {:.0}%", progress * 100.0),
        progress,
    });
}

fn install_update_with_progress(
    options: &Options,
    installer: &Path,
    sender: &Sender<UiMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let install_finished = Arc::new(AtomicBool::new(false));
    let reporter_finished = install_finished.clone();
    let reporter_sender = sender.clone();
    let reporter = thread::spawn(move || {
        let started = Instant::now();
        while !reporter_finished.load(Ordering::Relaxed) {
            let elapsed_seconds = started.elapsed().as_secs_f32();
            let install_progress = 0.49 * (1.0 - (-elapsed_seconds / 30.0).exp());
            send_update_progress(
                &reporter_sender,
                "Installing update",
                0.5 + install_progress,
            );
            thread::sleep(Duration::from_millis(250));
        }
    });

    let install_result = install_update(options, installer);
    install_finished.store(true, Ordering::Relaxed);
    if reporter.join().is_err() {
        return Err("update progress reporter stopped unexpectedly".into());
    }
    install_result?;
    send_update_progress(sender, "Installing update", 1.0);

    Ok(())
}

fn is_newer_version(current: &str, latest: &str) -> Result<bool, semver::Error> {
    Ok(semver::Version::parse(latest.trim_start_matches('v'))?
        > semver::Version::parse(current.trim_start_matches('v'))?)
}

fn asset_name(os: &str, architecture: &str) -> Option<&'static str> {
    match os {
        "windows" => match architecture {
            "x86" => Some("Ceedoku-Installer_Windows-x86.exe"),
            "aarch64" => Some("Ceedoku-Installer_Windows-arm64.exe"),
            "x86_64" => Some("Ceedoku-Installer_Windows-x64.exe"),
            _ => None,
        },
        "macos" => Some("Ceedoku-Installer_MacOS-universal.dmg"),
        "linux" => match architecture {
            "arm" => Some("Ceedoku-Installer_Linux-arm32.AppImage"),
            "aarch64" => Some("Ceedoku-Installer_Linux-arm64.AppImage"),
            "x86_64" => Some("Ceedoku-Installer_Linux-x64.AppImage"),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn install_update(options: &Options, installer: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let install_dir = options
        .app_path
        .parent()
        .ok_or("could not locate the installed client directory")?;
    let status = Command::new(installer)
        .arg("/S")
        .arg("--install-dir")
        .arg(install_dir)
        .stdin(Stdio::null())
        .status()?;
    require_success(status)
}

#[cfg(target_os = "linux")]
fn install_update(options: &Options, installer: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    let current = env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .unwrap_or_else(|| options.app_path.clone());
    let staging = current.with_extension("AppImage.update");
    fs::copy(installer, &staging)?;
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o755))?;
    fs::rename(staging, current)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn install_update(options: &Options, installer: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mount = options.user_data.join("updates").join("mount");
    fs::create_dir_all(&mount)?;
    let attach = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
        .arg(&mount)
        .arg(installer)
        .status()?;
    require_success(attach)?;

    let result = (|| {
        let source = fs::read_dir(&mount)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "app"))
            .ok_or("release disk image did not contain an app bundle")?;
        let target = options
            .app_path
            .ancestors()
            .nth(3)
            .ok_or("could not locate the installed app bundle")?;
        let stage = options.user_data.join("updates").join("Ceedoku Client.app");
        if stage.exists() {
            fs::remove_dir_all(&stage)?;
        }
        let copy = Command::new("ditto").arg(&source).arg(&stage).status()?;
        require_success(copy)?;
        let backup = target.with_extension("app.previous");
        let script = options.user_data.join("updates").join("install-update.sh");
        let script_contents = format!(
            "set -e\nrm -rf {backup}\nmv {target} {backup}\nif ditto {stage} {target}; then\n  rm -rf {backup}\nelse\n  mv {backup} {target}\n  exit 1\nfi\n",
            backup = shell_quote(&backup),
            target = shell_quote(target),
            stage = shell_quote(&stage)
        );
        fs::write(&script, script_contents)?;
        let install = Command::new("sh").arg(&script).status()?;
        if !install.success() {
            let command = format!("sh {}", shell_quote(&script));
            let apple_script = format!(
                "do shell script \"{}\" with administrator privileges",
                command.replace('\\', "\\\\").replace('"', "\\\"")
            );
            let elevated_install = Command::new("osascript")
                .args(["-e", &apple_script])
                .status()?;
            require_success(elevated_install)?;
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })();

    let _ = Command::new("hdiutil")
        .args(["detach", "-force"])
        .arg(&mount)
        .status();
    result
}

#[cfg(target_os = "macos")]
fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn require_success(status: ExitStatus) -> Result<(), Box<dyn std::error::Error>> {
    if status.success() {
        Ok(())
    } else {
        Err(format!("installer exited with {status}").into())
    }
}

fn launch_game(options: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new(&options.app_path);
    command
        .args(&options.app_args)
        .arg("--ceedoku-skip-updater")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(target_os = "linux")]
    {
        if let Some(appimage) = env::var_os("APPIMAGE") {
            command.env("APPIMAGE", appimage);
        }
    }

    command.spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{asset_name, game_index_changed, is_newer_version};

    #[test]
    fn compares_release_versions_without_tag_prefix() {
        assert!(is_newer_version("1.0.1", "v1.1.0").unwrap());
        assert!(!is_newer_version("v1.1.0", "1.1.0").unwrap());
    }

    #[test]
    fn only_updates_game_content_when_the_index_changes() {
        assert!(game_index_changed(None, b"latest"));
        assert!(game_index_changed(Some(b"cached"), b"latest"));
        assert!(!game_index_changed(Some(b"same"), b"same"));
    }

    #[test]
    fn selects_release_asset_for_each_supported_platform() {
        assert_eq!(
            asset_name("windows", "x86_64"),
            Some("Ceedoku-Installer_Windows-x64.exe")
        );
        assert_eq!(
            asset_name("windows", "x86"),
            Some("Ceedoku-Installer_Windows-x86.exe")
        );
        assert_eq!(
            asset_name("windows", "aarch64"),
            Some("Ceedoku-Installer_Windows-arm64.exe")
        );
        assert_eq!(
            asset_name("macos", "aarch64"),
            Some("Ceedoku-Installer_MacOS-universal.dmg")
        );
        assert_eq!(
            asset_name("linux", "arm"),
            Some("Ceedoku-Installer_Linux-arm32.AppImage")
        );
        assert_eq!(
            asset_name("linux", "aarch64"),
            Some("Ceedoku-Installer_Linux-arm64.AppImage")
        );
        assert_eq!(
            asset_name("linux", "x86_64"),
            Some("Ceedoku-Installer_Linux-x64.AppImage")
        );
        assert_eq!(asset_name("freebsd", "x86_64"), None);
    }
}
