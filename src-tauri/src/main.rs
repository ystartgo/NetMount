// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde_json::{to_string_pretty, Value};
//use tauri::AppHandle;
use std::env;
//use std::error::Error;
use std::fs;
use std::path::Path;

//use std::io::Read;
//use std::path::Path;
use tauri::Manager;

mod autostart;
mod localized;
mod tray;
mod utils;

use crate::autostart::is_autostart;
use crate::autostart::set_autostart;
use crate::utils::download_with_progress;
use crate::utils::ensure_single_instance;
#[cfg(target_os = "windows")]
use crate::utils::find_first_available_drive_letter;
use crate::utils::get_home_dir;
#[cfg(target_os = "windows")]
use crate::utils::is_winfsp_installed;
#[cfg(target_os = "windows")]
use crate::utils::set_window_shadow;

//use crate::localized::LANGUAGE_PACK;
//use crate::localized::get_localized_text;
use crate::localized::set_localized;

 const USER_DATA_PATH: &str = ".netmount";
const CONFIG_FILE: &str = "config.json";

fn main() {
    // 确保应用程序只有一个实例运行
    ensure_single_instance(USER_DATA_PATH);

    let home_dir = get_home_dir();

    if home_dir.join(USER_DATA_PATH).exists() {
        fs::create_dir_all(home_dir.join(USER_DATA_PATH)).unwrap()
    }

    //设置运行目录
    let exe_dir = env::current_exe()
        .expect("无法获取当前可执行文件路径")
        .parent()
        .expect("无法获取父目录")
        .to_path_buf();
    println!("exe_dir: {}", exe_dir.display());

    let binding = env::current_exe().expect("Failed to get the current executable path");
    let exe_flie_name = Path::new(&binding)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap();

    if !cfg!(debug_assertions) {
        if cfg!(target_os = "linux") {
            let resources_dir = exe_dir
                .parent()
                .expect("无法获取父目录")
                .join("lib")
                .join(exe_flie_name);
            env::set_current_dir(&resources_dir).expect("更改工作目录失败");
        }

        if cfg!(target_os = "windows") {
            env::set_current_dir(&exe_dir).expect("更改工作目录失败");
        }

        if cfg!(target_os = "macos") {
            // 在macOS上，进一步定位到.app内部的Contents/Resources目录
            let resources_dir = exe_dir.parent().expect("无法获取父目录").join("Resources");
            println!("resources_dir: {}", resources_dir.display());
            // 设置运行目录到Resources
            if let Err(e) = env::set_current_dir(&resources_dir) {
                eprintln!("更改工作目录到Resources失败: {}", e);
                // 根据实际情况处理错误，如返回错误信息或终止程序
            }
        }
    }

    //run_command("ls").expect("运行ls命令失败");
    //run_command("dir").expect("运行ls命令失败");

    // 根据不同的操作系统配置Tauri Builder
    let builder = tauri::Builder::default()
        .system_tray(tray::menu()) // 设置系统托盘菜单
        .on_system_tray_event(tray::handler) // 设置系统托盘事件处理器
        .invoke_handler(tauri::generate_handler![
            set_localized,
            read_config_file,
            write_config_file,
            download_file,
            get_autostart_state,
            set_autostart_state,
            get_winfsp_install_state,
            get_available_drive_letter,
            set_devtools_state,
            fs_exist_dir,
            fs_make_dir,
            restart_self
        ])
        .setup(|_app| {
            #[cfg(target_os = "windows")]
            set_window_shadow(_app); // 设置窗口阴影
            Ok(())
        });

    // 运行Tauri应用，使用`generate_context!()`来加载应用配置
    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    let _=&run_command;
}


#[tauri::command]
fn set_devtools_state(app: tauri::AppHandle, state: bool) {
    let window = app.get_window("main").unwrap();
    if state {
        window.open_devtools();
    } else {
        window.close_devtools();
    }
}

use std::io::ErrorKind;
use std::path::PathBuf;

#[tauri::command]
fn fs_exist_dir(path: &str) -> bool {
    let home_dir = get_home_dir();
    // 替换路径中的波浪线 (~) 为用home目录
    let mut resolved_path = PathBuf::new();
    if path.starts_with("~") {
        resolved_path.push(home_dir);
        resolved_path.push(&path[1..]); // 跳过波浪线
    } else {
        resolved_path.push(path);
    }
    is_directory(path)
}

