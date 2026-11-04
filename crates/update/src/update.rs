//! Aktualizacje z portfolio redzikowski.com – serwer podaje wydania w formacie GitHub API,
//! więc program nie potrzebuje żadnego klucza.
//!
//! Wydanie zawiera `OpenDTP-Setup-<wersja>.exe` i `SHA256SUMS.txt`. Instalator pobieramy do
//! pamięci podręcznej, sprawdzamy sumę i uruchamiamy po cichu – sam zamyka i uruchamia program.

use std::io::{Read, Write};
use std::path::PathBuf;

use sha2::{Digest, Sha256};

const FEED: &str = "https://redzikowski.com/api/releases/repos/BlackSingularities/OpenDTP/releases/latest";
const UA: &str = "OpenDTP";

fn get(url: &str, accept: &str) -> Result<ureq::Response, String> {
    ureq::get(url)
        .set("User-Agent", UA)
        .set("Accept", accept)
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => "Brak wydań do pobrania".to_string(),
            e => e.to_string(),
        })
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    asset: String,
    name: String,
    sums: Option<String>,
}

fn parse(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn newer(a: &str, b: &str) -> bool {
    parse(a) > parse(b)
}

/// Najnowsze wydanie, jeśli nowsze od tej wersji.
pub fn check() -> Result<Option<Release>, String> {
    let r: serde_json::Value = get(FEED, "application/json")?.into_json().map_err(|e| e.to_string())?;
    let version = r["tag_name"].as_str().unwrap_or_default().trim_start_matches('v').to_string();
    if version.is_empty() || !newer(&version, crate::version()) {
        return Ok(None);
    }
    let assets = r["assets"].as_array().cloned().unwrap_or_default();
    let find = |pred: &dyn Fn(&str) -> bool| assets.iter().find(|a| a["name"].as_str().is_some_and(|n| pred(&n.to_lowercase()))).cloned();
    // Wersja przenośna pobiera archiwum ZIP, zainstalowana – instalator.
    let exe = if installed() { find(&|n| n.starts_with("opendtp-setup-") && n.ends_with(".exe")) } else { find(&|n| n.ends_with("-portable.zip")) };
    let Some(exe) = exe else { return Ok(None) };
    Ok(Some(Release {
        version,
        notes: r["body"].as_str().unwrap_or_default().to_string(),
        asset: exe["browser_download_url"].as_str().unwrap_or_default().to_string(),
        name: exe["name"].as_str().unwrap_or("OpenDTP-Setup.exe").to_string(),
        sums: find(&|n| n == "sha256sums.txt").and_then(|a| a["browser_download_url"].as_str().map(str::to_string)),
    }))
}

/// Pobiera instalator i sprawdza sumę SHA-256. `progress(pobrane, całość)`.
pub fn download(rel: &Release, progress: impl Fn(u64, u64)) -> Result<PathBuf, String> {
    let expected = match &rel.sums {
        Some(url) => get(url, "application/octet-stream")?
            .into_string()
            .map_err(|e| e.to_string())?
            .lines()
            .find(|l| l.trim_end().ends_with(&rel.name))
            .and_then(|l| l.split_whitespace().next())
            .map(|h| h.to_lowercase()),
        None => None,
    };
    let expected = expected.ok_or("Wydanie nie ma sumy kontrolnej instalatora")?;
    let resp = get(&rel.asset, "application/octet-stream")?;
    let total: u64 = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let folder = crate::paths::cache().join("update");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let path = folder.join(&rel.name);
    let mut f = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    let mut rd = resp.into_reader();
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        let n = rd.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        h.update(&buf[..n]);
        done += n as u64;
        progress(done, total.max(done));
    }
    drop(f);
    if format!("{:x}", h.finalize()) != expected {
        let _ = std::fs::remove_file(&path);
        return Err("Suma kontrolna pobranego pliku się nie zgadza – aktualizacja przerwana".into());
    }
    Ok(path)
}

/// Wersja przenośna: wyciąga nowy OpenDTP.exe z archiwum i podmienia go po zamknięciu programu
/// (skrypt czeka na koniec procesu, kopiuje plik i uruchamia program ponownie).
pub fn apply_portable(zip_path: &std::path::Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut entry = z.by_name("OpenDTP.exe").map_err(|_| "Archiwum nie zawiera OpenDTP.exe".to_string())?;
    let new = zip_path.with_file_name("OpenDTP.new.exe");
    let mut out = std::fs::File::create(&new).map_err(|e| e.to_string())?;
    std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    drop(out);
    let script = zip_path.with_file_name("opendtp-update.cmd");
    let pid = std::process::id();
    let body = format!(
        concat!(
            "@echo off\r\n",
            ":wait\r\n",
            "tasklist /FI \"PID eq {pid}\" | find \"{pid}\" >nul && (timeout /t 1 /nobreak >nul & goto wait)\r\n",
            "copy /y \"{new}\" \"{exe}\" >nul\r\n",
            "start \"\" \"{exe}\"\r\n",
            "del \"{new}\"\r\n"
        ),
        new = new.display(),
        exe = exe.display(),
        pid = pid
    );
    std::fs::write(&script, body).map_err(|e| e.to_string())?;
    use std::os::windows::process::CommandExt;
    std::process::Command::new("cmd").args(["/C", &script.display().to_string()]).creation_flags(0x0800_0000).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Uruchamia instalator po cichu z uprawnieniami administratora (UAC – instalacja w Program Files).
/// Instalator zamyka OpenDTP, podmienia pliki i – gdy `restart` – uruchamia program ponownie.
pub fn apply(setup: &std::path::Path, restart: bool) -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let mut args = String::from("/SILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS");
    if restart {
        args.push_str(" /RESTARTAPPLICATIONS");
    }
    // Zawsze ten sam folder, w którym działa program.
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf())) {
        args.push_str(&format!(" /DIR=\"{}\"", dir.display()));
    }
    let file = HSTRING::from(setup.as_os_str());
    let params = HSTRING::from(args);
    let verb = HSTRING::from("runas");
    let r = unsafe { ShellExecuteW(None, PCWSTR(verb.as_ptr()), PCWSTR(file.as_ptr()), PCWSTR(params.as_ptr()), PCWSTR::null(), SW_SHOWNORMAL) };
    // Wartość > 32 – sukces; mniejsza – błąd (np. odmowa w oknie UAC).
    if r.0 as isize > 32 {
        Ok(())
    } else {
        Err(crate::lang::tr("Instalacja anulowana albo brak uprawnień administratora").to_string())
    }
}

/// Program zainstalowany instalatorem (aktualizacja możliwa) czy przenośny.
pub fn installed() -> bool {
    std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("unins000.exe").exists())).unwrap_or(false)
}
