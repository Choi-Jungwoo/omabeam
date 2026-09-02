//! Installs and updates OmaBeam's user-level Omarchy integration.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::beam::PORT;

const KEYBINDING: &str = include_str!("../packaging/omarchy/keybinding.lua");
const MENU_ENTRY: &str = include_str!("../packaging/omarchy/menu-entry.jsonc");
const KEYBINDING_BEGIN: &str = "-- >>> OmaBeam setup >>>";
const KEYBINDING_END: &str = "-- <<< OmaBeam setup <<<";
const MENU_BEGIN: &str = "  // >>> OmaBeam setup >>>";
const MENU_END: &str = "  // <<< OmaBeam setup <<<";
const MENU_KEY: &str = "\"trigger.share.omabeam\"";

struct Lan {
    interface: String,
    subnet: String,
}

pub(super) fn run() -> Result<(), String> {
    require_omarchy()?;
    let lan = detect_lan()?;
    configure_firewall(&lan)?;

    let config = config_dir()?;
    let menu_path = config.join("omarchy/extensions/omarchy-menu.jsonc");
    let keybinding_path = config.join("hypr/bindings.lua");
    let menu_changed = update_file(&menu_path, "{\n}\n", updated_menu)?;
    let keybinding_changed = update_file(&keybinding_path, "", updated_keybindings)?;

    if keybinding_changed {
        reload_hyprland()?;
    }

    println!(
        "Firewall: TCP {PORT} allowed from {} on {}",
        lan.subnet, lan.interface
    );
    println!(
        "Share menu: {}",
        if menu_changed {
            "updated"
        } else {
            "up to date"
        }
    );
    println!(
        "Shortcut: {}",
        if keybinding_changed {
            "Super+B updated"
        } else {
            "Super+B up to date"
        }
    );
    Ok(())
}

fn require_omarchy() -> Result<(), String> {
    let status = Command::new("omarchy")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("could not run Omarchy: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Omarchy is required to run setup".into())
    }
}

fn config_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".config"))
        .ok_or_else(|| "HOME is not set; could not locate the Omarchy configuration".into())
}

fn detect_lan() -> Result<Lan, String> {
    let default_route = command_output(
        Command::new("ip").args(["-4", "route", "show", "default"]),
        "could not inspect the default network route",
    )?;
    let route = default_route
        .lines()
        .next()
        .ok_or_else(|| "could not find a default IPv4 route".to_owned())?;
    let interface = word_after(route, "dev")
        .ok_or_else(|| "the default IPv4 route has no network interface".to_owned())?;
    let address = word_after(route, "src")
        .ok_or_else(|| "the default IPv4 route has no source address".to_owned())?;

    let link_routes = command_output(
        Command::new("ip").args(["-4", "route", "show", "dev", interface, "scope", "link"]),
        "could not inspect the local network",
    )?;
    let subnet = link_routes
        .lines()
        .find(|line| word_after(line, "src") == Some(address))
        .and_then(|line| line.split_whitespace().next())
        .filter(|subnet| subnet.contains('/'))
        .ok_or_else(|| "could not find the subnet for the default IPv4 route".to_owned())?;

    Ok(Lan {
        interface: interface.to_owned(),
        subnet: subnet.to_owned(),
    })
}

fn word_after<'a>(line: &'a str, needle: &str) -> Option<&'a str> {
    let mut words = line.split_whitespace();
    while let Some(word) = words.next() {
        if word == needle {
            return words.next();
        }
    }
    None
}

fn command_output(command: &mut Command, context: &str) -> Result<String, String> {
    let output = command
        .output()
        .map_err(|error| format!("{context}: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            context.to_owned()
        } else {
            format!("{context}: {detail}")
        });
    }
    String::from_utf8(output.stdout).map_err(|_| format!("{context}: output was not UTF-8"))
}

