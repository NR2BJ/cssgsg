//! `mozc` 기능: build/mozc-out(tools/mozc/build.sh)의 정적 라이브러리를 링크한다.
//! staticlib(libcssgsg_core.a)에는 Mozc가 같이 묶인다. C++ 표준 라이브러리와 프레임워크는 최종 링크(Xcode)가 붙인다.
fn main() {
    println!("cargo:rerun-if-env-changed=CSSGSG_MOZC_DIR");
    if std::env::var_os("CARGO_FEATURE_MOZC").is_none() {
        return;
    }
    let dir = std::env::var("CSSGSG_MOZC_DIR")
        .unwrap_or_else(|_| format!("{}/../build/mozc-out", std::env::var("CARGO_MANIFEST_DIR").unwrap()));
    let lib = format!("{dir}/lib/libcssgsg_mozc.a");
    assert!(
        std::path::Path::new(&lib).exists(),
        "{lib}가 없다. 먼저 bash tools/mozc/build.sh로 Mozc를 빌드한다"
    );
    println!("cargo:rerun-if-changed={lib}");
    println!("cargo:rustc-link-search=native={dir}/lib");
    println!("cargo:rustc-link-lib=static=cssgsg_mozc");
    // 테스트·CLI처럼 cargo가 직접 링크할 때 필요한 것(최종 앱은 Xcode가 붙인다).
    println!("cargo:rustc-link-lib=c++");
    for framework in ["Foundation", "CoreFoundation", "Carbon", "IOKit", "Security"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
