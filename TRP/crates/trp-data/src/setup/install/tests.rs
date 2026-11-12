//! Tests of locating the install: a file naming its folder, however it was saved or pasted.

use super::*;

/// A file naming a folder is read however it was saved: by an editor,
/// with the mark UTF-8 may begin with, by Windows PowerShell's `>` (UTF-16
/// with its mark), with the quotes of "Copy as path", with lines after.
#[test]
fn a_hint_is_read_however_it_was_saved() {
    let dir = std::env::temp_dir().join(format!("trp-hint-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("spt_dir.txt");
    let wide = |text: &str, big: bool| -> Vec<u8> {
        let mut bytes = if big { vec![0xFE, 0xFF] } else { vec![0xFF, 0xFE] };
        for unit in text.encode_utf16() {
            bytes.extend(if big { unit.to_be_bytes() } else { unit.to_le_bytes() });
        }
        bytes
    };
    let saved: Vec<(&str, Vec<u8>)> = vec![
        ("plain", b"D:\\SPT".to_vec()),
        ("a line's end and spaces", b"  D:\\SPT  \r\n".to_vec()),
        ("UTF-8 with its mark", [&[0xEF, 0xBB, 0xBF][..], b"D:\\SPT\r\n"].concat()),
        ("UTF-16, as PowerShell 5.1's >", wide("D:\\SPT\r\n", false)),
        ("UTF-16 the other way about", wide("D:\\SPT", true)),
        ("quoted", b"\"D:\\SPT\"".to_vec()),
        ("an empty line first", b"\r\n\r\nD:\\SPT\r\nsomething else".to_vec()),
    ];
    for (how, bytes) in saved {
        std::fs::write(&file, bytes).unwrap();
        assert_eq!(read_hint(&file).as_deref(), Some("D:\\SPT"), "{how}");
    }
    std::fs::write(&file, b"  \r\n").unwrap();
    assert_eq!(read_hint(&file), None, "a file naming nothing");
    assert_eq!(read_hint(&dir.join("not there.txt")), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pasted_folder_is_cleaned() {
    assert_eq!(clean_path("  \"D:\\Games\\SPT\"  \n"), "D:\\Games\\SPT");
    assert_eq!(clean_path("D:\\Games\\S P T"), "D:\\Games\\S P T");
    assert_eq!(clean_path("\u{feff}D:\\SPT\r\n"), "D:\\SPT");
    assert_eq!(clean_path(""), "");
}

/// The `local` folder is the workspace's from anywhere under it, made or
/// not, whatever `local` folder lies nearer; with no workspace above, the
/// first `local` folder going up.
#[test]
fn the_local_folder_is_the_workspace_s_from_anywhere_under_it() {
    let top = std::env::temp_dir().join(format!("trp-local-test-{}", std::process::id()));
    let workspace = top.join("workspace");
    let deep = workspace.join("target").join("release");
    std::fs::create_dir_all(workspace.join("crates")).unwrap();
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(workspace.join("Cargo.toml"), "").unwrap();
    // The first start: no `local` folder yet.
    assert_eq!(local_folder_from(&workspace), workspace.join("local"));
    assert_eq!(local_folder_from(&deep), workspace.join("local"));
    // A `local` folder nearer than the workspace's is not taken for it.
    std::fs::create_dir_all(workspace.join("target").join("local")).unwrap();
    assert_eq!(local_folder_from(&deep), workspace.join("local"));
    // No workspace above: the first `local` folder going up.
    let loose = top.join("loose");
    std::fs::create_dir_all(loose.join("local")).unwrap();
    std::fs::create_dir_all(loose.join("bin")).unwrap();
    assert_eq!(local_folder_from(&loose.join("bin")), loose.join("local"));
    let _ = std::fs::remove_dir_all(&top);
}
