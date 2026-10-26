//! The install, read once for the whole binary. `Database::load` parses the item database (tens
//! of megabytes of JSON) and nearly every test called it for itself, forty-two times a run; since
//! 0.100.3 the tests borrow the one copy. The assets are not shared: `Assets` caches as it reads
//! and the tests take it by `&mut`, so each makes its own from the install.
use std::sync::OnceLock;
use trp_data::{Database, Install};

static INSTALL: OnceLock<Option<Install>> = OnceLock::new();
static DATABASE: OnceLock<Option<Database>> = OnceLock::new();

/// Where the install is, or None, once with a note, where none is configured: the test then
/// passes with that note, as every install-bound test does.
pub fn install() -> Option<Install> {
    INSTALL
        .get_or_init(|| match Install::locate() {
            Ok(install) => Some(install),
            Err(_) => {
                eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
                None
            }
        })
        .clone()
}

/// The item database, read once and borrowed by every test after.
pub fn database() -> Option<&'static Database> {
    DATABASE.get_or_init(|| install().map(|install| Database::load(&install).expect("database"))).as_ref()
}
