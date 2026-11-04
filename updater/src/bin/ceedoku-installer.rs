#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zip::ZipArchive;

const INSTALLER_PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/installer-payload.zip"));
const INSTALLER_ICON: &[u8] = include_bytes!("../../../assets/ceedoku.png");

enum InstallMessage {
    Progress(f32),
    Complete,
    Error(String),
}

const CREATE_NO_WINDOW: u32 = 0x08000000;

struct InstallerApp {
    receiver: Receiver<InstallMessage>,
    status: String,
    progress: f32,
    error: Option<String>,
}

impl InstallerApp {
    fn new(receiver: Receiver<InstallMessage>) -> Self {
        Self {
            receiver,
            status: "Installing Ceedoku - 1%".to_owned(),
            progress: 0.01,
            error: None,
        }
    }
}

impl eframe::App for InstallerApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                InstallMessage::Progress(progress) => {
                    self.progress = progress.clamp(0.01, 1.0);
                    self.status = format!("Installing Ceedoku - {:.0}%", self.progress * 100.0);
                }
                InstallMessage::Complete => {
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                InstallMessage::Error(error) => self.error = Some(error),
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
                        egui::RichText::new("Ceedoku is installing")
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

                    if let Some(error) = &self.error {
                        ui.label(
                            egui::RichText::new(error)
                                .color(egui::Color32::from_rgb(255, 150, 140)),
                        );
                        ui.add_space(12.0);
                        if ui.button("Close").clicked() {
                            context.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    } else {
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                            ui.add(
                                egui::ProgressBar::new(self.progress)
                                    .fill(egui::Color32::from_rgb(115, 167, 255))
                                    .desired_height(18.0),
                            );
                            ui.add_space(16.0);
                            ui.label(
                                egui::RichText::new(&self.status)
                                    .color(egui::Color32::from_rgb(149, 164, 176)),
                            );
                        });
                    }
                });
            });

        context.request_repaint_after(Duration::from_millis(100));
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Ceedoku installation failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--ui-test") {
        return run_ui_test();
    }

    let install_dir = args
        .windows(2)
        .find(|pair| pair[0] == "--install-dir")
        .map(|pair| PathBuf::from(&pair[1]));
    let install_dir = match install_dir {
        Some(path) => path,
        None => install_directory()?,
    };

    if args.iter().any(|arg| arg == "--uninstall") {
        return uninstall(&install_dir);
    }

    if args
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("/S") || arg == "--silent")
    {
        return install_payload(&install_dir, &|_| {});
    }

    let icon = load_icon(INSTALLER_ICON)?;
    let (sender, receiver) = mpsc::channel();
    let worker_sender = sender.clone();
    let worker_install_dir = install_dir.clone();
    let worker = thread::spawn(move || {
        let result = install_payload(&worker_install_dir, &|progress| {
            let _ = worker_sender.send(InstallMessage::Progress(progress));
        });
        match result {
            Ok(()) => {
                thread::sleep(Duration::from_millis(500));
                let _ = worker_sender.send(InstallMessage::Complete);
                Ok(())
            }
            Err(error) => {
                let _ = worker_sender.send(InstallMessage::Error(error.to_string()));
                Err(error.to_string())
            }
        }
    });

    let native_options = eframe::NativeOptions {
        centered: true,
        viewport: egui::ViewportBuilder::default()
            .with_title("Ceedoku Installer")
            .with_inner_size([440.0, 250.0])
            .with_min_inner_size([440.0, 250.0])
            .with_max_inner_size([440.0, 250.0])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_icon(icon),
        ..Default::default()
    };

    let ui_result = eframe::run_native(
        "Ceedoku Installer",
        native_options,
        Box::new(move |_| Ok(Box::new(InstallerApp::new(receiver)))),
    );
    let worker_result = worker
        .join()
        .map_err(|_| "installer worker stopped unexpectedly")?;
    ui_result?;
    worker_result.map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
    Ok(())
}