#[tauri::command]
fn fs_make_dir(path: &str) -> bool {
    let home_dir = get_home_dir();
    // 替换路径中的波浪线 (~) 为用home目录
    let mut resolved_path = PathBuf::new();
    if path.starts_with("~") {
        resolved_path.push(home_dir);
        resolved_path.push(&path[1..]); // 跳过波浪线
    } else {
        resolved_path.push(path);
    }

    // 创建所有必要的父目录
    if let Some(parent) = resolved_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            match e.kind() {
                ErrorKind::NotFound => (),
                _ => {
                    eprintln!("Error preparing directory structure: {}", e);
                    return false;
                }
            }
        }
    }

    // 尝试创建目标目录
    match fs::create_dir(&resolved_path) {
        Ok(_) => true,
        Err(e) => {
            eprintln!("Error creating directory: {}", e);
            false
        }
    }
}

fn is_directory(path: &str) -> bool {
    match fs::metadata(path) {
        Ok(metadata) => metadata.is_dir(),
        Err(_) => false,
    }
}

#[tauri::command]
fn restart_self() {
    utils::restart_self()
}

use std::error::Error;
use std::process::Command;
fn run_command(cmd: &str) -> Result<(), Box<dyn Error>> {
    let cmd_str = if cfg!(target_os = "windows") {
        format!("{}", cmd.replace("/", "\\"))
    } else {
        format!("{}", cmd)
    };

    let child = if cfg!(target_os = "windows") {
        Command::new("cmd").arg("/c").arg(cmd_str).spawn()?
    } else {
        Command::new("sh").arg("-c").arg(cmd_str).spawn()?
    };
    child.wait_with_output()?;
    Ok(())
}

#[tauri::command]
fn get_winfsp_install_state() -> Result<bool, usize> {
    #[cfg(not(target_os = "windows"))]
    return Ok(false);

    #[cfg(target_os = "windows")]
    match is_winfsp_installed() {
        Ok(is_enabled) => Ok(is_enabled),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
fn get_autostart_state() -> Result<bool, usize> {
    match is_autostart() {
        Ok(is_enabled) => Ok(is_enabled),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
fn set_autostart_state(enabled: bool) -> Result<(), ()> {
    let _ = set_autostart(enabled);
    Ok(())
}

#[tauri::command]
fn download_file(url: String, out_path: String) -> Result<bool, usize> {
    download_with_progress(&url, &out_path, |total_size, downloaded| {
        println!(
            "下载进度: {}/{}  {}%",
            total_size,
            downloaded,
            (100 * downloaded / total_size)
        );
    })
    .expect("下载失败");
    Ok(true)
}

#[tauri::command]
fn get_available_drive_letter() -> Result<String, String> {
    #[cfg(not(target_os = "windows"))]
    return Ok(String::from(""));
    #[cfg(target_os = "windows")]
    match find_first_available_drive_letter() {
        Ok(Some(drive)) => Ok(drive),
        Ok(None) => Ok(String::from("")),
        Err(e) => Ok(format!("{}", e)),
    }
}

#[tauri::command]
fn exit_app(app_handle: tauri::AppHandle) {
    let _ = app_handle.emit_all("exit_app", {});
}

#[tauri::command]
fn read_config_file(path: Option<&str>) -> Result<Value, String> {
    let path = path.unwrap_or(CONFIG_FILE);
    let home_dir = get_home_dir();
    let content_result = fs::read_to_string(if path == CONFIG_FILE {
        home_dir.join(USER_DATA_PATH).join(path)
    } else {
        PathBuf::from(path)
    });
    match content_result {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(config) => Ok(config),
            Err(json_error) => Err(format!("Failed to parse JSON from file: {}", json_error)),
        },
        Err(io_error) => Err(format!("Failed to read file: {}", io_error)),
    }
}

#[tauri::command]
async fn write_config_file(config_data: Value, path: Option<&str>) -> Result<(), String> {
    let path = path.unwrap_or(CONFIG_FILE);
    let home_dir = get_home_dir();
    let pretty_config = to_string_pretty(&config_data)
        .map_err(|json_error| format!("Failed to serialize JSON: {}", json_error))?;

    fs::write(
        if path == CONFIG_FILE {
            home_dir.join(USER_DATA_PATH).join(path)
        } else {
            PathBuf::from(path)
        },
        pretty_config,
    )
    .map_err(|io_error| format!("Failed to write file: {}", io_error))?;

    Ok(())
}
