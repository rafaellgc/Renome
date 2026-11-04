//! Builds the copy of libarchive in `libarchive/` as a static library with
//! nothing from the system: only its own readers (RAR among them) are
//! needed, and the program must run where those libraries are missing.

fn main() {
    println!("cargo:rerun-if-changed=libarchive");
    let target = std::env::var("TARGET").unwrap_or_default();
    let mut c = cmake::Config::new("libarchive");
    for off in [
        "BUILD_SHARED_LIBS",
        "ENABLE_TEST",
        "ENABLE_TAR",
        "ENABLE_CPIO",
        "ENABLE_CAT",
        "ENABLE_UNZIP",
        "ENABLE_WERROR",
        "ENABLE_OPENSSL",
        "ENABLE_MBEDTLS",
        "ENABLE_NETTLE",
        "ENABLE_MD",
        "ENABLE_LIBB2",
        "ENABLE_LZ4",
        "ENABLE_LZO",
        "ENABLE_LZMA",
        "ENABLE_ZSTD",
        "ENABLE_ZLIB",
        "ENABLE_BZip2",
        "ENABLE_LIBXML2",
        "ENABLE_BSDXML",
        "ENABLE_EXPAT",
        "ENABLE_WIN32_XMLLITE",
        "ENABLE_PCREPOSIX",
        "ENABLE_PCRE2POSIX",
        "ENABLE_LIBGCC",
        "ENABLE_CNG",
        "ENABLE_XATTR",
        "ENABLE_ACL",
        "ENABLE_ICONV",
        "ENABLE_INSTALL",
    ] {
        c.define(off, "OFF");
    }
    // Patterns are not used; on Windows the search for a library of them
    // ends in an error.
    c.define("POSIX_REGEX_LIB", "NONE");
    // Debug C runtimes do not mix with Rust's on Windows.
    c.profile("Release").build_target("archive_static");
    let lib = c.build().join("build").join("libarchive");
    // Visual Studio puts it in a folder per configuration.
    println!(
        "cargo:rustc-link-search=native={}",
        lib.join("Release").display()
    );
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=static=archive");
    if target.contains("windows") {
        println!("cargo:rustc-link-lib=bcrypt");
        println!("cargo:rustc-link-lib=advapi32");
    }
}
