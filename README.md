# 🎓 ExamPlus — Hệ Thống Thi Trắc Nghiệm Chuẩn GDPT 2018

> **Mã nguồn công khai · Chỉ sử dụng cho giáo dục phi thương mại**  
> *Public Source Code · Strictly for Non-Commercial Educational Use Only*

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?logo=rust)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%20x64-blue.svg?logo=windows)](https://www.microsoft.com/windows)
[![License: Non-Commercial Educational](https://img.shields.io/badge/License-Non--Commercial%20Educational-green.svg)](LICENSE)
[![Release](https://img.shields.io/badge/Release-v1.0.0-emerald.svg)](https://github.com/anhpt-udu-ptit/ExamPlus/releases)
[![Architecture](https://img.shields.io/badge/Architecture-Native%20Desktop%20%26%20Axum-purple.svg)](https://github.com/tokio-rs/axum)

---

## 1. Giới Thiệu & Điểm Nổi Bật

**ExamPlus** là giải pháp phần mềm thi trắc nghiệm Desktop độc lập dành riêng cho giáo viên, trường học và các tổ chức giáo dục tại Việt Nam. Phần mềm được thiết kế nhằm đáp ứng trọn vẹn chương trình **Giáo Dục Phổ Thông 2018 (GDPT 2018)** với cấu trúc 3 phần thi chuẩn hóa của Bộ Giáo dục và Đào tạo, hoạt động mượt mà trên các máy tính văn phòng cấu hình phổ thông mà không phụ thuộc vào kết nối Internet bên ngoài.

* **Cấu trúc 3 phần thi GDPT 2018:**
  * **Phần I — Trắc nghiệm nhiều lựa chọn:** 4 phương án $A, B, C, D$ truyền thống (0,25 điểm/câu).
  * **Phần II — Trắc nghiệm Đúng/Sai:** 4 lệnh hỏi độc lập $a, b, c, d$ với thuật toán chấm điểm lũy tiến chuẩn Bộ GD&ĐT (đúng 1 ý: 0,1 đ; đúng 2 ý: 0,25 đ; đúng 3 ý: 0,5 đ; đúng cả 4 ý: 1,0 đ).
  * **Phần III — Trắc nghiệm trả lời ngắn:** Điền số nguyên hoặc số thập phân, tự động chuẩn hóa dấu phẩy `,` hoặc chấm `.` và dấu âm `-`.
* **Bóc tách đề thi Word (.docx) & PDF tự động:** Nhập trực tiếp đề thi có sẵn của giáo viên; tự động nhận diện câu hỏi, bảng dữ liệu, hình vẽ đồ thị và công thức Toán.
* **Vector hóa công thức Toán MathType:** Giải mã nhị phân cấu trúc GDI WMF/EMF sang ảnh vector SVG Base64 sắc nét, hiển thị hoàn hảo trên mọi kích thước màn hình mà không bị vỡ hạt hay méo tỷ lệ.
* **Kiến trúc Mạng Kép (Dual-Network):**
  * **Chế độ LAN Cục bộ (100% Offline):** Hoạt động hoàn toàn trong mạng Wi-Fi hoặc mạng dây phòng máy của trường, độ trễ cực thấp ($< 5\text{ms}$), không tốn dung lượng Internet.
  * **Chế độ Xuyên Mạng (Online 4G):** Tích hợp đường hầm bảo mật Cloudflare Tunnel nhúng; học sinh ở xa dùng mạng 4G cá nhân quét mã QR vào làm bài ngay lập tức mà không cần mở cổng modem (No Port-Forwarding).
* **Bảo mật & Chống gian lận đa tầng:** Tự động khóa toàn màn hình (Fullscreen), phát hiện chuyển tab hoặc mất tiêu điểm, ngăn chặn đăng nhập đúp (chống thi hộ), watermark mờ định danh thí sinh, ghi nhật ký vi phạm thời gian thực về màn hình giám thị.
* **Xuất báo cáo kết quả tức thì:** Tự động tổng hợp điểm số và lịch sử làm bài xuất ra tệp Excel (`.xlsx`) chỉ với 1 click chuột.

---

## 2. Giao Diện & Minh Họa (Visual Showcase)

### 2.1. Studio Quản Lý Của Giáo Viên (Teacher Studio)
* **Xem trước đề thi & bóc tách công thức:** Hiển thị trực quan nội dung câu hỏi, bảng biến thiên và công thức MathType SVG ngay trong ứng dụng native.
* **Bộ sửa ma trận đáp án trực quan:** Cho phép giáo viên kiểm tra và điều chỉnh đáp án Đúng/Sai, tự động tính điểm lại toàn bộ bài thi khi có thay đổi đáp án (Re-grading).
* **Bảng điều khiển phòng thi & Bảng điểm Live:** Cập nhật tiến độ nộp bài, điểm số từng phần và cảnh báo vi phạm của từng thí sinh theo thời gian thực qua WebSockets.

### 2.2. Giao Diện Làm Bài Của Thí Sinh (Student Web Client)
* **Tối ưu trải nghiệm thi cử:** Thiết kế theo chuẩn giao diện hiện đại (lấy cảm hứng từ Quizlet & Canvas), chuyển động mượt mà với hiệu ứng vật lý lò xo (Spring physics).
* **Thanh điều hướng câu hỏi thông minh:** Đánh dấu trực quan trạng thái câu hỏi (🟢 Đã làm, 🟡 Chưa làm đủ ý Phần II, ⚪ Chưa làm, 🔵 Đang xem).
* **Modal xác nhận nộp bài chi tiết:** Cảnh báo cụ thể số lượng câu hỏi còn thiếu theo từng phần trước khi nộp bài.

---

## 3. Tính Năng Chi Tiết (Core Features)

| Phân hệ | Tính năng nổi bật |
|---|---|
| **Bộ Bóc Tách Đề Thi** | • Hỗ trợ tệp Microsoft Word (`.docx`) và tài liệu PDF chuẩn.<br>• Tự động phân chia 3 phần thi GDPT 2018 hoặc đề 100% trắc nghiệm truyền thống.<br>• Bóc tách ảnh vector WMF/EMF, đồ thị và bảng biến thiên.<br>• Tự động đọc bảng đáp án ở cuối đề thi (hỗ trợ đề đơn và đề đa mã). |
| **Xáo Đề & Khảo Thí** | • Tạo nhiều mã đề ngẫu nhiên với thuật toán xáo độc lập từng phần.<br>• Giữ nguyên cấu trúc logic nội dung và đồ thị tương ứng.<br>• Đồng bộ thời gian phòng thi với máy chủ, bảo vệ 30 giây ân hạn hết giờ. |
| **Giám Thị Thời Gian Thực** | • WebSocket Telemetry Hub nhận nhịp tim thí sinh liên tục.<br>• Bắt sự kiện rời trang (`visibilitychange`), chuyển tab (`blur`), thoát Fullscreen.<br>• Slide-over Audit Drawer hiển thị chi tiết dòng thời gian vi phạm của từng em.<br>• Cô lập dữ liệu thí sinh hoàn toàn giữa các phòng thi khác nhau. |
| **Báo Cáo & Dữ Liệu** | • Cơ sở dữ liệu SQLite cục bộ (WAL mode), đảm bảo an toàn dữ liệu 100% trên máy tính của giáo viên.<br>• Xuất bảng điểm chi tiết ra file Excel `.xlsx` gồm SBD, họ tên, lớp, điểm từng phần, tổng điểm và số lần vi phạm. |

---

## 4. Công Nghệ Sử Dụng (Tech Stack)

* **Backend & Core Engine:** [Rust 2021](https://www.rust-lang.org/) — Tối ưu hiệu năng, an toàn bộ nhớ tuyệt đối và tiêu thụ tài nguyên cực thấp (< 50MB RAM).
* **Embedded Web Server:** [Axum 0.7](https://github.com/tokio-rs/axum) + [Tokio Async Runtime](https://tokio.rs/) — Xử lý đồng thời hàng trăm kết nối WebSocket và HTTP request với độ trễ tối thiểu.
* **Cơ sở dữ liệu:** SQLite 3 nhúng qua crate `rusqlite` (WAL mode).
* **Desktop Application Shell:** `tao` + `wry` (Microsoft Edge WebView2) — Chạy ứng dụng cửa sổ native trên Windows mà không cần mở trình duyệt ngoài và không có màn hình đen terminal console.
* **Giao diện Client & Giáo viên:** HTML5, CSS3, JavaScript ES6+, [Tailwind CSS](https://tailwindcss.com/), bộ icon độc lập [Lucide Icons SVG](https://lucide.dev/) (chạy 100% offline).
* **Đường hầm mạng:** Tích hợp `cloudflared` (Cloudflare Quick Tunnel daemon) để kết nối từ xa an toàn.

---

## 5. Yêu Cầu Hệ Thống & Cài Đặt (Installation)

### 5.1. Yêu cầu hệ thống
* **Hệ điều hành:** Windows 10 hoặc Windows 11 (phiên bản 64-bit).
* **Thành phần phụ thuộc:** **Microsoft Edge WebView2 Runtime**.
  > *Lưu ý:* Hầu hết các máy tính Windows 10/11 hiện đại đã tích hợp sẵn WebView2. Nếu máy của bạn là bản Windows cũ chưa có, vui lòng tải cài đặt chính thức từ Microsoft: [Tải WebView2 Runtime (Evergreen Bootstrapper)](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).

### 5.2. Cách 1: Sử dụng Bản Portable (Khuyên dùng cho Giáo viên)
Không cần cài đặt phức tạp, có thể copy vào USB để mang đi dạy tại các phòng máy trường học:
1. Tải gói nén phát hành chính thức: `ExamPlus-v1.0.0-Windows-x64-Portable.zip`.
2. Giải nén vào một thư mục trên máy tính (hoặc trên USB).
3. Click đúp chuột vào tệp `ExamPlus.exe` (hoặc `Start-ExamPlus.bat`) để mở phần mềm.
4. *(Tùy chọn)* Click chuột phải vào `Create-Desktop-Shortcut.ps1` -> Chọn **Run with PowerShell** để tạo lối tắt ứng dụng ra màn hình Desktop.

### 5.3. Cách 2: Biên dịch từ mã nguồn (Build from Source)
Dành cho lập trình viên và nhà nghiên cứu muốn biên dịch trực tiếp từ mã nguồn Rust:
```bash
# 1. Clone kho lưu trữ
git clone https://github.com/anhpt-udu-ptit/ExamPlus.git
cd ExamPlus

# 2. Cài đặt toolchain Rust (nếu chưa có)
rustup default stable-x86_64-pc-windows-msvc
# hoặc dùng MinGW-w64: rustup default stable-x86_64-pc-windows-gnu

# 3. Chạy kiểm thử tự động toàn bộ workspace
cargo test --workspace

# 4. Biên dịch bản Release tối ưu
cargo build --release --bin examplus-app
```
Tệp thực thi sau khi biên dịch sẽ nằm tại: `target/release/examplus-app.exe`.

---

## 6. Biến Môi Trường Cấu Hình (Environment Variables)

ExamPlus hoạt động với các giá trị mặc định an toàn mà không cần cấu hình thêm. Tuy nhiên, bạn có thể tùy biến thông qua tệp `.env` (tham khảo tệp mẫu `.env.example`):

| Biến môi trường | Mặc định | Mô tả chi tiết |
|---|---|---|
| `PORT` | `8080` | Cổng HTTP / WebSocket máy chủ ExamPlus lắng nghe. |
| `HOST` | `0.0.0.0` | Địa chỉ IP ràng buộc (`0.0.0.0` cho phép các thiết bị cùng mạng Wi-Fi/LAN truy cập). |
| `RUST_LOG` | `info` | Mức độ chi tiết của nhật ký hệ thống (`error`, `warn`, `info`, `debug`, `trace`). |
| `GOOGLE_CLIENT_ID` | *(Rỗng)* | Google OAuth 2.0 Client ID (dành cho tính năng đồng bộ Google Drive). |
| `GOOGLE_CLIENT_SECRET` | *(Rỗng)* | Google OAuth 2.0 Client Secret (dành cho tính năng đồng bộ Google Drive). |
| `EXAMPLUS_PDFTOPPM_PATH` | *(Tự tìm)* | Đường dẫn tùy chỉnh tới công cụ `pdftoppm.exe` (thuộc bộ Poppler). |
| `EXAMPLUS_TESSERACT_PATH` | *(Tự tìm)* | Đường dẫn tùy chỉnh tới công cụ `tesseract.exe` (Tesseract OCR). |
| `TESSDATA_PREFIX` | *(Tự tìm)* | Thư mục chứa dữ liệu ngôn ngữ OCR (`vie.traineddata`). |

---

## 7. Hướng Dẫn Sử Dụng (User Manual)

### 7.1. Luồng dành cho Giáo viên (Tổ chức thi)
1. **Khởi động:** Mở `ExamPlus.exe`, giao diện Bảng điều khiển Giáo viên sẽ hiển thị trực tiếp trong cửa sổ ứng dụng.
2. **Nạp đề thi:**
   - Vào tab **"Quản Lý Đề Thi"** -> Bấm **"Nạp Đề Word / PDF"**.
   - Chọn tệp đề thi (`.docx` hoặc `.pdf`). Hệ thống sẽ tự động bóc tách các câu hỏi và công thức toán.
   - Kiểm tra nội dung và chỉnh sửa đáp án ở cột bên phải nếu cần thiết -> Bấm **"Lưu Thay Đổi"**.
3. **Mở phòng thi:**
   - Bấm nút **"Tạo Phòng Thi"**, đặt tên phòng thi, thời gian làm bài (ví dụ: 50 phút).
   - Chọn kết nối mạng:
     - **Mạng LAN nội bộ:** Học sinh truy cập theo địa chỉ IP cục bộ hiển thị trên màn hình (ví dụ: `http://192.168.1.15:8080/?room=...`).
     - **Mạng Online (4G):** Bấm nút **"Bật Cloudflare Tunnel"** để lấy link đường hầm bảo mật HTTPS công khai.
   - Bấm **"Xem Mã QR"** để chiếu mã QR lên máy chiếu cho học sinh quét bằng điện thoại.
4. **Giám sát & Chấm điểm:**
   - Theo dõi danh sách thí sinh đăng nhập và trạng thái làm bài trên Live Dashboard.
   - Khi hết giờ thi hoặc khi tất cả đã nộp bài, bấm **"Xuất Excel (.xlsx)"** để tải về bảng điểm chính thức.

### 7.2. Luồng dành cho Thí sinh (Làm bài thi)
1. **Vào phòng thi:** Kết nối điện thoại hoặc máy tính vào cùng mạng Wi-Fi với máy giáo viên -> Quét mã QR hoặc nhập đường dẫn phòng thi vào trình duyệt.
2. **Đăng nhập:** Nhập chính xác **Số Báo Danh (SBD)**, **Họ và Tên**, **Lớp** -> Bấm **"Vào Phòng Thi"**.
3. **Làm bài thi:**
   - Ứng dụng sẽ tự động kích hoạt chế độ toàn màn hình (Fullscreen).
   - Làm bài lần lượt theo 3 phần thi bằng cách bấm chọn phương án hoặc điền số. Thí sinh có thể bấm số câu trên thanh điều hướng để nhảy nhanh đến câu muốn làm.
4. **Nộp bài:** Bấm **"Nộp Bài"** -> Kiểm tra bảng thông báo các câu đã làm / còn thiếu -> Xác nhận nộp bài. Điểm số sẽ hiển thị ngay lập tức nếu giáo viên cho phép xem điểm.

---

## 8. Đóng Góp & Giấy Phép Sử Dụng (License & Contributing)

### 8.1. Chính sách Giấy phép
Mã nguồn ExamPlus được phát hành công khai theo giấy phép riêng:  
**GIẤY PHÉP MÃ NGUỒN EXAMPLUS - PHIÊN BẢN GIÁO DỤC PHI THƯƠNG MẠI (EXAMPLUS-NC-EDU-1.0)**.  
Chi tiết toàn văn xem tại tệp [LICENSE](file:///e:/ExamPlus/LICENSE).

* **Quyền được cấp miễn phí:** Mọi cá nhân, giáo viên, học sinh, trường học và tổ chức giáo dục phi lợi nhuận được quyền tải về, cài đặt, chạy và sử dụng phần mềm ExamPlus hoàn toàn miễn phí phục vụ cho mục đích giảng dạy, học tập và tổ chức thi trắc nghiệm không thu tiền.
* **Quyền xem & fork trên GitHub:** Người dùng GitHub được quyền xem mã nguồn và fork kho lưu trữ để nghiên cứu kỹ thuật cá nhân theo Điều khoản Nền tảng GitHub.
* **Bảo lưu quyền tác giả:** Tác giả Phạm Tuấn Anh và Nhóm Phát Triển ExamPlus bảo lưu độc quyền mọi quyền đối với việc sửa đổi, tạo bản phái sinh và phân phối các bản phát hành nhị phân công khai.

### 8.2. Cảnh Báo Nghiêm Cấm Thương Mại Hóa (Strict Commercial Prohibition)
> ⚠️ **CẢNH BÁO PHÁP LÝ:** Nghiêm cấm tuyệt đối mọi hành vi sử dụng trực tiếp hoặc gián tiếp ExamPlus vào mục đích thương mại trong mọi lĩnh vực, bao gồm:
> 1. Bán, bán lại, cho thuê, cấp quyền phụ, thu phí tải về hoặc phân phối lại phần mềm dưới mọi hình thức.
> 2. Cung cấp ExamPlus dưới dạng dịch vụ thi có thu phí (SaaS) hoặc dịch vụ hỗ trợ kỹ thuật thu tiền.
> 3. Đóng gói, gộp hoặc nhúng mã nguồn ExamPlus vào sản phẩm phần mềm thương mại khác.
> 4. Khai thác doanh thu quảng cáo hoặc phục vụ vận hành kinh doanh tạo lợi nhuận cho doanh nghiệp.
> 5. **Sử dụng để tổ chức các lớp học thêm, trung tâm luyện thi, khóa đào tạo hoặc kỳ khảo thí thuộc các chương trình có thu học phí hoặc phí dịch vụ.**

*Quy tắc cấm thương mại áp dụng cho mọi cá nhân và tổ chức, bất kể phần mềm được cung cấp miễn phí.*

### 8.3. Liên hệ Cấp phép Thương mại & Hợp tác
Nếu bạn là tổ chức, doanh nghiệp hoặc đơn vị đào tạo có nhu cầu sử dụng ExamPlus cho mục đích thương mại, hoặc có nhu cầu tích hợp giải pháp thi riêng, vui lòng liên hệ trực tiếp với tác giả để thỏa thuận cấp phép bằng văn bản:

* **Tác giả:** Phạm Tuấn Anh
* **Email:** [examplus.edu@gmail.com](mailto:examplus.edu@gmail.com)
* **Website:** [https://examplus.vercel.app](https://examplus.vercel.app)
