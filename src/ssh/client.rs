use crate::config::{ConnectionMode, ResolvedServer};
use crate::wallix::WallixMenuEntry;
#[cfg(unix)]
use crate::wallix::{parse_wallix_menu, select_id_for_server};
use anyhow::Result;
#[cfg(unix)]
use nix::pty::{ForkptyResult, Winsize, forkpty};
#[cfg(unix)]
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::Command;
#[cfg(unix)]
use std::{
    ffi::CString,
    io::{Read, Write},
};

#[path = "client/args.rs"]
mod args;
pub use args::build_ssh_args;
#[cfg(unix)]
pub(crate) use args::build_wallix_bastion_args;

#[path = "client/wallix_pty.rs"]
mod wallix_pty;
#[cfg(unix)]
pub(crate) use wallix_pty::{
    connect_wallix_via_pty_with_selection, contains_ssh_auth_prompt, contains_wallix_prompt,
    parse_wallix_page_position, setup_askpass_script, spawn_wallix_pty, wallix_connected_directly,
};
#[cfg(all(unix, test))]
pub(crate) use wallix_pty::{
    contains_wallix_return_selector_prompt, contains_wallix_target_address_prompt,
};

#[path = "client/connect.rs"]
mod connect;
pub use connect::{
    connect, connect_blocking, connect_blocking_wallix_with_selection,
    connect_wallix_with_selection, fetch_wallix_menu_entries, is_control_master_active,
};

#[cfg(test)]
#[path = "client/tests.rs"]
mod tests;
