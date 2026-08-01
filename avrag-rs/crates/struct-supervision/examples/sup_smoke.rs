fn main() {
    let bytes = std::fs::read("/tmp/ipd.grids.json").unwrap();
    let input = avrag_struct_supervision::SuperviseInput::from_json_bytes(&bytes).unwrap();
    let s = avrag_struct_supervision::session::Session::new(&input).unwrap();
    assert_eq!(s.grids.len(), 1);
    assert_eq!(s.grids[0].n_rows(), 370);
    let r = s.t_run_check(&serde_json::json!({"sql": "SELECT COUNT(*) FROM t0"}));
    assert!(r.contains("370"), "{r}");
    let r2 = s.t_run_check(&serde_json::json!({"sql": "ATTACH '/etc/passwd'"}));
    assert!(r2.contains("守卫"), "{r2}");
    println!("SMOKE OK: ipd 370 rows; run_check guard works");
    let brief = s.briefing("huawei_ipd_370_activities.xlsx.md");
    assert!(brief.contains("共 1 张表"));
    println!(
        "briefing head: {}",
        &brief.chars().take(120).collect::<String>()
    );
}
