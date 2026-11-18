use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=ORLogin.manifest");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/icon.svg");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let manifest_path = manifest_dir.join("ORLogin.manifest");
    let manifest_bytes = fs::read(&manifest_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", manifest_path.display(), e));

    let ico_path = manifest_dir.join("assets").join("icon.ico");
    let ico_bytes = fs::read(&ico_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", ico_path.display(), e));

    // Build a single 32-bit Windows PE .res stream containing RT_ICON + RT_GROUP_ICON
    // and RT_MANIFEST (type 24, ID 1) so icon & requireAdministrator embed on orlogin.exe
    // without forcing elevation on `cargo test` harness binaries.
    let res = build_win32_res(&ico_bytes, &manifest_bytes);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let res_path = out_dir.join("orlogin.res");
    fs::write(&res_path, &res)
        .unwrap_or_else(|e| panic!("failed to write {}: {}", res_path.display(), e));

    println!("cargo:rustc-link-arg-bin=orlogin={}", res_path.display());

    for flag in [
        "/DEPENDENTLOADFLAG:0x800",
        "/guard:cf",
        "/CETCOMPAT",
        "/DYNAMICBASE",
        "/NXCOMPAT",
        "/HIGHENTROPYVA",
    ] {
        println!("cargo:rustc-link-arg={flag}");
    }
}

fn build_win32_res(ico: &[u8], manifest: &[u8]) -> Vec<u8> {
    assert!(ico.len() >= 6, "icon.ico too small");
    let reserved = u16::from_le_bytes([ico[0], ico[1]]);
    let ico_type = u16::from_le_bytes([ico[2], ico[3]]);
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    assert!(
        reserved == 0 && ico_type == 1 && count > 0,
        "invalid ICONDIR header in icon.ico"
    );
    assert!(
        ico.len() >= 6 + count * 16,
        "truncated ICONDIRENTRY table in icon.ico"
    );

    let mut res = Vec::with_capacity(ico.len() + manifest.len() + 512);

    // Record 0: Mandatory 32-byte empty resource header marking a 32-bit .res stream.
    write_res_record(&mut res, 0, 0, 0, 0, &[]);

    // RT_ICON (3) entries + RT_GROUP_ICON (14, ID 1)
    let mut grp_dir = Vec::with_capacity(6 + count * 14);
    grp_dir.extend_from_slice(&0u16.to_le_bytes()); // idReserved
    grp_dir.extend_from_slice(&1u16.to_le_bytes()); // idType = 1 (icon)
    grp_dir.extend_from_slice(&(count as u16).to_le_bytes()); // idCount

    for i in 0..count {
        let entry_off = 6 + i * 16;
        let entry = &ico[entry_off..entry_off + 16];
        let size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        assert!(offset + size <= ico.len(), "icon frame {i} out of bounds");

        let icon_id = (i + 1) as u16;
        write_res_record(&mut res, 3, icon_id, 0x1010, 0x0409, &ico[offset..offset + size]);

        grp_dir.extend_from_slice(&entry[0..12]);
        grp_dir.extend_from_slice(&icon_id.to_le_bytes());
    }

    write_res_record(&mut res, 14, 1, 0x1030, 0x0409, &grp_dir);

    // RT_MANIFEST (24), ID = 1 (CREATEPROCESS_MANIFEST_RESOURCE_ID)
    write_res_record(&mut res, 24, 1, 0x1030, 0x0409, manifest);

    res
}

fn write_res_record(
    out: &mut Vec<u8>,
    type_id: u16,
    name_id: u16,
    mem_flags: u16,
    lang_id: u16,
    data: &[u8],
) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes()); // DataSize
    out.extend_from_slice(&32u32.to_le_bytes()); // HeaderSize
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&type_id.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&name_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&mem_flags.to_le_bytes()); // MemoryFlags
    out.extend_from_slice(&lang_id.to_le_bytes()); // LanguageId
    out.extend_from_slice(&0u32.to_le_bytes()); // Version
    out.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
    out.extend_from_slice(data);
    while out.len() % 4 != 0 {
        out.push(0);
    }
}