fn configure_firewall(lan: &Lan) -> Result<(), String> {
    let port = PORT.to_string();
    let status = Command::new("sudo")
        .args([
            "ufw",
            "allow",
            "in",
            "on",
            &lan.interface,
            "from",
            &lan.subnet,
            "to",
            "any",
            "port",
            &port,
            "proto",
            "tcp",
            "comment",
            "OmaBeam",
        ])
        .status()
        .map_err(|error| format!("could not run sudo ufw: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "could not allow TCP port {PORT} through UFW; rerun setup and approve sudo"
        ))
    }
}

fn update_file(
    path: &Path,
    default: &str,
    update: fn(&str) -> Result<String, String>,
) -> Result<bool, String> {
    let original = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => default.to_owned(),
        Err(error) => return Err(format!("could not read {}: {error}", path.display())),
    };
    let updated = update(&original).map_err(|error| format!("{}: {error}", path.display()))?;
    if updated == original {
        return Ok(false);
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    if path.exists() {
        let backup = backup_path(path)?;
        fs::copy(path, &backup)
            .map_err(|error| format!("could not back up {}: {error}", path.display()))?;
    }
    fs::write(path, updated)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    Ok(true)
}

fn backup_path(path: &Path) -> Result<PathBuf, String> {
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("{} has no valid file name", path.display()))?;
    Ok(path.with_file_name(format!("{name}.bak.omabeam")))
}

fn updated_keybindings(contents: &str) -> Result<String, String> {
    let block = format!(
        "{KEYBINDING_BEGIN}\n{}\n{KEYBINDING_END}\n",
        KEYBINDING.trim_end()
    );
    if let Some(updated) =
        replace_managed_block(contents, KEYBINDING_BEGIN, KEYBINDING_END, &block)?
    {
        return Ok(updated);
    }

    let had_legacy_binding = contents
        .lines()
        .any(|line| line.contains("o.bind(\"SUPER + B\"") && line.contains("\"OmaBeam\""));
    let mut updated = if had_legacy_binding {
        contents
            .split_inclusive('\n')
            .filter(|line| {
                let line = line.trim();
                line != "hl.unbind(\"SUPER + B\")"
                    && !(line.contains("o.bind(\"SUPER + B\"") && line.contains("\"OmaBeam\""))
            })
            .collect()
    } else {
        contents.to_owned()
    };
    append_block(&mut updated, &block);
    Ok(updated)
}

fn updated_menu(contents: &str) -> Result<String, String> {
    let block = format!("{MENU_BEGIN}\n{}\n{MENU_END}\n", menu_member());
    if let Some(updated) = replace_managed_block(contents, MENU_BEGIN, MENU_END, &block)? {
        return Ok(updated);
    }

    let mut updated = remove_legacy_menu_entries(contents)?;
    let leading_whitespace = updated.len() - updated.trim_start().len();
    let root = (updated.as_bytes().get(leading_whitespace) == Some(&b'{'))
        .then_some(leading_whitespace + 1)
        .ok_or_else(|| "menu configuration has no root object".to_owned())?;
    let insert_at = if updated.as_bytes().get(root..root + 2) == Some(b"\r\n") {
        root + 2
    } else if updated.as_bytes().get(root) == Some(&b'\n') {
        root + 1
    } else {
        updated.insert(root, '\n');
        root + 1
    };
    updated.insert_str(insert_at, &block);
    Ok(updated)
}

fn menu_member() -> &'static str {
    let start = MENU_ENTRY
        .find('\n')
        .expect("menu entry template must have an opening line")
        + 1;
    let end = MENU_ENTRY
        .rfind('}')
        .expect("menu entry template must have a closing brace");
    MENU_ENTRY[start..end].trim_end()
}

