use super::*;

#[test]
fn test_process_group_guard_disarm_and_kill() {
    let mut guard = GroupGuard::new(Some(12345));
    assert_eq!(guard.pid, Some(12345));
    guard.disarm();
    assert_eq!(guard.pid, None);
}

#[cfg(unix)]
#[tokio::test]
async fn test_process_group_kill_terminates_group() {
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;
    use std::time::Duration;

    let mut cmd = tokio::process::Command::new("sleep");
    cmd.arg("4343");
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    configure_process_group(&mut cmd);

    let mut child = cmd.spawn().unwrap();
    let pid = child.id().unwrap();

    let mut guard = GroupGuard::new(Some(pid));
    guard.kill();

    let wait_res = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
    assert!(wait_res.is_ok(), "child should terminate after group kill");
    let status = wait_res.unwrap().unwrap();
    assert_eq!(
        status.signal(),
        Some(9),
        "process should be terminated by SIGKILL (9)"
    );
}
