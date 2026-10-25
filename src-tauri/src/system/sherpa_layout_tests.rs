// The sherpa-onnx FFI structs are hand-written, so a field the header has and
// we don't (v1.13.6 added one) shifts every later field without a compile
// error. Each FFI module lists its structs with `c_layout!`; this test checks
// the Rust sizes/offsets against layout.txt, which scripts/sherpa-layout.c
// prints from the committed header (CI re-runs it and diffs). docs/adr/0002.

// One line per struct: `<C name> <size> <field>=<offset> ...`, the same format
// the C program prints.
macro_rules! c_layout {
    ($($rust:ident = $c:ident { $($f:ident),* $(,)? })*) => {{
        let mut s = String::new();
        $(
            s += &format!("{} {}", stringify!($c), std::mem::size_of::<$rust>());
            $( s += &format!(" {}={}", stringify!($f), std::mem::offset_of!($rust, $f)); )*
            s.push('\n');
        )*
        s
    }};
}

#[test]
fn ffi_structs_match_the_c_header() {
    let rust = [super::sherpa::layout_lines(), super::sherpa_stt::layout_lines(), super::diarize::layout_lines()].concat();
    let c = include_str!("../../sidecars/sherpa/include/layout.txt").replace("\r\n", "\n");
    for (r, c) in rust.lines().zip(c.lines()) {
        assert_eq!(r, c, "Rust struct layout differs from the sherpa-onnx header");
    }
    assert_eq!(rust.lines().count(), c.lines().count(), "struct list differs from layout.txt");
}
