/// Application helpers for launching and querying Windows apps.
/// Full implementation will be ported from WinPaste.

/// Launch a UWP application with a file.
pub fn launch_uwp_with_file(_package: &str, _file: &str) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

/// Get the system default application for a file extension.
pub fn get_system_default_app(_ext: &str) -> String {
    String::new()
}
