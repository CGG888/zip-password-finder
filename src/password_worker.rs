use crossbeam_channel::{Receiver, Sender};
use indicatif::ProgressBar;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::{fs, thread};

pub fn password_checker(
    index: usize,
    file_path: &Path,
    receive_password: Receiver<String>,
    send_password_found: Sender<String>,
    stop_signal: Arc<AtomicBool>,
    progress_bar: ProgressBar,
) -> JoinHandle<()> {
    let file = fs::File::open(file_path).expect("File should exist");
    thread::Builder::new()
        .name(format!("worker-{}", index))
        .spawn(move || {
            let mut archive = zip::ZipArchive::new(file).expect("Archive validated before-hand");
            while !stop_signal.load(Ordering::Relaxed) {
                match receive_password.recv() {
                    Err(_) => break, // disconnected
                    Ok(password) => {
                        // From the Rust doc:
                        // This function sometimes accepts wrong password. This is because the ZIP spec only allows us to check for a 1/256 chance that the password is correct.
                        // There are many passwords out there that will also pass the validity checks we are able to perform.
                        // This is a weakness of the ZipCrypto algorithm, due to its fairly primitive approach to cryptography.
                        let res = archive.by_index_decrypt(0, password.as_bytes());
                        match res {
                            Err(e) => panic!("Unexpected error {:?}", e),
                            Ok(Err(_)) => (), // invalid password
                            Ok(Ok(mut zip)) => {
                                // Validate password by reading the zip file to make sure it is not merely a hash collision.
                                // Conflicts are pretty rare to not care about reusing the buffer.
                                let mut buffer = Vec::with_capacity(zip.size() as usize);
                                match zip.read_to_end(&mut buffer) {
                                    Err(_) => (), // password collision - continue
                                    Ok(_) => {
                                        // Send password and continue processing while waiting for signal
                                        send_password_found
                                            .send(password)
                                            .expect("Send found password should not fail");
                                    }
                                }
                            }
                        }
                        progress_bar.inc(1);
                    }
                }
            }
        })
        .unwrap()
}