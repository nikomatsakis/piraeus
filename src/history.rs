use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
struct PiraeusConfig {
    #[serde(default)]
    recent_remote_ports: Vec<u16>,
}

// {impl}`requirements.forwarding.add.port_history`
pub fn config_path() -> Result<PathBuf, String> {
    let home = env::var_os("HOME").map(PathBuf::from);
    let xdg_config_home = env::var_os("XDG_CONFIG_HOME");
    choose_config_path(home.as_deref(), xdg_config_home.as_deref())
}

// {impl}`requirements.forwarding.add.port_history`
pub fn load_and_migrate_recent_ports() -> Result<Vec<u16>, String> {
    let home = env::var_os("HOME").map(PathBuf::from);
    let xdg_config_home = env::var_os("XDG_CONFIG_HOME");
    let config_directory = choose_config_directory(home.as_deref(), xdg_config_home.as_deref())?;
    let new_path = config_directory.join("piraeus").join("config.toml");
    let legacy_paths = legacy_config_paths(home.as_deref(), &config_directory);
    load_or_import(&new_path, &legacy_paths)
}

// {impl}`requirements.forwarding.add.port_history`
fn choose_config_path(
    home: Option<&Path>,
    xdg_config_home: Option<&OsStr>,
) -> Result<PathBuf, String> {
    Ok(choose_config_directory(home, xdg_config_home)?
        .join("piraeus")
        .join("config.toml"))
}

// {impl}`requirements.forwarding.add.port_history`
fn choose_config_directory(
    home: Option<&Path>,
    xdg_config_home: Option<&OsStr>,
) -> Result<PathBuf, String> {
    if let Some(xdg) = xdg_config_home {
        let xdg = Path::new(xdg);
        if xdg.is_absolute() {
            return Ok(xdg.to_path_buf());
        }
    }
    home.map(|home| home.join(".config")).ok_or_else(|| {
        "HOME is not set, so ~/.config/piraeus/config.toml cannot be located.".into()
    })
}

// {impl}`requirements.forwarding.add.port_history`
fn legacy_config_paths(home: Option<&Path>, config_directory: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = home {
        paths.push(home.join(".piraeus.toml"));
    }
    paths.push(config_directory.join("pfm").join("config.toml"));
    if let Some(home) = home {
        paths.push(home.join(".pfm.toml"));
    }
    paths
}

// {impl}`requirements.forwarding.add.port_history`
fn load_or_import(new_path: &Path, legacy_paths: &[PathBuf]) -> Result<Vec<u16>, String> {
    if path_exists(new_path)? {
        return load_recent_ports(new_path);
    }
    for legacy_path in legacy_paths {
        if path_exists(legacy_path)? {
            let ports = load_recent_ports(legacy_path)?;
            save_recent_ports(new_path, &ports)?;
            return Ok(ports);
        }
    }
    Ok(Vec::new())
}

// {impl}`requirements.forwarding.add.port_history`
fn path_exists(path: &Path) -> Result<bool, String> {
    path.try_exists()
        .map_err(|error| format!("Could not inspect {}: {error}", path.display()))
}

// {impl}`requirements.forwarding.add.port_history`
pub fn load_recent_ports(path: &Path) -> Result<Vec<u16>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Could not read {}: {error}", path.display())),
    };
    let config: PiraeusConfig = toml::from_str(&text)
        .map_err(|error| format!("Could not parse {}: {error}", path.display()))?;
    Ok(config.recent_remote_ports)
}