fn run_ui_test() -> Result<(), Box<dyn std::error::Error>> {
    let icon = load_icon(INSTALLER_ICON)?;
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        for percent in 1..=100 {
            let _ = sender.send(InstallMessage::Progress(percent as f32 / 100.0));
            thread::sleep(Duration::from_millis(100));
        }
    });

    let native_options = eframe::NativeOptions {
        centered: true,
        viewport: egui::ViewportBuilder::default()
            .with_title("Ceedoku Installer UI Test")
            .with_inner_size([440.0, 250.0])
            .with_min_inner_size([440.0, 250.0])
            .with_max_inner_size([440.0, 250.0])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_icon(icon),
        ..Default::default()
    };

    let result = eframe::run_native(
        "Ceedoku Installer UI Test",
        native_options,
        Box::new(move |_| Ok(Box::new(InstallerApp::new(receiver)))),
    );
    if worker.join().is_err() {
        return Err("installer UI test worker stopped unexpectedly".into());
    }
    result?;
    Ok(())
}

fn install_directory() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let local_app_data = env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is not set")?;
    Ok(PathBuf::from(local_app_data)
        .join("Programs")
        .join("Ceedoku Client"))
}

fn load_icon(bytes: &[u8]) -> Result<egui::IconData, Box<dyn std::error::Error>> {
    let image = image::load_from_memory(bytes)?.into_rgba8();
    let (width, height) = image.dimensions();
    Ok(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn install_payload(
    install_dir: &Path,
    report_progress: &dyn Fn(f32),
) -> Result<(), Box<dyn std::error::Error>> {
    if INSTALLER_PAYLOAD.is_empty() {
        return Err(
            "installer payload is missing; rebuild with CEEDOKU_INSTALLER_PAYLOAD set".into(),
        );
    }

    let parent = install_dir
        .parent()
        .ok_or("could not determine the installation parent directory")?;
    fs::create_dir_all(parent)?;

    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let stage = parent.join(format!(
        ".ceedoku-installing-{}-{nonce}",
        std::process::id()
    ));
    let backup = parent.join(format!(".ceedoku-previous-{}-{nonce}", std::process::id()));
    remove_directory_if_present(&stage)?;
    fs::create_dir_all(&stage)?;

    let result = extract_payload(&stage, report_progress);
    if let Err(error) = result {
        remove_directory_if_present(&stage)?;
        return Err(error);
    }

    let uninstaller = stage.join("Uninstall Ceedoku.ps1");
    fs::write(&uninstaller, uninstaller_script(install_dir))?;
    report_progress(0.92);

    if install_dir.exists() {
        fs::rename(install_dir, &backup)?;
    }
    if let Err(error) = fs::rename(&stage, install_dir) {
        if backup.exists() {
            fs::rename(&backup, install_dir)?;
        }
        return Err(error.into());
    }

    let setup_result = configure_installation(install_dir);
    if setup_result.is_err() && backup.exists() {
        remove_directory_if_present(install_dir)?;
        fs::rename(&backup, install_dir)?;
        return setup_result;
    }
    setup_result?;
    remove_directory_if_present(&backup)?;
    report_progress(1.0);
    Ok(())
}

fn extract_payload(
    destination: &Path,
    report_progress: &dyn Fn(f32),
) -> Result<(), Box<dyn std::error::Error>> {
    let mut archive = ZipArchive::new(std::io::Cursor::new(INSTALLER_PAYLOAD))?;
    let mut total_bytes = 0u64;
    for index in 0..archive.len() {
        total_bytes += archive.by_index(index)?.size();
    }
    if total_bytes == 0 {
        return Err("installer payload is empty".into());
    }

    let mut extracted_bytes = 0u64;
    let mut buffer = [0; 128 * 1024];
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_symlink() {
            return Err("installer payload contains an unsupported symbolic link".into());
        }
        let relative_path = entry
            .enclosed_name()
            .ok_or("installer payload contains an invalid path")?
            .to_owned();
        let output_path = destination.join(relative_path);
        if entry.is_dir() {
            fs::create_dir_all(output_path)?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::File::create(output_path)?;
        loop {
            let count = entry.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count])?;
            extracted_bytes += count as u64;
            report_progress(0.01 + (extracted_bytes as f64 / total_bytes as f64) as f32 * 0.89);
        }
    }
    Ok(())
}

