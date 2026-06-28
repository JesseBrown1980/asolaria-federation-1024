use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use asolaria_server_cosign_ledger::py_parity::{
    compute_append, parse_line, persisted_line, AppendOut, PyChainErr, PyRow, ResumeState,
};

use crate::resume;

pub struct Writer {
    pub shadow: File,
    pub head: ResumeState,
    pub shadow_path: PathBuf,
}

pub fn ensure_clean_tail(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let _ = OpenOptions::new().create(true).append(true).open(path)?;
        return Ok(());
    }
    let mut f = OpenOptions::new().read(true).open(path)?;
    let len = f.metadata()?.len();
    if len == 0 {
        return Ok(());
    }
    f.seek(SeekFrom::End(-1))?;
    let mut b = [0u8; 1];
    f.read_exact(&mut b)?;
    if b[0] != b'\n' {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "shadow ledger tail is not newline-terminated",
        ));
    }
    Ok(())
}

pub fn append_shadow(
    writer: &mut Writer,
    payload: PyRow,
    now: &str,
) -> Result<AppendOut, PyChainErr> {
    writer.head = resume::resume_from(&writer.shadow_path).map_err(|_| PyChainErr::Io)?;
    let (row, out) = compute_append(&writer.head, payload, now);
    let line = persisted_line(&row);
    let before = writer
        .shadow
        .seek(SeekFrom::End(0))
        .map_err(|_| PyChainErr::Io)?;
    if writer.shadow.write_all(line.as_bytes()).is_err()
        || writer.shadow.flush().is_err()
        || writer.shadow.sync_data().is_err()
    {
        let _ = writer.shadow.set_len(before);
        return Err(PyChainErr::Io);
    }
    writer.head = ResumeState {
        head_seq: out.seq,
        head_hash: out.row_hash.clone(),
    };
    Ok(out)
}

pub fn parse_payload(body: &[u8]) -> Result<PyRow, PyChainErr> {
    parse_line(body)
}
