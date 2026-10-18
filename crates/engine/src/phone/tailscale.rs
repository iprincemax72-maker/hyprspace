use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::util::no_window;

pub const PORT: u16 = 47823;

fn cli() -> Option<PathBuf> {
    crate::open::on_path(&["tailscale"]).or_else(|| {
        let fixed = if cfg!(windows) {
            r"C:\Program Files\Tailscale\tailscale.exe"
        } else {
            "/Applications/Tailscale.app/Contents/MacOS/Tailscale"
        };
        Some(PathBuf::from(fixed)).filter(|p| p.is_file())
    })
}

fn run(args: &[&str]) -> Result<String, String> {
    let cli = cli().ok_or("Tailscale isn't installed on this computer.")?;
    let mut child = no_window(&mut Command::new(cli))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't run Tailscale: {e}"))?;
    let until = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Ok(Some(s)) = child.try_wait() {
            break Some(s);
        }
        if Instant::now() > until {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let (mut out, mut err) = (String::new(), String::new());
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    match status {
        Some(s) if s.success() => Ok(out),
        // it waits when the tailnet hasn't turned HTTPS on, after printing where to do it
        _ => Err(trouble(&format!("{out}\n{err}"))),
    }
}

fn trouble(out: &str) -> String {
    if let Some(link) = out
        .split_whitespace()
        .find(|w| w.starts_with("https://login.tailscale.com"))
    {
        return format!("Turn on HTTPS for your tailnet first: {link}");
    }
    let line = out.lines().map(str::trim).find(|l| !l.is_empty());
    match line {
        Some(l) => format!("Tailscale said: {l}"),
        None => "Tailscale didn't answer.".into(),
    }
}

fn name() -> Result<String, String> {
    let out = run(&["status", "--json", "--peers=false"])?;
    let v: serde_json::Value =
        serde_json::from_str(&out).map_err(|_| "Tailscale didn't say who this computer is.")?;
    let dns = v["Self"]["DNSName"]
        .as_str()
        .unwrap_or("")
        .trim_end_matches('.');
    if dns.is_empty() {
        return Err("This computer isn't signed in to Tailscale.".into());
    }
    Ok(dns.to_string())
}

pub fn serve(local: u16) -> Result<Option<String>, String> {
    if cli().is_none() {
        return Ok(None);
    }
    let Ok(name) = name() else {
        return Ok(None);
    };
    run(&[
        "serve",
        "--bg",
        &format!("--https={PORT}"),
        &format!("http://127.0.0.1:{local}"),
    ])?;
    Ok(Some(format!("https://{name}:{PORT}/")))
}

pub fn unserve() {
    let _ = run(&["serve", &format!("--https={PORT}"), "off"]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_waiting_serve_points_at_the_admin_page() {
        let out = "Serve is not enabled on your tailnet.\nTo enable, visit:\n\n         https://login.tailscale.com/f/serve?node=abc\n";
        assert_eq!(
            trouble(out),
            "Turn on HTTPS for your tailnet first: https://login.tailscale.com/f/serve?node=abc"
        );
        assert_eq!(trouble("boom\n"), "Tailscale said: boom");
    }
}
