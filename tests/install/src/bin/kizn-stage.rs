//! `just stage-release <dir> [version]`: stages a stub release (or real
//! bundles from KIZN_STAGE_ASSETS) under <dir>, for trying the install
//! scripts by hand against a local server.

use std::path::PathBuf;

use kizn_install_tests::Stage;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(dir) = args.next() else {
        eprintln!("usage: kizn-stage <dir> [version]");
        std::process::exit(2);
    };
    let version = args.next().unwrap_or_else(|| "0.1.0".into());
    let stage = Stage { root: PathBuf::from(&dir), real: std::env::var_os("KIZN_STAGE_ASSETS").map(PathBuf::from) };
    if let Err(e) = stage.release(&version, !version.contains('-')) {
        eprintln!("kizn-stage: {e}");
        std::process::exit(1);
    }
    println!("staged kizn v{version} in {dir}");
    println!("serve it:  python3 -m http.server -d {dir} 8765");
    println!("install:   KIZN_RELEASE_BASE=http://127.0.0.1:8765 sh docs/install.sh");
}
