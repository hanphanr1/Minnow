//! Listening TCP ports owned by a worktree's terminal process trees, read from
//! the system's own socket table (`netstat -ano`). Nothing is guessed: a port is
//! listed only when its owning PID is inside a Minnow terminal's job.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct ListeningPort {
    pub port: u16,
    pub address: String,
    pub pid: u32,
    pub terminal_id: String,
    pub terminal_name: String,
}

/// Parses `netstat -ano` TCP rows in LISTENING state into (address, port, pid).
pub fn parse_netstat(text: &str) -> Vec<(String, u16, u32)> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let columns: Vec<&str> = line.split_whitespace().collect();
        if columns.len() < 5
            || !columns[0].eq_ignore_ascii_case("TCP")
            || !columns[3].eq_ignore_ascii_case("LISTENING")
        {
            continue;
        }
        let local = columns[1];
        let Some(colon) = local.rfind(':') else { continue };
        let (Ok(port), Ok(pid)) = (local[colon + 1..].parse::<u16>(), columns[4].parse::<u32>()) else {
            continue;
        };
        rows.push((local[..colon].trim_matches(|c| c == '[' || c == ']').to_owned(), port, pid));
    }
    rows
}

fn netstat(protocol: &str) -> Result<String, String> {
    let mut command = std::process::Command::new("netstat");
    command.args(["-ano", "-p", protocol]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output().map_err(|error| format!("cannot read the socket table: {error}"))?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Ports whose owner is one of `owners` (pid -> terminal id and name).
pub fn listening(owners: &HashMap<u32, (String, String)>) -> Result<Vec<ListeningPort>, String> {
    if owners.is_empty() {
        return Ok(Vec::new());
    }
    let text = format!("{}\n{}", netstat("TCP")?, netstat("TCPv6")?);
    let mut ports: Vec<ListeningPort> = parse_netstat(&text)
        .into_iter()
        .filter_map(|(address, port, pid)| {
            owners.get(&pid).map(|(terminal_id, terminal_name)| ListeningPort {
                port,
                address,
                pid,
                terminal_id: terminal_id.clone(),
                terminal_name: terminal_name.clone(),
            })
        })
        .collect();
    ports.sort_by(|left, right| left.port.cmp(&right.port).then(left.address.cmp(&right.address)));
    ports.dedup_by(|left, right| left.port == right.port && left.pid == right.pid);
    Ok(ports)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ipv4_and_ipv6_listeners_only() {
        let text = "\r\nActive Connections\r\n\r\n  Proto  Local Address          Foreign Address        State           PID\r\n  TCP    0.0.0.0:5173           0.0.0.0:0              LISTENING       4120\r\n  TCP    127.0.0.1:52000        127.0.0.1:52001        ESTABLISHED     4120\r\n  TCP    [::]:3000              [::]:0                 LISTENING       77\r\n  UDP    0.0.0.0:53             *:*                                    9\r\n";
        assert_eq!(
            parse_netstat(text),
            vec![("0.0.0.0".to_string(), 5173, 4120), ("::".to_string(), 3000, 77)]
        );
    }
}