// {impl}`requirements.forwarding.add.port_history`
pub fn save_recent_ports(path: &Path, ports: &[u16]) -> Result<(), String> {
    let text = toml::to_string_pretty(&PiraeusConfig {
        recent_remote_ports: ports.to_vec(),
    })
    .map_err(|error| format!("Could not encode recent ports: {error}"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    }
    fs::write(path, text).map_err(|error| format!("Could not write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    // {test}`requirements.forwarding.add.port_history`
    #[test]
    fn chooses_xdg_config_home_when_it_is_absolute() {
        let home = Path::new("/example/home");
        assert_eq!(
            choose_config_path(Some(home), Some(OsStr::new("/example/xdg"))).unwrap(),
            Path::new("/example/xdg/piraeus/config.toml")
        );
        assert_eq!(
            choose_config_path(Some(home), Some(OsStr::new("relative/xdg"))).unwrap(),
            Path::new("/example/home/.config/piraeus/config.toml")
        );
        assert_eq!(
            choose_config_path(Some(home), None).unwrap(),
            Path::new("/example/home/.config/piraeus/config.toml")
        );
        assert_eq!(
            choose_config_path(Some(home), Some(OsStr::new(""))).unwrap(),
            Path::new("/example/home/.config/piraeus/config.toml")
        );
        assert_eq!(
            choose_config_path(None, Some(OsStr::new("/example/xdg"))).unwrap(),
            Path::new("/example/xdg/piraeus/config.toml")
        );
        assert_eq!(
            legacy_config_paths(Some(home), Path::new("/example/xdg")),
            [
                PathBuf::from("/example/home/.piraeus.toml"),
                PathBuf::from("/example/xdg/pfm/config.toml"),
                PathBuf::from("/example/home/.pfm.toml")
            ]
        );
    }

    // {test}`requirements.forwarding.add.port_history`
    #[test]
    fn saved_ports_survive_a_reload() {
        let directory = temporary_directory();
        let path = directory.join("piraeus").join("config.toml");
        assert_eq!(load_recent_ports(&path).unwrap(), Vec::<u16>::new());
        save_recent_ports(&path, &[8031, 8030]).unwrap();
        assert_eq!(load_recent_ports(&path).unwrap(), [8031, 8030]);
        fs::write(&path, "recent_remote_ports = [not-a-port]").unwrap();
        assert!(
            load_recent_ports(&path)
                .unwrap_err()
                .contains("Could not parse")
        );
        fs::remove_dir_all(&directory).unwrap();
    }

    // {test}`requirements.forwarding.add.port_history`
    #[test]
    fn imports_legacy_history_from_home_or_xdg() {
        let directory = temporary_directory();
        let piraeus_home = directory.join("piraeus-home");
        let legacy_piraeus = piraeus_home.join(".piraeus.toml");
        let new_piraeus = choose_config_path(Some(&piraeus_home), None).unwrap();
        save_recent_ports(&legacy_piraeus, &[8030]).unwrap();
        assert_eq!(
            load_or_import(
                &new_piraeus,
                &legacy_config_paths(Some(&piraeus_home), &piraeus_home.join(".config"))
            )
            .unwrap(),
            [8030]
        );
        assert_eq!(load_recent_ports(&new_piraeus).unwrap(), [8030]);
        assert!(legacy_piraeus.exists());

        let xdg_home = directory.join("xdg-home");
        let xdg = directory.join("xdg");
        let legacy_xdg = xdg.join("pfm").join("config.toml");
        let new_xdg = choose_config_path(Some(&xdg_home), Some(xdg.as_os_str())).unwrap();
        save_recent_ports(&legacy_xdg, &[9000]).unwrap();
        let paths = legacy_config_paths(Some(&xdg_home), &xdg);
        assert_eq!(load_or_import(&new_xdg, &paths).unwrap(), [9000]);
        assert_eq!(load_recent_ports(&new_xdg).unwrap(), [9000]);
        assert!(legacy_xdg.exists());

        let pfm_home = directory.join("pfm-home");
        let legacy_pfm = pfm_home.join(".pfm.toml");
        let new_pfm = choose_config_path(Some(&pfm_home), None).unwrap();
        save_recent_ports(&legacy_pfm, &[7000]).unwrap();
        assert_eq!(
            load_or_import(
                &new_pfm,
                &legacy_config_paths(Some(&pfm_home), &pfm_home.join(".config"))
            )
            .unwrap(),
            [7000]
        );
        assert_eq!(load_recent_ports(&new_pfm).unwrap(), [7000]);
        assert!(legacy_pfm.exists());

        save_recent_ports(&new_xdg, &[3000]).unwrap();
        assert_eq!(load_or_import(&new_xdg, &paths).unwrap(), [3000]);
        fs::remove_dir_all(&directory).unwrap();
    }

    // {test}`requirements.forwarding.add.port_history`
    fn temporary_directory() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("piraeus-history-{}-{suffix}", std::process::id()))
    }
}
