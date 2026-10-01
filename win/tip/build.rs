fn main() {
    // DllGetClassObject 같은 표준 COM 내보내기를 .def 없이 내보내면 MSVC 링커가 "PRIVATE이어야 한다"(LNK4104)고
    // 경고한다. rustc가 만드는 .def에 PRIVATE를 붙일 방법이 없고, 가져오기 라이브러리에 남는 것뿐이라 그 경고만 끈다.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/IGNORE:4104");
    }
}
