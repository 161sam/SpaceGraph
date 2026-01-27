use anyhow::Result;
use spacegraph_core::{id_process, Delta, Msg, Node, NodeId};
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;

fn list_pids() -> HashSet<i32> {
    let mut set = HashSet::new();
    if let Ok(rd) = std::fs::read_dir("/proc") {
        for ent in rd.flatten() {
            if let Ok(pid) = ent.file_name().to_string_lossy().parse::<i32>() {
                set.insert(pid);
            }
        }
    }
    set
}

pub fn spawn(node_id: &str, tx: mpsc::Sender<Msg>) -> Result<()> {
    let node_id = node_id.to_string();
    tokio::spawn(async move {
        let mut prev = list_pids();
        let mut batch_id: u64 = 1;

        loop {
            tokio::time::sleep(Duration::from_millis(750)).await;
            let cur = list_pids();

            // new pids
            let mut any = false;
            let begin = Msg::Event { delta: Delta::BatchBegin { id: batch_id } };
            let end = Msg::Event { delta: Delta::BatchEnd { id: batch_id } };

            for pid in cur.difference(&prev) {
                any = true;
                let id = id_process(&node_id, *pid);
                // minimal node (details come with next snapshot refresh; MVP ok)
                let node = Node::Process {
                    pid: *pid,
                    ppid: 0,
                    exe: "<unknown>".into(),
                    cmdline: "<new>".into(),
                    uid: 0,
                };
                let _ = tx.send(Msg::Event { delta: Delta::UpsertNode { id, node } }).await;
            }

            // removed pids
            for pid in prev.difference(&cur) {
                any = true;
                let id: NodeId = id_process(&node_id, *pid);
                let _ = tx.send(Msg::Event { delta: Delta::RemoveNode { id } }).await;
            }

            if any {
                let _ = tx.send(begin).await;
                let _ = tx.send(end).await;
                batch_id = batch_id.wrapping_add(1);
            }

            prev = cur;
        }
    });

    Ok(())
}