fn replace_managed_block(
    contents: &str,
    begin: &str,
    end: &str,
    block: &str,
) -> Result<Option<String>, String> {
    match (contents.find(begin), contents.find(end)) {
        (None, None) => Ok(None),
        (Some(begin_at), Some(end_at)) if begin_at < end_at => {
            let start = contents[..begin_at]
                .rfind('\n')
                .map_or(0, |newline| newline + 1);
            let finish = contents[end_at..]
                .find('\n')
                .map_or(contents.len(), |newline| end_at + newline + 1);
            let mut updated =
                String::with_capacity(contents.len() - (finish - start) + block.len());
            updated.push_str(&contents[..start]);
            updated.push_str(block);
            updated.push_str(&contents[finish..]);
            Ok(Some(updated))
        }
        _ => Err(
            "OmaBeam setup markers are incomplete; remove the broken managed block and retry"
                .into(),
        ),
    }
}

fn append_block(contents: &mut String, block: &str) {
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    if !contents.is_empty() && !contents.ends_with("\n\n") {
        contents.push('\n');
    }
    contents.push_str(block);
}

fn remove_legacy_menu_entries(contents: &str) -> Result<String, String> {
    let mut updated = contents.to_owned();
    while let Some(key_at) = updated.find(MENU_KEY) {
        let colon = updated[key_at + MENU_KEY.len()..]
            .find(':')
            .map(|offset| key_at + MENU_KEY.len() + offset)
            .ok_or_else(|| "the existing OmaBeam menu entry has no value".to_owned())?;
        let object = updated[colon + 1..]
            .find('{')
            .map(|offset| colon + 1 + offset)
            .ok_or_else(|| "the existing OmaBeam menu entry is not an object".to_owned())?;
        let object_end = matching_brace(&updated, object)
            .ok_or_else(|| "the existing OmaBeam menu entry is incomplete".to_owned())?;

        let mut start = key_at;
        while start > 0 && matches!(updated.as_bytes()[start - 1], b' ' | b'\t') {
            start -= 1;
        }
        let mut finish = object_end + 1;
        while finish < updated.len() && matches!(updated.as_bytes()[finish], b' ' | b'\t') {
            finish += 1;
        }
        if updated.as_bytes().get(finish) == Some(&b',') {
            finish += 1;
        }
        if updated.as_bytes().get(finish) == Some(&b'\r') {
            finish += 1;
        }
        if updated.as_bytes().get(finish) == Some(&b'\n') {
            finish += 1;
        }
        updated.replace_range(start..finish, "");
    }
    Ok(updated)
}

fn matching_brace(contents: &str, opening: usize) -> Option<usize> {
    let mut depth = 0;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in contents.as_bytes()[opening..].iter().copied().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(opening + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn reload_hyprland() -> Result<(), String> {
    command_output(
        Command::new("hyprctl").arg("reload"),
        "could not reload Hyprland",
    )?;
    let errors = command_output(
        Command::new("hyprctl").arg("configerrors"),
        "could not validate the Hyprland configuration",
    )?;
    if errors.trim().is_empty() {
        Ok(())
    } else {
        Err(format!("Hyprland reported configuration errors:\n{errors}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integration_blocks_install_migrate_and_update() {
        let old_binding = format!(
            "-- custom\n{}\n",
            KEYBINDING.lines().last().expect("OmaBeam binding")
        );
        let installed_binding = updated_keybindings(&old_binding).unwrap();
        assert!(installed_binding.starts_with("-- custom\n"));
        assert_eq!(installed_binding.matches("OmaBeam\"").count(), 1);
        assert!(installed_binding.contains(KEYBINDING_BEGIN));
        let stale_binding = installed_binding.replace(KEYBINDING.trim_end(), "old binding");
        assert_eq!(
            updated_keybindings(&stale_binding).unwrap(),
            installed_binding
        );

        let old_menu = format!("{{\n  \"custom\": {{}},\n{}\n}}\n", menu_member());
        let installed_menu = updated_menu(&old_menu).unwrap();
        assert!(installed_menu.contains("\"custom\": {}"));
        assert_eq!(installed_menu.matches(MENU_KEY).count(), 1);
        assert!(installed_menu.contains(MENU_BEGIN));
        let stale_menu = installed_menu.replace(menu_member(), "  \"old\": {},");
        assert_eq!(updated_menu(&stale_menu).unwrap(), installed_menu);
    }
}
