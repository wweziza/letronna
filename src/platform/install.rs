//! First-run registration on Windows: copies the executable into
//! `%LOCALAPPDATA%\Programs\Letronna`, adds a Start Menu shortcut, and writes the
//! Uninstall registry entry so Letronna shows up in Apps & features.
//! `letronna --uninstall` reverses all of it.

use crate::prelude::*;
use std::{os::windows::process::CommandExt, path::Path, process::Command};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn install_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(format!("Programs\\{BRAND}")))
}

fn shortcut() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("APPDATA")?)
            .join("Microsoft\\Windows\\Start Menu\\Programs")
            .join(format!("{BRAND}.lnk")),
    )
}

fn registry_key() -> Vec<u16> {
    wide(&format!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{BRAND}"
    ))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn powershell(script: &str) {
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

fn set_value(key: &[u16], name: &str, value: &str) {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};
    let name = wide(name);
    let data = wide(value);
    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            REG_SZ,
            data.as_ptr() as *const _,
            (data.len() * 2) as u32,
        );
    }
}

fn read_value(key: &[u16], name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    let name = wide(name);
    let mut buf = [0u16; 512];
    let mut size = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr() as *mut _,
            &mut size,
        )
    };
    (status == 0).then(|| {
        let len = (size as usize / 2).saturating_sub(1);
        String::from_utf16_lossy(&buf[..len])
    })
}

/// Returns false if another Letronna is already running, after bringing its window forward.
pub(crate) fn single_instance() -> bool {
    use windows_sys::Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
        System::Threading::CreateMutexW,
        UI::WindowsAndMessaging::{FindWindowW, SW_SHOW, SetForegroundWindow, ShowWindow},
    };
    let name = wide(r"Local\LetronnaSingleInstance");
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() || GetLastError() != ERROR_ALREADY_EXISTS {
            return true;
        }
        let class = wide("Zed::Window");
        let title = wide(BRAND);
        let hwnd = FindWindowW(class.as_ptr(), title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        }
    }
    false
}

/// Registers this build if it is not yet installed or is newer than the installed one.
/// Returns true when the installed copy was launched instead and this process should exit,
/// so the taskbar always sees one executable path.
pub(crate) fn ensure_installed() -> bool {
    let (Some(dir), Some(lnk), Ok(current)) = (install_dir(), shortcut(), std::env::current_exe())
    else {
        return false;
    };
    let target = dir.join(format!("{BRAND}.exe"));
    let key = registry_key();
    let installed_version = read_value(&key, "DisplayVersion");
    let up_to_date = installed_version.as_deref() == Some(VERSION)
        && lnk.exists()
        && !newer_than(&current, &target);
    let relocated = !same_file(&current, &target);
    if up_to_date && !relocated {
        return false;
    }
    let _ = std::fs::create_dir_all(&dir);
    if relocated && std::fs::copy(&current, &target).is_err() {
        // The installed copy is running and holds the file; stop it and try once more.
        powershell(&format!(
            "Get-Process letronna -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -eq '{}' }} | Stop-Process -Force",
            target.display()
        ));
        std::thread::sleep(std::time::Duration::from_millis(600));
        let _ = std::fs::copy(&current, &target);
    }
    powershell(&format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}'); \
         $s.TargetPath='{}'; $s.WorkingDirectory='{}'; $s.Description='{BRAND}'; $s.Save()",
        lnk.display(),
        target.display(),
        dir.display()
    ));
    let exe = target.display().to_string();
    set_value(&key, "DisplayName", BRAND);
    set_value(&key, "DisplayVersion", VERSION);
    set_value(&key, "Publisher", "Gadlus Engineering");
    set_value(&key, "InstallLocation", &dir.display().to_string());
    set_value(&key, "DisplayIcon", &exe);
    set_value(&key, "UninstallString", &format!("\"{exe}\" --uninstall"));
    set_value(&key, "URLInfoAbout", "https://github.com/wweziza/letronna");
    set_value(&key, "NoModify", "1");
    set_value(&key, "NoRepair", "1");
    relocated && Command::new(&target).spawn().is_ok()
}

/// Removes the shortcut, the registry entry, and the install folder, then exits.
pub(crate) fn uninstall() -> ! {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RegDeleteTreeW};
    if let Some(lnk) = shortcut() {
        let _ = std::fs::remove_file(lnk);
    }
    unsafe {
        RegDeleteTreeW(HKEY_CURRENT_USER, registry_key().as_ptr());
    }
    if let Some(dir) = install_dir() {
        let _ = Command::new("cmd")
            .args([
                "/C",
                &format!(
                    "timeout /t 2 /nobreak >nul & rmdir /s /q \"{}\"",
                    dir.display()
                ),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    std::process::exit(0)
}

fn newer_than(a: &Path, b: &Path) -> bool {
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (modified(a), modified(b)) {
        (Some(a), Some(b)) => a > b,
        (Some(_), None) => true,
        _ => false,
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
