use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn home_dir() -> PathBuf {
    PathBuf::from(env::var("HOME").expect("HOME environment variable not set"))
}

fn local_bin() -> PathBuf {
    home_dir().join(".local").join("bin")
}

fn build_release(project_dir: &Path) -> Result<(), String> {
    println!("🔨  Building Spear in release mode...");
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(project_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to run cargo: {e}"))?;

    if !status.success() {
        return Err("cargo build --release failed".into());
    }
    Ok(())
}

fn install_binary(project_dir: &Path) -> Result<PathBuf, String> {
    let src = project_dir.join("target").join("release").join("spear");
    if !src.exists() {
        return Err(format!(
            "Built binary not found at {src:?}. Did `cargo build --release` succeed?"
        ));
    }

    let bin_dir = local_bin();
    fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Could not create {bin_dir:?}: {e}"))?;

    let dest = bin_dir.join("spear");
    fs::copy(&src, &dest).map_err(|e| format!("Failed to copy binary: {e}"))?;

    // Ensure executable bit is set
    let mut perms = fs::metadata(&dest)
        .map_err(|e| format!("Could not stat {dest:?}: {e}"))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&dest, perms)
        .map_err(|e| format!("Could not chmod {dest:?}: {e}"))?;

    println!("✅  Installed binary → {dest:?}");
    Ok(dest)
}

