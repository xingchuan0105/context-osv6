unsafe extern "C" {
    fn once_probe() -> i32;
}
fn main() {
    std::thread::spawn(|| {
        for _ in 0..4 {
            assert_eq!(unsafe { once_probe() }, 1);
        }
    })
    .join()
    .unwrap();
    eprintln!("PASS native call_once");
}