fn configure_installation(install_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let install = powershell_quote(install_dir);
    let executable = powershell_quote(&install_dir.join("Ceedoku Client.exe"));
    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
$install = {install}
$exe = {executable}
$shell = New-Object -ComObject WScript.Shell
$startMenu = Join-Path ([Environment]::GetFolderPath('Programs')) 'Ceedoku'
New-Item -ItemType Directory -Path $startMenu -Force | Out-Null
foreach ($shortcutPath in @((Join-Path $startMenu 'Ceedoku.lnk'), (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Ceedoku.lnk'))) {{
    $shortcut = $shell.CreateShortcut($shortcutPath)
    $shortcut.TargetPath = $exe
    $shortcut.WorkingDirectory = $install
    $shortcut.IconLocation = "$exe,0"
    $shortcut.Save()
}}
$appPaths = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\App Paths\Ceedoku.exe'
New-Item -Path $appPaths -Force | Out-Null
Set-Item -Path $appPaths -Value $exe
Set-ItemProperty -Path $appPaths -Name Path -Value $install
$extension = 'HKCU:\Software\Classes\.csf'
New-Item -Path $extension -Force | Out-Null
Set-Item -Path $extension -Value 'Ceedoku.SaveFile'
$saveFile = 'HKCU:\Software\Classes\Ceedoku.SaveFile'
New-Item -Path $saveFile -Force | Out-Null
Set-Item -Path $saveFile -Value 'Ceedoku Save File'
$iconKey = Join-Path $saveFile 'DefaultIcon'
New-Item -Path $iconKey -Force | Out-Null
Set-Item -Path $iconKey -Value "$exe,0"
$commandKey = Join-Path $saveFile 'shell\open\command'
New-Item -Path $commandKey -Force | Out-Null
Set-Item -Path $commandKey -Value ('"' + $exe + '" "%1"')
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Ceedoku Client'
New-Item -Path $uninstallKey -Force | Out-Null
Set-ItemProperty -Path $uninstallKey -Name DisplayName -Value 'Ceedoku Client'
Set-ItemProperty -Path $uninstallKey -Name DisplayIcon -Value $exe
Set-ItemProperty -Path $uninstallKey -Name UninstallString -Value ('powershell.exe -NoProfile -ExecutionPolicy Bypass -File "' + (Join-Path $install 'Uninstall Ceedoku.ps1') + '"')
New-ItemProperty -Path $uninstallKey -Name NoModify -Value 1 -PropertyType DWord -Force | Out-Null
New-ItemProperty -Path $uninstallKey -Name NoRepair -Value 1 -PropertyType DWord -Force | Out-Null"#,
    );
    run_powershell(&script)
}

fn uninstaller_script(install_dir: &Path) -> String {
    let install = powershell_quote(install_dir);
    format!(
        r#"$ErrorActionPreference = 'Stop'
$install = {install}
$startMenu = Join-Path ([Environment]::GetFolderPath('Programs')) 'Ceedoku'
Remove-Item -LiteralPath (Join-Path $startMenu 'Ceedoku.lnk') -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $startMenu -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Ceedoku.lnk') -Force -ErrorAction SilentlyContinue
Remove-Item -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\App Paths\Ceedoku.exe' -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Ceedoku Client' -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path 'HKCU:\Software\Classes\.csf' -Force -ErrorAction SilentlyContinue
Remove-Item -Path 'HKCU:\Software\Classes\Ceedoku.SaveFile' -Recurse -Force -ErrorAction SilentlyContinue
$cleanup = Join-Path $env:TEMP ('ceedoku-uninstall-' + [guid]::NewGuid().ToString() + '.ps1')
@'
Start-Sleep -Seconds 2
Remove-Item -LiteralPath {install} -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $PSCommandPath -Force -ErrorAction SilentlyContinue
'@ | Set-Content -LiteralPath $cleanup
Start-Process powershell.exe -WindowStyle Hidden -ArgumentList ('-NoProfile -ExecutionPolicy Bypass -File "' + $cleanup + '"')"#
    )
}

fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

fn run_powershell(script: &str) -> Result<(), Box<dyn std::error::Error>> {
    let status = powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .status()?;
    if !status.success() {
        return Err(format!("Windows setup command failed with {status}").into());
    }
    Ok(())
}

fn uninstall(install_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let script = install_dir.join("Uninstall Ceedoku.ps1");
    if !script.is_file() {
        return Err(format!("uninstaller is missing: {}", script.display()).into());
    }
    let status = powershell_command()
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script)
        .status()?;
    if !status.success() {
        return Err(format!("uninstaller failed with {status}").into());
    }
    Ok(())
}

fn powershell_command() -> Command {
    let mut command = Command::new("powershell.exe");
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

fn remove_directory_if_present(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}
