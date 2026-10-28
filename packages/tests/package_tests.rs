//! Tests for the Elvaris package format library.

use packages::{
    Architecture, Dependency, Manifest, PackageBuilder, PackageReader, Permission,
    signing::{sign_package, verify_package_signature, KeyPair, PublicKey},
};

fn test_manifest() -> Manifest {
    Manifest::new(
        "test-app".to_string(),
        "org.elvaris.test-app".to_string(),
        "1.0.0".to_string(),
        Architecture::X86_64,
        "/bin/test-app".to_string(),
    )
}

#[test]
fn manifest_default_is_valid() {
    let manifest = test_manifest();
    assert!(manifest.validate().is_ok());
}

#[test]
fn manifest_empty_name_rejected() {
    let mut m = test_manifest();
    m.name = String::new();
    assert!(m.validate().is_err());
}

#[test]
fn manifest_invalid_identifier_rejected() {
    let mut m = test_manifest();
    m.identifier = "no-dots".to_string();
    assert!(m.validate().is_err());
}

#[test]
fn manifest_invalid_version_rejected() {
    let mut m = test_manifest();
    m.version = "not-a-version".to_string();
    assert!(m.validate().is_err());
}

#[test]
fn manifest_absolute_path_required() {
    let mut m = test_manifest();
    m.entrypoint = "relative/path".to_string();
    assert!(m.validate().is_err());
}

#[test]
fn manifest_path_traversal_rejected() {
    let mut m = test_manifest();
    m.files = vec!["/etc/../passwd".to_string()];
    assert!(m.validate().is_err());
}

#[test]
fn manifest_duplicate_file_rejected() {
    let mut m = test_manifest();
    m.files = vec!["/bin/app".to_string(), "/bin/app".to_string()];
    assert!(m.validate().is_err());
}

#[test]
fn manifest_unknown_permission_rejected() {
    assert!(Permission::from_str("unknown.permission").is_err());
}

#[test]
fn permission_as_str_roundtrip() {
    let perms = vec![
        Permission::ConsoleInput,
        Permission::ConsoleOutput,
        Permission::FilesystemRead,
        Permission::FilesystemWrite,
        Permission::NetworkClient,
        Permission::NetworkServer,
        Permission::ProcessInspect,
        Permission::DeviceAccess,
        Permission::SystemShutdown,
    ];
    for p in perms {
        let s = p.as_str();
        assert_eq!(Permission::from_str(s).unwrap(), p);
    }
}

#[test]
fn package_create_and_read_roundtrip() {
    let manifest = test_manifest();
    let files = vec![
        ("/bin/test-app".to_string(), b"hello world".to_vec(), 0o755, 2u8),
        ("/etc/test-app.conf".to_string(), b"config data".to_vec(), 0o644, 0u8),
    ];

    let mut builder = PackageBuilder::new(manifest.clone());
    for (path, data, perms, ftype) in &files {
        builder = builder.add_file(path.clone(), data.clone(), *perms, *ftype);
    }

    let package_bytes = builder.build().unwrap();
    let package = PackageReader::read(&package_bytes).unwrap();

    assert_eq!(package.manifest.name, "test-app");
    assert_eq!(package.manifest.identifier, "org.elvaris.test-app");
    assert_eq!(package.files.len(), 2);
    assert_eq!(package.file_data("/bin/test-app").unwrap(), b"hello world");
    assert_eq!(package.file_data("/etc/test-app.conf").unwrap(), b"config data");
}

#[test]
fn package_invalid_magic_rejected() {
    let mut bad = vec![0u8; 100];
    bad[0..6].copy_from_slice(b"BADPKG");
    let result = PackageReader::read(&bad);
    assert!(result.is_err());
}

#[test]
fn package_path_traversal_rejected() {
    let manifest = test_manifest();
    let mut builder = PackageBuilder::new(manifest);
    builder = builder.add_file(
        "/etc/../passwd".to_string(),
        b"data".to_vec(),
        0o644,
        0u8,
    );
    let result = builder.build();
    assert!(result.is_err());
}

#[test]
fn signing_and_verification_roundtrip() {
    let manifest = test_manifest();
    let builder = PackageBuilder::new(manifest);
    let package_bytes = builder.build().unwrap();

    let key_pair = KeyPair::generate();
    let (signed, _) = sign_package(&key_pair, &package_bytes);

    // Strip signature for verification
    let unsigned_len = signed.len() - 65;
    let signature = &signed[unsigned_len..unsigned_len + 64];
    let public_key = key_pair.public_key();

    assert!(verify_package_signature(&signed[..unsigned_len], signature, &public_key).is_ok());
}

#[test]
fn verification_wrong_key_fails() {
    let manifest = test_manifest();
    let builder = PackageBuilder::new(manifest);
    let package_bytes = builder.build().unwrap();

    let key_pair = KeyPair::generate();
    let (signed, _) = sign_package(&key_pair, &package_bytes);

    let wrong_key = KeyPair::generate();
    let public_key = wrong_key.public_key();

    let unsigned_len = signed.len() - 65;
    let signature = &signed[unsigned_len..unsigned_len + 64];
    assert!(verify_package_signature(&signed[..unsigned_len], signature, &public_key).is_err());
}

#[test]
fn deterministic_package_output() {
    let manifest = test_manifest();
    let files = vec![(
        "/bin/deterministic".to_string(),
        b"same content".to_vec(),
        0o755,
        2u8,
    )];

    let mut builder1 = PackageBuilder::new(manifest.clone());
    builder1 = builder1.add_file(
        files[0].0.clone(),
        files[0].1.clone(),
        files[0].2,
        files[0].3,
    );

    let mut builder2 = PackageBuilder::new(manifest);
    builder2 = builder2.add_file(
        files[0].0.clone(),
        files[0].1.clone(),
        files[0].2,
        files[0].3,
    );

    let pkg1 = builder1.build().unwrap();
    let pkg2 = builder2.build().unwrap();

    // Unsigned packages should be byte-identical
    assert_eq!(pkg1, pkg2);
}

#[test]
fn dependency_parsing() {
    let dep = Dependency {
        identifier: "org.elvaris.libc".to_string(),
        constraint: ">= 1.0.0".to_string(),
    };
    assert_eq!(dep.identifier, "org.elvaris.libc");
    assert_eq!(dep.constraint, ">= 1.0.0");
}

#[test]
fn architecture_from_str() {
    assert_eq!(
        Architecture::from_str("x86_64").unwrap(),
        Architecture::X86_64
    );
    assert_eq!(
        Architecture::from_str("aarch64").unwrap(),
        Architecture::AArch64
    );
    assert!(Architecture::from_str("armv7").is_err());
}

#[test]
fn package_file_lookup() {
    let manifest = test_manifest();
    let mut builder = PackageBuilder::new(manifest);
    builder = builder.add_file(
        "/bin/lookup-test".to_string(),
        b"content".to_vec(),
        0o755,
        2u8,
    );
    let bytes = builder.build().unwrap();
    let pkg = PackageReader::read(&bytes).unwrap();

    let entry = pkg.find_file("/bin/lookup-test").unwrap();
    assert_eq!(entry.path, "/bin/lookup-test");
    assert_eq!(entry.size, 7);
    assert_eq!(entry.file_type, 2);
    assert_eq!(pkg.file_data("/bin/lookup-test").unwrap(), b"content");
    assert_eq!(pkg.file_data("/nonexistent"), None);
}