fn create_autostart_entry(binary: &Path) -> Result<(), String> {
    let dir = home_dir().join(".config").join("autostart");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create autostart dir: {e}"))?;

    let path = dir.join("spear.desktop");
    let content = format!(
        "[Desktop Entry]\n\
         Name=Spear Daemon\n\
         Comment=Start Spear launcher daemon in background\n\
         Exec=bash -c \"{binary} --quit ; {binary}\"\n\
         Icon=com.antigravity.spear\n\
         Terminal=false\n\
         Type=Application\n\
         Categories=Utility;\n\
         StartupNotify=false\n\
         X-GNOME-Autostart-enabled=true\n",
        binary = binary.display()
    );

    let mut f = fs::File::create(&path).map_err(|e| format!("Could not write autostart: {e}"))?;
    f.write_all(content.as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;

    println!("✅  Autostart entry → {path:?}");
    Ok(())
}

fn install_desktop_entry(project_dir: &Path, binary: &Path) -> Result<(), String> {
    let desktop_src = project_dir
        .join("data")
        .join("applications")
        .join("com.antigravity.spear.desktop");
    let icon_src = project_dir
        .join("data")
        .join("icons")
        .join("hicolor")
        .join("scalable")
        .join("apps")
        .join("com.antigravity.spear.svg");

    if !desktop_src.exists() {
        return Err(format!("Desktop file not found at {desktop_src:?}"));
    }
    if !icon_src.exists() {
        return Err(format!("Application icon not found at {icon_src:?}"));
    }

    let applications_dir = home_dir().join(".local").join("share").join("applications");
    let icons_dir = home_dir()
        .join(".local")
        .join("share")
        .join("icons")
        .join("hicolor")
        .join("scalable")
        .join("apps");
    fs::create_dir_all(&applications_dir)
        .map_err(|e| format!("Could not create applications dir {applications_dir:?}: {e}"))?;
    fs::create_dir_all(&icons_dir)
        .map_err(|e| format!("Could not create icons dir {icons_dir:?}: {e}"))?;

    let desktop_dest = applications_dir.join("com.antigravity.spear.desktop");
    let icon_dest = icons_dir.join("com.antigravity.spear.svg");
    let desktop_content = fs::read_to_string(&desktop_src)
        .map_err(|e| format!("Failed to read desktop entry: {e}"))?
        .replace("Exec=spear\n", &format!("Exec={}\n", binary.display()));
    fs::write(&desktop_dest, desktop_content)
        .map_err(|e| format!("Failed to install desktop entry: {e}"))?;
    fs::copy(&icon_src, &icon_dest)
        .map_err(|e| format!("Failed to install application icon: {e}"))?;

    println!("✅  Desktop entry → {desktop_dest:?}");
    println!("✅  Application icon → {icon_dest:?}");
    Ok(())
}

fn register_gnome_shortcut(binary: &Path, shortcut: &str) -> Result<(), String> {
    println!("⌨️   Registering GNOME global shortcut ({shortcut})…");

    // Read existing custom keybindings
    let existing_output = Command::new("gsettings")
        .args([
            "get",
            "org.gnome.settings-daemon.plugins.media-keys",
            "custom-keybindings",
        ])
        .output()
        .map_err(|e| format!("gsettings get failed: {e}"))?;

    let existing = String::from_utf8_lossy(&existing_output.stdout)
        .trim()
        .to_string();

    // Parse existing paths
    let mut paths: Vec<String> = if existing == "@as []" || existing == "[]" || existing.is_empty()
    {
        vec![]
    } else {
        existing
            .trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .map(|s| s.trim().trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    // Check if we already have a "Spear" binding; find or create a slot
    let mut spear_path: Option<String> = None;
    for p in &paths {
        let out = Command::new("gsettings")
            .args([
                "get",
                &format!(
                    "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:{p}"
                ),
                "name",
            ])
            .output();
        if let Ok(o) = out {
            let name = String::from_utf8_lossy(&o.stdout)
                .trim()
                .trim_matches('\'')
                .to_string();
            if name == "Spear" {
                spear_path = Some(p.clone());
                break;
            }
        }
    }

    if spear_path.is_none() {
        // Find next free custom index
        let indices: Vec<usize> = paths
            .iter()
            .filter_map(|p| {
                p.trim_end_matches('/')
                    .rsplit("custom")
                    .next()
                    .and_then(|n| n.parse().ok())
            })
            .collect();
        let mut idx = 0usize;
        while indices.contains(&idx) {
            idx += 1;
        }
        let new_path = format!(
            "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom{idx}/"
        );
        paths.push(new_path.clone());
        spear_path = Some(new_path);
    }

    let sp = spear_path.unwrap();
    let schema = format!(
        "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:{sp}"
    );

    let run = |args: &[&str]| -> Result<(), String> {
        let status = Command::new("gsettings")
            .args(args)
            .status()
            .map_err(|e| format!("gsettings error: {e}"))?;
        if !status.success() {
            return Err(format!("gsettings failed: {args:?}"));
        }
        Ok(())
    };

    run(&["set", &schema, "name", "Spear"])?;
    run(&[
        "set",
        &schema,
        "command",
        &binary.to_string_lossy(),
    ])?;
    run(&["set", &schema, "binding", shortcut])?;

    // Write back the updated paths list
    let list = paths
        .iter()
        .map(|p| format!("'{p}'"))
        .collect::<Vec<_>>()
        .join(", ");
    let list = format!("[{list}]");
    run(&[
        "set",
        "org.gnome.settings-daemon.plugins.media-keys",
        "custom-keybindings",
        &list,
    ])?;

    println!("✅  Shortcut '{shortcut}' registered.");
    Ok(())
}

fn install_custom_icons(project_dir: &Path) -> Result<(), String> {
    let src_dir = project_dir.join("icons");
    if !src_dir.exists() {
        println!("⚠️  No icons directory found at {src_dir:?}, skipping icon installation.");
        return Ok(());
    }

    let dest_dir = home_dir().join(".config").join("spear").join("icons");
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Could not create icons dir {dest_dir:?}: {e}"))?;

    println!("🎨  Installing custom icons to {dest_dir:?}...");
    
    let entries = fs::read_dir(&src_dir)
        .map_err(|e| format!("Failed to read source icons dir: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name() {
                let dest_file = dest_dir.join(name);
                fs::copy(&path, &dest_file)
                    .map_err(|e| format!("Failed to copy icon {name:?}: {e}"))?;
            }
        }
    }

    println!("✅  Installed custom icons.");
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // The installer itself lives in target/release/install (or target/debug/install).
    // Walk up to the project root (the directory that contains Cargo.toml).
    let exe = env::current_exe().expect("Cannot determine current executable path");
    // exe is something like <project>/target/release/install
    let project_dir = exe
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists())
        .unwrap_or_else(|| {
            // Fallback: use current working directory
            &std::path::Path::new(".")
        })
        .to_path_buf();

    println!("📦  Spear Launcher Installer");
    println!("    Project root : {project_dir:?}");

    // 1. Build
    if let Err(e) = build_release(&project_dir) {
        eprintln!("❌  Build failed: {e}");
        std::process::exit(1);
    }

    // 2. Install binary
    let binary = match install_binary(&project_dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌  {e}");
            std::process::exit(1);
        }
    };

    // 2b. Install custom icons
    if let Err(e) = install_custom_icons(&project_dir) {
        eprintln!("⚠️   Custom icons installation failed: {e}");
    }

    // 2c. Install desktop launcher and application icon
    if let Err(e) = install_desktop_entry(&project_dir, &binary) {
        eprintln!("⚠️   Desktop entry installation failed: {e}");
    }

    // 4. Autostart entry
    if let Err(e) = create_autostart_entry(&binary) {
        eprintln!("⚠️   Autostart entry: {e}");
    }

    // 5. GNOME shortcut (optional – skip if gsettings is unavailable)
    let shortcut = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
        .unwrap_or("<Alt>space");

    if let Err(e) = register_gnome_shortcut(&binary, shortcut) {
        eprintln!("⚠️   Shortcut registration skipped: {e}");
        eprintln!("    You can manually add it in GNOME Settings → Keyboard → Custom Shortcuts.");
        eprintln!("    Name: Spear   Command: {binary:?}   Shortcut: {shortcut}");
    }

    println!();
    println!("🎉  Installation complete!");
    println!("════════════════════════════════════════");
    println!("  Binary    : {binary:?}");
    println!("  Shortcut  : {shortcut}");
    println!();
    println!("  To start the daemon now:");
    println!("    spear");
    println!();
    println!("  To toggle the launcher:");
    println!("    Press {shortcut}");
    println!();
    println!("  To quit the daemon:");
    println!("    spear --quit");
    println!("════════════════════════════════════════");

    // Ensure ~/.local/bin is on PATH hint
    let bin_dir = local_bin();
    let path_val = env::var("PATH").unwrap_or_default();
    if !path_val
        .split(':')
        .any(|p| PathBuf::from(p) == bin_dir)
    {
        println!();
        println!("⚠️   {bin_dir:?} is not in your PATH.");
        println!(
            "    Add the following line to your ~/.bashrc or ~/.zshrc and restart your shell:"
        );
        println!("      export PATH=\"$HOME/.local/bin:$PATH\"");
    }
}
