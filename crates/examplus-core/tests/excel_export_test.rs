use examplus_core::export::{
    export_exam_results, StudentGradeRecord, ViolationExportRecord,
};
use tempfile::NamedTempFile;

#[test]
fn test_excel_export_creates_valid_file() {
    let tmp_file = NamedTempFile::new().expect("Tạo temp file");
    let file_path = tmp_file.path().with_extension("xlsx");

    let grades = vec![
        StudentGradeRecord {
            student_code: "11001".to_string(),
            full_name: "Trần Thị Bình".to_string(),
            class_name: "11A2".to_string(),
            variant_code: "132".to_string(),
            part1_score: 3.0,
            part2_score: 4.0,
            part3_score: 3.0,
            total_score: 10.0,
            submitted_at: Some("2026-11-08 09:45:00".to_string()),
            violation_count: 0,
        },
        StudentGradeRecord {
            student_code: "11002".to_string(),
            full_name: "Lê Hoàng Long".to_string(),
            class_name: "11A2".to_string(),
            variant_code: "209".to_string(),
            part1_score: 2.5,
            part2_score: 2.0,
            part3_score: 1.5,
            total_score: 6.0,
            submitted_at: Some("2026-11-08 09:50:12".to_string()),
            violation_count: 2,
        },
    ];

    let violations = vec![ViolationExportRecord {
        student_code: "11002".to_string(),
        full_name: "Lê Hoàng Long".to_string(),
        class_name: "11A2".to_string(),
        violation_type: "TAB_SWITCH".to_string(),
        duration_seconds: 7,
        details: "Chuyển tab sang trình duyệt khác".to_string(),
        timestamp: "2026-11-08 09:20:15".to_string(),
    }];

    export_exam_results(
        &file_path,
        "Khảo sát chất lượng Toán 11",
        "Toán học",
        &grades,
        &violations,
    )
    .expect("Xuất file Excel thành công");

    assert!(file_path.exists());
    let metadata = std::fs::metadata(&file_path).expect("Đọc metadata file excel");
    assert!(metadata.len() > 1000, "File Excel phải có dung lượng lớn hơn 1KB");

    // Dọn dẹp tệp tạm
    let _ = std::fs::remove_file(file_path);
}
